use super::*;
use crate::{digest, fault, operation};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[path = "lifecycle_preflight.rs"]
mod readonly_preflight;
pub use readonly_preflight::preflight;

#[path = "lifecycle_classification.rs"]
mod classification;
use classification::classify;

#[cfg(test)]
#[path = "lifecycle_test_support.rs"]
mod test_support;

fn verify_permission_files(
    driver: &mut dyn LifecycleDriver,
    permission: &LaunchPermission,
    phase: &str,
) -> Result<Vec<ApplicationIdentity>> {
    let app = &permission.application;
    driver
        .verify_application(app)
        .map_err(|e| pin_failure_context(e, phase, "selected_launcher", &app.executable))?;
    for file in &permission.allowed_files {
        driver
            .verify_file(file)
            .map_err(|e| pin_failure_context(e, phase, "allowed_file", &file.path))?;
    }
    let mut apps = vec![app.clone()];
    for app in &permission.observed_applications {
        driver
            .verify_application(app)
            .map_err(|e| pin_failure_context(e, phase, "observed_application", &app.executable))?;
        if !apps.contains(app) {
            apps.push(app.clone());
        }
    }
    for package in &permission.observed_packages {
        driver
            .verify_observed_package(package, &permission.allowed_existing_windows)
            .map_err(|e| {
                pin_failure_context(
                    e,
                    phase,
                    "observed_package",
                    &package.application.executable,
                )
            })?;
        if !apps.contains(&package.application) {
            apps.push(package.application.clone());
        }
    }
    Ok(apps)
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
}
pub fn valid_windows_path(s: &str) -> bool {
    let p = s.strip_prefix(r"\\?\").unwrap_or(s);
    let b = p.as_bytes();
    b.len() >= 4
        && s.encode_utf16().count() <= 4096
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && b[2] == b'\\'
        && !p[3..].contains(['/', ':', '\0'])
        && p[3..].split('\\').count() <= 64
        && p[3..]
            .split('\\')
            .all(|c| !c.is_empty() && c != "." && c != ".." && !c.ends_with([' ', '.']))
}
fn hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn path_key(path: &str) -> String {
    path.strip_prefix(r"\\?\")
        .unwrap_or(path)
        .to_ascii_lowercase()
}
fn valid_window(w: &WindowIdentity) -> bool {
    !w.window.is_empty()
        && w.window.len() <= 32
        && w.pid > 0
        && w.process_created > 0
        && !w.native_epoch.is_empty()
        && w.native_epoch.len() <= 128
}
impl LifecycleGrant {
    pub fn validate(&self, identity: &Identity, now: u64) -> Result<()> {
        if &self.expected_identity != identity {
            return Err(fault(
                "session_mismatch",
                "The lifecycle grant must match actual native host/guest/session/seat/desktop.",
            ));
        }
        if self.expires_unix_ms <= now || self.expires_unix_ms.saturating_sub(now) > 3_600_000 {
            return Err(fault(
                "lifecycle_grant_expired",
                "Use an explicitly allocated future expiry of at most one hour; never extend after a clock rollback.",
            ));
        }
        if !identifier(&self.run_id)
            || (!self.route_vm_id.is_empty()
                && uuid::Uuid::parse_str(&self.route_vm_id)
                    .ok()
                    .is_none_or(|u| u.to_string() != self.route_vm_id))
            || self.launches.len() > 8
            || self.activations.len() > 32
            || (self.launches.is_empty() && self.activations.is_empty())
        {
            return Err(fault(
                "invalid_lifecycle_grant",
                "Bound run ID, configured VM UUID route and explicit permissions; no implicit targets.",
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        for p in &self.launches {
            let app_valid =
                |a: &ApplicationIdentity| valid_windows_path(&a.executable) && hash(&a.sha256);
            if !identifier(&p.authorization_id)
                || !ids.insert(&p.authorization_id)
                || !app_valid(&p.application)
                || !p.application_startup_effects_authorized
                || p.observed_applications.len() + p.observed_packages.len() > 8
                || !p.observed_applications.iter().all(app_valid)
                || !p.observed_packages.iter().all(|package| {
                    let name = &package.package_full_name;
                    let root = &package.install_path;
                    app_valid(&package.application)
                        && !name.is_empty()
                        && name.len() <= 127
                        && name
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                        && valid_windows_path(root)
                        && path_key(&package.application.executable)
                            .starts_with(&(path_key(root) + "\\"))
                        && !p.allowed_existing_windows.is_empty()
                        && path_key(&package.application.executable)
                            != path_key(&p.application.executable)
                        && !p
                            .allowed_files
                            .iter()
                            .any(|f| path_key(&f.path) == path_key(&package.application.executable))
                        && !p.observed_applications.iter().any(|a| {
                            path_key(&a.executable) == path_key(&package.application.executable)
                        })
                        && p.observed_packages
                            .iter()
                            .filter(|q| {
                                path_key(&q.application.executable)
                                    == path_key(&package.application.executable)
                            })
                            .count()
                            == 1
                })
                || p.allowed_files.len() > 16
                || !p
                    .allowed_files
                    .iter()
                    .all(|f| valid_windows_path(&f.path) && hash(&f.sha256))
                || p.allowed_existing_windows.len() > MAX_WINDOWS
                || !p.allowed_existing_windows.iter().all(valid_window)
                || p.arguments.len() > 32
                || p.arguments
                    .iter()
                    .map(|a| a.encode_utf16().count())
                    .sum::<usize>()
                    > 8192
                || p.arguments.iter().any(|a| a.contains('\0'))
            {
                return Err(fault(
                    "invalid_launch_permission",
                    "Specify exact local absolute app/file paths, SHA-256, argv and bounded allowed existing HWND/PID/birth/epoch tuples. Normal startup/session restore needs explicit authorization.",
                ));
            }
        }
        for p in &self.activations {
            if !identifier(&p.authorization_id)
                || !ids.insert(&p.authorization_id)
                || !valid_window(&p.window)
            {
                return Err(fault(
                    "invalid_activation_permission",
                    "Each activation must name one exact native HWND/PID/birth/epoch.",
                ));
            }
        }
        if serde_json::to_vec(self)
            .map_err(|_| fault("invalid_lifecycle_grant", "Grant serialization failed."))?
            .len()
            > MAX_METADATA_BYTES
        {
            return Err(fault("invalid_lifecycle_grant", "Grant exceeds 64 KiB."));
        }
        Ok(())
    }
}

/// Immutable reservation before native dispatch. A new request ID cannot replay a launch authorization.
#[derive(Serialize, Deserialize)]
struct Record {
    fingerprint: String,
    receipt: serde_json::Value,
}
struct Reservation {
    path: PathBuf,
    fingerprint: String,
}
fn journal_path(
    root: &Path,
    g: &LifecycleGrant,
    kind: &str,
    authorization: &str,
    request: &str,
) -> PathBuf {
    let once = if kind == "launch" {
        authorization
    } else {
        request
    };
    let route_key = uuid::Uuid::parse_str(&g.route_vm_id)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| g.route_vm_id.clone());
    let key = digest(
        &serde_json::to_vec(&(&g.expected_identity, &g.run_id, &route_key, kind, once))
            .expect("serializable"),
    );
    root.join("lifecycle").join(format!("{key}.json"))
}
fn read_record(path: &Path) -> Result<Record> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((MAX_METADATA_BYTES + 4097) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_METADATA_BYTES + 4096 {
        return Err(fault(
            "uncertain_journal",
            "Oversized lifecycle receipt; never replay the reservation.",
        ));
    }
    serde_json::from_slice(&bytes).map_err(|_| {
        fault(
            "uncertain_journal",
            "Interrupted lifecycle reservation exists. Never replay; inspect native state.",
        )
    })
}
fn write_record(path: &Path, record: &Record) -> Result<()> {
    let bytes = serde_json::to_vec(record)
        .map_err(|_| fault("receipt_error", "Never replay a lifecycle dispatch."))?;
    if bytes.len() > MAX_METADATA_BYTES + 4096 {
        return Err(fault(
            "receipt_budget",
            "Lifecycle receipt exceeds bounded journal size; reservation remains no-replay.",
        ));
    }
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)?;
    f.write_all(&bytes)?;
    f.sync_all()?;
    #[cfg(unix)]
    File::open(path.parent().expect("journal parent"))?.sync_all()?;
    Ok(())
}
impl Reservation {
    fn create(path: PathBuf, fingerprint: String, receipt: serde_json::Value) -> Result<Self> {
        fs::create_dir_all(path.parent().expect("journal parent"))?;
        #[cfg(unix)]
        File::open(path.parent().and_then(Path::parent).expect("state root"))?.sync_all()?;
        write_record(
            &path,
            &Record {
                fingerprint: fingerprint.clone(),
                receipt,
            },
        )?;
        Ok(Self { path, fingerprint })
    }
    fn finish(&self, receipt: impl Serialize) -> Result<()> {
        write_record(
            &self.path.with_extension("result.json"),
            &Record {
                fingerprint: self.fingerprint.clone(),
                receipt: serde_json::to_value(receipt)
                    .map_err(|_| fault("receipt_error", "Read reservation; do not replay."))?,
            },
        )
    }
}
fn existing(path: &Path, fingerprint: &str) -> Result<Option<serde_json::Value>> {
    if !path.exists() {
        return Ok(None);
    }
    let first = read_record(path)?;
    if first.fingerprint != fingerprint {
        return Err(fault(
            "idempotency_conflict",
            "The once-only authorization/request was reserved for different exact arguments. Do not replay.",
        ));
    }
    let final_path = path.with_extension("result.json");
    let r = if final_path.exists() {
        read_record(&final_path)?
    } else {
        first
    };
    if r.fingerprint != fingerprint {
        return Err(fault(
            "uncertain_journal",
            "Mismatched result; never replay.",
        ));
    }
    let mut value = r.receipt;
    value["replay"] = "forbidden_deduplicated".into();
    Ok(Some(value))
}
pub fn receipt(
    root: &Path,
    g: &LifecycleGrant,
    kind: &str,
    authorization: &str,
    request: &str,
) -> Result<serde_json::Value> {
    if !identifier(request)
        || (kind != "launch" && kind != "activation")
        || !(if kind == "launch" {
            g.launches
                .iter()
                .any(|p| p.authorization_id == authorization)
        } else {
            g.activations
                .iter()
                .any(|p| p.authorization_id == authorization)
        })
    {
        return Err(fault(
            "unknown_lifecycle_permission",
            "Use a permission from this connection grant and its request ID.",
        ));
    }
    let path = journal_path(root, g, kind, authorization, request);
    let final_path = path.with_extension("result.json");
    let first = read_record(&path)?;
    let r = if final_path.exists() {
        let final_record = read_record(&final_path)?;
        if final_record.fingerprint != first.fingerprint {
            return Err(fault(
                "uncertain_journal",
                "Final lifecycle sidecar does not match its durable reservation; never report it as a trusted result or replay.",
            ));
        }
        final_record
    } else {
        first
    };
    if r.receipt["authorization_id"] != authorization {
        return Err(fault(
            "receipt_scope_mismatch",
            "Request belongs to another authorization.",
        ));
    }
    Ok(r.receipt)
}
fn check(d: &mut dyn LifecycleDriver, g: &LifecycleGrant, stage: &str) -> Result<()> {
    operation::checkpoint(stage)?;
    g.validate(&d.identity()?, now_ms())
}
fn inventory_ok(i: &Inventory, apps: &[ApplicationIdentity], g: &LifecycleGrant) -> Result<()> {
    if i.identity != g.expected_identity {
        return Err(fault(
            "session_mismatch",
            "Native window inventory crossed sessions.",
        ));
    }
    if !i.complete
        || i.windows.len() > MAX_WINDOWS
        || serde_json::to_vec(i)
            .expect("inventory serialization")
            .len()
            > 20_000
        || i.windows
            .iter()
            .any(|w| !valid_window(&w.key) || !apps.contains(&w.application))
    {
        return Err(fault(
            "incomplete_window_inventory",
            "Use computer.lifecycle_preflight for bounded native counts, coverage failure details and cleanup state. Partial or oversized inventory is never absence.",
        ));
    }
    let mut keys = std::collections::BTreeSet::new();
    if !i.windows.iter().all(|w| {
        keys.insert((
            &w.key.window,
            w.key.pid,
            w.key.process_created,
            &w.key.native_epoch,
        ))
    }) {
        return Err(fault(
            "duplicate_window_inventory",
            "Native inventory contains duplicated identities.",
        ));
    }
    Ok(())
}
fn event_ok(e: &EventBatch) -> Result<()> {
    if !e.complete
        || e.events.len() > MAX_EVENTS
        || serde_json::to_vec(e).expect("event serialization").len() > 20_000
    {
        return Err(fault(
            "incomplete_native_events",
            "Event overflow, missing hooks or partial coverage makes lifecycle attribution uncertain.",
        ));
    }
    Ok(())
}
fn cleanup_fault() -> Fault {
    fault(
        "native_resource_release_uncertain",
        "At least one native resource cleanup failed or is unproven. Do not report success, resume native actions or replay; inspect computer.operation and the durable receipt.",
    )
}
fn ensure_cleanup_clean(audit: &operation::CleanupAudit) -> Result<()> {
    if audit.uncertain() {
        Err(cleanup_fault())
    } else {
        Ok(())
    }
}
struct EventScope<'a> {
    d: &'a mut dyn LifecycleDriver,
    finished: bool,
}
impl EventScope<'_> {
    fn end(&mut self) -> Result<EventBatch> {
        let r = self.d.end_events();
        self.finished = true;
        if r.as_ref().ok().is_none_or(|e| !e.hooks_released) {
            operation::resource_release_uncertain();
        }
        r
    }
}
impl Drop for EventScope<'_> {
    fn drop(&mut self) {
        if !self.finished
            && self
                .d
                .end_events()
                .as_ref()
                .ok()
                .is_none_or(|e| !e.hooks_released)
        {
            operation::resource_release_uncertain();
        }
    }
}

