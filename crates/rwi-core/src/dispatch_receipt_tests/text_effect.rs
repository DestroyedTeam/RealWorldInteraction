//! Synthetic engine/journal coverage only; no assertion of Native field effects.
use super::*;
use crate::text_effect::{State, TextEffect, UnknownReason};

struct Reporting {
    fixture: fixture::Fixture,
    retained: Option<TextEffect>,
    next: Option<TextEffect>,
    calls: Arc<AtomicU64>,
    drains: Arc<AtomicU64>,
    failure: u8,
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
    fn take_text_effect(&mut self) -> Option<TextEffect> {
        self.drains.fetch_add(1, Ordering::SeqCst);
        self.retained.take()
    }
    fn act(
        &mut self,
        b: &Binding,
        r: &Resolved,
        i: &Intent,
        v: Option<&str>,
    ) -> Result<Option<bool>> {
        if self.failure == 1 {
            return Err(fault("fixture_pre_dispatch", "No input."));
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.retained = self.next.take();
        if self.failure == 2 {
            return Err(fault("fixture_partial", "Never replay."));
        }
        self.fixture.act(b, r, i, v)?;
        Ok(None)
    }
}
fn effect(state: State) -> TextEffect {
    TextEffect {
        state,
        reason: None,
        expected_utf16_units: Some(18),
        observed_utf16_units: Some(if state == State::ValueMatchesExpected {
            18
        } else {
            10
        }),
    }
}
fn backend(next: Option<TextEffect>, failure: u8) -> Reporting {
    Reporting {
        fixture: fixture::Fixture::default(),
        retained: Some(effect(State::ValueMatchesExpected)),
        next,
        calls: Arc::new(AtomicU64::new(0)),
        drains: Arc::new(AtomicU64::new(0)),
        failure,
    }
}
fn action(engine: &mut Engine, request: &str, intent: Intent) -> (Receipt, String, String) {
    let obs = engine.observe("test").unwrap();
    let reference = engine
        .resolve("test", &obs.frame, Locator::Element { id: "Input".into() })
        .unwrap();
    let lease = engine.claim("test", 60000).unwrap();
    let result = engine
        .act(
            "test",
            &reference.reference,
            &lease,
            request,
            intent,
            Some("RWI 中λ🙂𝄞 e\u{301}👩\u{200d}💻".into()),
        )
        .unwrap();
    engine.release(&lease).unwrap();
    (result, reference.reference, lease)
}

#[test]
fn f21_text_effect_engine_persists_match_and_mismatch_without_goal_or_replay() {
    for state in [State::ValueMatchesExpected, State::ValueDiffersObserved] {
        let dir = tempfile::tempdir().unwrap();
        let b = backend(Some(effect(state)), 0);
        let calls = b.calls.clone();
        let drains = b.drains.clone();
        let mut engine = Engine::new(dir.path().into()).unwrap();
        engine.register("test".into(), Box::new(b)).unwrap();
        let (receipt, reference, lease) = action(&mut engine, "text-once", Intent::Text);
        assert_eq!(receipt.text_effect, Some(effect(state)));
        assert_eq!(receipt.system_accepted, None);
        assert!(!receipt.goal_verified);
        assert_eq!(receipt.dispatch, "returned");
        assert_eq!(receipt.replay, "forbidden");
        assert_eq!(drains.load(Ordering::SeqCst), 2);
        drop(engine);
        let mut engine = Engine::new(dir.path().into()).unwrap();
        let mut b = backend(None, 0);
        b.calls = calls.clone();
        b.drains = drains.clone();
        engine.register("test".into(), Box::new(b)).unwrap();
        let receipt = engine
            .act(
                "test",
                &reference,
                &lease,
                "text-once",
                Intent::Text,
                Some("RWI 中λ🙂𝄞 e\u{301}👩\u{200d}💻".into()),
            )
            .unwrap();
        assert!(receipt.deduplicated);
        assert_eq!(receipt.text_effect, Some(effect(state)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(drains.load(Ordering::SeqCst), 2);
    }
}

#[test]
fn f21_text_effect_engine_clears_stale_before_reportless_act() {
    let dir = tempfile::tempdir().unwrap();
    let b = backend(None, 0);
    let drains = b.drains.clone();
    let mut engine = Engine::new(dir.path().into()).unwrap();
    engine.register("test".into(), Box::new(b)).unwrap();
    let (receipt, _, _) = action(&mut engine, "no-witness", Intent::Text);
    assert_eq!(receipt.text_effect, None);
    assert_eq!(drains.load(Ordering::SeqCst), 2);
    assert!(
        serde_json::to_value(receipt)
            .unwrap()
            .get("text_effect")
            .is_none()
    );
}

#[test]
fn f21_text_effect_engine_nontext_never_promotes_backend_sample() {
    let dir = tempfile::tempdir().unwrap();
    let b = backend(Some(effect(State::ValueMatchesExpected)), 0);
    let mut engine = Engine::new(dir.path().into()).unwrap();
    engine.register("test".into(), Box::new(b)).unwrap();
    let (nontext, _, _) = action(&mut engine, "chord-once", Intent::KeyChord);
    assert_eq!(nontext.text_effect, None);
    let (text, _, _) = action(&mut engine, "text-next", Intent::Text);
    assert_eq!(text.text_effect, None);
}

#[test]
fn f21_text_effect_engine_partial_keeps_dispatch_uncertainty_independent() {
    let dir = tempfile::tempdir().unwrap();
    let sample = TextEffect::unknown(UnknownReason::DispatchIncomplete);
    let mut engine = Engine::new(dir.path().into()).unwrap();
    engine
        .register("test".into(), Box::new(backend(Some(sample.clone()), 2)))
        .unwrap();
    let (receipt, _, _) = action(&mut engine, "partial", Intent::Text);
    assert_eq!(receipt.dispatch, "uncertain");
    assert_eq!(receipt.text_effect, Some(sample));
    assert_eq!(receipt.system_accepted, None);
    assert!(!receipt.goal_verified);
}

#[test]
fn f21_text_effect_engine_predispatch_error_cannot_inherit_stale_success() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::new(dir.path().into()).unwrap();
    engine
        .register("test".into(), Box::new(backend(None, 1)))
        .unwrap();
    let (receipt, _, _) = action(&mut engine, "before-dispatch", Intent::Text);
    assert_eq!(receipt.dispatch, "rejected_before_dispatch");
    assert_eq!(receipt.text_effect, None);
}

#[test]
fn f21_text_effect_old_receipt_deserializes_absent_extension() {
    let json = serde_json::json!({"request_id":"old","frame":"f","scope":{"window":"w","pid":42,"element":null},"dispatch":"returned","dispatch_error":null,"system_accepted":null,"goal_verified":false,"replay":"forbidden","deduplicated":false});
    let receipt: Receipt = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(receipt.text_effect, None);
    assert_eq!(serde_json::to_value(receipt).unwrap(), json);
    assert_eq!(fixture::Fixture::default().take_text_effect(), None);
}
