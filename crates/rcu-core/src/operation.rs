//! Cooperative cancellation. A pending native call retains the execution slot.
//! Dropping an async waiter is NOT proof that its native worker has stopped.
use crate::{Result, fault};
use serde::Serialize;
use std::{
    cell::RefCell,
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub operation_id: String,
    pub cancel_requested: bool,
    pub native_call_pending: bool,
    pub terminal: bool,
    pub resources_released: bool,
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
                terminal: false,
                resources_released: false,
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
        s.terminal = true;
        s.resources_released = true;
        s.native_call_pending = false;
        s.stage = "finished".into();
    }
    fn check(&self, stage: &str) -> Result<()> {
        let mut s = self.state.lock().unwrap_or_else(|p| p.into_inner());
        s.stage = stage.into();
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
pub fn native_call(stage: &str) -> Result<NativeCall> {
    checkpoint(stage)?;
    let op = CURRENT.with(|c| c.borrow().clone());
    if let Some(op) = &op {
        op.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .native_call_pending = true;
    }
    Ok(NativeCall(op))
}
impl Drop for NativeCall {
    fn drop(&mut self) {
        if let Some(op) = &self.0 {
            op.state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .native_call_pending = false;
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
}