pub struct LaunchRequest<'a> {
    pub authorization_id: &'a str,
    pub request_id: &'a str,
    pub timeout_ms: u64,
}
/// Called only while Engine holds the cooperative desktop lease and monotonic grant deadline.
pub fn launch(
    root: &Path,
    g: &LifecycleGrant,
    d: &mut dyn LifecycleDriver,
    req: LaunchRequest<'_>,
) -> Result<LaunchReceipt> {
    if !identifier(req.request_id) || !(1..=5000).contains(&req.timeout_ms) {
        return Err(fault(
            "invalid_lifecycle_request",
            "Use bounded request ID and 1..5000ms observation horizon.",
        ));
    }
    let p = g
        .launches
        .iter()
        .find(|p| p.authorization_id == req.authorization_id)
        .ok_or_else(|| {
            fault(
                "unknown_lifecycle_permission",
                "Choose one exact preauthorized application launch.",
            )
        })?;
    let path = journal_path(root, g, "launch", req.authorization_id, req.request_id);
    let fingerprint =
        digest(&serde_json::to_vec(&(p, req.timeout_ms)).expect("launch permission serialization"));
    if let Some(value) = existing(&path, &fingerprint)? {
        return serde_json::from_value(value)
            .map_err(|_| fault("uncertain_journal", "Never replay a corrupt receipt."));
    }
    let cleanup = operation::CleanupAudit::begin();
    check(d, g, "lifecycle_preflight")?;
    let mut scope = EventScope { d, finished: false };
    // Open cleanup scope before verification, so pinned resources release on preflight errors.
    let apps = verify_permission_files(scope.d, p, "launch_preflight")?;
    scope.d.begin_events()?;
    let before = scope.d.inventory(&apps)?;
    inventory_ok(&before, &apps, g)?;
    if before
        .windows
        .iter()
        .any(|w| !p.allowed_existing_windows.contains(&w.key))
    {
        return Err(fault(
            "existing_window_not_authorized",
            "Normal launch may reuse an existing window. Allocate its exact native identity first; never close it or invent a clean-window switch.",
        ));
    }
    let baseline = scope.d.events()?;
    event_ok(&baseline)?;
    check(scope.d, g, "lifecycle_before_reservation")?;
    let mut r = LaunchReceipt {
        request_id: req.request_id.into(),
        authorization_id: req.authorization_id.into(),
        state: LaunchState::Uncertain,
        before: Some(before),
        after: None,
        native_events: baseline.clone(),
        native_launch: None,
        candidates: vec![],
        correlation:
            "Durably reserved; outcome unknown until native evidence is saved. No automatic replay."
                .into(),
        error: None,
        replay: "forbidden".into(),
        resources_released: false,
        resource_release_uncertain: false,
        goal_verified: false,
        input_authority_granted: false,
    };
    let reservation = Reservation::create(
        path,
        fingerprint,
        serde_json::to_value(&r).expect("receipt serialization"),
    )?;
    let mut attempted = false;
    let outcome = (|| -> Result<()> {
        check(scope.d, g, "lifecycle_before_dispatch")?;
        let check_inventory = scope.d.inventory(&apps)?;
        inventory_ok(&check_inventory, &apps, g)?;
        if check_inventory.windows != r.before.as_ref().expect("before exists").windows {
            return Err(fault(
                "window_inventory_changed",
                "Existing window inventory changed before launch. Yield; do not dispatch.",
            ));
        }
        check(scope.d, g, "lifecycle_native_dispatch")?;
        ensure_cleanup_clean(&cleanup)?;
        let call = operation::native_call("CreateProcessW")?;
        attempted = true;
        r.native_launch = Some(scope.d.launch(p)?);
        drop(call);
        if !r.native_launch.as_ref().expect("launch exists").accepted {
            r.state = LaunchState::NotDispatched;
            r.error = r.native_launch.as_ref().and_then(|n| n.error.clone());
            return Ok(());
        }
        let start = Instant::now();
        loop {
            check(scope.d, g, "lifecycle_observe")?;
            let after = scope.d.inventory(&apps)?;
            inventory_ok(&after, &apps, g)?;
            r.after = Some(after);
            r.native_events = scope.d.events()?;
            event_ok(&r.native_events)?;
            if start.elapsed() >= Duration::from_millis(req.timeout_ms) {
                break;
            }
            scope.d.pause(
                25.min(
                    req.timeout_ms
                        .saturating_sub(start.elapsed().as_millis() as u64),
                )
                .max(1),
            );
        }
        Ok(())
    })();
    let observation_ok = outcome.is_ok();
    if let Err(e) = outcome {
        r.state = if attempted {
            LaunchState::Uncertain
        } else {
            LaunchState::NotDispatched
        };
        r.error = Some(e);
    }
    let pre_cleanup_event_count = r.native_events.events.len();
    match scope.end() {
        Ok(events) => {
            r.resources_released = events.hooks_released
                && r.native_launch
                    .as_ref()
                    .is_none_or(|n| n.process_handles_released);
            if let Err(e) = event_ok(&events) {
                r.state = LaunchState::Uncertain;
                r.error = Some(e);
            }
            r.native_events = events;
        }
        Err(e) => {
            r.state = LaunchState::Uncertain;
            r.error = Some(e);
        }
    }
    r.resource_release_uncertain = cleanup.uncertain();
    r.resources_released &= !r.resource_release_uncertain;
    if !r.resources_released {
        operation::resource_release_uncertain();
        r.state = LaunchState::Uncertain;
        r.resource_release_uncertain = true;
        if r.error.is_none() {
            r.error = Some(cleanup_fault());
        }
    }
    if observation_ok
        && r.error.is_none()
        && r.resources_released
        && r.native_events.complete
        && r.native_launch.as_ref().is_some_and(|n| n.accepted)
    {
        classify(&mut r, baseline.events.len());
        if r.state != LaunchState::Ambiguous
            && r.native_events
                .events
                .iter()
                .skip(pre_cleanup_event_count)
                .any(|e| e.event == "create" || e.event == "destroy")
        {
            r.state = LaunchState::Uncertain;
            r.correlation="Native creation/destruction arrived during final unhook drain, after the last complete inventory; do not infer a final stable window identity.".into();
        }
    }
    reservation.finish(&r)?;
    Ok(r)
}

