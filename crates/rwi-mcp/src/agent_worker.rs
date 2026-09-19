//! Optional guarded native worker route. It decorates the frozen backend and
//! lifecycle driver; no nineteen-tool service, UIA or input algorithm is replaced.
#[cfg(any(windows, test))]
use rwi_core::{
    Backend, Binding, DiscoveryQuery, Identity, Intent, Oracle, Predicate, Resolved, Scene, Scope,
    WindowSelection, lifecycle::*,
};
use rwi_core::{Result, fault};
#[cfg(any(windows, test))]
use rwi_native_link::{
    budget::{Budget, MODEL_END_MS, MODEL_USABLE_MS, MODEL_WORKER_MS, now_ms},
    types::{ModelReadySpec, Stage, WorkerGuardSpec, identifier},
};
#[cfg(any(windows, test))]
use rwi_windows::agent_contract::validate_local_dos_path;
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
                    .is_none_or(|hash| !rwi_windows::agent_contract::valid_sha256(hash))
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
                .is_none_or(|epoch| !rwi_windows::agent_contract::valid_sha256(epoch)))
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
        || !rwi_windows::agent_contract::valid_sha256(guard_sha256)
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
    fn(&mut B, &Binding, &Resolved) -> Result<rwi_windows::agent_contract::SelectedInputProof>;
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
    fn begin_activation_events(&mut self, window: &WindowIdentity) -> Result<()> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.begin_activation_events(window)
    }
    fn native_effect_count(&self) -> Option<u64> {
        self.inner.native_effect_count()
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
    fn window_presentation(&mut self, window: &WindowIdentity) -> Result<WindowPresentation> {
        self.guard.check(GuardOperation::Read)?;
        self.inner.window_presentation(window)
    }
    fn restore(&mut self, window: &WindowIdentity) -> Result<bool> {
        self.guard.check(GuardOperation::Activate)?;
        let result = self.inner.restore(window);
        if result.is_err() {
            self.guard.hold();
        }
        result
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
mod native;
#[cfg(windows)]
pub use native::*;
#[cfg(test)]
mod tests;
