//! Explicit application/window lifecycle. A launch observation never grants input.
use crate::{Fault, Identity, Result};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_WINDOWS: usize = 32;
pub const MAX_EVENTS: usize = 128;
pub const MAX_METADATA_BYTES: usize = 65_536;

/// Add caller provenance without changing the native fault or treating another
/// process's metadata read as proof of this driver's file access.
pub fn pin_failure_context(mut error: Fault, phase: &str, role: &str, path: &str) -> Fault {
    let diagnostic = error
        .diagnostic
        .get_or_insert_with(|| Box::new(serde_json::json!({})));
    if !diagnostic.is_object() {
        **diagnostic = serde_json::json!({"original_diagnostic": **diagnostic});
    }
    diagnostic["verification_context"] = serde_json::json!({
        "phase": phase, "role": role, "authorized_path": path,
    });
    error
}

#[path = "lifecycle_controller.rs"]
mod controller;
pub use controller::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplicationIdentity {
    pub executable: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WindowIdentity {
    pub window: String,
    pub pid: u32,
    pub process_created: u64,
    pub native_epoch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AllowedFile {
    pub path: String,
    pub sha256: String,
}

/// Read-only recognition of an already-running OS-managed packaged image.
/// Never authorizes launching this path or weakens launcher/allowed-file pins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObservedPackage {
    pub application: ApplicationIdentity,
    pub package_full_name: String,
    pub install_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LaunchPermission {
    pub authorization_id: String,
    pub application: ApplicationIdentity,
    pub arguments: Vec<String>,
    pub allowed_files: Vec<AllowedFile>,
    /// Explicit images that may own the UI after a broker hands off the launch.
    pub observed_applications: Vec<ApplicationIdentity>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub observed_packages: Vec<ObservedPackage>,
    /// Existing app windows that this launch may reuse. Others block dispatch.
    pub allowed_existing_windows: Vec<WindowIdentity>,
    /// Normal startup may restore app-managed session state; never claims clean tabs.
    pub application_startup_effects_authorized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActivationPermission {
    pub authorization_id: String,
    pub window: WindowIdentity,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LifecycleGrant {
    pub run_id: String,
    pub route_vm_id: String,
    pub expected_identity: Identity,
    pub expires_unix_ms: u64,
    pub launches: Vec<LaunchPermission>,
    pub activations: Vec<ActivationPermission>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WindowEvidence {
    pub key: WindowIdentity,
    pub application: ApplicationIdentity,
    pub title: String,
    pub foreground: bool,
    /// Missing facts in historical receipts never imply an interactive target.
    #[serde(default)]
    pub candidate_facts: Option<WindowCandidateFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WindowCandidateFacts {
    pub visible: bool,
    pub cloaked: Option<bool>,
    pub owned: bool,
    pub owner_chain_complete: bool,
}
impl WindowEvidence {
    pub fn interactive_root(&self) -> bool {
        self.candidate_facts.as_ref().is_some_and(|facts| {
            facts.visible
                && facts.cloaked == Some(false)
                && !facts.owned
                && facts.owner_chain_complete
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Inventory {
    pub identity: Identity,
    pub windows: Vec<WindowEvidence>,
    pub complete: bool,
    pub native_windows_enumerated: usize,
    pub captured_unix_ms: u64,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NativeEvent {
    pub event: String,
    pub window: String,
    pub event_time_ms: u32,
    pub thread_id: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct EventBatch {
    pub events: Vec<NativeEvent>,
    pub complete: bool,
    pub hooks_released: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct NativeLaunch {
    pub api: String,
    pub accepted: bool,
    pub process_id: Option<u32>,
    pub process_created: Option<u64>,
    pub process_handles_released: bool,
    pub error: Option<Fault>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LaunchState {
    NewWindowObserved,
    ReusedExistingWindow,
    LaunchedNoWindow,
    Ambiguous,
    Uncertain,
    NotDispatched,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LaunchReceipt {
    pub request_id: String,
    pub authorization_id: String,
    pub state: LaunchState,
    pub before: Option<Inventory>,
    pub after: Option<Inventory>,
    pub native_events: EventBatch,
    pub native_launch: Option<NativeLaunch>,
    pub candidates: Vec<WindowIdentity>,
    pub correlation: String,
    pub error: Option<Fault>,
    pub replay: String,
    pub resources_released: bool,
    #[serde(default)]
    pub resource_release_uncertain: bool,
    pub goal_verified: bool,
    pub input_authority_granted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActivationReceipt {
    pub request_id: String,
    pub authorization_id: String,
    pub window: WindowIdentity,
    pub state: String,
    pub api_accepted: Option<bool>,
    pub before_foreground: Option<WindowIdentity>,
    pub after_foreground: Option<WindowIdentity>,
    pub user_intervened: bool,
    /// Bounded observation evidence, not a lossless record of all user activity.
    #[serde(default)]
    pub native_events: EventBatch,
    pub error: Option<Fault>,
    pub goal_verified: bool,
    pub input_authority_granted: bool,
    pub replay: String,
    #[serde(default)]
    pub resources_released: bool,
    #[serde(default)]
    pub resource_release_uncertain: bool,
}

/// Read-only diagnostic: no durable launch reservation, focus change or input exists here.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PreflightReceipt {
    pub authorization_id: String,
    pub ready_for_launch: bool,
    pub inventory: Option<Inventory>,
    pub native_events: EventBatch,
    pub error: Option<Fault>,
    pub resources_released: bool,
    pub resource_release_uncertain: bool,
    pub reservation_created: bool,
    pub launch_dispatched: bool,
    pub activation_dispatched: bool,
    pub input_dispatched: bool,
    pub goal_verified: bool,
}

/// Native adapter boundary. Test drivers are Rust only and never run an OS action.
/// Drivers must unhook/close their own native resources on every exit including Drop.
pub trait LifecycleDriver {
    fn identity(&mut self) -> Result<Identity>;
    fn verify_application(&mut self, app: &ApplicationIdentity) -> Result<()>;
    fn verify_file(&mut self, file: &AllowedFile) -> Result<()>;
    fn verify_observed_package(&mut self, _: &ObservedPackage, _: &[WindowIdentity]) -> Result<()> {
        Err(crate::fault(
            "observed_package_proof_unsupported",
            "This driver cannot prove an already-running managed package; no fallback.",
        ))
    }
    fn inventory(&mut self, apps: &[ApplicationIdentity]) -> Result<Inventory>;
    fn begin_events(&mut self) -> Result<()>;
    /// Observe target lifetime and desktop foreground, not unrelated window traffic.
    fn begin_activation_events(&mut self, _: &WindowIdentity) -> Result<()> {
        self.begin_events()
    }
    /// Cumulative events; missing/overflowed coverage must remain incomplete.
    fn events(&mut self) -> Result<EventBatch>;
    fn end_events(&mut self) -> Result<EventBatch>;
    fn launch(&mut self, permission: &LaunchPermission) -> Result<NativeLaunch>;
    fn foreground(&mut self) -> Result<Option<WindowIdentity>>;
    fn user_input_tick(&mut self) -> Result<u64>;
    fn check_window(&mut self, window: &WindowIdentity) -> Result<()>;
    /// SetForegroundWindow ONCE, never AttachThreadInput/key injection/foreground retry.
    fn activate(&mut self, window: &WindowIdentity) -> Result<bool>;
    /// Optional proof that a rejected launch/activation never entered its native
    /// effect API. None preserves conservative uncertainty for unknown drivers.
    fn native_effect_count(&self) -> Option<u64> {
        None
    }
    fn pause(&mut self, milliseconds: u64);
}
