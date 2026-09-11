//! Cooperative cancellation. A pending native call retains the execution slot.
//! Dropping an async waiter is NOT proof that its native worker has stopped.
use crate::{Result, fault};
use serde::Serialize;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub operation_id: String,
    pub cancel_requested: bool,
    pub native_call_pending: bool,
    pub native_call_depth: u32,
    pub finish_requested: bool,
    pub terminal: bool,
    pub resources_released: bool,
    pub resource_release_uncertain: bool,
    pub stage: String,
    pub strong_native_preemption: bool,
}
#[derive(Clone)]
pub struct Operation {
    state: Arc<Mutex<Status>>,
    deadline: Instant,
}
#[derive(Default)]
pub struct Registry {
    entries: Mutex<VecDeque<Operation>>,
}
impl Registry {
    pub fn begin(&self, id: Option<String>) -> Result<Operation> {
        let id = id.unwrap_or_else(crate::id);
        if id.is_empty() || id.len() > 128 {
            return Err(fault("invalid_operation_id", "Use 1..128 characters."));
        }
        let mut entries = self.entries.lock().map_err(|_| {
            fault(
                "operation_registry_unavailable",
                "Restart the observation connection.",
            )
        })?;
        if entries.iter().any(|e| e.status().operation_id == id) {
            return Err(fault(
                "operation_id_reused",
                "Use a new operation ID; this is not an input replay key.",
            ));
        }
        while entries.len() >= 64 {
            let Some(i) = entries.iter().position(|e| e.status().terminal) else {
                return Err(fault("server_busy", "An operation is still running."));
            };
            entries.remove(i);
        }
        let op = Operation {
            state: Arc::new(Mutex::new(Status {
                operation_id: id,
                cancel_requested: false,
                native_call_pending: false,
                native_call_depth: 0,
                finish_requested: false,
                terminal: false,
                resources_released: false,
                resource_release_uncertain: false,
                stage: "queued".into(),
                strong_native_preemption: false,
            })),
            deadline: Instant::now() + Duration::from_secs(15),
        };
        entries.push_back(op.clone());
        Ok(op)
    }
    pub fn get(&self, id: &str, cancel: bool) -> Result<Status> {
        let entries = self.entries.lock().map_err(|_| {
            fault(
                "operation_registry_unavailable",
                "Restart the observation connection.",
            )
        })?;
        let op = entries
            .iter()
            .find(|e| e.status().operation_id == id)
            .ok_or_else(|| {
                fault(
                    "unknown_operation",
                    "Use a fresh caller-supplied operation_id; history is bounded to 64.",
                )
            })?;
        if cancel {
            op.cancel();
        }
        Ok(op.status())
    }
}
impl Operation {
    pub fn status(&self) -> Status {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub fn cancel(&self) {
        let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if !s.terminal {
            s.cancel_requested = true;
        }
    }
    pub fn finish(&self) {
        let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
        s.finish_requested = true;
        finish_if_quiescent(&mut s);
    }
    fn check(&self, stage: &str) -> Result<()> {
        let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
        s.stage = stage.into();
        if s.finish_requested {
            return Err(fault(
                "operation_finishing",
                "This operation cannot enter another native scope after finish was requested.",
            ));
        }
        if s.cancel_requested {
            return Err(fault(
                "operation_cancelled",
                "Reobserve on resume; never replay uncertain input.",
            ));
        }
        if Instant::now() >= self.deadline {
            s.cancel_requested = true;
            return Err(fault(
                "operation_deadline",
                "Native preemption is unavailable; resources remain held until the call returns.",
            ));
        }
        Ok(())
    }
}
thread_local! {
    static CURRENT: RefCell<Option<Operation>> = const { RefCell::new(None) };
    static DISPATCH_DEADLINE: RefCell<Option<Instant>> = const { RefCell::new(None) };
    static CLEANUP_AUDITS: RefCell<Vec<Rc<Cell<bool>>>> = const { RefCell::new(Vec::new()) };
}
fn finish_if_quiescent(s: &mut Status) {
    s.native_call_pending = s.native_call_depth != 0;
    s.terminal = s.finish_requested && !s.native_call_pending;
    s.resources_released = s.terminal && !s.resource_release_uncertain;
    if s.finish_requested {
        s.stage = if s.terminal {
            "finished"
        } else {
            "finishing_native_scopes"
        }
        .into();
    }
}
/// Read the active operation's sticky cleanup signal without claiming worker completion.
pub fn resource_release_is_uncertain() -> bool {
    CURRENT.with(|c| {
        c.borrow()
            .as_ref()
            .is_some_and(|op| op.status().resource_release_uncertain)
    })
}
/// Operation-local cleanup evidence for receipts, including direct core users without a Registry.
/// Nested audits all see failures; one successful closer can never erase another closer's failure.
pub struct CleanupAudit(Rc<Cell<bool>>);
impl CleanupAudit {
    pub fn begin() -> Self {
        let prior = resource_release_is_uncertain()
            || CLEANUP_AUDITS.with(|a| a.borrow().iter().any(|s| s.get()));
        let signal = Rc::new(Cell::new(prior));
        CLEANUP_AUDITS.with(|a| a.borrow_mut().push(signal.clone()));
        Self(signal)
    }
    pub fn uncertain(&self) -> bool {
        self.0.get() || resource_release_is_uncertain()
    }
}
impl Drop for CleanupAudit {
    fn drop(&mut self) {
        CLEANUP_AUDITS.with(|a| a.borrow_mut().retain(|s| !Rc::ptr_eq(s, &self.0)));
    }
}
/// Keep the lease lock through cleanup, but stop sending new batches at expiry.
pub fn with_dispatch_deadline<T>(deadline: Instant, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<Instant>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DISPATCH_DEADLINE.with(|c| *c.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(DISPATCH_DEADLINE.with(|c| c.replace(Some(deadline))));
    f()
}
pub fn run<T>(op: &Operation, f: impl FnOnce() -> T) -> T {
    struct Restore(Option<Operation>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.with(|c| *c.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(CURRENT.with(|c| c.replace(Some(op.clone()))));
    f()
}
pub fn checkpoint(stage: &str) -> Result<()> {
    CURRENT.with(|c| c.borrow().as_ref().map_or(Ok(()), |op| op.check(stage)))?;
    DISPATCH_DEADLINE.with(|c| {
        if c.borrow().is_some_and(|d| Instant::now() >= d) {
            Err(fault("lease_expired_during_dispatch", "No more input batches; cleanup still owns the desktop lock. Never replay partial input."))
        } else { Ok(()) }
    })
}
pub struct NativeCall(Option<Operation>);
/// Cleanup failures must survive worker completion; terminal is not release proof.
pub fn resource_release_uncertain() {
    CLEANUP_AUDITS.with(|a| {
        for signal in a.borrow().iter() {
            signal.set(true);
        }
    });
    CURRENT.with(|c| {
        if let Some(op) = c.borrow().as_ref() {
            let mut s = op.state.lock().unwrap_or_else(|p| p.into_inner());
            s.resource_release_uncertain = true;
            s.resources_released = false;
        }
    });
}
pub fn native_call(stage: &str) -> Result<NativeCall> {
    checkpoint(stage)?;
    let op = CURRENT.with(|c| c.borrow().clone());
    if let Some(op) = &op {
        let mut s = op.state.lock().unwrap_or_else(|p| p.into_inner());
        if s.finish_requested {
            return Err(fault(
                "operation_finishing",
                "No native scope may enter after finish was requested.",
            ));
        }
        s.native_call_depth = s.native_call_depth.checked_add(1).ok_or_else(|| {
            fault(
                "native_scope_limit",
                "Native call nesting exceeded its counter bound.",
            )
        })?;
        s.native_call_pending = true;
        s.terminal = false;
        s.resources_released = false;
    }
    Ok(NativeCall(op))
}
impl Drop for NativeCall {
    fn drop(&mut self) {
        if let Some(op) = &self.0 {
            let mut s = op.state.lock().unwrap_or_else(|p| p.into_inner());
            if s.native_call_depth == 0 {
                s.resource_release_uncertain = true;
            } else {
                s.native_call_depth -= 1;
            }
            finish_if_quiescent(&mut s);
        }
    }
}
/// The async request owns this, while the worker owns the actual operation.
pub struct CancelOnDrop(pub Operation);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_does_not_claim_native_worker_released() {
        let registry = Registry::default();
        let op = registry.begin(Some("pending".into())).unwrap();
        run(&op, || {
            let native = native_call("blocked_provider").unwrap();
            let s = registry.get("pending", true).unwrap();
            assert!(
                s.native_call_pending && s.cancel_requested && !s.resources_released && !s.terminal
            );
            drop(native);
            assert_eq!(
                checkpoint("next_chunk").unwrap_err().code,
                "operation_cancelled"
            );
        });
        assert!(!op.status().resources_released);
        op.finish();
        assert!(op.status().terminal && op.status().resources_released);
    }
    #[test]
    fn dropped_waiter_cancels_and_registry_is_bounded() {
        let r = Registry::default();
        let op = r.begin(Some("first".into())).unwrap();
        drop(CancelOnDrop(op.clone()));
        assert!(op.status().cancel_requested);
        op.finish();
        assert!(r.begin(Some("first".into())).is_err());
        for _ in 0..100 {
            r.begin(None).unwrap().finish();
        }
        assert_eq!(r.entries.lock().unwrap().len(), 64);
    }
    #[test]
    fn run_restores_thread_context_after_unwind() {
        let op = Registry::default().begin(None).unwrap();
        op.cancel();
        let _ = std::panic::catch_unwind(|| run(&op, || panic!("test")));
        assert!(checkpoint("outside").is_ok());
    }
    #[test]
    fn lease_deadline_stops_new_dispatch_and_context_restores() {
        let past = Instant::now() - Duration::from_millis(1);
        with_dispatch_deadline(past, || {
            assert_eq!(
                checkpoint("next_native_batch").unwrap_err().code,
                "lease_expired_during_dispatch"
            );
            assert!(native_call("sendinput").is_err());
        });
        assert!(checkpoint("outside").is_ok());
    }
    #[test]
    fn nested_scopes_and_finish_preserve_pending_until_last_drop() {
        let op = Registry::default().begin(None).unwrap();
        run(&op, || {
            let outer = native_call("outer").unwrap();
            let inner = native_call("inner").unwrap();
            assert_eq!(op.status().native_call_depth, 2);
            drop(inner);
            assert!(op.status().native_call_pending);
            op.finish();
            assert!(op.status().finish_requested);
            assert!(!op.status().terminal && !op.status().resources_released);
            assert_eq!(
                native_call("after_finish").err().unwrap().code,
                "operation_finishing"
            );
            drop(outer);
            assert_eq!(op.status().native_call_depth, 0);
            assert!(!op.status().native_call_pending);
            assert!(op.status().terminal && op.status().resources_released);
        });
    }
    #[test]
    fn inner_unwind_does_not_finish_the_outer_native_scope() {
        let op = Registry::default().begin(None).unwrap();
        run(&op, || {
            let outer = native_call("outer").unwrap();
            let panic = std::panic::catch_unwind(|| {
                let _inner = native_call("inner").unwrap();
                panic!("offline unwind");
            });
            assert!(panic.is_err());
            assert_eq!(op.status().native_call_depth, 1);
            assert!(op.status().native_call_pending);
            drop(outer);
            assert!(!op.status().native_call_pending);
            assert!(!op.status().terminal);
        });
        op.finish();
        assert!(op.status().resources_released);
    }
    #[test]
    fn cleanup_audits_are_nested_sticky_and_do_not_leak_between_direct_calls() {
        let outer = CleanupAudit::begin();
        resource_release_uncertain();
        let inner = CleanupAudit::begin();
        assert!(inner.uncertain());
        assert!(outer.uncertain());
        drop(inner);
        drop(outer);
        assert!(!CleanupAudit::begin().uncertain());
        let op = Registry::default().begin(None).unwrap();
        run(&op, || {
            resource_release_uncertain();
            assert!(CleanupAudit::begin().uncertain());
        });
        op.finish();
        assert!(!op.status().resources_released);
    }
}
