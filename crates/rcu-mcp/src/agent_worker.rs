//! Optional guarded native worker route. It decorates the frozen backend and
//! lifecycle driver; no nineteen-tool service, UIA or input algorithm is replaced.
#[cfg(any(windows, test))]
use rcu_core::{
    Backend, Binding, DiscoveryQuery, Identity, Intent, Oracle, Predicate, Resolved, Scene, Scope,
    WindowSelection, lifecycle::*,
};
use rcu_core::{Result, fault};
#[cfg(any(windows, test))]
use rcu_native_link::{
    budget::{Budget, MODEL_END_MS, MODEL_USABLE_MS, MODEL_WORKER_MS, now_ms},
    types::{ModelReadySpec, Stage, WorkerGuardSpec, identifier},
};
#[cfg(any(windows, test))]
use rcu_windows::agent_contract::validate_local_dos_path;
#[cfg(any(windows, test))]
use serde_json::{Value, json};
use std::path::Path;
#[cfg(any(windows, test))]
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

#[cfg(any(windows, test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardOperation {
    Read,
    Input,
    Launch,
    Activate,
}

pub fn validate_guard_cli(
    path: &Path,
    windows: bool,
    pipe: Option<&str>,
    macos: bool,
) -> Result<()> {
    if !path.is_absolute() || !windows || pipe.is_none_or(str::is_empty) || macos {
        return Err(fault(
            "agent_worker_cli_required",
            "--agent-guard requires an absolute private spec, --windows-desktop and one --pipe-server; no macOS/stdio/native fallback.",
        ));
    }
    Ok(())
}

#[cfg(any(windows, test))]
pub fn validate_spec(spec: &WorkerGuardSpec, guard_path: &str) -> Result<()> {
    validate_local_dos_path(guard_path)?;
    let blocked = spec.input_blocked_path.to_str().ok_or_else(|| {
        fault(
            "agent_worker_marker_path_invalid",
            "Blocked marker must have an exact local Unicode path.",
        )
    })?;
    validate_local_dos_path(blocked)?;
    let parent = guard_path.rsplit_once('\\').map(|x| x.0).ok_or_else(|| {
        fault(
            "agent_worker_guard_path_invalid",
            "Guard must be a file in the approved private run directory.",
        )
    })?;
    let expected_blocked = format!("{parent}\\blocked-{}.json", spec.task);
    if spec.version != 1
        || !identifier(&spec.task, 64)
        || !spec.stage.worker_stage()
        || spec.request_prefix != format!("{}-", spec.task)
        || spec.expected_ui_value.len() > 16384
        || blocked != expected_blocked
        || blocked == guard_path
        || spec.budget.maximum_elapsed_ms
            > if spec.stage == Stage::Model {
                MODEL_WORKER_MS
            } else {
                spec.stage.limit_ms()
            }
    {
        return Err(fault(
            "agent_worker_spec_invalid",
            "Require a strict locally approved worker stage, task prefix, bounded goal/budget and same-directory exact blocked marker.",
        ));
    }
    spec.owner.validate()?;
    match (spec.stage, &spec.model_ready_path) {
        (Stage::Model, Some(path)) => {
            let path = path.to_str().ok_or_else(|| {
                fault(
                    "agent_worker_ready_path_invalid",
                    "Ready path must be an exact local DOS path.",
                )
            })?;
            validate_local_dos_path(path)?;
            let (dir, name) = path.rsplit_once('\\').ok_or_else(|| {
                fault(
                    "agent_worker_ready_path_invalid",
                    "Ready file must be in the guard directory.",
                )
            })?;
            if dir != parent
                || name
                    .strip_prefix("ready-")
                    .and_then(|name| name.strip_suffix(".json"))
                    .is_none_or(|hash| !rcu_windows::agent_contract::valid_sha256(hash))
            {
                return Err(fault(
                    "agent_worker_ready_path_invalid",
                    "Ready file requires the same protected directory and exact ready-<sha256>.json basename.",
                ));
            }
        }
        (Stage::Model, None) | (_, Some(_)) => {
            return Err(fault(
                "agent_worker_ready_path_invalid",
                "Only Model requires a private ready record; no missing or cross-stage path.",
            ));
        }
        (_, None) => {}
    }
    spec.budget
        .validate(now_ms())
        .map_err(|e| fault("agent_worker_budget_invalid", &e.to_string()))?;
    if spec.stage == Stage::Model
        && (spec.expected_foreground.is_none()
            || spec
                .expected_root_native_epoch
                .as_deref()
                .is_none_or(|epoch| !rcu_windows::agent_contract::valid_sha256(epoch)))
    {
        return Err(fault(
            "agent_worker_model_root_required",
            "Model input requires the exact allocated root window/PID/process birth.",
        ));
    }
    if let Some(root) = &spec.expected_foreground {
        let token = root.window.strip_prefix("hwnd:").unwrap_or("");
        if token.is_empty()
            || token.len() > 16
            || !token.bytes().all(|b| b.is_ascii_hexdigit())
            || u64::from_str_radix(token, 16).ok().is_none_or(|v| v == 0)
            || root.pid == 0
            || root.process_created == 0
        {
            return Err(fault(
                "agent_worker_model_root_invalid",
                "Use the native root HWND/PID/process birth, not a title or replacement process.",
            ));
        }
    }
    Ok(())
}

