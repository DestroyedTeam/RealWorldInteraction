//! Distinguish rejection before native dispatch from uncertain native effects.
use rcu_core::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
pub struct Observed {
    pub inner: Box<dyn Backend>,
    pub entered: Arc<AtomicBool>,
}
impl Backend for Observed {
    fn identity(&self) -> Identity {
        self.inner.identity()
    }
    fn capabilities(&self) -> serde_json::Value {
        self.inner.capabilities()
    }
    fn runtime_status(&self) -> serde_json::Value {
        self.inner.runtime_status()
    }
    fn observe(&mut self) -> Result<Scene> {
        self.inner.observe()
    }
    fn observe_region(&mut self, r: [u32; 4]) -> Result<Scene> {
        self.inner.observe_region(r)
    }
    fn revalidate(&mut self, r: &Resolved) -> Result<Scene> {
        self.inner.revalidate(r)
    }
    fn discover(&mut self) -> Result<serde_json::Value> {
        self.inner.discover()
    }
    fn discover_with(&mut self, q: &DiscoveryQuery) -> Result<serde_json::Value> {
        self.inner.discover_with(q)
    }
    fn bind_window(&mut self, s: &WindowSelection) -> Result<serde_json::Value> {
        self.inner.bind_window(s)
    }
    fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> Result<Oracle> {
        self.inner.inspect(s, p, e)
    }
    fn lifecycle_grant(&self) -> Result<lifecycle::LifecycleGrant> {
        self.inner.lifecycle_grant()
    }
    fn lifecycle_deadline(&self) -> Result<Instant> {
        self.inner.lifecycle_deadline()
    }
    fn validate_activation(&self, w: &lifecycle::WindowIdentity) -> Result<()> {
        self.inner.validate_activation(w)
    }
    fn lifecycle_driver(&mut self) -> Result<Box<dyn lifecycle::LifecycleDriver>> {
        let driver = self.inner.lifecycle_driver()?;
        self.entered.store(true, Ordering::Release);
        Ok(driver)
    }
    fn act(
        &mut self,
        b: &Binding,
        r: &Resolved,
        i: &Intent,
        v: Option<&str>,
    ) -> Result<Option<bool>> {
        let before = self.inner.input_dispatch_count();
        self.entered.store(true, Ordering::Release);
        let result = self.inner.act(b, r, i, v);
        let after = self.inner.input_dispatch_count();
        if before.is_some() && before == after {
            // Native Windows counters prove a pre-dispatch rejection. Preserve
            // its receipt without unnecessarily fencing the entire input domain.
            self.entered.store(false, Ordering::Release);
        }
        result
    }
}
