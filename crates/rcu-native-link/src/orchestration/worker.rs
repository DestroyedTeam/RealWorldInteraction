//! Only freshly spawned fixed Rust workers are owned/terminated. User apps are not children here.
#[cfg(any(windows, feature = "fixture"))]
use crate::sha;
use crate::{
    budget::{Budget, BudgetSpec},
    frame,
    mcp::Ledger,
    net,
    private::Directory,
    types::{AgentConfig, Approval, Stage},
};
#[cfg(windows)]
use crate::{private::core_error, types::WorkerGuardSpec};
use serde_json::{Value, json};
#[cfg(any(windows, feature = "fixture", test))]
use std::process::Stdio;
use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
#[cfg(any(windows, feature = "fixture", test))]
use tokio::{io::AsyncReadExt, process::Command};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    process::Child,
    sync::Mutex,
};

pub type Read = Box<dyn AsyncRead + Unpin + Send>;
pub type Write = Box<dyn AsyncWrite + Unpin + Send>;
pub struct OwnedWorker {
    child: Mutex<Child>,
    stderr: Mutex<StderrState>,
    force_requested: AtomicBool,
    wire_recovery: StdMutex<Option<Vec<u8>>>,
    pub record: StdMutex<Value>,
    #[cfg(windows)]
    retained: StdMutex<Option<Arc<rcu_windows::agent_binding::OwnedProcess>>>,
}
pub type Registry = Arc<StdMutex<Vec<Arc<OwnedWorker>>>>;
#[derive(Clone, Debug)]
pub struct ExitEvidence {
    pub observed: bool,
    pub code: Option<i32>,
    pub forced: bool,
    pub error: Option<String>,
    pub stderr: StderrEvidence,
    pub elapsed_ms: u64,
}

const STOP_BUDGET: Duration = Duration::from_secs(8);
const ALIVE_LOCK_BUDGET: Duration = Duration::from_millis(250);
#[cfg(any(windows, feature = "fixture", test))]
const STDERR_CAP: usize = 65536;

#[derive(Clone, Debug, serde::Serialize)]
pub struct StderrEvidence {
    pub joined: bool,
    pub abort_requested: bool,
    pub eof_observed: bool,
    pub capped: bool,
    pub bytes_retained: usize,
    pub sha256: Option<String>,
    pub error: Option<String>,
}
impl StderrEvidence {
    fn unknown(reason: &str, abort_requested: bool) -> Self {
        Self {
            joined: false,
            abort_requested,
            eof_observed: false,
            capped: false,
            bytes_retained: 0,
            sha256: None,
            error: Some(reason.into()),
        }
    }
    fn clean(&self) -> bool {
        self.joined
            && !self.abort_requested
            && self.eof_observed
            && !self.capped
            && self.error.is_none()
    }
}
struct StderrCapture {
    bytes: Vec<u8>,
    capped: bool,
    error: Option<String>,
}
#[derive(Default)]
struct StderrState {
    task: Option<tokio::task::JoinHandle<StderrCapture>>,
    final_evidence: Option<StderrEvidence>,
    abort_requested: bool,
}
impl Drop for OwnedWorker {
    fn drop(&mut self) {
        // There is no async join in Drop. Normal/error cleanup retains this
        // object in Registry and joins explicitly; final-owner teardown at
        // least requests collector cancellation rather than silently detaching.
        if let Some(task) = &self.stderr.get_mut().task {
            task.abort();
        }
    }
}

