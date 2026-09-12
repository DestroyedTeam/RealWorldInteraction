//! Platform adapter contract; no transport or workflow dependency.
use crate::{
    Binding, DiscoveryQuery, Identity, Intent, Oracle, Predicate, Resolved, Result, Scene, Scope,
    WindowSelection, fault, lifecycle,
};
use std::time::Instant;

pub trait Backend: Send {
    /// Optional native side-effect counter, for proving pre-dispatch rejection.
    fn input_dispatch_count(&self) -> Option<u64> {
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
    fn observe(&mut self) -> Result<Scene>;
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
    fn inspect(&mut self, scope: &Scope, predicate: &Predicate, expected: &str) -> Result<Oracle>;
}