pub struct ActivateRequest<'a> {
    pub authorization_id: &'a str,
    pub request_id: &'a str,
    pub expected_foreground: &'a Option<WindowIdentity>,
}
pub fn activate(
    root: &Path,
    g: &LifecycleGrant,
    d: &mut dyn LifecycleDriver,
    req: ActivateRequest<'_>,
) -> Result<ActivationReceipt> {
    if !identifier(req.request_id)
        || req
            .expected_foreground
            .as_ref()
            .is_some_and(|w| !valid_window(w))
    {
        return Err(fault(
            "invalid_lifecycle_request",
            "Use a bounded request ID and fresh exact foreground tuple (null only when no foreground exists).",
        ));
    }
    let p = g
        .activations
        .iter()
        .find(|p| p.authorization_id == req.authorization_id)
        .ok_or_else(|| {
            fault(
                "unknown_lifecycle_permission",
                "Choose a preauthorized exact window activation.",
            )
        })?;
    let path = journal_path(root, g, "activation", req.authorization_id, req.request_id);
    let fingerprint = digest(
        &serde_json::to_vec(&(p, req.expected_foreground)).expect("activation serialization"),
    );
    if let Some(value) = existing(&path, &fingerprint)? {
        return serde_json::from_value(value).map_err(|_| {
            fault(
                "uncertain_journal",
                "Never replay a corrupt activation receipt.",
            )
        });
    }
    let cleanup = operation::CleanupAudit::begin();
    let mut scope = EventScope { d, finished: false };
    check(scope.d, g, "activation_preflight")?;
    scope.d.check_window(&p.window)?;
    let input = scope.d.user_input_tick()?;
    let before = scope.d.foreground()?;
    if &before != req.expected_foreground {
        return Err(fault(
            "foreground_changed",
            "Yield to user/other app; obtain a fresh observation rather than stealing focus.",
        ));
    }
    let mut r = ActivationReceipt {
        request_id: req.request_id.into(),
        authorization_id: req.authorization_id.into(),
        window: p.window.clone(),
        state: "uncertain".into(),
        api_accepted: None,
        before_foreground: before,
        after_foreground: None,
        user_intervened: false,
        error: None,
        goal_verified: false,
        input_authority_granted: false,
        replay: "forbidden".into(),
        resources_released: false,
        resource_release_uncertain: false,
    };
    let reservation = Reservation::create(
        path,
        fingerprint,
        serde_json::to_value(&r).expect("activation receipt serialization"),
    )?;
    let mut attempted = false;
    let result = (|| -> Result<()> {
        check(scope.d, g, "activation_before_dispatch")?;
        scope.d.check_window(&p.window)?;
        if scope.d.user_input_tick()? != input || scope.d.foreground()? != *req.expected_foreground
        {
            r.user_intervened = true;
            r.state = "yielded".into();
            return Ok(());
        }
        if req.expected_foreground.as_ref() == Some(&p.window) {
            r.after_foreground = req.expected_foreground.clone();
            r.state = "already_foreground".into();
            return Ok(());
        }
        ensure_cleanup_clean(&cleanup)?;
        let call = operation::native_call("SetForegroundWindow_once")?;
        attempted = true;
        r.api_accepted = Some(scope.d.activate(&p.window)?);
        drop(call);
        r.after_foreground = scope.d.foreground()?;
        if scope.d.user_input_tick()? != input {
            r.user_intervened = true;
            r.state = "yielded".into();
            return Ok(());
        }
        check(scope.d, g, "activation_verify")?;
        scope.d.check_window(&p.window)?;
        r.state = if r.after_foreground.as_ref() == Some(&p.window) {
            "foreground_observed"
        } else {
            "not_foreground"
        }
        .into();
        Ok(())
    })();
    if let Err(e) = result {
        r.state = if attempted {
            "uncertain"
        } else {
            "not_dispatched"
        }
        .into();
        r.error = Some(e);
    }
    match scope.end() {
        Ok(events) => r.resources_released = events.hooks_released,
        Err(e) => r.error = Some(e),
    }
    r.resource_release_uncertain = cleanup.uncertain() || !r.resources_released;
    r.resources_released &= !r.resource_release_uncertain;
    if r.resource_release_uncertain {
        operation::resource_release_uncertain();
        r.state = "uncertain".into();
        if r.error.is_none() {
            r.error = Some(cleanup_fault());
        }
    }
    reservation.finish(&r)?;
    Ok(r)
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod offline_tests;