#[cfg(any(windows, feature = "fixture", test))]
fn spawn_registered(
    command: &mut Command,
    registry: &Registry,
    mut record: Value,
) -> io::Result<Arc<OwnedWorker>> {
    // Fail before spawn on poison/contention/allocation failure. No std guard
    // crosses an await; insertion is completed in this same synchronous block.
    let mut registry = registry.try_lock().map_err(|error| match error {
        std::sync::TryLockError::Poisoned(_) => {
            crate::invalid("owned_registry_poisoned_before_spawn")
        }
        std::sync::TryLockError::WouldBlock => crate::invalid("owned_registry_busy_before_spawn"),
    })?;
    registry
        .try_reserve(1)
        .map_err(|_| crate::invalid("owned_registry_capacity_before_spawn"))?;
    let fields = record
        .as_object_mut()
        .ok_or_else(|| crate::invalid("owned_record_invalid_before_spawn"))?;
    fields.insert("pid".into(), Value::Null);
    let child = command.spawn()?;
    fields.insert("pid".into(), child.id().map_or(Value::Null, Value::from));
    let owned = Arc::new(OwnedWorker {
        child: Mutex::new(child),
        stderr: Mutex::new(StderrState::default()),
        force_requested: AtomicBool::new(false),
        wire_recovery: StdMutex::new(None),
        record: StdMutex::new(record),
        #[cfg(windows)]
        retained: StdMutex::new(None),
    });
    registry.push(owned.clone());
    Ok(owned)
}

#[cfg(any(windows, feature = "fixture", test))]
async fn capture_stderr(reader: impl AsyncRead + Unpin) -> StderrCapture {
    let mut bytes = Vec::new();
    let result = reader
        .take((STDERR_CAP + 1) as u64)
        .read_to_end(&mut bytes)
        .await;
    let capped = bytes.len() > STDERR_CAP;
    bytes.truncate(STDERR_CAP);
    StderrCapture {
        bytes,
        capped,
        error: result.err().map(|e| e.to_string()),
    }
}
#[cfg(any(windows, feature = "fixture", test))]
async fn complete_setup<T>(owned: &Arc<OwnedWorker>, setup: io::Result<T>) -> io::Result<T> {
    match setup {
        Ok(parts) => Ok(parts),
        Err(error) => {
            let cleanup = owned.stop().await;
            if let Ok(mut record) = owned.record.try_lock() {
                record["setup_failure"] = json!({"error":error.to_string(),"cleanup_observed":cleanup.observed,"cleanup_forced":cleanup.forced,"stderr":cleanup.stderr});
            }
            Err(error)
        }
    }
}

