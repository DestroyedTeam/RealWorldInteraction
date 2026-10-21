//! Synthetic backend/journal tests, never real pointer delivery or GUI effects.
use super::*;
use crate::pointer_dispatch::{PointerDispatch, PointerPost, ReceiverEffect};

struct Reporting {
    fixture: fixture::Fixture,
    retained: Option<PointerDispatch>,
    calls: Arc<AtomicU64>,
    failure: bool,
    reports: bool,
}
fn sample() -> PointerDispatch {
    let mut trace = PointerDispatch::background_window();
    trace.record_returned(PointerPost {
        event_type: 1,
        point: [50., 50.],
        click_count: 1,
        event_number: 7,
        source_state_id: -2,
        target_pid: 123,
        target_window: 456,
        flags: 0,
        pressure: 1.,
        post_return_elapsed_us: 5,
    });
    trace
}
impl Backend for Reporting {
    fn identity(&self) -> Identity {
        self.fixture.identity()
    }
    fn capabilities(&self) -> serde_json::Value {
        self.fixture.capabilities()
    }
    fn observe(&mut self) -> Result<Scene> {
        self.fixture.observe()
    }
    fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> Result<Oracle> {
        self.fixture.inspect(s, p, e)
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        Some(self.calls.load(Ordering::SeqCst))
    }
    fn take_pointer_dispatch(&mut self) -> Option<PointerDispatch> {
        self.retained.take()
    }
    fn act(
        &mut self,
        _: &Binding,
        _: &Resolved,
        _: &Intent,
        _: Option<&str>,
    ) -> Result<Option<bool>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.retained = self.reports.then(sample);
        if self.failure {
            Err(fault("synthetic_partial", "Never replay."))
        } else {
            Ok(None)
        }
    }
}

#[test]
fn f405_pointer_transcript_journal_keeps_uncertainty_and_deduplicates_without_posting() {
    for failure in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let calls = Arc::new(AtomicU64::new(0));
        let mut engine = Engine::new(dir.path().into()).unwrap();
        engine
            .register(
                "test".into(),
                Box::new(Reporting {
                    fixture: fixture::Fixture::default(),
                    retained: Some(sample()),
                    calls: calls.clone(),
                    failure,
                    reports: true,
                }),
            )
            .unwrap();
        let observation = engine.observe("test").unwrap();
        let reference = engine
            .resolve(
                "test",
                &observation.frame,
                Locator::Element { id: "Input".into() },
            )
            .unwrap();
        let lease = engine.claim("test", 60000).unwrap();
        let first = engine
            .act(
                "test",
                &reference.reference,
                &lease,
                "pointer-once",
                Intent::Click,
                None,
            )
            .unwrap();
        assert_eq!(
            first.pointer_dispatch.as_ref().unwrap().receiver_effect,
            ReceiverEffect::Unobserved
        );
        assert_eq!(first.pointer_dispatch, Some(sample()));
        assert_eq!(first.system_accepted, None);
        assert!(!first.goal_verified);
        assert_eq!(first.replay, "forbidden");
        assert_eq!(
            first.dispatch,
            if failure { "uncertain" } else { "returned" }
        );
        let second = engine
            .act(
                "test",
                &reference.reference,
                &lease,
                "pointer-once",
                Intent::Click,
                None,
            )
            .unwrap();
        assert!(second.deduplicated);
        assert_eq!(second.pointer_dispatch, first.pointer_dispatch);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn f405_pointer_trace_is_bounded_and_legacy_receipts_stay_absent() {
    let mut trace = sample();
    for _ in 0..10 {
        trace.record_returned(sample().events.remove(0));
    }
    assert_eq!(trace.events.len(), 4);
    assert!(trace.truncated);
    let old = serde_json::json!({"request_id":"old","frame":"f","scope":{"pid":1,"window":"w","element":null},"dispatch":"returned","dispatch_error":null,"system_accepted":null,"goal_verified":false,"replay":"forbidden","deduplicated":false});
    let receipt: Receipt = serde_json::from_value(old).unwrap();
    assert!(receipt.pointer_dispatch.is_none());
    assert!(
        serde_json::to_value(receipt)
            .unwrap()
            .get("pointer_dispatch")
            .is_none()
    );
    assert!(
        fixture::Fixture::default()
            .take_pointer_dispatch()
            .is_none()
    );
}

#[test]
fn f405_pointer_trace_does_not_reuse_stale_or_nonpointer_samples() {
    for (reports, intent, value) in [
        (false, Intent::Click, None),
        (true, Intent::Text, Some("x".into())),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().into()).unwrap();
        engine
            .register(
                "test".into(),
                Box::new(Reporting {
                    fixture: fixture::Fixture::default(),
                    retained: Some(sample()),
                    calls: Arc::new(AtomicU64::new(0)),
                    failure: false,
                    reports,
                }),
            )
            .unwrap();
        let observation = engine.observe("test").unwrap();
        let reference = engine
            .resolve(
                "test",
                &observation.frame,
                Locator::Element { id: "Input".into() },
            )
            .unwrap();
        let lease = engine.claim("test", 60000).unwrap();
        let receipt = engine
            .act(
                "test",
                &reference.reference,
                &lease,
                "not-a-pointer-sample",
                intent,
                value,
            )
            .unwrap();
        assert!(receipt.pointer_dispatch.is_none());
        assert!(!receipt.goal_verified);
        assert_eq!(receipt.system_accepted, None);
    }
}