#[cfg(any(windows, test))]
pub fn validate_model_ready(
    guard: &WorkerGuardSpec,
    guard_sha256: &str,
    ready: &ModelReadySpec,
    now: u64,
) -> Result<()> {
    ready
        .budget
        .validate(now)
        .map_err(|e| fault("agent_worker_ready_budget_invalid", &e.to_string()))?;
    if guard.stage != Stage::Model
        || ready.version != 1
        || ready.stage != Stage::Model
        || ready.task != guard.task
        || ready.guard_sha256 != guard_sha256
        || !rcu_windows::agent_contract::valid_sha256(guard_sha256)
        || ready.budget.maximum_elapsed_ms != MODEL_USABLE_MS
        || ready
            .budget
            .expires_unix_ms
            .checked_sub(ready.budget.issued_unix_ms)
            != Some(MODEL_USABLE_MS)
        || ready.budget.issued_unix_ms < guard.budget.issued_unix_ms
        || ready.budget.expires_unix_ms > guard.budget.expires_unix_ms.saturating_sub(MODEL_END_MS)
        || ready.budget.clock_drift_tolerance_ms > guard.budget.clock_drift_tolerance_ms
    {
        return Err(fault(
            "agent_worker_ready_binding_invalid",
            "Require this exact task/guard digest and a full absolute 1200-second ready window, bounded by the worker END reserve; never renew on late reads.",
        ));
    }
    Ok(())
}

/// Pure stage policy, also used by the real guard before native delegation.
#[cfg(any(windows, test))]
pub fn operation_allowed(stage: Stage, operation: GuardOperation) -> bool {
    match operation {
        GuardOperation::Read => stage.worker_stage(),
        GuardOperation::Input => stage == Stage::Model,
        GuardOperation::Launch => stage == Stage::Launch,
        GuardOperation::Activate => stage == Stage::Activate,
    }
}

#[cfg(any(windows, test))]
pub struct GuardGate {
    stage: Stage,
    budget: Budget,
    held: AtomicBool,
}
#[cfg(any(windows, test))]
impl GuardGate {
    pub fn new(stage: Stage, budget: Budget) -> Self {
        Self {
            stage,
            budget,
            held: AtomicBool::new(false),
        }
    }
    pub fn check(
        &self,
        operation: GuardOperation,
        native_checks: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let result = (|| {
            if self.held.load(Ordering::Acquire) {
                return Err(fault(
                    "agent_worker_sticky_hold",
                    "This worker cannot resume after uncertainty, revocation or binding loss.",
                ));
            }
            if !operation_allowed(self.stage, operation) {
                return Err(fault(
                    "agent_worker_wrong_stage",
                    "Native operation is outside the locally approved worker stage.",
                ));
            }
            self.budget
                .check()
                .map_err(|e| fault("agent_worker_budget_hold", &e.to_string()))?;
            native_checks()?;
            self.budget
                .check()
                .map_err(|e| fault("agent_worker_budget_hold", &e.to_string()))
        })();
        if result.is_err() {
            self.hold();
        }
        result
    }
    pub fn hold(&self) {
        self.held.store(true, Ordering::Release);
    }
}

#[cfg(any(windows, test))]
pub trait GuardChecks: Send + Sync {
    fn check(&self, operation: GuardOperation) -> Result<()>;
    fn hold(&self);
    fn deadline(&self) -> Instant;
    fn metadata(&self) -> Value;
}

#[cfg(any(windows, test))]
pub struct GuardedBackend<B: Backend> {
    inner: B,
    guard: Arc<dyn GuardChecks>,
    input_proof: Option<InputProofCheck<B>>,
}
#[cfg(any(windows, test))]
type InputProofCheck<B> =
    fn(&mut B, &Binding, &Resolved) -> Result<rcu_windows::agent_contract::SelectedInputProof>;
#[cfg(any(windows, test))]
impl<B: Backend> GuardedBackend<B> {
    pub fn new(inner: B, guard: Arc<dyn GuardChecks>) -> Self {
        Self {
            inner,
            guard,
            input_proof: None,
        }
    }
    fn with_input_proof(mut self, check: InputProofCheck<B>) -> Self {
        self.input_proof = Some(check);
        self
    }
}