impl OwnedWorker {
    fn audit_response(&self, root: &Path, raw: &[u8]) -> io::Result<()> {
        if raw.len() > frame::OUTBOUND {
            return Err(crate::invalid("worker_response_audit_limit"));
        }
        let sha = crate::sha(raw);
        let result = (|| {
            let directory = Directory::open(root)?;
            crate::private::content(
                &directory,
                &format!("wire-{sha}.ndjson"),
                raw,
                frame::OUTBOUND,
            )?;
            let mut record = self
                .record
                .try_lock()
                .map_err(|_| crate::invalid("worker_response_audit_record_unavailable"))?;
            record["last_wire_response_sha256"] = sha.clone().into();
            Ok(())
        })();
        if result.is_err() {
            // Storage/quota failure is HOLD, not permission to discard the
            // only raw response. Retain one bounded frame in its registered
            // owner; never print raw MCP/image/user data in an error/stderr.
            let mut recovery = self
                .wire_recovery
                .try_lock()
                .map_err(|_| crate::invalid("worker_response_audit_recovery_unavailable"))?;
            *recovery = Some(raw.to_vec());
            if let Ok(mut record) = self.record.try_lock() {
                record["last_wire_response_sha256"] = sha.into();
                record["wire_audit_memory_retained"] = true.into();
            }
        } else if let Ok(mut recovery) = self.wire_recovery.try_lock() {
            *recovery = None;
            if let Ok(mut record) = self.record.try_lock() {
                record["wire_audit_memory_retained"] = false.into();
            }
        }
        result
    }
    pub async fn stop(&self) -> ExitEvidence {
        self.stop_with_limits(STOP_BUDGET, Duration::from_secs(3), Duration::from_secs(2))
            .await
    }
    async fn stop_with_limits(
        &self,
        total: Duration,
        grace: Duration,
        stderr_reserve: Duration,
    ) -> ExitEvidence {
        let started = tokio::time::Instant::now();
        let total = total.min(STOP_BUDGET);
        let until = started + total;
        let child_until = until - stderr_reserve.min(total / 2);
        let (observed, code, mut error) = async {
            let mut child = match tokio::time::timeout_at(child_until, self.child.lock()).await {
                Ok(child) => child,
                Err(_) => return (false, None, Some("owned_child_lock_cleanup_timeout".into())),
            };
            match child.try_wait() {
                Ok(Some(status)) => return (true, status.code(), None),
                Ok(None) => {}
                Err(error) => return (false, None, Some(error.to_string())),
            }
            let grace_until = (tokio::time::Instant::now() + grace).min(child_until);
            match tokio::time::timeout_at(grace_until, child.wait()).await {
                Ok(Ok(status)) => return (true, status.code(), None),
                Ok(Err(error)) => return (false, None, Some(error.to_string())),
                Err(_) => {}
            }
            // Only this retained Child is signaled, never a numeric/historical
            // PID. Every wait shares the original absolute cleanup deadline.
            self.force_requested.store(true, Ordering::Release);
            if let Err(error) = child.start_kill() {
                return (false, None, Some(error.to_string()));
            }
            match tokio::time::timeout_at(child_until, child.wait()).await {
                Ok(Ok(status)) => (true, status.code(), None),
                Ok(Err(error)) => (false, None, Some(error.to_string())),
                Err(_) => (
                    false,
                    None,
                    Some("owned_child_reap_unconfirmed_hold".into()),
                ),
            }
        }
        .await;
        let stderr = self.finish_stderr(until).await;
        if !stderr.clean() && error.is_none() {
            error = Some("owned_stderr_cleanup_not_clean_hold".into());
        }
        let result = ExitEvidence {
            observed,
            code,
            forced: self.force_requested.load(Ordering::Acquire),
            error,
            stderr,
            elapsed_ms: started.elapsed().as_millis() as u64,
        };
        if let Ok(mut record) = self.record.try_lock() {
            record["last_cleanup"] = json!({"observed":result.observed,"code":result.code,"force_requested_ever":result.forced,"error":result.error,"stderr":result.stderr,"elapsed_ms":result.elapsed_ms,"budget_ms":total.min(STOP_BUDGET).as_millis(),"no_numeric_pid_kill":true});
        }
        result
    }
    async fn finish_stderr(&self, until: tokio::time::Instant) -> StderrEvidence {
        let mut state = match tokio::time::timeout_at(until, self.stderr.lock()).await {
            Ok(state) => state,
            Err(_) => return StderrEvidence::unknown("owned_stderr_lock_timeout", false),
        };
        if let Some(evidence) = &state.final_evidence {
            return evidence.clone();
        }
        let prior_abort = state.abort_requested;
        let Some(task) = state.task.as_mut() else {
            return StderrEvidence::unknown("owned_stderr_not_started", false);
        };
        let remaining = until.saturating_duration_since(tokio::time::Instant::now());
        let graceful_until =
            tokio::time::Instant::now() + (remaining / 2).min(Duration::from_secs(2));
        let (joined, abort_requested) =
            match tokio::time::timeout_at(graceful_until, &mut *task).await {
                Ok(result) => (Some(result), prior_abort),
                Err(_) => {
                    task.abort();
                    (tokio::time::timeout_at(until, &mut *task).await.ok(), true)
                }
            };
        state.abort_requested = abort_requested;
        let Some(joined) = joined else {
            // Keep the JoinHandle in the registered owner for a later bounded
            // reconciliation; do not drop it and detach a possibly live task.
            return StderrEvidence::unknown("owned_stderr_abort_join_unconfirmed", abort_requested);
        };
        let evidence = match joined {
            Ok(capture) => StderrEvidence {
                joined: true,
                abort_requested,
                eof_observed: !capture.capped && capture.error.is_none(),
                capped: capture.capped,
                bytes_retained: capture.bytes.len(),
                sha256: Some(crate::sha(&capture.bytes)),
                error: capture.error,
            },
            Err(error) => StderrEvidence {
                joined: true,
                abort_requested,
                eof_observed: false,
                capped: false,
                bytes_retained: 0,
                sha256: None,
                error: Some(if error.is_cancelled() {
                    "owned_stderr_cancelled".into()
                } else {
                    format!("owned_stderr_join_error:{error}")
                }),
            },
        };
        state.task = None;
        state.final_evidence = Some(evidence.clone());
        evidence
    }
    #[cfg(any(windows, feature = "fixture", test))]
    fn start_stderr(&self, reader: impl AsyncRead + Unpin + Send + 'static) -> io::Result<()> {
        let mut state = self
            .stderr
            .try_lock()
            .map_err(|_| crate::invalid("owned_stderr_start_lock_busy"))?;
        if state.task.is_some() || state.final_evidence.is_some() {
            return Err(crate::invalid("owned_stderr_already_started"));
        }
        // No await or fallible operation between spawn and retained assignment.
        state.task = Some(tokio::spawn(capture_stderr(reader)));
        Ok(())
    }
    pub async fn still_owned_alive(&self) -> io::Result<bool> {
        let mut child = tokio::time::timeout(ALIVE_LOCK_BUDGET, self.child.lock())
            .await
            .map_err(|_| crate::invalid("owned_child_liveness_lock_timeout"))?;
        Ok(child.try_wait()?.is_none())
    }
    pub fn verify_binding(&self, cfg: &AgentConfig, approval: &Approval) -> io::Result<()> {
        if cfg.backend.is_windows() {
            #[cfg(windows)]
            {
                let binding = approval
                    .native_binding
                    .as_ref()
                    .ok_or_else(|| crate::invalid("native_binding_missing"))?;
                let retained = self
                    .retained
                    .try_lock()
                    .map_err(|_| crate::invalid("owned_handle_lock"))?
                    .clone()
                    .ok_or_else(|| crate::invalid("owned_handle_missing"))?;
                retained.verify_current(binding).map_err(core_error)?;
            }
            #[cfg(not(windows))]
            {
                return Err(crate::invalid("windows_worker_on_non_windows"));
            }
        }
        let _ = approval;
        Ok(())
    }
}
pub struct Worker {
    pub input: Write,
    pub output: BufReader<Read>,
    pub ledger: Ledger,
    pub owned: Arc<OwnedWorker>,
    pub budget: Budget,
    pub audit_root: Option<PathBuf>,
    model_stage: bool,
    ready_written: bool,
    #[cfg(windows)]
    ready_binding: Option<(PathBuf, String, String)>,
}

