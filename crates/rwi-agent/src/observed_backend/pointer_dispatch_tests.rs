//! Memory-only wrapper regressions. No native read or input is permitted here.
use super::*;
use rwi_core::pointer_dispatch::PointerDispatch;

struct Retained(Option<PointerDispatch>);
impl Backend for Retained {
    fn identity(&self) -> Identity {
        panic!("draining evidence must not read native identity")
    }
    fn capabilities(&self) -> serde_json::Value {
        panic!("draining evidence must not read capabilities")
    }
    fn observe(&mut self) -> Result<Scene> {
        panic!("draining evidence must not observe")
    }
    fn inspect(&mut self, _: &Scope, _: &Predicate, _: &str) -> Result<Oracle> {
        panic!("draining evidence must not inspect")
    }
    fn act(
        &mut self,
        _: &Binding,
        _: &Resolved,
        _: &Intent,
        _: Option<&str>,
    ) -> Result<Option<bool>> {
        panic!("draining evidence must not send input")
    }
    fn take_pointer_dispatch(&mut self) -> Option<PointerDispatch> {
        self.0.take()
    }
}

#[test]
fn f406_pointer_trace_drains_once_without_changing_dispatch_custody() {
    for already_entered in [false, true] {
        let entered = Arc::new(AtomicBool::new(already_entered));
        let trace = PointerDispatch::background_window();
        let mut backend = Observed {
            inner: Box::new(Retained(Some(trace.clone()))),
            entered: entered.clone(),
        };
        assert_eq!(backend.take_pointer_dispatch(), Some(trace));
        assert_eq!(backend.take_pointer_dispatch(), None);
        assert_eq!(backend.inner.take_pointer_dispatch(), None);
        assert_eq!(entered.load(Ordering::Acquire), already_entered);
    }
}

#[test]
fn f406_nested_wrappers_do_not_drop_or_borrow_other_backend_evidence() {
    let entered = Arc::new(AtomicBool::new(false));
    let trace = PointerDispatch::background_window();
    let mut nested = Observed {
        inner: Box::new(Observed {
            inner: Box::new(Retained(Some(trace.clone()))),
            entered: entered.clone(),
        }),
        entered: entered.clone(),
    };
    let mut other = Observed {
        inner: Box::new(Retained(None)),
        entered: entered.clone(),
    };
    assert_eq!(other.take_pointer_dispatch(), None);
    assert_eq!(nested.take_pointer_dispatch(), Some(trace));
    assert_eq!(nested.take_pointer_dispatch(), None);
    assert_eq!(other.take_pointer_dispatch(), None);
    assert!(!entered.load(Ordering::Acquire));
}
