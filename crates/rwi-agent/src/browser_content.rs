//! Explicit browser-content lifecycle. Never invokes a native backend, focus or input API.
#[cfg(target_os = "macos")]
mod pipe;
mod policy;
use crate::{
    config::{Config, Principal},
    ledger::{Ledger, Reservation},
};
use policy::Policy;
use rwi_core::device::{Capability, DeviceResult, browser::SessionArgs};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const LIFETIME: Duration = Duration::from_secs(600);
trait ChildHandle: Send {
    fn facts(&self) -> Value;
    /// At most one CDP close and one own-child kill; later calls only reap.
    fn close(&mut self) -> Value;
}
struct StartFailure {
    detail: String,
    retained: Option<Box<dyn ChildHandle>>,
}
impl From<io::Error> for StartFailure {
    fn from(e: io::Error) -> Self {
        Self {
            detail: e.to_string(),
            retained: None,
        }
    }
}
type Factory =
    dyn Fn(&Policy, &Path, &str) -> Result<Box<dyn ChildHandle>, StartFailure> + Send + Sync;
struct Session {
    owner: String,
    nonce: String,
    expires: Instant,
    inner: Mutex<SessionState>,
}
struct SessionState {
    valid: bool,
    driver: Option<Box<dyn ChildHandle>>,
    expiry_reserved: bool,
}
pub(crate) struct BrowserRuntime {
    device: String,
    instance: String,
    root: PathBuf,
    policy: Result<Policy, String>,
    ledger: Mutex<io::Result<Ledger>>,
    sessions: Mutex<BTreeMap<String, Arc<Session>>>,
    workers: Arc<tokio::sync::Semaphore>,
    control_workers: Arc<tokio::sync::Semaphore>,
    factory: Arc<Factory>,
}
fn error(code: &str, detail: impl std::fmt::Display) -> DeviceResult {
    DeviceResult::error(code, &detail.to_string())
}
impl BrowserRuntime {
    pub fn new(config: &Config, instance: &str) -> Arc<Self> {
        let policy = Policy::read(config).map_err(|e| e.to_string());
        let root = config.state_root.join("browser-content");
        // Absent/invalid browser policy is default-off, not a Native service failure.
        let ledger = if policy.is_ok() {
            private_dir(&root).and_then(|()| Ledger::open(root.join("operations")))
        } else {
            Err(io::Error::other("browser_policy_not_enabled"))
        };
        let r = Arc::new(Self {
            device: config.device_id.clone(),
            instance: instance.into(),
            root,
            policy,
            ledger: Mutex::new(ledger),
            sessions: Mutex::new(BTreeMap::new()),
            workers: Arc::new(tokio::sync::Semaphore::new(4)),
            control_workers: Arc::new(tokio::sync::Semaphore::new(2)),
            factory: Arc::new(spawn_owned),
        });
        if r.policy.is_ok() {
            let weak = Arc::downgrade(&r);
            // No browser is started here. One weak, bounded-resource maintenance thread
            // expires owned sessions; it does not keep the Agent alive or hold a global I/O lock.
            std::thread::spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_secs(1));
                    let Some(r) = weak.upgrade() else { break };
                    r.expire();
                }
            });
        }
        r
    }
    pub fn capability(&self, principal: &Principal) -> Capability {
        let authorized = self
            .policy
            .as_ref()
            .ok()
            .and_then(|p| p.grant(principal))
            .is_some_and(|g| g.observe || g.create || g.close);
        let ready = self.ledger.lock().map(|l| l.is_ok()).unwrap_or(false);
        let implemented = cfg!(target_os = "macos");
        Capability {
            id: "browser.session".into(),
            implemented,
            available: implemented && authorized && ready,
            authorized,
            reason: if !implemented {
                Some("browser_platform_unsupported".into())
            } else if let Err(e) = &self.policy {
                Some(format!("browser_policy_disabled:{e}"))
            } else if !authorized {
                Some("browser_principal_permission_denied".into())
            } else if !ready {
                Some("browser_journal_unavailable".into())
            } else {
                None
            },
        }
    }
    pub async fn call(self: Arc<Self>, principal: &Principal, args: SessionArgs) -> DeviceResult {
        if let Err(e) = args.validate() {
            return error("invalid_arguments", e);
        }
        if args.device_id() != self.device {
            return error(
                "browser_device_mismatch",
                "No browser request was dispatched.",
            );
        }
        let p = match &self.policy {
            Ok(p) => p,
            Err(e) => return error("browser_policy_disabled", e),
        };
        let Some(g) = p.grant(principal) else {
            return error(
                "browser_permission_denied",
                "Separate browser policy is required; Native grants do not authorize this family.",
            );
        };
        let allowed = match &args {
            SessionArgs::Create { url, .. } => {
                g.create && g.close && g.observe && p.initial_urls.contains(url)
            }
            SessionArgs::Close { .. } => g.close,
            SessionArgs::Query { .. } => g.observe,
        };
        if !allowed {
            return error(
                "browser_permission_denied",
                "This lifecycle action or exact initial URL is not granted.",
            );
        }
        let owner = principal.certificate.as_str().to_owned();
        if let SessionArgs::Query { request_id, .. } = &args {
            let l = self.ledger.lock().unwrap_or_else(|e| e.into_inner());
            return match l.as_ref() { Ok(l)=>l.request_status(&owner,request_id).map(DeviceResult::value).unwrap_or_else(||error("browser_request_unknown","No browser record for this authenticated principal; Native records are not searched.")),Err(e)=>error("browser_journal_unavailable",e) };
        }
        let capacity = if matches!(&args, SessionArgs::Close { .. }) {
            &self.control_workers
        } else {
            &self.workers
        };
        let permit = match capacity.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                return error(
                    "browser_busy_not_dispatched",
                    "Browser worker capacity is bounded; no lifecycle side effect was reserved or sent.",
                );
            }
        };
        match tokio::task::spawn_blocking(move || {
            let _permit = permit;
            self.dispatch(&owner, args)
        })
        .await
        {
            Ok(r) => r,
            Err(e) => error(
                "browser_result_unknown",
                format!("Worker terminated: {e}; query the original request_id; do not replay."),
            ),
        }
    }
    fn reserve(
        &self,
        owner: &str,
        request: &str,
        payload: &Value,
        nonce: &str,
    ) -> Result<Reservation, DeviceResult> {
        let mut l = self.ledger.lock().unwrap_or_else(|e| e.into_inner());
        l.as_mut()
            .map_err(|e| error("browser_journal_unavailable", e))?
            .reserve_resource(
                owner,
                request,
                payload,
                nonce,
                &format!("browser-content:{}:{nonce}", self.device),
            )
            .map_err(|e| error("browser_reservation_failed", e))
    }
    fn finish(&self, key: &str, result: DeviceResult) -> DeviceResult {
        let l = self.ledger.lock().unwrap_or_else(|e| e.into_inner());
        if let Err(e) = l
            .as_ref()
            .map_err(|e| e.to_string())
            .and_then(|l| l.finish(key, &result).map_err(|e| e.to_string()))
        {
            return error(
                "browser_result_persistence_unknown",
                format!(
                    "{e}; lifecycle may have occurred. Query the original request_id; no replay."
                ),
            );
        }
        result
    }
    fn dispatch(&self, owner: &str, args: SessionArgs) -> DeviceResult {
        let payload = serde_json::to_value(&args).expect("typed browser arguments");
        match &args {
            SessionArgs::Create {
                request_id, url, ..
            } => {
                let nonce = uuid::Uuid::new_v4().to_string();
                // Reserve BEFORE profile creation or process spawn, not merely before CDP.
                let key = match self.reserve(owner, request_id, &payload, &nonce) {
                    Ok(Reservation::New(k)) => k,
                    Ok(Reservation::Existing(r)) => return r,
                    Err(e) => return e,
                };
                let reference = format!("browser:{}:{nonce}", self.instance);
                let session = Arc::new(Session {
                    owner: owner.into(),
                    nonce: nonce.clone(),
                    expires: Instant::now() + LIFETIME,
                    inner: Mutex::new(SessionState {
                        valid: true,
                        driver: None,
                        expiry_reserved: false,
                    }),
                });
                {
                    let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
                    if sessions.len() >= 4
                        || sessions.values().filter(|s| s.owner == owner).count() >= 2
                    {
                        return self.finish(&key,error("browser_session_capacity","Maximum four owned sessions and two per principal. No process was started."));
                    }
                    sessions.insert(reference.clone(), session.clone());
                }
                let policy = self.policy.as_ref().expect("authorized policy");
                let started =
                    (self.factory)(policy, &self.root.join(format!("profile-{nonce}")), url);
                let result = match started {
                    Ok(driver) => {
                        let facts = driver.facts();
                        session
                            .inner
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .driver = Some(driver);
                        DeviceResult::value(
                            json!({"family":"browser-content","session_ref":reference,"device_id":self.device,"agent_instance":self.instance,"request_id":request_id,
                            "state":"open","expires_after_ms":session.expires.saturating_duration_since(Instant::now()).as_millis(),"owned_process":facts,"background_requested":true,"os_foreground_observed":false,
                            "native_binding_required":false,"snapshot_implemented":false,"act_implemented":false,"automatic_replay":false,"goal_verified":false}),
                        )
                    }
                    Err(mut failure) => {
                        let retained = failure.retained.take();
                        let pending = retained.is_some();
                        if pending {
                            let mut state = session.inner.lock().unwrap_or_else(|e| e.into_inner());
                            state.valid = false;
                            state.driver = retained;
                        } else {
                            self.sessions
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .remove(&reference);
                        }
                        let mut result = error(
                            "browser_start_failed",
                            format!(
                                "{}; no alternate transport/profile/native fallback. Cleanup is not OS exit evidence unless explicitly reported.",
                                failure.detail
                            ),
                        );
                        result.data["owned_cleanup_pending"] = json!(pending);
                        if pending {
                            result.data["closed_cleanup_resource"] = json!(reference);
                        }
                        result
                    }
                };
                self.finish(&key, result)
            }
            SessionArgs::Close {
                request_id,
                session_ref,
                ..
            } => {
                // Duplicate original requests are recognized even after the reference is invalidated.
                // Reserve before looking up the session; changed payload cannot close a different target.
                let session = self
                    .sessions
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(session_ref)
                    .filter(|s| s.owner == owner)
                    .cloned();
                let nonce = session
                    .as_ref()
                    .map(|s| s.nonce.clone())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                let key = match self.reserve(owner, request_id, &payload, &nonce) {
                    Ok(Reservation::New(k)) => k,
                    Ok(Reservation::Existing(r)) => return r,
                    Err(e) => return e,
                };
                let result = if let Some(s) = session {
                    match s.inner.try_lock() {
                        Ok(mut state) if state.valid && state.driver.is_some() => {
                            state.valid = false; // Never re-authorize this reference, including uncertain close.
                            let cleanup = state.driver.as_mut().unwrap().close();
                            if cleanup["process_exit_observed"] == true {
                                self.sessions
                                    .lock()
                                    .unwrap_or_else(|e| e.into_inner())
                                    .remove(session_ref);
                            }
                            DeviceResult::value(
                                json!({"session_ref":session_ref,"state":"closed_reference","references_invalidated":true,"cleanup":cleanup,"automatic_replay":false,"goal_verified":false}),
                            )
                        }
                        Ok(_) => error(
                            "browser_session_closed_or_pending",
                            "Reference cannot authorize any further browser operation.",
                        ),
                        Err(_) => error(
                            "browser_session_busy_not_dispatched",
                            "Only this browser resource is busy; no global Native lock or fallback was acquired.",
                        ),
                    }
                } else {
                    error(
                        "browser_session_unknown",
                        "Reference is stale, from another Agent instance or another principal. No other resource is searched.",
                    )
                };
                self.finish(&key, result)
            }
            SessionArgs::Query { .. } => unreachable!("query is a separate read path"),
        }
    }
    fn expire(&self) {
        let sessions = self
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(r, s)| (r.clone(), s.clone()))
            .collect::<Vec<_>>();
        for (reference, s) in sessions {
            let Ok(mut state) = s.inner.try_lock() else {
                continue;
            };
            // A prior close never resends CDP or SIGKILL: the driver only reaps
            // after its one close/kill attempt. Do not leave a late-exiting child
            // occupying capacity until TTL or Agent shutdown.
            if !state.valid {
                if let Some(driver) = state.driver.as_mut()
                    && driver.close()["process_exit_observed"] == true
                {
                    self.sessions
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&reference);
                }
                continue;
            }
            if Instant::now() < s.expires || state.driver.is_none() || state.expiry_reserved {
                continue;
            }
            let id = format!("browser-expire-{}", s.nonce);
            let key = match self.reserve(
                &s.owner,
                &id,
                &json!({"action":"owned_session_expiry","session_ref":reference}),
                &s.nonce,
            ) {
                Ok(Reservation::New(k)) => k,
                _ => continue,
            };
            state.expiry_reserved = true;
            state.valid = false;
            let cleanup = state.driver.as_mut().unwrap().close();
            let done = cleanup["process_exit_observed"] == true;
            let _=self.finish(&key,DeviceResult::value(json!({"session_ref":reference,"state":"expired_reference","references_invalidated":true,"cleanup":cleanup,"automatic_replay":false})));
            if done {
                self.sessions
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&reference);
            }
        }
    }
}
#[cfg(target_os = "macos")]
fn spawn_owned(p: &Policy, path: &Path, url: &str) -> Result<Box<dyn ChildHandle>, StartFailure> {
    Ok(Box::new(pipe::OwnedChrome::start(p, path, url)?))
}
#[cfg(not(target_os = "macos"))]
fn spawn_owned(
    _p: &Policy,
    _path: &Path,
    _url: &str,
) -> Result<Box<dyn ChildHandle>, StartFailure> {
    Err(io::Error::other("browser_platform_unsupported").into())
}
#[cfg(unix)]
fn private_dir(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    match std::fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    let m = std::fs::symlink_metadata(path)?;
    if !m.is_dir() || m.file_type().is_symlink() || m.mode() & 0o077 != 0 {
        return Err(io::Error::other("browser_state_directory_not_private"));
    }
    // The audited private reader checks current UID and canonical parent before policy load.
    let _ = rwi_link_trust::PrivateDirectory::open(path)?;
    Ok(())
}
#[cfg(not(unix))]
fn private_dir(_path: &Path) -> io::Result<()> {
    Err(io::Error::other("browser_platform_unsupported"))
}

#[cfg(test)]
mod tests;