#[cfg(any(windows, feature = "fixture"))]
fn image(cfg: &AgentConfig) -> io::Result<()> {
    let m = std::fs::symlink_metadata(&cfg.worker_image.path)?;
    if !m.is_file()
        || m.file_type().is_symlink()
        || m.len() > 128 * 1024 * 1024
        || sha(&std::fs::read(&cfg.worker_image.path)?) != cfg.worker_image.sha256
    {
        return Err(crate::invalid("fixed_worker_image_mismatch"));
    }
    let expected = if cfg.backend.is_windows() {
        "rcu-mcp.exe"
    } else {
        "rcu-mcp"
    };
    if cfg.worker_image.path.file_name().and_then(|s| s.to_str()) != Some(expected) {
        return Err(crate::invalid("fixed_worker_basename"));
    }
    Ok(())
}
#[cfg(any(windows, test))]
fn lifecycle_metadata_stage(stage: Stage) -> bool {
    matches!(
        stage,
        Stage::Bootstrap | Stage::Preflight | Stage::Launch | Stage::Activate
    )
}
impl Worker {
    #[cfg(all(not(windows), not(feature = "fixture")))]
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        _cfg: &AgentConfig,
        _approval: &Approval,
        _stage: Stage,
        _budget: Budget,
        _state: &Path,
        _grants: &Directory,
        _input_grant: Value,
        _lifecycle_grant: Value,
        _root_epoch: Option<String>,
        _registry: &Registry,
    ) -> io::Result<Self> {
        Err(crate::invalid(
            "native_worker_requires_windows_fixture_not_compiled",
        ))
    }
    #[cfg(any(windows, feature = "fixture"))]
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn(
        cfg: &AgentConfig,
        approval: &Approval,
        stage: Stage,
        budget: Budget,
        state: &Path,
        grants: &Directory,
        input_grant: Value,
        lifecycle_grant: Value,
        root_epoch: Option<String>,
        registry: &Registry,
    ) -> io::Result<Self> {
        budget.check()?;
        image(cfg)?;
        let mut command = Command::new(&cfg.worker_image.path);
        command
            .env_clear()
            .current_dir(&cfg.install_root)
            .kill_on_drop(false)
            .stderr(Stdio::piped());
        #[cfg(windows)]
        // CREATE_NO_WINDOW only suppresses an extra console for this owned
        // worker (ignored for GUI-subsystem images); no token/session change.
        command.creation_flags(0x0800_0000);
        let pipe_id = format!("{}-{:?}-{}", approval.task, stage, uuid::Uuid::new_v4());
        #[cfg(windows)]
        let mut ready_binding = None;
        if cfg.backend.is_windows() {
            #[cfg(windows)]
            {
                let session = approval
                    .native_binding
                    .as_ref()
                    .ok_or_else(|| crate::invalid("native_binding_missing"))?;
                session
                    .require_same(
                        &rcu_windows::agent_binding::current_session().map_err(core_error)?,
                    )
                    .map_err(core_error)?;
                let owner =
                    rcu_windows::agent_binding::current_owner_proof().map_err(core_error)?;
                let foreground = if stage == Stage::Model {
                    let g: rcu_windows::InputGrant = serde_json::from_value(input_grant.clone())?;
                    g.validate(&session.native, crate::budget::now_ms())
                        .map_err(core_error)?;
                    Some(g.root)
                } else {
                    None
                };
                let spec = WorkerGuardSpec {
                    version: 1,
                    task: approval.task.clone(),
                    stage,
                    budget: budget.stage_spec(if stage == Stage::Model {
                        crate::budget::MODEL_WORKER_MS
                    } else {
                        stage.limit_ms()
                    })?,
                    owner,
                    input_blocked_path: grants
                        .path()
                        .join(format!("blocked-{}.json", approval.task)),
                    expected_foreground: foreground,
                    expected_root_native_epoch: root_epoch.clone(),
                    model_ready_path: (stage == Stage::Model).then(|| {
                        grants
                            .path()
                            .join(format!("ready-{}.json", sha(pipe_id.as_bytes())))
                    }),
                    expected_ui_value: approval.expected_ui_value.clone(),
                    request_prefix: approval.request_prefix.clone(),
                };
                let name = format!("guard-{}.json", sha(pipe_id.as_bytes()));
                let guard_bytes = serde_json::to_vec(&spec)?;
                grants.create_new(&name, &guard_bytes, 65536)?;
                if let Some(path) = &spec.model_ready_path {
                    ready_binding = Some((path.clone(), sha(&guard_bytes), approval.task.clone()));
                }
                command
                    .args(["--windows-desktop", "--state-dir"])
                    .arg(state)
                    .arg("--pipe-server")
                    .arg(&pipe_id)
                    .arg("--agent-guard")
                    .arg(grants.path().join(&name));
                if stage == Stage::Model {
                    let name = format!("input-{}.json", sha(pipe_id.as_bytes()));
                    grants.create_new(&name, &serde_json::to_vec(&input_grant)?, 16384)?;
                    command
                        .arg("--allow-input")
                        .arg("--windows-input-grant")
                        .arg(grants.path().join(name));
                }
                if lifecycle_metadata_stage(stage) {
                    let name = format!("lifecycle-{}.json", sha(pipe_id.as_bytes()));
                    grants.create_new(&name, &serde_json::to_vec(&lifecycle_grant)?, 65536)?;
                    command
                        .arg("--windows-lifecycle-grant")
                        .arg(grants.path().join(name));
                }
                command.stdin(Stdio::null()).stdout(Stdio::null());
            }
            #[cfg(not(windows))]
            {
                return Err(crate::invalid("windows_worker_on_non_windows"));
            }
        } else {
            #[cfg(not(feature = "fixture"))]
            {
                return Err(crate::invalid("fixture_backend_not_compiled"));
            }
            #[cfg(feature = "fixture")]
            {
                command
                    .arg("--synthetic-only")
                    .arg("--state-dir")
                    .arg(state)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped());
            }
        }
        let _ = (&pipe_id, grants, input_grant, lifecycle_grant, root_epoch);
        budget.check()?;
        let owned = spawn_registered(
            &mut command,
            registry,
            json!({"stage":stage,"image_sha256":cfg.worker_image.sha256,"owned_child":true}),
        )?;
        let setup_deadline =
            tokio::time::Instant::now() + budget.remaining().min(Duration::from_secs(5));
        let setup=async {
        let stderr=tokio::time::timeout_at(setup_deadline,owned.child.lock()).await.map_err(|_|crate::invalid("worker_setup_child_lock_timeout"))?.stderr.take().ok_or_else(||crate::invalid("worker_stderr_missing"))?;
        owned.start_stderr(stderr)?;
        let (input, output): (Write, Read) = if cfg.backend.is_windows() {
            #[cfg(windows)]
            {
                let child = tokio::time::timeout_at(setup_deadline,owned.child.lock()).await.map_err(|_|crate::invalid("worker_setup_child_lock_timeout"))?;
                let raw = child
                    .raw_handle()
                    .ok_or_else(|| crate::invalid("owned_worker_handle_missing"))?;
                    let retained = Arc::new(
                    unsafe {
                        rcu_windows::agent_binding::retain_owned_process(
                            raw,
                            &cfg.worker_image.path,
                        )
                    }
                        .map_err(core_error)?,
                    );
                    if retained.record().image_sha256 != cfg.worker_image.sha256 {
                        return Err(crate::invalid("spawned_worker_image_digest_mismatch"));
                    }
                drop(child);
                *owned
                    .record
                    .try_lock()
                    .map_err(|_| crate::invalid("owned_record_lock"))? =
                    serde_json::to_value(retained.record())?;
                *owned
                    .retained
                    .try_lock()
                    .map_err(|_| crate::invalid("owned_record_lock"))? = Some(retained.clone());
                let pipe = rcu_windows::pipe::connect_owned(
                    &pipe_id,
                    &retained,
                    approval.native_binding.as_ref().unwrap(),
                    setup_deadline.saturating_duration_since(tokio::time::Instant::now()),
                )
                .await
                .map_err(core_error)?;
                let (r, w) = tokio::io::split(pipe);
                (Box::new(w), Box::new(r))
            }
            #[cfg(not(windows))]
            {
                return Err(crate::invalid("unsupported_windows_worker"));
            }
        } else {
            let mut child = tokio::time::timeout_at(setup_deadline,owned.child.lock()).await.map_err(|_|crate::invalid("worker_setup_child_lock_timeout"))?;
            let i = child
                .stdin
                .take()
                .ok_or_else(|| crate::invalid("fixture_stdin_missing"))?;
            let o = child
                .stdout
                .take()
                .ok_or_else(|| crate::invalid("fixture_stdout_missing"))?;
            (Box::new(i), Box::new(o))
        };
        let identity = if cfg.backend.is_windows() {
            serde_json::to_value(&approval.native_binding.as_ref().unwrap().native)?
        } else {
            json!({"host":"fixture-host","guest":null,"session":"session-1","seat":"seat-0","desktop":"desktop-1"})
        };
        let ledger = Ledger::new(
            cfg.backend.target(),
            identity,
            stage == Stage::Model,
            &approval.request_prefix,
            &approval.expected_ui_value,
        );
        budget.check()?;
        Ok::<_,io::Error>((input,output,ledger))
        }.await;
        // Cancellation of this future also leaves child+collector retained by
        // Registry; every ordinary setup error uses the shared bounded cleanup.
        let (input, output, ledger) = complete_setup(&owned, setup).await?;
        Ok(Self {
            input,
            output: BufReader::new(output),
            ledger,
            owned,
            budget,
            audit_root: None,
            model_stage: stage == Stage::Model,
            ready_written: false,
            #[cfg(windows)]
            ready_binding,
        })
    }
    /// Called once, only after this worker's actual MCP/schema handshake. The
    /// guard reads this immutable private binding before granting native input.
    pub fn mark_model_ready(&mut self, spec: BudgetSpec) -> io::Result<()> {
        self.budget.check()?;
        if !self.model_stage
            || self.ready_written
            || spec.maximum_elapsed_ms != crate::budget::MODEL_USABLE_MS
            || spec.expires_unix_ms.saturating_sub(spec.issued_unix_ms)
                != crate::budget::MODEL_USABLE_MS
        {
            return Err(crate::invalid(
                "worker_model_ready_exact_nonrenewable_budget",
            ));
        }
        let ready = Budget::start(spec.clone())?;
        #[cfg(windows)]
        if let Some((path, guard_sha256, task)) = &self.ready_binding {
            let directory = Directory::open(
                path.parent()
                    .ok_or_else(|| crate::invalid("ready_parent"))?,
            )?;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| crate::invalid("ready_basename"))?;
            let bytes = serde_json::to_vec(&crate::types::ModelReadySpec {
                version: 1,
                task: task.clone(),
                stage: Stage::Model,
                guard_sha256: guard_sha256.clone(),
                budget: spec,
            })?;
            directory.create_new(name, &bytes, 65536)?;
            if directory.read(name, 65536)? != bytes {
                return Err(crate::invalid("ready_readback_mismatch"));
            }
        }
        self.budget = ready;
        self.ready_written = true;
        Ok(())
    }
    pub async fn rpc(&mut self, v: Value) -> io::Result<Value> {
        self.budget.check()?;
        self.ledger.request(&v)?;
        let mut raw = serde_json::to_vec(&v)?;
        raw.push(b'\n');
        net::write_frame(&mut self.input, &raw, frame::INBOUND).await?;
        if v.get("id").is_none() {
            return Ok(Value::Null);
        }
        let reply = tokio::time::timeout(
            self.budget.remaining().min(Duration::from_secs(30)),
            frame::read_raw_frame(&mut self.output, frame::OUTBOUND),
        )
        .await??
        .ok_or_else(|| crate::invalid("worker_eof_unknown"))?;
        if let Some(root) = &self.audit_root {
            self.owned.audit_response(root, &reply)?;
        }
        let value: Value = serde_json::from_slice(&reply)?;
        self.ledger.reply(&value, &reply)?;
        Ok(value)
    }
    pub async fn initialize(&mut self) -> io::Result<()> {
        self.rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"native-link-controller","version":"1"}}})).await?;
        self.rpc(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await?;
        self.rpc(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
            .await?;
        Ok(())
    }
    pub async fn finish(
        self,
        cfg: &AgentConfig,
        approval: &Approval,
    ) -> io::Result<crate::store::EndProof> {
        let mut proof = self.ledger.proof();
        let binding = self.owned.verify_binding(cfg, approval).is_ok();
        let Self {
            mut input,
            output,
            owned,
            ..
        } = self;
        let shutdown = tokio::time::timeout(Duration::from_secs(2), input.shutdown()).await;
        drop(input);
        drop(output);
        let exit = owned.stop().await;
        proof.owned_worker_exit_observed = exit.observed;
        proof.worker_exit_code = exit.code;
        proof.pipe_peer_bound = true;
        proof.native_binding_unchanged = binding;
        proof.evidence_level = if cfg.backend.is_windows() {
            "windows_native"
        } else {
            "actual_macos_owned_fixture_process"
        }
        .into();
        proof.resources_released &=
            exit.observed && !exit.forced && matches!(shutdown, Ok(Ok(()))) && exit.stderr.clean();
        proof.detail = json!({"owned_child_exit_observed":exit.observed,"forced_worker_termination":exit.forced,"cleanup_error":exit.error,"stderr":exit.stderr,"stop_elapsed_ms":exit.elapsed_ms,"stop_budget_ms":8000,"native_resources_not_inferred_from_exit":true,"user_application_terminated":false});
        Ok(proof)
    }
}

#[cfg(test)]
#[path = "worker/tests.rs"]
mod lifetime_tests;
