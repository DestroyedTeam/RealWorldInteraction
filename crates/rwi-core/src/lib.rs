//! Rust-only intent engine. OS input lives behind a target-bound native Backend.

mod foreground;
pub use foreground::ForegroundState;
mod checked_action;
#[cfg(test)]
mod journal_tail_regression;
mod live;
mod pixel_anchor;
mod visual;
pub use checked_action::CheckedAction;
pub use live::*;
pub use pixel_anchor::{PixelAnchor, RedMarkerAnchor};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    time::{Duration, Instant},
};
use uuid::Uuid;
pub use visual::*;
pub mod device;
#[cfg(any(test, feature = "fixture"))]
pub mod fixture;
pub mod input_trajectory;
pub mod lease;
pub mod lifecycle;
pub mod operation;
pub mod operation_journal;
#[cfg(test)]
mod pixel_anchor_tests;
#[cfg(test)]
mod tests;
pub mod trajectory;

pub const INLINE_BYTES: usize = 8 * 1024 * 1024;
pub const ARTIFACT_BUDGET: usize = 64 * 1024 * 1024;
pub const MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;
pub const SNAPSHOT_LIMIT: usize = 64;
/// Proposal lifetime only; every input still revalidates the original binding.
pub const INPUT_PROPOSAL_MAX_AGE_SECS: u64 = 120;
pub type Result<T> = std::result::Result<T, Fault>;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Fault {
    pub code: String,
    pub next_step: String,
    /// Optional structured evidence, never authorization or a retry instruction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<Box<serde_json::Value>>,
}
impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.next_step)
    }
}
impl std::error::Error for Fault {}
pub fn fault(code: &str, next: &str) -> Fault {
    Fault {
        code: code.into(),
        next_step: next.into(),
        diagnostic: None,
    }
}
impl From<std::io::Error> for Fault {
    fn from(_: std::io::Error) -> Self {
        fault(
            "storage_or_transport_error",
            "Inspect state; never replay an uncertain action.",
        )
    }
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn id() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub host: String,
    pub guest: Option<String>,
    pub session: String,
    pub seat: String,
    pub desktop: String,
}
impl Identity {
    pub fn key(&self) -> String {
        digest(&serde_json::to_vec(self).expect("identity serialization"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Scope {
    pub window: String,
    pub pid: u32,
    pub element: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Binding {
    pub scope: Scope,
    pub focus_window: String,
    pub layout: String,
    pub content: String,
    pub native_epoch: String,
    /// Bounded optimistic document precondition, not a filesystem/tab identity proof.
    #[serde(default)]
    pub mutation_context: Option<MutationContext>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MutationContext {
    pub window_title: String,
    pub document_state: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Element {
    pub id: String,
    pub name: String,
    /// Native role string (e.g. "AXButton", "AXTextField"): the control
    /// semantics an agent needs to infer what a node does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Native actions this element advertises (e.g. AXPress, AXShowMenu):
    /// what the control can do, for agent-side inference.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    /// Native enabled state when reported: whether the control can be
    /// operated at all right now (a disabled control fails closed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Native selection state when reported (AXSelected): whether this control
    /// is the app's current selection (e.g. the pressed tool button, or a
    /// selected shape). Lets a caller verify state before acting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    /// Native accessibility description/help (AXDescription/AXHelp), read only
    /// for elements that report no name: icon-only buttons usually carry their
    /// function here. Bounded to 64 chars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Bounded native value (AXValue) for controls that report one, e.g. a
    /// radio button's "1"/"0". Never raw document text: truncated to 64 chars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// True when `value` was truncated for transport.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub value_truncated: bool,
    pub point: [f64; 2],
    pub visible: Option<bool>,
    pub occluded: Option<bool>,
    pub state_revision: Option<String>,
}
impl Element {
    pub fn revision(&self) -> String {
        self.action_revision(None)
    }
    /// Explicit native semantic actions address an exact object, not a pointer
    /// hit. Preserve all semantic attributes; only omit pointer hit-test hints.
    /// Pointer/text/key references retain their complete original revision.
    pub fn action_revision(&self, native_action: Option<&str>) -> String {
        if native_action.is_none() {
            return digest(&serde_json::to_vec(self).expect("finite element metadata"));
        }
        let mut evidence = self.clone();
        evidence.visible = None;
        evidence.occluded = None;
        digest(&serde_json::to_vec(&evidence).expect("finite element metadata"))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Scene {
    pub identity: Identity,
    pub assurance: String,
    pub binding: Option<Binding>,
    pub mutation_ready: bool,
    pub blocked: Option<bool>,
    pub image_size: [u32; 2],
    pub pixel_to_desktop: Option<[f64; 6]>,
    pub elements: Vec<Element>,
    pub captured_unix_ms: u64,
    pub atomicity: String,
    pub image_normalization: String,
    #[serde(skip)]
    pub png: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Click,
    /// One checked pointer-move event. No button transition or text value;
    /// dispatch acknowledgement does not establish an application's hover effect.
    MovePointer,
    DoubleClick,
    ContextMenu,
    Text,
    KeyChord,
    Scroll,
    Drag(trajectory::DragPath),
}
impl Intent {
    pub fn validate(&self, value: Option<&str>) -> Result<()> {
        if matches!(self, Self::MovePointer) && value.is_some() {
            return Err(fault(
                "invalid_input_value",
                "Pointer movement takes no value, button transition, or semantic action.",
            ));
        }
        if let Self::Drag(path) = self {
            if value.is_some() {
                return Err(fault(
                    "invalid_input_value",
                    "Drag carries a typed path, not a text value.",
                ));
            }
            path.validate()?;
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Locator {
    Semantic {
        name: String,
    },
    Element {
        id: String,
    },
    /// Window-level reference for placement-class operations: no element and no
    /// input-grade visibility requirement. Input intents cannot use it.
    Window,
    Pixel {
        x: f64,
        y: f64,
    },
    /// Explicit opt-in for a compact, red marker cluster on an animated bitmap
    /// surface. The cluster itself is revalidated; ordinary pixels stay strict.
    RedMarkerCluster {
        x: f64,
        y: f64,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Predicate {
    WindowTitle,
    FocusedWindow,
    ElementValue,
    Modal,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Resolved {
    /// Service-issued exact native action; never a pointer reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_action: Option<String>,
    pub reference: String,
    pub target: String,
    pub frame: String,
    pub point: [f64; 2],
    pub scope: Scope,
    pub element_revision: Option<String>,
    /// Window-level placement references carry no element and must never be
    /// used for input intents.
    #[serde(default)]
    pub placement_only: bool,
    /// Exact observed window-relative ROI, issued from the retained snapshot.
    /// Callers cannot supply this through an action's reference argument.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_region: Option<[u32; 4]>,
    /// Internal bounded local image anchor for animated bitmap surfaces.
    /// Not accepted from or returned to an MCP caller; other platforms remain
    /// strict unless their native backend explicitly opts in.
    #[serde(skip)]
    #[schemars(skip)]
    pub pixel_anchor: Option<PixelAnchor>,
    /// This invocation needed the local fallback at its first full native
    /// revalidation. Native code must check the bitmap AX surface before any
    /// activation or pointer dispatch. Never retained in the public reference.
    #[serde(skip)]
    #[schemars(skip)]
    pub pixel_anchor_used: bool,
    /// Internal opt-in local red-component signature for dynamic bitmap scenes.
    #[serde(skip)]
    #[schemars(skip)]
    pub red_marker_anchor: Option<RedMarkerAnchor>,
}
/// Semantic references bind native geometry/focus and the selected element, not
/// unrelated image pixels. Pixel-only references remain deliberately conservative.
pub fn grounding_matches(expected: &Binding, current: &Scene, target: &Resolved) -> bool {
    let Some(now) = &current.binding else {
        return false;
    };
    if now.scope != expected.scope
        || now.focus_window != expected.focus_window
        || now.layout != expected.layout
        || now.native_epoch != expected.native_epoch
        || now.mutation_context != expected.mutation_context
    {
        return false;
    }
    match &target.scope.element {
        Some(id) => current
            .elements
            .iter()
            .find(|e| &e.id == id)
            .is_some_and(|e| {
                e.point == target.point
                    && match &target.native_action {
                        Some(action) => e.enabled != Some(false) && e.actions.contains(action),
                        None => e.visible == Some(true) && e.occluded == Some(false),
                    }
                    && target.element_revision.as_ref()
                        == Some(&e.action_revision(target.native_action.as_deref()))
            }),
        None => {
            now.content == expected.content
                || target.red_marker_anchor.as_ref().is_some_and(|anchor| {
                    anchor.matches(
                        &current.png,
                        current.image_size,
                        target.point,
                        current.pixel_to_desktop,
                    )
                })
                || target.pixel_anchor.as_ref().is_some_and(|anchor| {
                    anchor.matches(
                        &current.png,
                        current.image_size,
                        target.point,
                        current.pixel_to_desktop,
                    )
                })
        }
    }
}
/// Evaluate a single fresh scene once and retain only aggregate anchor metrics.
/// This is not a preflight ticket or input authority. Never reuse the result for
/// a later native batch/scene; native identity, leases and hit guards still run.
pub fn evaluate_grounding(
    expected: &Binding,
    current: &Scene,
    target: &Resolved,
) -> (bool, Option<serde_json::Value>, Option<serde_json::Value>) {
    let Some(now) = &current.binding else {
        return (false, None, None);
    };
    if now.scope != expected.scope
        || now.focus_window != expected.focus_window
        || now.layout != expected.layout
        || now.native_epoch != expected.native_epoch
        || now.mutation_context != expected.mutation_context
    {
        return (false, None, None);
    }
    if target.scope.element.is_some() {
        return (grounding_matches(expected, current, target), None, None);
    }
    if now.content == expected.content {
        return (true, None, None);
    }
    let marker = target.red_marker_anchor.as_ref().map(|anchor| {
        anchor.diagnostic(
            &current.png,
            current.image_size,
            target.point,
            current.pixel_to_desktop,
        )
    });
    if marker.as_ref().is_some_and(|d| d["matched"] == true) {
        return (true, None, marker);
    }
    let local = target.pixel_anchor.as_ref().map(|anchor| {
        anchor.diagnostic(
            &current.png,
            current.image_size,
            target.point,
            current.pixel_to_desktop,
        )
    });
    let matched = local.as_ref().is_some_and(|d| d["matched"] == true);
    (matched, local, marker)
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Oracle {
    pub identity: Identity,
    pub scope: Scope,
    pub assurance: String,
    pub matched: bool,
    pub source: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Receipt {
    pub request_id: String,
    pub frame: String,
    pub scope: Scope,
    pub dispatch: String,
    pub dispatch_error: Option<Fault>,
    pub system_accepted: Option<bool>,
    pub goal_verified: bool,
    pub replay: String,
    pub deduplicated: bool,
}
pub struct Verification<'a> {
    pub target: &'a str,
    pub frame: &'a str,
    pub element: Option<String>,
    pub request: Option<String>,
    pub predicate: Predicate,
    pub expected: &'a str,
    pub timeout_ms: u64,
    pub stable_samples: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Journal {
    fingerprint: String,
    receipt: Receipt,
}
pub use backend::{Backend, WindowAttachment};
mod backend;
mod grounding_diagnostics;
#[cfg(test)]
mod region_tests;
mod window_targets;
pub use grounding_diagnostics::grounding_diagnostic;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WindowSelection {
    pub window: String,
    pub pid: u32,
    pub native_epoch: String,
    pub relation: WindowRelation,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryQuery {
    pub cursor: Option<String>,
    pub pid: Option<u32>,
    pub title_contains: Option<String>,
}
impl DiscoveryQuery {
    pub fn validate(&self) -> Result<()> {
        if self
            .cursor
            .as_ref()
            .is_some_and(|c| c.len() > 96 || c.is_empty())
            || self.pid == Some(0)
            || self
                .title_contains
                .as_ref()
                .is_some_and(|s| s.is_empty() || s.len() > 256)
            || (self.cursor.is_some() && (self.pid.is_some() || self.title_contains.is_some()))
        {
            return Err(fault(
                "invalid_discovery_query",
                "Use a bounded PID/title filter on the first page, or its opaque cursor alone.",
            ));
        }
        Ok(())
    }
}
/// Requested placement target for an explicit window move. At least one
/// criterion is required; a bare value never matches anything by accident.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DisplayTarget {
    /// Match the built-in panel when true (reject it when false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub built_in: Option<bool>,
    /// Exact native display id when the caller pinned one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    /// Stable macOS ColorSync display UUID; can be combined with the transient
    /// native id and built_in selector to fail closed if topology changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
}
impl DisplayTarget {
    pub fn validate(&self) -> Result<()> {
        if self.built_in.is_none() && self.id.is_none() && self.uuid.is_none() {
            return Err(fault(
                "invalid_display_target",
                "Provide built_in, an exact display id, and/or a display UUID.",
            ));
        }
        if self.uuid.as_ref().is_some_and(|uuid| {
            uuid.len() != 36
                || !uuid.bytes().enumerate().all(|(index, byte)| {
                    if [8, 13, 18, 23].contains(&index) {
                        byte == b'-'
                    } else {
                        byte.is_ascii_hexdigit()
                    }
                })
        }) {
            return Err(fault(
                "invalid_display_target",
                "Use a canonical 36-character display UUID.",
            ));
        }
        Ok(())
    }

    /// Every supplied selector must describe the same currently active display.
    /// A stale numeric id cannot silently override a pinned UUID.
    pub fn matches(&self, built_in: bool, id: u32, uuid: Option<&str>) -> bool {
        self.built_in.is_none_or(|expected| expected == built_in)
            && self.id.is_none_or(|expected| expected == id)
            && self.uuid.as_ref().is_none_or(|expected| {
                uuid.is_some_and(|current| current.eq_ignore_ascii_case(expected))
            })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WindowRelation {
    Initial,
    OwnedPopup,
    ReturnToOwner,
}
struct Snapshot {
    target: String,
    at: Instant,
    scene: Scene,
    artifact_id: Option<String>,
    source_region: Option<[u32; 4]>,
}
/// One window of the bound process as seen through the accessibility layer.
/// Advisory inventory: unknown purposes are reported verbatim, never guessed.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Popup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subrole: Option<String>,
    /// Coarse class derived from the AX subrole (standard/dialog/sheet/panel/alert/other/unknown).
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<[f64; 4]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<serde_json::Value>,
}
fn intent_kind(intent: &Intent) -> &'static str {
    match intent {
        Intent::Click => "click",
        Intent::MovePointer => "move_pointer",
        Intent::DoubleClick => "double_click",
        Intent::ContextMenu => "context_menu",
        Intent::Text => "text",
        Intent::KeyChord => "key_chord",
        Intent::Scroll => "scroll",
        Intent::Drag(_) => "drag",
    }
}
pub fn popup_kind(subrole: Option<&str>) -> &'static str {
    match subrole {
        Some("AXStandardWindow") | Some("AXDocumentWindow") => "standard",
        Some("AXDialog") | Some("AXSystemDialog") => "dialog",
        Some("AXSheet") => "sheet",
        Some("AXFloatingWindow") => "panel",
        Some("AXSystemAlert") => "alert",
        Some(_) => "other",
        None => "unknown",
    }
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct Observation {
    pub frame: String,
    pub artifact_id: Option<String>,
    pub png_sha256: Option<String>,
    pub scene: Scene,
    /// Windows of the bound process, including non-foreground dialogs, sheets and
    /// panels, so a caller can deliver them without inferring from pixels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub popups: Vec<Popup>,
    pub adapter_status: serde_json::Value,
}
struct Artifact {
    id: String,
    bytes: Vec<u8>,
    hash: String,
}
struct RegisteredTarget {
    identity: Identity,
    backend: Box<dyn Backend>,
    parent: Option<String>,
}

/// Creation evidence shares the reference's bounded lifetime. Keep Resolved's
/// public wire/test construction unchanged; never retain a separate reason map.
#[derive(Clone)]
struct StoredReference {
    value: Resolved,
    pixel_anchor_creation_reason: Option<&'static str>,
}
impl std::ops::Deref for StoredReference {
    type Target = Resolved;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
impl std::ops::DerefMut for StoredReference {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

pub struct Engine {
    targets: BTreeMap<String, RegisteredTarget>,
    state: PathBuf,
    snapshots: VecDeque<(String, Snapshot)>,
    refs: VecDeque<StoredReference>,
    artifacts: VecDeque<Artifact>,
    pub leases: lease::LeaseBook,
}
impl Engine {
    pub fn new(state: PathBuf) -> Result<Self> {
        Self::with_lease_root(state.clone(), state)
    }
    /// Receipt namespaces may differ by principal; physical desktop leases must
    /// still share one root across every cooperating principal and context.
    pub fn with_lease_root(state: PathBuf, lease_root: PathBuf) -> Result<Self> {
        fs::create_dir_all(state.join("receipts"))?;
        Ok(Self {
            targets: BTreeMap::new(),
            state: state.clone(),
            snapshots: VecDeque::new(),
            refs: VecDeque::new(),
            artifacts: VecDeque::new(),
            leases: lease::LeaseBook::new(lease_root),
        })
    }
    pub fn register(&mut self, target: String, backend: Box<dyn Backend>) -> Result<()> {
        if target.is_empty() || self.targets.contains_key(&target) || self.targets.len() >= 16 {
            return Err(fault(
                "invalid_target",
                "Register at most 16 explicit, unique targets.",
            ));
        }
        self.targets.insert(
            target,
            RegisteredTarget {
                identity: backend.identity(),
                backend,
                parent: None,
            },
        );
        Ok(())
    }
    pub fn targets(&self) -> serde_json::Value {
        serde_json::json!(self.targets.iter().map(|(k,b)|serde_json::json!({"target":k,"identity":b.identity,"capabilities":b.backend.capabilities(),"parent_target":b.parent})).collect::<Vec<_>>())
    }
    pub fn discover(&mut self, target: &str) -> Result<serde_json::Value> {
        operation::checkpoint("discover")?;
        self.backend(target)?.discover()
    }
    pub fn discover_with(
        &mut self,
        target: &str,
        query: DiscoveryQuery,
    ) -> Result<serde_json::Value> {
        query.validate()?;
        operation::checkpoint("discover")?;
        self.backend(target)?.discover_with(&query)
    }
    pub fn bind_window(
        &mut self,
        target: &str,
        selection: &WindowSelection,
    ) -> Result<serde_json::Value> {
        operation::checkpoint("bind_window")?;
        let result = self.backend(target)?.bind_window(selection)?;
        self.snapshots.retain(|(_, s)| s.target != target);
        self.refs.retain(|r| r.target != target);
        Ok(result)
    }
    fn lifecycle_execute<T>(
        &mut self,
        target: &str,
        lease: &str,
        f: impl FnOnce(
            &std::path::Path,
            &lifecycle::LifecycleGrant,
            &mut dyn lifecycle::LifecycleDriver,
        ) -> Result<T>,
    ) -> Result<T> {
        let identity = self.identity(target)?;
        let grant = self.backend(target)?.lifecycle_grant()?;
        grant.validate(&identity, lifecycle::now_ms())?;
        let book = self.leases.shared();
        let held = book
            .lock()
            .map_err(|_| fault("lease_error", "Restart observation session."))?;
        let key = identity.key();
        let deadline = held
            .deadline(lease, target, &key)?
            .min(self.backend(target)?.lifecycle_deadline()?);
        let root = self.state.clone();
        let result = operation::with_dispatch_deadline(deadline, || {
            operation::checkpoint("lifecycle_worker")?;
            let mut driver = self.backend(target)?.lifecycle_driver()?;
            f(&root, &grant, driver.as_mut())
        });
        // Side effects and uncertain dispatch invalidate every alias on this desktop.
        let aliases = self
            .targets
            .iter()
            .filter(|(_, t)| t.identity.key() == key)
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>();
        self.snapshots.retain(|(_, s)| !aliases.contains(&s.target));
        self.refs.retain(|r| !aliases.contains(&r.target));
        drop(held);
        result
    }
    pub fn launch_application(
        &mut self,
        target: &str,
        lease: &str,
        req: lifecycle::LaunchRequest<'_>,
    ) -> Result<lifecycle::LaunchReceipt> {
        self.lifecycle_execute(target, lease, |root, g, d| {
            lifecycle::launch(root, g, d, req)
        })
    }
    pub fn activate_window(
        &mut self,
        target: &str,
        lease: &str,
        req: lifecycle::ActivateRequest<'_>,
    ) -> Result<lifecycle::ActivationReceipt> {
        let grant = self.backend(target)?.lifecycle_grant()?;
        let permission = grant
            .activations
            .iter()
            .find(|p| p.authorization_id == req.authorization_id)
            .ok_or_else(|| {
                fault(
                    "unknown_lifecycle_permission",
                    "Choose an exact preauthorized window.",
                )
            })?;
        self.backend(target)?
            .validate_activation(&permission.window)?;
        self.lifecycle_execute(target, lease, |root, g, d| {
            lifecycle::activate(root, g, d, req)
        })
    }
    pub fn lifecycle_preflight(
        &mut self,
        target: &str,
        authorization: &str,
    ) -> Result<lifecycle::PreflightReceipt> {
        let cleanup = operation::CleanupAudit::begin();
        let identity = self.identity(target)?;
        let grant = self.backend(target)?.lifecycle_grant()?;
        grant.validate(&identity, lifecycle::now_ms())?;
        let deadline = self.backend(target)?.lifecycle_deadline()?;
        let mut result = operation::with_dispatch_deadline(deadline, || {
            operation::checkpoint("lifecycle_preflight_worker")?;
            let mut driver = self.backend(target)?.lifecycle_driver()?;
            lifecycle::preflight(&grant, driver.as_mut(), authorization)
        })?;
        // Include driver construction/destruction signals, not only individual closers.
        if cleanup.uncertain() {
            result.ready_for_launch = false;
            result.resources_released = false;
            result.resource_release_uncertain = true;
            if result.error.is_none() {
                result.error = Some(fault(
                    "native_resource_release_uncertain",
                    "Lifecycle preflight cleanup is unproven; no action was dispatched.",
                ));
            }
        }
        Ok(result)
    }
    pub fn lifecycle_receipt(
        &mut self,
        target: &str,
        kind: &str,
        authorization: &str,
        request: &str,
    ) -> Result<serde_json::Value> {
        let grant = self.backend(target)?.lifecycle_grant()?;
        lifecycle::receipt(&self.state, &grant, kind, authorization, request)
    }
    fn backend(&mut self, target: &str) -> Result<&mut Box<dyn Backend>> {
        self.targets
            .get_mut(target)
            .map(|r| &mut r.backend)
            .ok_or_else(|| {
                fault(
                    "unknown_target",
                    "Use computer.targets; never infer host/guest routing.",
                )
            })
    }
    fn identity(&self, target: &str) -> Result<Identity> {
        self.targets
            .get(target)
            .map(|t| t.identity.clone())
            .ok_or_else(|| {
                fault(
                    "unknown_target",
                    "Use computer.targets; never infer host/guest routing.",
                )
            })
    }
    fn scene(&mut self, target: &str, region: Option<[u32; 4]>) -> Result<Scene> {
        let expected = self.identity(target)?;
        let scene = match region {
            Some(region) => self.backend(target)?.observe_region(region)?,
            None => self.backend(target)?.observe()?,
        };
        if scene.identity != expected {
            return Err(fault(
                "session_mismatch",
                "Stop and bind the intended session explicitly.",
            ));
        }
        Ok(scene)
    }
    pub fn observe(&mut self, target: &str) -> Result<Observation> {
        self.observe_with_region(target, None)
    }
    pub fn observe_with_region(
        &mut self,
        target: &str,
        region: Option<[u32; 4]>,
    ) -> Result<Observation> {
        operation::checkpoint("observe")?;
        let mut scene = self.scene(target, region)?;
        operation::checkpoint("capture_returned")?;
        let frame = id();
        let mut artifact = None;
        let mut hash = None;
        if scene.png.len() > MAX_FRAME_BYTES {
            return Err(fault(
                "screenshot_size_limit",
                "Native PNG exceeds the 32 MiB original-artifact limit; distinguish capture from encoding/transport. Do not change desktop resolution.",
            ));
        }
        if !scene.png.is_empty() {
            let bytes = std::mem::take(&mut scene.png);
            let h = digest(&bytes);
            let aid = id();
            while self.artifact_bytes() + bytes.len() > ARTIFACT_BUDGET
                || self.artifacts.len() >= SNAPSHOT_LIMIT
            {
                self.artifacts.pop_front();
            }
            self.artifacts.push_back(Artifact {
                id: aid.clone(),
                bytes,
                hash: h.clone(),
            });
            artifact = Some(aid);
            hash = Some(h);
        }
        if serde_json::to_vec(&scene)
            .map_err(|_| fault("invalid_scene", "Inspect adapter metadata."))?
            .len()
            > 256 * 1024
        {
            return Err(fault(
                "observation_budget",
                "Narrow the accessibility tree.",
            ));
        }
        self.snapshots.push_back((
            frame.clone(),
            Snapshot {
                target: target.into(),
                at: Instant::now(),
                scene: scene.clone(),
                artifact_id: artifact.clone(),
                source_region: region,
            },
        ));
        while self.snapshots.len() > SNAPSHOT_LIMIT {
            self.snapshots.pop_front();
        }
        let popups = match &scene.binding {
            Some(binding) => self
                .backend(target)?
                .popups(&binding.scope)
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let mut adapter_status = self.backend(target)?.capabilities();
        if let Some(capabilities) = adapter_status.as_object_mut() {
            capabilities.insert(
                "input_proposal_max_age_ms".into(),
                serde_json::json!(INPUT_PROPOSAL_MAX_AGE_SECS * 1000),
            );
        }
        Ok(Observation {
            frame,
            artifact_id: artifact,
            png_sha256: hash,
            scene,
            popups,
            adapter_status,
        })
    }
    pub fn artifact_bytes(&self) -> usize {
        self.artifacts.iter().map(|a| a.bytes.len()).sum()
    }
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }
    pub fn artifact(
        &self,
        aid: &str,
        offset: usize,
        length: usize,
    ) -> Result<(Vec<u8>, String, usize)> {
        let a = self.artifacts.iter().find(|a| a.id == aid).ok_or_else(|| {
            fault(
                "artifact_expired",
                "Capture again; artifacts are held under a 64 MiB total budget.",
            )
        })?;
        if length == 0 || length > INLINE_BYTES || offset > a.bytes.len() {
            return Err(fault(
                "invalid_range",
                "Use a bounded byte range within the raw PNG.",
            ));
        }
        let end = offset.saturating_add(length).min(a.bytes.len());
        Ok((a.bytes[offset..end].to_vec(), a.hash.clone(), a.bytes.len()))
    }
    /// Internal full-original access for bounded view encoding; not a raw transport chunk.
    pub fn original(&self, target: &str, frame: &str, aid: &str) -> Result<(&[u8], &Scene)> {
        let s = self.snapshot(target, frame)?;
        let a = self
            .artifacts
            .iter()
            .find(|a| a.id == aid)
            .ok_or_else(|| fault("artifact_expired", "Observe again."))?;
        if s.artifact_id.as_deref() != Some(aid) {
            return Err(fault(
                "artifact_frame_mismatch",
                "Use the original artifact from this exact frame.",
            ));
        }
        Ok((&a.bytes, &s.scene))
    }
    fn snapshot(&self, target: &str, frame: &str) -> Result<&Snapshot> {
        self.snapshots
            .iter()
            .find(|(id, s)| id == frame && s.target == target)
            .map(|(_, s)| s)
            .ok_or_else(|| fault("unknown_frame", "Observe this exact target again."))
    }
    pub fn resolve(&mut self, target: &str, frame: &str, locator: Locator) -> Result<Resolved> {
        self.resolve_action(target, frame, locator, None)
    }
    fn resolve_action(
        &mut self,
        target: &str,
        frame: &str,
        locator: Locator,
        native_action: Option<&str>,
    ) -> Result<Resolved> {
        let use_local_anchor = self.backend(target)?.local_pixel_anchor();
        let snapshot = self.snapshot(target, frame)?;
        let scene = &snapshot.scene;
        let binding = scene.binding.as_ref().ok_or_else(|| {
            fault(
                "target_binding_unknown",
                "Native window/PID binding is required.",
            )
        })?;
        let mut scope = binding.scope.clone();
        let mut element_revision = None;
        let placement_only = matches!(locator, Locator::Window);
        let mut pixel = None;
        let mut red_marker_cluster = false;
        let point = match locator {
            locator @ (Locator::Semantic { .. } | Locator::Element { .. }) => {
                let matches = scene
                    .elements
                    .iter()
                    .filter(|e| match &locator {
                        Locator::Semantic { name } => e.name == *name,
                        Locator::Element { id } => e.id == *id,
                        _ => false,
                    })
                    .collect::<Vec<_>>();
                if matches.len() != 1 {
                    return Err(fault(
                        "ambiguous_target",
                        "Choose one grounded visible element.",
                    ));
                }
                let e = matches[0];
                if let Some(action) = native_action {
                    if !e.actions.iter().any(|a| a == action) {
                        return Err(fault(
                            "unsupported_action",
                            "The exact element does not advertise this native action; no input sent. Use its observed actions, not a pointer fallback.",
                        ));
                    }
                    if e.enabled == Some(false) {
                        return Err(fault(
                            "element_disabled",
                            "The exact observed element is disabled; no native action sent.",
                        ));
                    }
                } else if e.visible != Some(true) || e.occluded != Some(false) {
                    return Err(fault(
                        "visibility_unknown",
                        "Resolve occlusion before input.",
                    ));
                }
                scope.element = Some(e.id.clone());
                element_revision = Some(e.action_revision(native_action));
                e.point
            }
            Locator::Window => {
                scope.element = None;
                // This reference is rejected by every input intent. A logical
                // AX dialog has no bitmap or pixel mapping, yet an exact native
                // backend may still place its window. Never invent a mapping:
                // the finite zero is only a non-input placeholder in Resolved.
                scene
                    .pixel_to_desktop
                    .map_or([0.0, 0.0], |[a, b, c, d, e, f]| {
                        let (x, y) = (
                            scene.image_size[0] as f64 / 2.0,
                            scene.image_size[1] as f64 / 2.0,
                        );
                        [a * x + c * y + e, b * x + d * y + f]
                    })
            }
            Locator::Pixel { x, y } => {
                if !x.is_finite()
                    || !y.is_finite()
                    || x < 0.0
                    || y < 0.0
                    || x >= scene.image_size[0] as f64
                    || y >= scene.image_size[1] as f64
                {
                    return Err(fault(
                        "outside_frame",
                        "Use coordinates inside the original image.",
                    ));
                }
                let [a, b, c, d, e, f] = scene.pixel_to_desktop.ok_or_else(|| {
                    fault(
                        "coordinate_mapping_unknown",
                        "Do not assume scale=1 or host coordinates for a guest.",
                    )
                })?;
                scope.element = None;
                pixel = Some([x, y]);
                [a * x + c * y + e, b * x + d * y + f]
            }
            Locator::RedMarkerCluster { x, y } => {
                if !use_local_anchor {
                    return Err(fault(
                        "red_marker_anchor_unavailable",
                        "This native backend cannot revalidate a local marker cluster; no reference created.",
                    ));
                }
                if !x.is_finite()
                    || !y.is_finite()
                    || x < 0.0
                    || y < 0.0
                    || x >= scene.image_size[0] as f64
                    || y >= scene.image_size[1] as f64
                {
                    return Err(fault(
                        "outside_frame",
                        "Use coordinates inside the original image.",
                    ));
                }
                let [a, b, c, d, e, f] = scene.pixel_to_desktop.ok_or_else(|| {
                    fault(
                        "coordinate_mapping_unknown",
                        "Do not assume scale=1 or host coordinates for a guest.",
                    )
                })?;
                scope.element = None;
                pixel = Some([x, y]);
                red_marker_cluster = true;
                [a * x + c * y + e, b * x + d * y + f]
            }
        };
        if !point.iter().all(|p| p.is_finite()) {
            return Err(fault(
                "invalid_transform",
                "Inspect native layout metadata.",
            ));
        }
        let (pixel_anchor, pixel_anchor_creation_reason) = if let Some(pixel) = pixel
            && !red_marker_cluster
        {
            if !use_local_anchor {
                (None, Some("backend_not_opted_in"))
            } else if let Some(artifact_id) = &snapshot.artifact_id {
                if let Some(artifact) = self.artifacts.iter().find(|entry| &entry.id == artifact_id)
                {
                    match PixelAnchor::from_png_diagnosed(
                        &artifact.bytes,
                        scene.image_size,
                        pixel,
                        point,
                    ) {
                        Ok(anchor) => (Some(anchor), None),
                        Err(reason) => (None, Some(reason.as_str())),
                    }
                } else {
                    // The original frame still exists, but its identified bytes
                    // are no longer retained. Do not recreate from a new frame.
                    (None, Some("source_artifact_evicted"))
                }
            } else {
                (None, Some("source_artifact_absent"))
            }
        } else {
            // Semantic, placement and explicit red-marker references are not
            // ordinary pixel-anchor construction failures.
            (None, None)
        };
        let red_marker_anchor = if red_marker_cluster {
            let (pixel, artifact) = pixel.and_then(|pixel| {
                snapshot.artifact_id.as_ref().and_then(|id| {
                    self.artifacts
                        .iter()
                        .find(|artifact| &artifact.id == id)
                        .map(|artifact| (pixel, artifact))
                })
            }).ok_or_else(|| fault(
                "red_marker_anchor_unavailable",
                "A preserved native screenshot is required to anchor a red marker cluster; no reference created.",
            ))?;
            Some(RedMarkerAnchor::from_png(&artifact.bytes, scene.image_size, pixel, point).ok_or_else(|| fault(
                "red_marker_cluster_not_found",
                "The exact point must fall on one compact red marker in a local cluster; no reference created and no fallback was attempted.",
            ))?)
        } else {
            None
        };
        let r = Resolved {
            native_action: native_action.map(str::to_owned),
            reference: id(),
            target: target.into(),
            frame: frame.into(),
            point,
            scope,
            element_revision,
            placement_only,
            source_region: snapshot.source_region,
            pixel_anchor,
            pixel_anchor_used: false,
            red_marker_anchor,
        };
        self.refs.push_back(StoredReference {
            value: r.clone(),
            pixel_anchor_creation_reason,
        });
        while self.refs.len() > 256 {
            self.refs.pop_front();
        }
        Ok(r)
    }
    pub fn claim(&mut self, target: &str, ttl: u64) -> Result<String> {
        let key = self.identity(target)?.key();
        self.leases.claim(target, &key, ttl)
    }
    pub fn release(&mut self, token: &str) -> Result<()> {
        self.leases.release(token)
    }
    fn operations_path(&self) -> PathBuf {
        self.state.join("operations.jsonl")
    }
    /// Best-effort user-facing operation journal: one bounded JSON line per
    /// dispatched action. It records what was attempted, never that it took
    /// effect, and it is never replayed.
    fn log_operation(&self, receipt: &Receipt, target: &str, intent: &Intent, resolved: &Resolved) {
        let entry = serde_json::json!({
            "ts_unix_ms": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
            "target": target,
            "request_id": receipt.request_id,
            "intent": intent_kind(intent),
            "point": resolved.point,
            "scope": resolved.scope,
            "dispatch": receipt.dispatch,
            "dispatch_error": receipt.dispatch_error.as_ref().map(|e| e.code.clone()),
        });
        let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.operations_path())
        else {
            return;
        };
        let mut line = serde_json::to_vec(&entry).unwrap_or_default();
        line.push(b'\n');
        let _ = file.write_all(&line);
    }
    /// Bounded tail of the operation journal, newest last.
    pub fn recent_operations(&self, limit: usize) -> Vec<serde_json::Value> {
        operation_journal::recent(&self.operations_path(), limit)
    }
    fn journal_path(&self, key: &str, request: &str) -> PathBuf {
        self.state
            .join("receipts")
            .join(digest(format!("{key}:{request}").as_bytes()) + ".jsonl")
    }
    /// Visual-only pointer feedback: show or hide the native overlay marker at
    /// a desktop point. Never input, never focus, never the real cursor.
    pub fn feedback(
        &mut self,
        target: &str,
        visible: bool,
        point: Option<[f64; 2]>,
    ) -> Result<serde_json::Value> {
        let _ = self.identity(target)?.key();
        self.backend(target)?.feedback(visible, point)
    }
    /// Explicitly move the referenced window onto the requested display.
    /// Same lease, freshness and durable no-replay guarantees as an act;
    /// nothing moves a window implicitly.
    pub fn move_window(
        &mut self,
        target: &str,
        lease: &str,
        reference: &str,
        request: &str,
        display: &DisplayTarget,
    ) -> Result<serde_json::Value> {
        display.validate()?;
        if request.is_empty() || request.len() > 128 {
            return Err(fault("invalid_arguments", "Bound request_id length."));
        }
        let key = self.identity(target)?.key();
        let path = self.journal_path(&key, request);
        let fingerprint = digest(&serde_json::to_vec(&(target, reference, display)).unwrap());
        if path.exists() {
            let mut text = String::new();
            File::open(&path)?.take(65536).read_to_string(&mut text)?;
            let mut records = text
                .split_inclusive('\n')
                .filter(|s| s.ends_with('\n'))
                .filter_map(|s| serde_json::from_str::<Journal>(s).ok());
            let first = records.next().ok_or_else(|| {
                fault(
                    "uncertain_journal",
                    "Interrupted reservation exists; do not replay.",
                )
            })?;
            if first.fingerprint != fingerprint {
                return Err(fault(
                    "idempotency_conflict",
                    "Do not reuse request_id for another move.",
                ));
            }
            let mut receipt = records.next_back().unwrap_or(first).receipt;
            receipt.deduplicated = true;
            return Ok(serde_json::json!({"receipt": receipt, "deduplicated": true}));
        }
        operation::checkpoint("before_revalidation")?;
        let resolved = self
            .refs
            .iter()
            .find(|r| r.reference == reference && r.target == target)
            .cloned()
            .ok_or_else(|| fault("unknown_reference", "Observe and resolve this target."))?;
        let snapshot = self.snapshot(target, &resolved.frame)?;
        if snapshot.at.elapsed() > Duration::from_secs(10) {
            return Err(fault("stale_reference", "Reobserve; no move sent."));
        }
        let book = self.leases.shared();
        let held = book
            .lock()
            .map_err(|_| fault("lease_error", "Restart observation session."))?;
        held.check(lease, target, &key)?;
        operation::checkpoint("before_durable_reservation")?;
        let mut receipt = Receipt {
            request_id: request.into(),
            frame: resolved.frame.clone(),
            scope: resolved.scope.clone(),
            dispatch: "uncertain".into(),
            dispatch_error: None,
            system_accepted: None,
            goal_verified: false,
            replay: "forbidden".into(),
            deduplicated: false,
        };
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let write = |f: &mut File, r: &Receipt| -> Result<()> {
            let data = serde_json::to_vec(&Journal {
                fingerprint: fingerprint.clone(),
                receipt: r.clone(),
            })
            .unwrap();
            f.write_all(&data)?;
            f.write_all(b"\n")?;
            f.sync_all()?;
            Ok(())
        };
        write(&mut file, &receipt)?;
        #[cfg(unix)]
        File::open(path.parent().unwrap())?.sync_all()?;
        let mut placed = None;
        let dispatch_before = self.backend(target)?.input_dispatch_count();
        match operation::with_dispatch_deadline(held.deadline(lease, target, &key)?, || {
            self.backend(target)?.move_window(&resolved, display)
        }) {
            Ok(value) => {
                receipt.dispatch = "returned".into();
                placed = Some(value);
            }
            Err(error) => {
                let dispatch_after = self.backend(target)?.input_dispatch_count();
                record_dispatch_failure(&mut receipt, error, dispatch_before, dispatch_after);
            }
        }
        write(&mut file, &receipt)?;
        drop(held);
        // A moved window invalidates frames and references on this desktop.
        let aliases = self
            .targets
            .iter()
            .filter(|(_, t)| t.identity.key() == key)
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>();
        self.snapshots.retain(|(_, s)| !aliases.contains(&s.target));
        self.refs.retain(|r| !aliases.contains(&r.target));
        Ok(match placed {
            Some(value) => serde_json::json!({"receipt": receipt, "placement": value}),
            None => serde_json::json!({"receipt": receipt}),
        })
    }

    pub fn act(
        &mut self,
        target: &str,
        reference: &str,
        lease: &str,
        request: &str,
        intent: Intent,
        value: Option<String>,
    ) -> Result<Receipt> {
        self.act_inner(target, reference, lease, request, intent, value, None)
    }
    #[allow(clippy::too_many_arguments)]
    fn act_inner(
        &mut self,
        target: &str,
        reference: &str,
        lease: &str,
        request: &str,
        intent: Intent,
        value: Option<String>,
        checked_fingerprint: Option<String>,
    ) -> Result<Receipt> {
        if request.is_empty()
            || request.len() > 128
            || value.as_ref().is_some_and(|v| v.len() > 16384)
        {
            return Err(fault(
                "invalid_arguments",
                "Bound request_id and value lengths.",
            ));
        }
        intent.validate(value.as_deref())?;
        let key = self.identity(target)?.key();
        let path = self.journal_path(&key, request);
        let fingerprint = checked_fingerprint.unwrap_or_else(|| {
            digest(&serde_json::to_vec(&(target, reference, &intent, &value)).unwrap())
        });
        if path.exists() {
            let mut text = String::new();
            File::open(&path)?.take(65536).read_to_string(&mut text)?;
            let mut records = text
                .split_inclusive('\n')
                .filter(|s| s.ends_with('\n'))
                .filter_map(|s| serde_json::from_str::<Journal>(s).ok());
            let first = records.next().ok_or_else(|| {
                fault(
                    "uncertain_journal",
                    "Interrupted reservation exists; do not replay.",
                )
            })?;
            if first.fingerprint != fingerprint {
                return Err(fault(
                    "idempotency_conflict",
                    "Do not reuse request_id for another action or reference.",
                ));
            }
            let mut result = records.next_back().unwrap_or(first).receipt;
            result.deduplicated = true;
            return Ok(result);
        }
        operation::checkpoint("before_revalidation")?;
        let mut resolved = self
            .refs
            .iter()
            .find(|r| r.reference == reference && r.target == target)
            .cloned()
            .ok_or_else(|| fault("unknown_reference", "Observe and resolve this target."))?;
        if let Some(action) = &resolved.native_action
            && (!matches!(intent, Intent::Click) || value.as_ref() != Some(action))
        {
            return Err(fault(
                "native_action_mismatch",
                "Native-action references cannot dispatch pointer, text, or a different action.",
            ));
        }
        if resolved.placement_only {
            return Err(fault(
                "invalid_reference_class",
                "Window-level references are for placement only; resolve an element or pixel for input.",
            ));
        }
        let snapshot = self.snapshot(target, &resolved.frame)?;
        if snapshot.at.elapsed() > Duration::from_secs(INPUT_PROPOSAL_MAX_AGE_SECS) {
            return Err(fault(
                "stale_reference",
                "Input proposal exceeded 120 seconds; reobserve. No input sent.",
            ));
        }
        let before = snapshot.scene.clone();
        let current = self.backend(target)?.revalidate(&resolved)?;
        if current.identity != self.identity(target)? {
            return Err(fault(
                "session_mismatch",
                "Native revalidation belongs to another session.",
            ));
        }
        if !current.mutation_ready || !before.mutation_ready {
            return Err(fault(
                "capability_unavailable",
                "Native input binding is not ready.",
            ));
        }
        let (grounded, local_pixel, red_marker) = before
            .binding
            .as_ref()
            .map(|binding| evaluate_grounding(binding, &current, &resolved))
            .unwrap_or((false, None, None));
        if !grounded {
            let reason = local_pixel
                .as_ref()
                .and_then(|d| d["reason"].as_str())
                .or_else(|| red_marker.as_ref().and_then(|d| d["reason"].as_str()));
            let message = reason.map_or_else(
                || {
                    let full_frame_changed_without_anchor = resolved.scope.element.is_none()
                        && resolved.pixel_anchor.is_none()
                        && resolved.red_marker_anchor.is_none()
                        && before.binding.as_ref().zip(current.binding.as_ref())
                            .is_some_and(|(a, b)| a.content != b.content);
                    if full_frame_changed_without_anchor {
                        if let Some(reason) = resolved.pixel_anchor_creation_reason {
                            format!("Pixel reference has no local anchor ({reason}); full-frame content changed. Inspect the source snapshot or a stable textured click surface; no input sent, never replay.")
                        } else {
                            "Pixel reference has no local anchor; full-frame content changed. Inspect a fresh stable textured surface; no input sent.".to_owned()
                        }
                    } else {
                        "Reference grounding refused; inspect fresh binding/element state. No input sent.".to_owned()
                    }
                },
                |reason| format!("Local pixel anchor refused: {reason}. Inspect a fresh stable click surface; no input sent, never replay."),
            );
            let mut error = fault("stale_reference", &message);
            // Same-scene metrics are reused, not recomputed from the PNG; this
            // does not renew authority or skip any later native input check.
            let mut diagnostic = grounding_diagnostic(&before, &current, &resolved);
            if let Some(reason) = resolved.pixel_anchor_creation_reason {
                diagnostic["local_anchor_creation_reason"] = serde_json::json!(reason);
            }
            if let Some(local) = local_pixel {
                diagnostic["local_pixel_anchor"] = local;
            }
            if let Some(marker) = red_marker {
                diagnostic["red_marker_cluster_anchor"] = marker;
            }
            error.diagnostic = Some(Box::new(diagnostic));
            return Err(error);
        }
        resolved.pixel_anchor_used = (resolved.pixel_anchor.is_some()
            || resolved.red_marker_anchor.is_some())
            && before.binding.as_ref().map(|binding| &binding.content)
                != current.binding.as_ref().map(|binding| &binding.content);
        if current.blocked != Some(false) {
            return Err(fault("target_blocked", "Inspect modal/occlusion state."));
        }
        let book = self.leases.shared();
        let held = book
            .lock()
            .map_err(|_| fault("lease_error", "Restart observation session."))?;
        held.check(lease, target, &key)?;
        operation::checkpoint("before_durable_reservation")?;
        let mut receipt = Receipt {
            request_id: request.into(),
            frame: resolved.frame.clone(),
            scope: resolved.scope.clone(),
            dispatch: "uncertain".into(),
            dispatch_error: None,
            system_accepted: None,
            goal_verified: false,
            replay: "forbidden".into(),
            deduplicated: false,
        };
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let write = |f: &mut File, r: &Receipt| -> Result<()> {
            let data = serde_json::to_vec(&Journal {
                fingerprint: fingerprint.clone(),
                receipt: r.clone(),
            })
            .unwrap();
            f.write_all(&data)?;
            f.write_all(b"\n")?;
            f.sync_all()?;
            Ok(())
        };
        write(&mut file, &receipt)?;
        // Persist the directory entry before dispatch as well as the reservation bytes.
        #[cfg(unix)]
        File::open(path.parent().unwrap())?.sync_all()?;
        let expected = before
            .binding
            .as_ref()
            .ok_or_else(|| fault("target_binding_unknown", "Native binding required."))?;
        let dispatch_before = self.backend(target)?.input_dispatch_count();
        match operation::with_dispatch_deadline(held.deadline(lease, target, &key)?, || {
            self.backend(target)?
                .act(expected, &resolved, &intent, value.as_deref())
        }) {
            Ok(accepted) => {
                receipt.dispatch = "returned".into();
                receipt.system_accepted = accepted;
            }
            Err(error) => {
                let dispatch_after = self.backend(target)?.input_dispatch_count();
                record_dispatch_failure(&mut receipt, error, dispatch_before, dispatch_after);
            }
        }
        write(&mut file, &receipt)?;
        drop(held);
        let aliases = self
            .targets
            .iter()
            .filter(|(_, t)| t.identity.key() == key)
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        self.refs.retain(|r| !aliases.contains(&r.target));
        self.log_operation(&receipt, target, &intent, &resolved);
        Ok(receipt)
    }
    pub fn verify(&mut self, verification: Verification<'_>) -> Result<serde_json::Value> {
        let Verification {
            target,
            frame,
            element,
            request,
            predicate,
            expected,
            timeout_ms,
            stable_samples,
        } = verification;
        if timeout_ms > 10000 || !(1..=5).contains(&stable_samples) || expected.len() > 16384 {
            return Err(fault(
                "invalid_arguments",
                "Bound verification timeout, sample count and value.",
            ));
        }
        let scene = &self.snapshot(target, frame)?.scene;
        let binding = scene
            .binding
            .as_ref()
            .ok_or_else(|| fault("target_binding_unknown", "Window/PID attestation required."))?;
        let mut scope = binding.scope.clone();
        scope.element = element.clone();
        if matches!(predicate, Predicate::ElementValue) && element.is_none() {
            return Err(fault(
                "element_required",
                "Bind the expected value to a specific element.",
            ));
        }
        if element
            .as_ref()
            .is_some_and(|id| !scene.elements.iter().any(|e| &e.id == id))
        {
            return Err(fault(
                "unknown_element",
                "Use an element present in this frame.",
            ));
        }
        let identity = scene.identity.clone();
        if let Some(req) = &request {
            let text = fs::read_to_string(self.journal_path(&identity.key(), req))?;
            let record: Journal = serde_json::from_str(text.lines().next().unwrap_or(""))
                .map_err(|_| fault("unknown_operation", "Use a valid action request ID."))?;
            if record.receipt.scope != scope {
                return Err(fault(
                    "verification_scope_mismatch",
                    "Operation targeted another window/PID/element.",
                ));
            }
        }
        let start = Instant::now();
        let mut consecutive = 0;
        let mut samples = 0;
        loop {
            operation::checkpoint("verify_poll")?;
            let oracle = self
                .backend(target)?
                .inspect(&scope, &predicate, expected)?;
            if oracle.identity != identity || oracle.scope != scope {
                return Err(fault(
                    "verification_scope_mismatch",
                    "Same value in another target is not success.",
                ));
            }
            samples += 1;
            consecutive = if oracle.matched { consecutive + 1 } else { 0 };
            if consecutive >= stable_samples || start.elapsed() >= Duration::from_millis(timeout_ms)
            {
                let attested = oracle.assurance == "native-attested"
                    || oracle.assurance == "synthetic-fixture";
                return Ok(
                    serde_json::json!({"goal_verified":attested&&consecutive>=stable_samples,"predicate_matched":consecutive>=stable_samples,"frame":frame,"request_id":request,"scope":scope,"samples":samples,"oracle":oracle}),
                );
            }
            std::thread::sleep(
                Duration::from_millis(25)
                    .min(Duration::from_millis(timeout_ms).saturating_sub(start.elapsed())),
            );
        }
    }
}

#[cfg(test)]
mod popup_tests {
    use super::popup_kind;
    #[test]
    fn popup_kinds_are_coarse_and_never_guess_a_purpose() {
        assert_eq!(popup_kind(Some("AXSheet")), "sheet");
        assert_eq!(popup_kind(Some("AXDialog")), "dialog");
        assert_eq!(popup_kind(Some("AXSystemDialog")), "dialog");
        assert_eq!(popup_kind(Some("AXFloatingWindow")), "panel");
        assert_eq!(popup_kind(Some("AXStandardWindow")), "standard");
        assert_eq!(popup_kind(Some("AXSomethingNew")), "other");
        assert_eq!(popup_kind(None), "unknown");
    }
}

// Only positive native counter evidence distinguishes rejection from unknown effect.
// Saturated/missing/reset counters cannot prove that nothing was dispatched.
fn record_dispatch_failure(
    receipt: &mut Receipt,
    error: Fault,
    before: Option<u64>,
    after: Option<u64>,
) {
    if before.is_some_and(|n| n < u64::MAX) && before == after {
        receipt.dispatch = "rejected_before_dispatch".into();
    }
    receipt.dispatch_error = Some(error);
}

#[cfg(test)]
mod dispatch_receipt_tests;