#[cfg(any(windows, test))]
impl<B: Backend> Backend for GuardedBackend<B> {
    fn identity(&self) -> Identity {
        self.inner.identity()
    }
    fn capabilities(&self) -> Value {
        if let Err(error) = self.guard.check(GuardOperation::Read) {
            return json!({"backend":"windows-native-agent-held","input":false,"guard_error":error,"agent_worker":self.guard.metadata()});
        }
        let mut capabilities = self.inner.capabilities();
        if let Some(object) = capabilities.as_object_mut() {
            object.insert("agent_worker".into(), self.guard.metadata());
        }
        capabilities
    }
    fn lifecycle_grant(&self) -> Result<LifecycleGrant> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.lifecycle_grant()
    }
    fn lifecycle_deadline(&self) -> Result<Instant> {
        self.guard.check(GuardOperation::Read)?;
        Ok(self.inner.lifecycle_deadline()?.min(self.guard.deadline()))
    }
    fn lifecycle_driver(&mut self) -> Result<Box<dyn LifecycleDriver>> {
        self.guard.check(GuardOperation::Read)?;
        Ok(Box::new(GuardedLifecycleDriver {
            inner: self.inner.lifecycle_driver()?,
            guard: self.guard.clone(),
        }))
    }
    fn validate_activation(&self, window: &WindowIdentity) -> Result<()> {
        self.guard.check(GuardOperation::Activate)?;
        self.inner.validate_activation(window)
    }
    fn observe(&mut self) -> Result<Scene> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.observe()
    }
    fn observe_region(&mut self, region: [u32; 4]) -> Result<Scene> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.observe_region(region)
    }
    fn discover(&mut self) -> Result<Value> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.discover()
    }
    fn discover_with(&mut self, query: &DiscoveryQuery) -> Result<Value> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.discover_with(query)
    }
    fn bind_window(&mut self, selection: &WindowSelection) -> Result<Value> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.bind_window(selection)
    }
    fn revalidate(&mut self, reference: &Resolved) -> Result<Scene> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.revalidate(reference)
    }
    fn act(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        intent: &Intent,
        value: Option<&str>,
    ) -> Result<Option<bool>> {
        self.guard.check(GuardOperation::Input)?;
        if let Some(check) = self.input_proof {
            let proof = check(&mut self.inner, expected, target)
                .and_then(|proof| proof.require_target(expected, target));
            if let Err(error) = proof {
                self.guard.hold();
                return Err(error);
            }
        }
        self.guard.check(GuardOperation::Input)?;
        let result = self.inner.act(expected, target, intent, value);
        if result.is_err() {
            self.guard.hold();
        }
        result
    }
    fn inspect(&mut self, scope: &Scope, predicate: &Predicate, expected: &str) -> Result<Oracle> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.inspect(scope, predicate, expected)
    }
}

