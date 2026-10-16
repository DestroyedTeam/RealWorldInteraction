//! Synthetic engine contract tests; no native UI is opened or input sent.
use crate::*;
use fixture::Fixture;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy)]
enum Outcome {
    Returned,
    Rejected,
    Uncertain,
    UnknownCounter,
}
struct FocusFixture {
    app: Fixture,
    attempts: Arc<AtomicU64>,
    effects: Arc<AtomicU64>,
    foreground: Arc<Mutex<ForegroundState>>,
    outcome: Outcome,
}
impl Backend for FocusFixture {
    fn identity(&self) -> Identity {
        self.app.identity()
    }
    fn capabilities(&self) -> serde_json::Value {
        self.app.capabilities()
    }
    fn observe(&mut self) -> Result<Scene> {
        self.app.observe()
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        if matches!(self.outcome, Outcome::UnknownCounter) {
            None
        } else {
            Some(self.effects.load(Ordering::SeqCst))
        }
    }
    fn focus_window(
        &mut self,
        _: &Resolved,
        expected: &ForegroundState,
    ) -> Result<serde_json::Value> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        if *self.foreground.lock().unwrap() != *expected {
            return Err(fault(
                "foreground_changed",
                "Synthetic receiver changed; no send.",
            ));
        }
        match self.outcome {
            Outcome::Rejected | Outcome::UnknownCounter => {
                Err(fault("fixture_native_error", "No fallback or retry."))
            }
            Outcome::Uncertain => {
                self.effects.fetch_add(1, Ordering::SeqCst);
                Err(fault(
                    "fixture_native_error",
                    "Effect uncertain; no replay.",
                ))
            }
            Outcome::Returned => {
                self.effects.fetch_add(1, Ordering::SeqCst);
                Ok(
                    serde_json::json!({"state":"synthetic_foreground_observed", "goal_verified":false, "input_authority_granted":false}),
                )
            }
        }
    }
    fn act(
        &mut self,
        _: &Binding,
        _: &Resolved,
        _: &Intent,
        _: Option<&str>,
    ) -> Result<Option<bool>> {
        panic!("Explicit focus must never fall back to keys, pointer input or act")
    }
    fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> Result<Oracle> {
        self.app.inspect(s, p, e)
    }
}
struct Harness {
    dir: tempfile::TempDir,
    engine: Engine,
    app: Fixture,
    attempts: Arc<AtomicU64>,
    effects: Arc<AtomicU64>,
    foreground: Arc<Mutex<ForegroundState>>,
    before: ForegroundState,
    outcome: Outcome,
}
impl Harness {
    fn new(outcome: Outcome) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let before = ForegroundState {
            pid: 77,
            window: "cgwindow:9".into(),
            process_start: "77:1:2".into(),
            native_epoch: "a".repeat(64),
        };
        let mut h = Self {
            engine: Engine::new(dir.path().into()).unwrap(),
            dir,
            app: Fixture::default(),
            attempts: Arc::new(AtomicU64::new(0)),
            effects: Arc::new(AtomicU64::new(0)),
            foreground: Arc::new(Mutex::new(before.clone())),
            before,
            outcome,
        };
        h.register();
        h
    }
    fn register(&mut self) {
        for target in ["test", "alias"] {
            self.engine
                .register(
                    target.into(),
                    Box::new(FocusFixture {
                        app: self.app.clone(),
                        attempts: self.attempts.clone(),
                        effects: self.effects.clone(),
                        foreground: self.foreground.clone(),
                        outcome: self.outcome,
                    }),
                )
                .unwrap();
        }
    }
    fn resolve(&mut self, target: &str, locator: Locator) -> Resolved {
        let o = self.engine.observe(target).unwrap();
        self.engine.resolve(target, &o.frame, locator).unwrap()
    }
    fn focus(&mut self, r: &Resolved, lease: &str, request: &str) -> Result<serde_json::Value> {
        self.engine
            .focus_window("test", lease, &r.reference, request, &self.before)
    }
    fn assert_no_effect(&self) {
        assert_eq!(self.attempts.load(Ordering::SeqCst), 0);
        assert_eq!(self.effects.load(Ordering::SeqCst), 0);
        assert_eq!(
            std::fs::read_dir(self.dir.path().join("receipts"))
                .unwrap()
                .count(),
            0
        );
    }
}
#[test]
fn foreground_explicit_return_is_not_goal_and_invalidates_every_desktop_alias() {
    let mut h = Harness::new(Outcome::Returned);
    let alias = h.resolve("alias", Locator::Window);
    let r = h.resolve("test", Locator::Window);
    let lease = h.engine.claim("test", 60000).unwrap();
    let result = h.focus(&r, &lease, "one-focus").unwrap();
    assert_eq!(result["receipt"]["dispatch"], "returned");
    assert_eq!(result["receipt"]["goal_verified"], false);
    assert_eq!(result["activation"]["input_authority_granted"], false);
    assert_eq!(
        result["grounding_invalidated"]["scope"],
        "same_native_desktop"
    );
    assert_eq!(result["grounding_invalidated"]["target_count"], 2);
    assert_eq!(
        result["grounding_invalidated"]["context"],
        "current_executor"
    );
    assert_eq!(
        result["grounding_invalidated"]["next"],
        "snapshot_then_resolve"
    );
    assert_eq!(h.attempts.load(Ordering::SeqCst), 1);
    assert!(!h.engine.refs.iter().any(|v| v.reference == alias.reference));
    assert!(!h.engine.snapshots.iter().any(|(_, s)| s.target == "alias"));
    h.engine.release(&lease).unwrap();
}
#[test]
fn foreground_refuses_control_pixel_wrong_target_bad_tuple_and_expired_window() {
    for case in 0..5 {
        let mut h = Harness::new(Outcome::Returned);
        let locator = match case {
            0 => Locator::Semantic {
                name: "Settings".into(),
            },
            1 => Locator::Pixel { x: 200., y: 100. },
            _ => Locator::Window,
        };
        let r = h.resolve(if case == 2 { "alias" } else { "test" }, locator);
        if case == 3 {
            h.before.pid = 0;
        }
        if case == 4 {
            h.engine.snapshots.back_mut().unwrap().1.at -= Duration::from_secs(11);
        }
        let lease = h.engine.claim("test", 60000).unwrap();
        let error = h.focus(&r, &lease, "no-focus").unwrap_err();
        assert_eq!(
            error.code,
            match case {
                0 | 1 => "window_reference_required",
                2 => "unknown_reference",
                3 => "invalid_foreground_tuple",
                _ => "stale_reference",
            }
        );
        h.assert_no_effect();
        h.engine.release(&lease).unwrap();
    }
}
#[test]
fn foreground_fresh_binding_rejects_layout_epoch_modal_identity_and_document_change() {
    for case in 0..6 {
        let mut h = Harness::new(Outcome::Returned);
        let r = h.resolve("test", Locator::Window);
        let lease = h.engine.claim("test", 60000).unwrap();
        {
            let mut a = h.app.0.lock().unwrap();
            match case {
                0 => a.transform = None,
                1 => a.epoch += 1,
                2 => a.blocked = Some(true),
                3 => a.ready = false,
                4 => a.identity.session = "other-session".into(),
                _ => a.value = "other-document".into(),
            }
        }
        assert_eq!(
            h.focus(&r, &lease, "changed").unwrap_err().code,
            "stale_reference"
        );
        h.assert_no_effect();
        h.engine.release(&lease).unwrap();
    }
}
#[test]
fn foreground_checks_lease_and_cancellation_before_reservation() {
    let mut h = Harness::new(Outcome::Returned);
    let r = h.resolve("test", Locator::Window);
    assert!(h.focus(&r, "not-held", "no-lease").is_err());
    h.assert_no_effect();
    let lease = h.engine.claim("test", 60000).unwrap();
    let op = operation::Registry::default().begin(None).unwrap();
    op.cancel();
    assert_eq!(
        operation::run(&op, || h.focus(&r, &lease, "cancelled"))
            .unwrap_err()
            .code,
        "operation_cancelled"
    );
    h.assert_no_effect();
    h.engine.release(&lease).unwrap();
}
#[test]
fn foreground_changed_before_dispatch_has_zero_effect_and_durable_rejection() {
    let mut h = Harness::new(Outcome::Returned);
    let r = h.resolve("test", Locator::Window);
    let lease = h.engine.claim("test", 60000).unwrap();
    h.foreground.lock().unwrap().native_epoch = "b".repeat(64);
    let result = h.focus(&r, &lease, "changed-before-send").unwrap();
    assert_eq!(result["receipt"]["dispatch"], "rejected_before_dispatch");
    assert_eq!(
        result["receipt"]["dispatch_error"]["code"],
        "foreground_changed"
    );
    assert_eq!(h.effects.load(Ordering::SeqCst), 0);
    assert_eq!(result["grounding_invalidated"]["target_count"], 2);
    h.engine.release(&lease).unwrap();
}
#[test]
fn foreground_native_error_is_counted_not_named_and_never_replayed_after_restart() {
    for (outcome, expected) in [
        (Outcome::Returned, "returned"),
        (Outcome::Rejected, "rejected_before_dispatch"),
        (Outcome::Uncertain, "uncertain"),
        (Outcome::UnknownCounter, "uncertain"),
    ] {
        let mut h = Harness::new(outcome);
        let r = h.resolve("test", Locator::Window);
        let lease = h.engine.claim("test", 60000).unwrap();
        let first = h.focus(&r, &lease, "durable-once").unwrap();
        assert_eq!(first["receipt"]["dispatch"], expected);
        assert_eq!(first["receipt"]["replay"], "forbidden");
        assert_eq!(first["receipt"]["goal_verified"], false);
        assert_eq!(
            first["grounding_invalidated"]["context"],
            "current_executor"
        );
        assert_eq!(first["grounding_invalidated"]["target_count"], 2);
        h.engine.release(&lease).unwrap();
        h.engine = Engine::new(h.dir.path().into()).unwrap();
        h.register();
        let duplicate = h.focus(&r, &lease, "durable-once").unwrap();
        assert_eq!(duplicate["receipt"]["dispatch"], expected);
        assert_eq!(duplicate["receipt"]["deduplicated"], true);
        // Cached durable readback must not claim a new invalidation or input.
        assert!(duplicate["grounding_invalidated"].is_null());
        assert_eq!(h.attempts.load(Ordering::SeqCst), 1);
        h.before.native_epoch = "c".repeat(64);
        assert_eq!(
            h.focus(&r, &lease, "durable-once").unwrap_err().code,
            "idempotency_conflict"
        );
        assert_eq!(h.attempts.load(Ordering::SeqCst), 1);
    }
}
