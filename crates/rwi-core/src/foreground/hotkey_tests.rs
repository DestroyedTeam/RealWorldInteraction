//! Offline state machine, never global native input.
use crate::{fixture::Fixture, *};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

struct KeyboardFixture {
    app: Fixture,
    attempts: Arc<AtomicU64>,
    effects: Arc<AtomicU64>,
    foreground: Arc<Mutex<ForegroundState>>,
    fail_after: bool,
    known_counter: bool,
    supported: bool,
}
impl Backend for KeyboardFixture {
    fn identity(&self) -> Identity {
        self.app.identity()
    }
    fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({"foreground_keyboard":{"implemented":self.supported}})
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        self.known_counter
            .then(|| self.effects.load(Ordering::SeqCst))
    }
    fn observe(&mut self) -> Result<Scene> {
        self.app.observe()
    }
    fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> Result<Oracle> {
        self.app.inspect(s, p, e)
    }
    fn act(
        &mut self,
        _: &Binding,
        _: &Resolved,
        _: &Intent,
        _: Option<&str>,
    ) -> Result<Option<bool>> {
        panic!("System keyboard must not route to addressed input")
    }
    fn hotkey(&mut self, key: &str, expected: &ForegroundState) -> Result<serde_json::Value> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        if *self.foreground.lock().unwrap() != *expected {
            return Err(fault("foreground_changed", "no send"));
        }
        if key != "cmd+tab" {
            return Err(fault("unsupported_key", "no send"));
        }
        self.effects.fetch_add(2, Ordering::SeqCst);
        if self.fail_after {
            return Err(fault("fixture_partial_error", "effect unknown; no replay"));
        }
        self.foreground.lock().unwrap().pid = 78;
        Ok(serde_json::json!({"synthetic_keyboard_dispatch":true,"goal_verified":false}))
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
}
impl Harness {
    fn new(fail_after: bool, known_counter: bool, supported: bool) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let before = ForegroundState {
            pid: 77,
            window: "cgwindow:9".into(),
            process_start: "77:1:2".into(),
            native_epoch: "a".repeat(64),
        };
        let app = Fixture::default();
        let attempts = Arc::new(AtomicU64::new(0));
        let effects = Arc::new(AtomicU64::new(0));
        let foreground = Arc::new(Mutex::new(before.clone()));
        let mut engine = Engine::new(dir.path().into()).unwrap();
        for target in ["desktop", "alias"] {
            engine
                .register(
                    target.into(),
                    Box::new(KeyboardFixture {
                        app: app.clone(),
                        attempts: attempts.clone(),
                        effects: effects.clone(),
                        foreground: foreground.clone(),
                        fail_after,
                        known_counter,
                        supported,
                    }),
                )
                .unwrap();
        }
        Self {
            dir,
            engine,
            app,
            attempts,
            effects,
            foreground,
            before,
        }
    }
    fn send(&mut self, lease: &str, request: &str, key: &str) -> Result<serde_json::Value> {
        self.engine
            .hotkey("desktop", lease, request, key, &self.before)
    }
}
#[test]
fn global_hotkey_is_distinct_durable_and_invalidates_all_desktop_proposals_without_claiming_goal() {
    let mut h = Harness::new(false, true, true);
    let o = h.engine.observe("alias").unwrap();
    let r = h
        .engine
        .resolve("alias", &o.frame, Locator::Window)
        .unwrap();
    let lease = h.engine.claim("desktop", 60000).unwrap();
    let result = h.send(&lease, "system-switch", "cmd+tab").unwrap();
    assert_eq!(result["receipt"]["dispatch"], "returned");
    assert_eq!(result["receipt"]["goal_verified"], false);
    assert_eq!(
        result["receipt"]["system_accepted"],
        serde_json::Value::Null
    );
    assert_eq!(result["receipt"]["replay"], "forbidden");
    assert!(h.engine.refs.iter().all(|x| x.reference != r.reference));
    assert!(h.engine.snapshots.is_empty());
    h.engine.release(&lease).unwrap();
    let again = h.send("expired-lease", "system-switch", "cmd+tab").unwrap();
    assert_eq!(again["deduplicated"], true);
    assert_eq!(h.attempts.load(Ordering::SeqCst), 1);
    assert_eq!(h.effects.load(Ordering::SeqCst), 2);
    assert_eq!(
        h.send("expired-lease", "system-switch", "shift+cmd+tab")
            .unwrap_err()
            .code,
        "idempotency_conflict"
    );
}
#[test]
fn global_hotkey_rejects_bad_schema_scope_and_lease_before_any_native_attempt() {
    for case in 0..6 {
        let mut h = Harness::new(false, true, case != 5);
        if case == 4 {
            h.before.pid = 0;
        }
        let lease = if case == 3 {
            "not-owned".into()
        } else {
            h.engine.claim("desktop", 60000).unwrap()
        };
        let oversized = "x".repeat(65);
        let key = match case {
            0 => "",
            1 => "cmd+\n",
            2 => &oversized,
            _ => "cmd+tab",
        };
        assert!(h.send(&lease, "guarded-global", key).is_err());
        assert_eq!(h.attempts.load(Ordering::SeqCst), 0);
        assert_eq!(h.effects.load(Ordering::SeqCst), 0);
        assert_eq!(
            std::fs::read_dir(h.dir.path().join("receipts"))
                .unwrap()
                .count(),
            0
        );
    }
}
#[test]
fn global_hotkey_known_predispatch_refusal_vs_unknown_or_partial_effect_keeps_no_replay() {
    for (known, partial) in [(true, false), (false, false), (true, true)] {
        let mut h = Harness::new(partial, known, true);
        if !partial {
            h.foreground.lock().unwrap().pid = 79;
        }
        let lease = h.engine.claim("desktop", 60000).unwrap();
        let r = h.send(&lease, "refused-system", "cmd+tab").unwrap();
        assert_eq!(
            r["receipt"]["dispatch"],
            if known && !partial {
                "rejected_before_dispatch"
            } else {
                "uncertain"
            }
        );
        assert_eq!(r["receipt"]["goal_verified"], false);
        h.send(&lease, "refused-system", "cmd+tab").unwrap();
        assert_eq!(h.attempts.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn global_hotkey_interrupted_reservation_and_precancelled_operation_never_post_or_replay() {
    let mut h = Harness::new(false, true, true);
    let lease = h.engine.claim("desktop", 60000).unwrap();
    let path = h
        .engine
        .journal_path(&h.app.identity().key(), "interrupted-system");
    std::fs::write(path, b"{partial").unwrap();
    assert_eq!(
        h.send(&lease, "interrupted-system", "cmd+tab")
            .unwrap_err()
            .code,
        "uncertain_journal"
    );
    let op = operation::Registry::default().begin(None).unwrap();
    op.cancel();
    assert!(operation::run(&op, || h.send(&lease, "cancelled-system", "cmd+tab")).is_err());
    op.finish();
    assert!(op.status().resources_released);
    assert_eq!(h.attempts.load(Ordering::SeqCst), 0);
}

#[test]
fn global_hotkey_refuses_window_attachments_and_expired_shared_lease() {
    for attached in [true, false] {
        let mut h = Harness::new(false, true, true);
        let lease = h.engine.claim("desktop", 100).unwrap();
        let target = if attached {
            h.engine.targets.get_mut("alias").unwrap().parent = Some("desktop".into());
            "alias"
        } else {
            std::thread::sleep(std::time::Duration::from_millis(120));
            "desktop"
        };
        let error = h
            .engine
            .hotkey(target, &lease, "scope-system", "cmd+tab", &h.before)
            .unwrap_err();
        if attached {
            assert_eq!(error.code, "desktop_target_required");
        }
        assert_eq!(h.attempts.load(Ordering::SeqCst), 0);
        assert_eq!(
            std::fs::read_dir(h.dir.path().join("receipts"))
                .unwrap()
                .count(),
            0
        );
    }
}