#[cfg(any(windows, test))]
struct GuardedLifecycleDriver {
    inner: Box<dyn LifecycleDriver>,
    guard: Arc<dyn GuardChecks>,
}
#[cfg(any(windows, test))]
impl LifecycleDriver for GuardedLifecycleDriver {
    fn identity(&mut self) -> Result<Identity> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.identity()
    }
    fn verify_application(&mut self, app: &ApplicationIdentity) -> Result<()> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.verify_application(app)
    }
    fn verify_file(&mut self, file: &AllowedFile) -> Result<()> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.verify_file(file)
    }
    fn verify_observed_package(
        &mut self,
        package: &ObservedPackage,
        allocated: &[WindowIdentity],
    ) -> Result<()> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.verify_observed_package(package, allocated)
    }
    fn inventory(&mut self, apps: &[ApplicationIdentity]) -> Result<Inventory> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.inventory(apps)
    }
    fn begin_events(&mut self) -> Result<()> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.begin_events()
    }
    fn events(&mut self) -> Result<EventBatch> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.events()
    }
    // Release-only cleanup is deliberately not gated by expired authorization.
    fn end_events(&mut self) -> Result<EventBatch> {
        self.inner.end_events()
    }
    fn launch(&mut self, permission: &LaunchPermission) -> Result<NativeLaunch> {
        self.guard.check(GuardOperation::Launch)?;
        let result = self.inner.launch(permission);
        if result.is_err() {
            self.guard.hold();
        }
        result
    }
    fn foreground(&mut self) -> Result<Option<WindowIdentity>> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.foreground()
    }
    fn user_input_tick(&mut self) -> Result<u64> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.user_input_tick()
    }
    fn check_window(&mut self, window: &WindowIdentity) -> Result<()> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.check_window(window)
    }
    fn activate(&mut self, window: &WindowIdentity) -> Result<bool> {
        self.guard.check(GuardOperation::Activate)?;
        let result = self.inner.activate(window);
        if result.is_err() {
            self.guard.hold();
        }
        result
    }
    fn pause(&mut self, milliseconds: u64) {
        if self.guard.check(GuardOperation::Read).is_ok() {
            self.inner.pause(
                milliseconds.min(
                    self.guard
                        .deadline()
                        .saturating_duration_since(Instant::now())
                        .as_millis() as u64,
                ),
            );
        }
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use rcu_windows::{
        InputGrant, NativeWindows,
        agent_binding::{
            PeerProcess, PrivateDirectory, current_session, read_private_file_no_reparse,
        },
    };
    use std::{path::PathBuf, sync::OnceLock};

    static GUARD: OnceLock<Arc<WindowsGuard>> = OnceLock::new();
    pub struct WindowsGuard {
        spec: WorkerGuardSpec,
        gate: GuardGate,
        source_sha256: String,
        _directory: PrivateDirectory,
        peer: OnceLock<Arc<PeerProcess>>,
        model_ready: OnceLock<(String, Budget)>,
    }
    impl WindowsGuard {
        pub fn load(path: &Path) -> Result<Arc<Self>> {
            let bytes = read_private_file_no_reparse(path, 65536)?;
            let spec: WorkerGuardSpec = serde_json::from_slice(&bytes).map_err(|_| fault("agent_worker_spec_decode", "Worker guard uses the exact locally approved schema; no unknown fields or remote replacement."))?;
            validate_spec(
                &spec,
                path.to_str().ok_or_else(|| {
                    fault(
                        "agent_worker_guard_path_invalid",
                        "Guard path is not valid Unicode.",
                    )
                })?,
            )?;
            spec.owner.session.require_same(&current_session()?)?;
            let directory = PrivateDirectory::open(path.parent().ok_or_else(|| {
                fault(
                    "agent_worker_guard_path_invalid",
                    "Guard must live under a private run directory.",
                )
            })?)?;
            let budget = Budget::start(spec.budget.clone())
                .map_err(|e| fault("agent_worker_budget_invalid", &e.to_string()))?;
            let gate = GuardGate::new(spec.stage, budget);
            let guard = Arc::new(Self {
                spec,
                gate,
                source_sha256: rcu_core::digest(&bytes),
                _directory: directory,
                peer: OnceLock::new(),
                model_ready: OnceLock::new(),
            });
            guard.check_before_peer()?;
            Ok(guard)
        }
        pub fn owner(&self) -> &rcu_windows::agent_contract::OwnerProof {
            &self.spec.owner
        }
        pub fn connection_budget(&self) -> Duration {
            self.gate.budget.remaining().min(Duration::from_secs(120))
        }
        fn check_before_peer(&self) -> Result<()> {
            self.gate
                .budget
                .check()
                .map_err(|e| fault("agent_worker_budget_hold", &e.to_string()))?;
            match std::fs::symlink_metadata(&self.spec.input_blocked_path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                _ => {
                    return Err(fault(
                        "agent_worker_revocation_hold",
                        "Revocation marker exists or its absence cannot be proven; do not dispatch.",
                    ));
                }
            }
            self.spec.owner.session.require_same(&current_session()?)
        }
        pub fn bind_peer(&self, peer: PeerProcess) -> Result<()> {
            self.check_before_peer()?;
            peer.verify_current()?;
            if peer.record() != &self.spec.owner.process {
                return Err(fault(
                    "agent_worker_owner_mismatch",
                    "Connected pipe client differs from the exact local owner proof.",
                ));
            }
            self.peer.set(Arc::new(peer)).map_err(|_| {
                fault(
                    "agent_worker_peer_already_bound",
                    "The native worker permits exactly one pipe owner; no reconnect.",
                )
            })?;
            self.check(GuardOperation::Read)
        }
        pub fn constrain_grants(
            &self,
            input: &mut Option<InputGrant>,
            lifecycle: &mut Option<LifecycleGrant>,
        ) -> Result<()> {
            if self.spec.stage == Stage::Model {
                let grant = input.as_ref().ok_or_else(|| {
                    fault(
                        "agent_worker_input_grant_required",
                        "Model stage requires its separate original native input grant.",
                    )
                })?;
                if Some(&grant.root) != self.spec.expected_foreground.as_ref()
                    || grant.expected_identity != self.spec.owner.session.native
                {
                    return Err(fault(
                        "agent_worker_input_scope_mismatch",
                        "The native input grant and guard must bind the same root and native session.",
                    ));
                }
            } else if input.is_some() {
                return Err(fault(
                    "agent_worker_input_wrong_stage",
                    "Only Model stage can carry an input grant.",
                ));
            }
            // Narrow only: original per-gesture and lifecycle checks retain a
            // monotonic deadline no later than this worker's remaining budget.
            let expires = now_ms()
                .saturating_add(self.gate.budget.remaining().as_millis() as u64)
                .min(self.spec.budget.expires_unix_ms);
            if let Some(grant) = input {
                grant.expires_unix_ms = grant.expires_unix_ms.min(expires);
            }
            if let Some(grant) = lifecycle {
                if grant.expected_identity != self.spec.owner.session.native {
                    return Err(fault(
                        "agent_worker_lifecycle_scope_mismatch",
                        "Lifecycle and worker guard native identities differ.",
                    ));
                }
                grant.expires_unix_ms = grant.expires_unix_ms.min(expires);
            }
            Ok(())
        }
    }
    impl WindowsGuard {
        fn check_model_ready(&self) -> Result<()> {
            let path = self.spec.model_ready_path.as_ref().ok_or_else(|| {
                fault(
                    "agent_worker_model_not_ready",
                    "Model has no private ready record; no input before MCP handshake completes.",
                )
            })?;
            let bytes = read_private_file_no_reparse(path, 16384)?;
            let hash = rcu_core::digest(&bytes);
            if let Some((expected, budget)) = self.model_ready.get() {
                if expected != &hash {
                    return Err(fault(
                        "agent_worker_ready_changed",
                        "Ready authority changed after first observation; never renew or replace the model budget.",
                    ));
                }
                return budget
                    .check()
                    .map_err(|e| fault("agent_worker_ready_budget_hold", &e.to_string()));
            }
            let ready: ModelReadySpec = serde_json::from_slice(&bytes).map_err(|_| {
                fault(
                    "agent_worker_ready_decode",
                    "Private ready record must use the exact strict schema.",
                )
            })?;
            validate_model_ready(&self.spec, &self.source_sha256, &ready, now_ms())?;
            // Absolute expires-now limits a late first read; it cannot grant a
            // fresh full duration merely because no input was attempted yet.
            let budget = Budget::start(ready.budget)
                .map_err(|e| fault("agent_worker_ready_budget_hold", &e.to_string()))?;
            let _ = self.model_ready.set((hash.clone(), budget));
            let (expected, budget) = self.model_ready.get().ok_or_else(|| {
                fault(
                    "agent_worker_ready_unknown",
                    "Could not retain immutable model ready budget.",
                )
            })?;
            if expected != &hash {
                return Err(fault(
                    "agent_worker_ready_changed",
                    "Concurrent ready observations differed.",
                ));
            }
            budget
                .check()
                .map_err(|e| fault("agent_worker_ready_budget_hold", &e.to_string()))
        }
    }
    impl GuardChecks for WindowsGuard {
        fn check(&self, operation: GuardOperation) -> Result<()> {
            self.gate.check(operation, || {
                self.check_before_peer()?;
                self.peer
                    .get()
                    .ok_or_else(|| {
                        fault(
                            "agent_worker_peer_not_bound",
                            "No original native call is allowed before owned pipe binding.",
                        )
                    })?
                    .verify_current()?;
                // HWND root equality is not a selected-window proof: a legitimate
                if operation == GuardOperation::Input {
                    self.check_model_ready()?;
                }
                // owned dialog has a different HWND. The actual NativeWindows
                // callback and its per-gesture checks validate that exact scope.
                Ok(())
            })
        }
        fn hold(&self) {
            self.gate.hold();
        }
        fn deadline(&self) -> Instant {
            self.gate.budget.deadline()
        }
        fn metadata(&self) -> Value {
            json!({"version":1,"task":self.spec.task,"stage":self.spec.stage,"request_prefix":self.spec.request_prefix,"expected_ui_goal":self.spec.expected_ui_value,"guard_sha256":self.source_sha256,"held":self.gate.held.load(Ordering::Acquire),"pipe_owner_bound":self.peer.get().is_some(),"model_foreground_policy":"immutable allocated root; explicit fresh native selected root/owned-popup/return, full owner chain, token/session/desktop and UIA proof; per-balanced-gesture revalidation","native_runtime_verified":false})
        }
    }
    pub fn install(guard: Arc<WindowsGuard>) -> Result<()> {
        GUARD.set(guard).map_err(|_| {
            fault(
                "agent_worker_guard_already_installed",
                "Guard authority is immutable for this single worker process.",
            )
        })
    }
    pub fn installed() -> Option<&'static Arc<WindowsGuard>> {
        GUARD.get()
    }
    pub fn wrap(
        mut inner: NativeWindows,
        guard: Arc<WindowsGuard>,
    ) -> Result<GuardedBackend<NativeWindows>> {
        if guard.spec.stage == Stage::Model {
            let dispatch_guard = guard.clone();
            inner.configure_agent_input_scope(
                guard.spec.owner.session.clone(),
                guard.spec.expected_root_native_epoch.clone(),
                Arc::new(move || dispatch_guard.check(GuardOperation::Input)),
            )?;
            Ok(GuardedBackend::new(inner, guard)
                .with_input_proof(NativeWindows::fresh_agent_selected_proof))
        } else {
            Ok(GuardedBackend::new(inner, guard))
        }
    }
    pub fn private_grant(path: PathBuf, cap: usize) -> Result<Vec<u8>> {
        read_private_file_no_reparse(&path, cap)
    }
}
#[cfg(windows)]
pub use native::*;

