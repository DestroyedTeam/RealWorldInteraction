//! Platform adapter contract; no transport or workflow dependency.
use crate::{
    Binding, DiscoveryQuery, Identity, Intent, Oracle, Predicate, Resolved, Result, Scene, Scope,
    WindowSelection, fault, lifecycle,
};
use std::time::Instant;

/// A separately owned window backend plus read-only selection evidence.
pub struct WindowAttachment {
    pub backend: Box<dyn Backend>,
    pub details: serde_json::Value,
}

pub trait Backend: Send {
    /// Opt in only when the native backend also verifies the exact bitmap/AX
    /// surface at dispatch. Every other platform keeps full-content matching.
    fn local_pixel_anchor(&self) -> bool {
        false
    }
    /// Optional native side-effect counter, for proving pre-dispatch rejection.
    fn input_dispatch_count(&self) -> Option<u64> {
        None
    }
    /// Drain only the last synchronous act's bounded text sample; no Native I/O.
    /// Unsupported backends return None. Core clears stale samples before dispatch.
    fn take_text_effect(&mut self) -> Option<crate::text_effect::TextEffect> {
        None
    }
    fn identity(&self) -> Identity;
    fn capabilities(&self) -> serde_json::Value;
    /// Fresh device-scoped status, without taking an input slot or capturing UI.
    /// Target capability descriptors may be cached by a remote adapter.
    fn runtime_status(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
    fn lifecycle_grant(&self) -> Result<lifecycle::LifecycleGrant> {
        Err(fault(
            "capability_unavailable",
            "No explicit application/window lifecycle grant on this target.",
        ))
    }
    fn lifecycle_deadline(&self) -> Result<Instant> {
        Err(fault(
            "capability_unavailable",
            "No monotonic lifecycle authorization deadline.",
        ))
    }
    /// Driver is thread-affine: create, use and drop inside one native worker.
    fn lifecycle_driver(&mut self) -> Result<Box<dyn lifecycle::LifecycleDriver>> {
        Err(fault(
            "capability_unavailable",
            "This backend has no native lifecycle driver.",
        ))
    }
    fn validate_activation(&self, _window: &lifecycle::WindowIdentity) -> Result<()> {
        Err(fault(
            "capability_unavailable",
            "This backend has no fresh native activation catalog.",
        ))
    }
    /// Explicit window move onto a requested display. Backends without it
    /// fail closed; the move itself must be native, bounded and audit-logged.
    fn move_window(
        &mut self,
        _reference: &crate::Resolved,
        _display: &crate::DisplayTarget,
    ) -> Result<serde_json::Value> {
        Err(fault(
            "capability_unavailable",
            "Native window move is not implemented on this platform; nothing was moved.",
        ))
    }
    /// Read-only exact-object focus eligibility. Native facts are trusted backend
    /// values, never Deserialize/model-supplied custody or a cached sidecar.
    fn focus_element_preflight(
        &mut self,
        _reference: &Resolved,
    ) -> Result<crate::semantic_focus::FocusNativeFacts> {
        Err(fault(
            "capability_unavailable",
            "Semantic focus preflight is not implemented by this backend; no fallback.",
        ))
    }
    /// Explicit semantic mutation: at most the approved zero/one AXFocused write.
    /// No pointer/key/activation fallback. Platform backends default unsupported.
    fn focus_element(
        &mut self,
        _expected: &Binding,
        _reference: &Resolved,
        _plan: crate::semantic_focus::FocusWritePlan,
    ) -> Result<crate::semantic_focus::FocusReadback> {
        Err(fault(
            "capability_unavailable",
            "Semantic element focus is not implemented by this backend; no setter or fallback.",
        ))
    }
    /// Explicit whole plain-text value replacement. No default implementation.
    fn set_text_preflight(
        &mut self,
        _reference: &Resolved,
    ) -> Result<crate::set_text::SetTextNativeFacts> {
        Err(fault(
            "capability_unavailable",
            "Whole text replacement is unsupported; no typing/focus fallback.",
        ))
    }
    fn set_text(
        &mut self,
        _expected: &Binding,
        _reference: &Resolved,
        _text: &str,
    ) -> Result<crate::set_text::SetTextReadback> {
        Err(fault(
            "capability_unavailable",
            "Whole text replacement is unsupported; no typing/focus fallback.",
        ))
    }
    /// Route-specific readonly window revalidation. Existing placement callers
    /// keep their old behavior; native implementations may omit capture/tree.
    fn window_local_revalidate(&mut self, reference: &Resolved) -> Result<Scene> {
        self.revalidate(reference)
    }
    /// Trusted fresh exact selected-window facts; neutral platforms unsupported.
    fn select_window_local_preflight(
        &mut self,
        _reference: &Resolved,
        _kind: crate::window_local::WindowLocalKind,
    ) -> Result<crate::window_local::WindowLocalNativeFacts> {
        Err(fault(
            "capability_unavailable",
            "Explicit local window selection is unsupported; no fallback.",
        ))
    }
    /// At most one explicit Main OR Key attribute write, never activate/type/raise.
    fn select_window_local(
        &mut self,
        _expected: &Binding,
        _reference: &Resolved,
        _kind: crate::window_local::WindowLocalKind,
        _plan: crate::window_local::WindowLocalWritePlan,
    ) -> Result<crate::window_local::WindowLocalReadback> {
        Err(fault(
            "capability_unavailable",
            "This window-local kind is unsupported; no setter or fallback.",
        ))
    }
    /// Explicit foreground operation, never a background-input fallback.
    fn focus_window(
        &mut self,
        _reference: &Resolved,
        _expected: &crate::ForegroundState,
    ) -> Result<serde_json::Value> {
        Err(fault(
            "capability_unavailable",
            "Explicit bound-window foreground is unavailable on this backend; no fallback.",
        ))
    }
    /// Explicit device/session foreground keyboard stream. Never an application
    /// addressed-input fallback; unsupported backends send nothing.
    fn hotkey(
        &mut self,
        _key: &str,
        _expected: &crate::ForegroundState,
    ) -> Result<serde_json::Value> {
        Err(fault(
            "capability_unavailable",
            "Foreground/system keyboard input is unavailable on this backend; no fallback.",
        ))
    }
    /// Visual-only pointer feedback for the user, driven by the native overlay.
    /// It is never input and backends without an overlay fail closed.
    fn feedback(&mut self, _visible: bool, _point: Option<[f64; 2]>) -> Result<serde_json::Value> {
        Err(fault(
            "capability_unavailable",
            "No native pointer-feedback overlay on this platform; nothing was drawn.",
        ))
    }
    /// Advisory per-process window inventory for popup delivery. Backends
    /// without native window enumeration return nothing rather than guessing.
    fn popups(&mut self, _scope: &crate::Scope) -> Result<Vec<crate::Popup>> {
        Ok(Vec::new())
    }
    fn observe(&mut self) -> Result<Scene>;
    fn capture_visual(&mut self, _: crate::VisualOptions) -> Result<crate::VisualCapture> {
        Err(fault(
            "capability_unavailable",
            "This backend has no independent read-only visual capture; observe is not substituted.",
        ))
    }
    fn live(
        &mut self,
        _: crate::LiveAction,
        _: Option<crate::LiveOptions>,
    ) -> Result<serde_json::Value> {
        Err(fault(
            "capability_unavailable",
            "Session-scoped local Live preview is not implemented on this backend.",
        ))
    }
    fn live_view(&mut self, _: crate::LiveViewUpdate) -> Result<serde_json::Value> {
        Err(fault(
            "capability_unavailable",
            "This backend has no Host-view visibility binding.",
        ))
    }
    fn live_controller(&self) -> Result<std::sync::Arc<dyn crate::LiveController>> {
        Err(fault(
            "capability_unavailable",
            "Session-scoped local Live preview is not implemented on this backend.",
        ))
    }
    fn observe_region(&mut self, _region: [u32; 4]) -> Result<Scene> {
        Err(fault(
            "capability_unavailable",
            "Native ROI capture is not implemented by this adapter; a derived image tile is a separate operation.",
        ))
    }
    fn discover(&mut self) -> Result<serde_json::Value> {
        Err(fault(
            "capability_unavailable",
            "Native window discovery is not implemented by this adapter.",
        ))
    }
    fn discover_with(&mut self, query: &DiscoveryQuery) -> Result<serde_json::Value> {
        if query.cursor.is_some() || query.pid.is_some() || query.title_contains.is_some() {
            return Err(fault(
                "capability_unavailable",
                "This backend does not provide filtered window discovery.",
            ));
        }
        self.discover()
    }
    fn bind_window(&mut self, _selection: &WindowSelection) -> Result<serde_json::Value> {
        Err(fault(
            "capability_unavailable",
            "This adapter has a fixed window binding.",
        ))
    }
    /// Create an independent binding from a fresh explicit catalog selection.
    /// Must not activate, place, capture, send input or change the source binding.
    /// The returned backend must retain the same physical desktop identity.
    fn attach_window(&mut self, _selection: &WindowSelection) -> Result<WindowAttachment> {
        Err(fault(
            "capability_unavailable",
            "Independent window attachment is not implemented on this target.",
        ))
    }
    /// Native adapters may avoid capture/PNG encoding for semantic references.
    fn revalidate(&mut self, _reference: &Resolved) -> Result<Scene> {
        self.observe()
    }
    /// MUST recheck expected binding at dispatch. Do not automatically retry.
    fn act(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        intent: &Intent,
        value: Option<&str>,
    ) -> Result<Option<bool>>;
    /// Explicit paired route. Default backends refuse; ordinary drag is not
    /// widened, and the native producer must independently guard both endpoints.
    fn drag_between(
        &mut self,
        _pair: &crate::drag_between::Pair,
        _destination: &mut dyn Backend,
    ) -> Result<Option<bool>> {
        Err(fault(
            "capability_unavailable",
            "Paired native drag is unavailable on this backend; no fallback.",
        ))
    }
    fn drag_surface(
        &mut self,
        _endpoint: &crate::drag_between::Endpoint,
        _initial: bool,
    ) -> Result<crate::drag_between::Surface> {
        Err(fault(
            "capability_unavailable",
            "This backend cannot guard a paired native drag endpoint.",
        ))
    }
    fn inspect(&mut self, scope: &Scope, predicate: &Predicate, expected: &str) -> Result<Oracle>;
}
