use super::*;
mod pointer_dispatch;
mod text_effect;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

struct Rejecting {
    fixture: fixture::Fixture,
    attempts: Arc<AtomicU64>,
    effects: Option<u64>,
    sends: bool,
}
impl Backend for Rejecting {
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
        self.effects
    }
    fn act(
        &mut self,
        _: &Binding,
        _: &Resolved,
        _: &Intent,
        _: Option<&str>,
    ) -> Result<Option<bool>> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        if self.sends {
            self.effects = self.effects.map(|n| n.saturating_add(1));
        }
        Err(fault("same_native_error", "Inspect; never replay."))
    }
}

#[test]
fn native_counts_not_error_names_control_durable_rejection_and_deduplication() {
    for (count, sends, expected) in [
        (Some(0), false, "rejected_before_dispatch"),
        (Some(8), false, "rejected_before_dispatch"),
        (Some(0), true, "uncertain"),
        (None, false, "uncertain"),
        (Some(u64::MAX), true, "uncertain"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let calls = Arc::new(AtomicU64::new(0));
        let make = || {
            Box::new(Rejecting {
                fixture: fixture::Fixture::default(),
                attempts: calls.clone(),
                effects: count,
                sends,
            })
        };
        let mut engine = Engine::new(dir.path().into()).unwrap();
        engine.register("test".into(), make()).unwrap();
        let observation = engine.observe("test").unwrap();
        let reference = engine
            .resolve(
                "test",
                &observation.frame,
                Locator::Element {
                    id: observation.scene.elements[0].id.clone(),
                },
            )
            .unwrap();
        let lease = engine.claim("test", 60000).unwrap();
        let first = engine
            .act(
                "test",
                &reference.reference,
                &lease,
                "one-shot",
                Intent::Click,
                None,
            )
            .unwrap();
        assert_eq!(first.dispatch, expected);
        assert_eq!(
            first.dispatch_error.as_ref().unwrap().code,
            "same_native_error"
        );
        assert_eq!(first.replay, "forbidden");
        assert!(!first.goal_verified);
        engine.release(&lease).unwrap();
        drop(engine);
        let mut engine = Engine::new(dir.path().into()).unwrap();
        engine.register("test".into(), make()).unwrap();
        let replay = engine
            .act(
                "test",
                &reference.reference,
                &lease,
                "one-shot",
                Intent::Click,
                None,
            )
            .unwrap();
        assert_eq!(replay.dispatch, expected);
        assert!(replay.deduplicated);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