#[cfg(test)]
mod tests {
    use super::*;
    use rcu_windows::agent_contract::{
        AuthenticationId, OwnerProof, ProcessBinding, SessionBinding,
    };
    fn spec() -> WorkerGuardSpec {
        let session = SessionBinding {
            native: Identity {
                host: format!("windows-machine:{}", "1".repeat(64)),
                guest: None,
                session: "wts:1:logon:123:luid:0:456".into(),
                seat: "WinSta0".into(),
                desktop: "Default".into(),
            },
            user_sid: "S-1-5-21-1-2-3-1001".into(),
            session_id: 1,
            authentication_id: AuthenticationId {
                high: 0,
                low: 0x456,
            },
            logon_time_100ns: 123,
            input_desktop: "Default".into(),
        };
        WorkerGuardSpec {
            version: 1,
            task: "test-task".into(),
            stage: Stage::Model,
            budget: rcu_native_link::budget::BudgetSpec {
                issued_unix_ms: now_ms(),
                expires_unix_ms: now_ms() + 60_000,
                maximum_elapsed_ms: 60_000,
                clock_drift_tolerance_ms: 100,
            },
            owner: OwnerProof {
                process: ProcessBinding {
                    pid: 123,
                    creation_filetime_100ns: 456,
                    user_sid: session.user_sid.clone(),
                    session_id: 1,
                    authentication_id: session.authentication_id,
                    image_path: r"C:\private\agent.exe".into(),
                    image_sha256: "2".repeat(64),
                },
                session,
            },
            input_blocked_path: r"C:\private\blocked-test-task.json".into(),
            expected_foreground: Some(rcu_windows::NativeWindowKey {
                window: "hwnd:123".into(),
                pid: 1234,
                process_created: 999,
            }),
            expected_root_native_epoch: Some("3".repeat(64)),
            model_ready_path: Some(format!(r"C:\private\ready-{}.json", "4".repeat(64)).into()),
            expected_ui_value: "approved value".into(),
            request_prefix: "test-task-".into(),
        }
    }
    #[test]
    fn guard_spec_rejects_unknown_wrong_stage_marker_task_prefix_and_grant() {
        let original = spec();
        validate_spec(&original, r"C:\private\guard.json").unwrap();
        let mut bad = vec![original.clone(); 9];
        bad[0].version = 2;
        bad[1].stage = Stage::Cleanup;
        bad[2].request_prefix = "other-".into();
        bad[3].input_blocked_path = r"C:\other\blocked-test-task.json".into();
        bad[4].task = "../escape".into();
        bad[5].expected_foreground = None;
        bad[6].owner.process.creation_filetime_100ns = 0;
        bad[7].budget.expires_unix_ms = now_ms() - 1;
        bad[8].input_blocked_path = r"C:\private\other.json".into();
        for candidate in bad {
            assert!(validate_spec(&candidate, r"C:\private\guard.json").is_err());
        }
        let mut value = serde_json::to_value(original).unwrap();
        value["remote_command"] = "shell".into();
        assert!(serde_json::from_value::<WorkerGuardSpec>(value).is_err());
    }
    #[test]
    fn cli_and_stage_are_separate_fail_closed_gates() {
        let absolute = std::env::temp_dir().join("guard.json");
        validate_guard_cli(&absolute, true, Some("local-id"), false).unwrap();
        for (windows, pipe, macos) in [
            (false, Some("p"), false),
            (true, None, false),
            (true, Some("p"), true),
        ] {
            assert!(validate_guard_cli(&absolute, windows, pipe, macos).is_err());
        }
        assert!(validate_guard_cli(Path::new("relative"), true, Some("p"), false).is_err());
        for stage in [
            Stage::Prepare,
            Stage::Bootstrap,
            Stage::Preflight,
            Stage::Launch,
            Stage::Activate,
            Stage::Model,
            Stage::Oracle,
            Stage::Cleanup,
        ] {
            assert_eq!(
                operation_allowed(stage, GuardOperation::Input),
                stage == Stage::Model
            );
            assert_eq!(
                operation_allowed(stage, GuardOperation::Launch),
                stage == Stage::Launch
            );
            assert_eq!(
                operation_allowed(stage, GuardOperation::Activate),
                stage == Stage::Activate
            );
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn actual_native_feature_cli_refuses_missing_route_relative_path_and_non_windows() {
        let current = std::env::current_exe().unwrap();
        let binary = current.parent().unwrap().parent().unwrap().join("rcu-mcp");
        assert!(
            binary.is_file(),
            "cargo test -p rcu-mcp must build the actual binary for the original stdio integration suite"
        );
        let absolute = std::env::temp_dir()
            .join("never-read-rcu-guard.json")
            .to_str()
            .unwrap()
            .to_owned();
        for (args, code) in [
            (
                vec!["--agent-guard", absolute.as_str()],
                "agent_worker_cli_required",
            ),
            (
                vec![
                    "--agent-guard",
                    "relative",
                    "--windows-desktop",
                    "--pipe-server",
                    "offline",
                ],
                "agent_worker_cli_required",
            ),
            (
                vec![
                    "--agent-guard",
                    absolute.as_str(),
                    "--windows-desktop",
                    "--pipe-server",
                    "offline",
                ],
                "unsupported_platform",
            ),
            (
                vec![
                    "--agent-guard",
                    absolute.as_str(),
                    "--agent-guard",
                    absolute.as_str(),
                ],
                "invalid_cli",
            ),
        ] {
            let output = std::process::Command::new(&binary)
                .args(args)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains(code));
        }
    }

    struct TestGuard {
        gate: GuardGate,
        native_allowed: AtomicBool,
    }
    impl TestGuard {
        fn new(stage: Stage) -> Arc<Self> {
            Arc::new(Self {
                gate: GuardGate::new(stage, Budget::start(spec().budget).unwrap()),
                native_allowed: AtomicBool::new(true),
            })
        }
    }
    impl GuardChecks for TestGuard {
        fn check(&self, op: GuardOperation) -> Result<()> {
            self.gate.check(op, || {
                if self.native_allowed.load(Ordering::Acquire) {
                    Ok(())
                } else {
                    Err(fault(
                        "simulated_native_fact_hold",
                        "Offline peer/session/marker/focus negative; not a native query.",
                    ))
                }
            })
        }
        fn hold(&self) {
            self.gate.hold();
        }
        fn deadline(&self) -> Instant {
            self.gate.budget.deadline()
        }
        fn metadata(&self) -> Value {
            json!({"test_only":true,"held":self.gate.held.load(Ordering::Acquire)})
        }
    }

    #[test]
    fn guard_native_fact_loss_is_sticky_and_prevents_delegation() {
        let fixture = rcu_core::fixture::Fixture::default();
        let guard = TestGuard::new(Stage::Model);
        let mut backend = GuardedBackend::new(fixture.clone(), guard.clone());
        backend.observe().unwrap();
        assert_eq!(backend.capabilities()["agent_worker"]["test_only"], true);
        guard.native_allowed.store(false, Ordering::Release);
        assert!(backend.observe().is_err());
        guard.native_allowed.store(true, Ordering::Release);
        assert!(backend.observe().is_err());
        assert_eq!(backend.capabilities()["input"], false);
        assert_eq!(fixture.0.lock().unwrap().calls, 0);
    }

    #[test]
    fn wrapped_frozen_fixture_dispatches_once_and_uncertainty_holds_new_effects() {
        for disconnect in [false, true] {
            let state = tempfile::tempdir().unwrap();
            let fixture = rcu_core::fixture::Fixture::default();
            fixture.0.lock().unwrap().disconnect = disconnect;
            let guard = TestGuard::new(Stage::Model);
            let mut engine = rcu_core::Engine::new(state.path().into()).unwrap();
            engine
                .register(
                    "fixture".into(),
                    Box::new(GuardedBackend::new(fixture.clone(), guard)),
                )
                .unwrap();
            let frame = engine.observe("fixture").unwrap().frame;
            let lease = engine.claim("fixture", 60_000).unwrap();
            let receipt = engine
                .act_checked(rcu_core::CheckedAction {
                    target: "fixture",
                    frame: &frame,
                    element: "Input",
                    lease: &lease,
                    request: "worker-once",
                    intent: Intent::Text,
                    value: Some("worker original model".into()),
                })
                .unwrap();
            assert_eq!(
                receipt.dispatch,
                if disconnect { "uncertain" } else { "returned" }
            );
            assert!(!receipt.goal_verified);
            if disconnect {
                assert!(engine.observe("fixture").is_err());
                assert!(
                    engine
                        .act_checked(rcu_core::CheckedAction {
                            target: "fixture",
                            frame: &frame,
                            element: "Input",
                            lease: &lease,
                            request: "new-id",
                            intent: Intent::Text,
                            value: Some("must not happen".into())
                        })
                        .is_err()
                );
            } else {
                let result = engine
                    .verify(rcu_core::Verification {
                        target: "fixture",
                        frame: &frame,
                        element: Some("Input".into()),
                        request: Some("worker-once".into()),
                        predicate: Predicate::ElementValue,
                        expected: "worker original model",
                        timeout_ms: 100,
                        stable_samples: 1,
                    })
                    .unwrap();
                assert_eq!(result["goal_verified"], true);
            }
            engine.release(&lease).unwrap();
            let actual = fixture.0.lock().unwrap();
            assert_eq!(actual.calls, 1);
            assert_eq!(actual.value, "worker original model");
        }
    }

    #[test]
    fn shared_guard_gate_preserves_deadline_and_wrong_stage_hold() {
        let gate = GuardGate::new(Stage::Preflight, Budget::start(spec().budget).unwrap());
        assert!(gate.check(GuardOperation::Read, || Ok(())).is_ok());
        assert!(
            gate.check(GuardOperation::Launch, || panic!(
                "wrong stage must precede native code"
            ))
            .is_err()
        );
        assert!(
            gate.check(GuardOperation::Read, || panic!("HOLD must not resume"))
                .is_err()
        );
        let mut budget = spec().budget;
        budget.maximum_elapsed_ms = 1;
        let gate = GuardGate::new(Stage::Model, Budget::start(budget).unwrap());
        std::thread::sleep(Duration::from_millis(3));
        assert!(
            gate.check(GuardOperation::Input, || panic!(
                "expired budget must precede native code"
            ))
            .is_err()
        );
    }

    #[test]
    fn model_ready_requires_exact_absolute_full_window_and_cannot_renew_late() {
        let now = now_ms();
        let mut guard = spec();
        guard.budget.issued_unix_ms = now - 90_000;
        guard.budget.expires_unix_ms = guard.budget.issued_unix_ms + MODEL_WORKER_MS;
        guard.budget.maximum_elapsed_ms = MODEL_WORKER_MS;
        let hash = "a".repeat(64);
        let ready = ModelReadySpec {
            version: 1,
            task: guard.task.clone(),
            stage: Stage::Model,
            guard_sha256: hash.clone(),
            budget: rcu_native_link::budget::BudgetSpec {
                issued_unix_ms: now - 50_000,
                expires_unix_ms: now - 50_000 + MODEL_USABLE_MS,
                maximum_elapsed_ms: MODEL_USABLE_MS,
                clock_drift_tolerance_ms: 100,
            },
        };
        validate_model_ready(&guard, &hash, &ready, now).unwrap();
        let budget = Budget::start(ready.budget.clone()).unwrap();
        assert!(budget.remaining() <= Duration::from_millis(MODEL_USABLE_MS - 50_000));
        assert!(validate_model_ready(&guard, &hash, &ready, ready.budget.expires_unix_ms).is_err());
        let mut bad = vec![ready.clone(); 8];
        bad[0].task = "other-task".into();
        bad[1].guard_sha256 = "b".repeat(64);
        bad[2].stage = Stage::Activate;
        bad[3].budget.maximum_elapsed_ms -= 1;
        bad[4].budget.expires_unix_ms += 1;
        bad[5].budget.issued_unix_ms = guard.budget.issued_unix_ms - 1;
        bad[5].budget.expires_unix_ms = bad[5].budget.issued_unix_ms + MODEL_USABLE_MS;
        bad[6].budget.clock_drift_tolerance_ms += 1;
        bad[7].budget.issued_unix_ms = now + 50_000;
        bad[7].budget.expires_unix_ms = bad[7].budget.issued_unix_ms + MODEL_USABLE_MS;
        for (i, value) in bad.iter().enumerate() {
            assert!(
                validate_model_ready(&guard, &hash, value, now).is_err(),
                "negative {i}"
            );
        }
        let mut missing = guard.clone();
        missing.model_ready_path = None;
        assert!(validate_spec(&missing, r"C:\private\guard.json").is_err());
        missing.model_ready_path = Some(format!(r"C:\other\ready-{}.json", "4".repeat(64)).into());
        assert!(validate_spec(&missing, r"C:\private\guard.json").is_err());
    }

    #[test]
    fn selected_scope_unknown_holds_before_backend_dispatch_and_never_recovers() {
        fn unknown(
            _: &mut rcu_core::fixture::Fixture,
            _: &Binding,
            _: &Resolved,
        ) -> Result<rcu_windows::agent_contract::SelectedInputProof> {
            Err(fault(
                "simulated_selected_scope_unknown",
                "Pure negative callback, no native observation.",
            ))
        }
        let state = tempfile::tempdir().unwrap();
        let fixture = rcu_core::fixture::Fixture::default();
        let guard = TestGuard::new(Stage::Model);
        let mut engine = rcu_core::Engine::new(state.path().into()).unwrap();
        engine
            .register(
                "fixture".into(),
                Box::new(
                    GuardedBackend::new(fixture.clone(), guard.clone()).with_input_proof(unknown),
                ),
            )
            .unwrap();
        let frame = engine.observe("fixture").unwrap().frame;
        let lease = engine.claim("fixture", 60_000).unwrap();
        let receipt = engine
            .act_checked(rcu_core::CheckedAction {
                target: "fixture",
                frame: &frame,
                element: "Input",
                lease: &lease,
                request: "scope-never-dispatched",
                intent: Intent::Text,
                value: Some("never".into()),
            })
            .unwrap();
        assert!(!receipt.goal_verified);
        assert_eq!(fixture.0.lock().unwrap().calls, 0);
        assert!(guard.gate.held.load(Ordering::Acquire));
        assert!(engine.observe("fixture").is_err());
        engine.release(&lease).unwrap();
    }

    struct CleanupDriver(Arc<std::sync::atomic::AtomicUsize>);
    impl LifecycleDriver for CleanupDriver {
        fn identity(&mut self) -> Result<Identity> {
            Ok(spec().owner.session.native)
        }
        fn verify_application(&mut self, _: &ApplicationIdentity) -> Result<()> {
            Ok(())
        }
        fn verify_file(&mut self, _: &AllowedFile) -> Result<()> {
            Ok(())
        }
        fn inventory(&mut self, _: &[ApplicationIdentity]) -> Result<Inventory> {
            Err(fault("not_tested", "No inventory simulated."))
        }
        fn begin_events(&mut self) -> Result<()> {
            Ok(())
        }
        fn events(&mut self) -> Result<EventBatch> {
            Ok(EventBatch::default())
        }
        fn end_events(&mut self) -> Result<EventBatch> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(EventBatch {
                events: vec![],
                complete: true,
                hooks_released: true,
            })
        }
        fn launch(&mut self, _: &LaunchPermission) -> Result<NativeLaunch> {
            panic!("held launch must not delegate")
        }
        fn foreground(&mut self) -> Result<Option<WindowIdentity>> {
            Ok(None)
        }
        fn user_input_tick(&mut self) -> Result<u64> {
            Ok(0)
        }
        fn check_window(&mut self, _: &WindowIdentity) -> Result<()> {
            Ok(())
        }
        fn activate(&mut self, _: &WindowIdentity) -> Result<bool> {
            panic!("held activation must not delegate")
        }
        fn pause(&mut self, _: u64) {
            panic!("held pause must not delegate")
        }
    }
    #[test]
    fn lifecycle_cleanup_is_delegated_after_hold_without_authorizing_input() {
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let guard = TestGuard::new(Stage::Launch);
        let mut driver = GuardedLifecycleDriver {
            inner: Box::new(CleanupDriver(count.clone())),
            guard: guard.clone(),
        };
        driver.begin_events().unwrap();
        guard.hold();
        assert!(driver.events().is_err());
        assert!(driver.identity().is_err());
        driver.pause(50);
        assert!(driver.end_events().unwrap().hooks_released);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
}
