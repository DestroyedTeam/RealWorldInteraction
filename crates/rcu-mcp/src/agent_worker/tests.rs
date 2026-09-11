use super::*;
use rcu_windows::agent_contract::{AuthenticationId, OwnerProof, ProcessBinding, SessionBinding};
fn spec() -> WorkerGuardSpec {
    let session = SessionBinding {
        native: Identity {
            host: format!("windows-machine:{}", "1".repeat(64)),
            guest: None,
            session: "wts:1:logon:123:luid:0:456".into(),
            seat: "WinSta0".into(),
            desktop: "Default".into(),
        },
        user_sid: "S-1-5-21-1-2-3-1001".into(),
        session_id: 1,
        authentication_id: AuthenticationId {
            high: 0,
            low: 0x456,
        },
        logon_time_100ns: 123,
        input_desktop: "Default".into(),
    };
    WorkerGuardSpec {
        privilege_mode: rcu_windows::agent_contract::PrivilegeMode::Ordinary,
        version: 1,
        task: "test-task".into(),
        stage: Stage::Model,
        budget: rcu_native_link::budget::BudgetSpec {
            issued_unix_ms: now_ms(),
            expires_unix_ms: now_ms() + 60_000,
            maximum_elapsed_ms: 60_000,
            clock_drift_tolerance_ms: 100,
        },
        owner: OwnerProof {
            process: ProcessBinding {
                pid: 123,
                creation_filetime_100ns: 456,
                user_sid: session.user_sid.clone(),
                session_id: 1,
                authentication_id: session.authentication_id,
                image_path: r"C:\private\agent.exe".into(),
                image_sha256: "2".repeat(64),
            },
            session,
        },
        input_blocked_path: r"C:\private\blocked-test-task.json".into(),
        expected_foreground: Some(rcu_windows::NativeWindowKey {
            window: "hwnd:123".into(),
            pid: 1234,
            process_created: 999,
        }),
        expected_root_native_epoch: Some("3".repeat(64)),
        model_ready_path: Some(format!(r"C:\private\ready-{}.json", "4".repeat(64)).into()),
        expected_ui_value: "approved value".into(),
        request_prefix: "test-task-".into(),
    }
}
#[test]
fn guard_spec_rejects_unknown_wrong_stage_marker_task_prefix_and_grant() {
    let original = spec();
    validate_spec(&original, r"C:\private\guard.json").unwrap();
    let mut bad = vec![original.clone(); 9];
    bad[0].version = 2;
    bad[1].stage = Stage::Cleanup;
    bad[2].request_prefix = "other-".into();
    bad[3].input_blocked_path = r"C:\other\blocked-test-task.json".into();
    bad[4].task = "../escape".into();
    bad[5].expected_foreground = None;
    bad[6].owner.process.creation_filetime_100ns = 0;
    bad[7].budget.expires_unix_ms = now_ms() - 1;
    bad[8].input_blocked_path = r"C:\private\other.json".into();
    for candidate in bad {
        assert!(validate_spec(&candidate, r"C:\private\guard.json").is_err());
    }
    let mut value = serde_json::to_value(original).unwrap();
    value["remote_command"] = "shell".into();
    assert!(serde_json::from_value::<WorkerGuardSpec>(value).is_err());
}
#[test]
fn cli_and_stage_are_separate_fail_closed_gates() {
    let absolute = std::env::temp_dir().join("guard.json");
    validate_guard_cli(&absolute, true, Some("local-id"), false).unwrap();
    for (windows, pipe, macos) in [
        (false, Some("p"), false),
        (true, None, false),
        (true, Some("p"), true),
    ] {
        assert!(validate_guard_cli(&absolute, windows, pipe, macos).is_err());
    }
    assert!(validate_guard_cli(Path::new("relative"), true, Some("p"), false).is_err());
    for stage in [
        Stage::Prepare,
        Stage::Bootstrap,
        Stage::Preflight,
        Stage::Launch,
        Stage::Activate,
        Stage::Model,
        Stage::Oracle,
        Stage::Cleanup,
    ] {
        assert_eq!(
            operation_allowed(stage, GuardOperation::Input),
            stage == Stage::Model
        );
        assert_eq!(
            operation_allowed(stage, GuardOperation::Launch),
            stage == Stage::Launch
        );
        assert_eq!(
            operation_allowed(stage, GuardOperation::Activate),
            stage == Stage::Activate
        );
    }
}

#[cfg(not(windows))]
#[test]
fn actual_native_feature_cli_refuses_missing_route_relative_path_and_non_windows() {
    let current = std::env::current_exe().unwrap();
    let binary = current.parent().unwrap().parent().unwrap().join("rcu-mcp");
    assert!(
        binary.is_file(),
        "cargo test -p rcu-mcp must build the actual binary for the original stdio integration suite"
    );
    let absolute = std::env::temp_dir()
        .join("never-read-rcu-guard.json")
        .to_str()
        .unwrap()
        .to_owned();
    for (args, code) in [
        (
            vec!["--agent-guard", absolute.as_str()],
            "agent_worker_cli_required",
        ),
        (
            vec![
                "--agent-guard",
                "relative",
                "--windows-desktop",
                "--pipe-server",
                "offline",
            ],
            "agent_worker_cli_required",
        ),
        (
            vec![
                "--agent-guard",
                absolute.as_str(),
                "--windows-desktop",
                "--pipe-server",
                "offline",
            ],
            "unsupported_platform",
        ),
        (
            vec![
                "--agent-guard",
                absolute.as_str(),
                "--agent-guard",
                absolute.as_str(),
            ],
            "invalid_cli",
        ),
    ] {
        let output = std::process::Command::new(&binary)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains(code));
    }
}

struct TestGuard {
    gate: GuardGate,
    native_allowed: AtomicBool,
}
impl TestGuard {
    fn new(stage: Stage) -> Arc<Self> {
        Arc::new(Self {
            gate: GuardGate::new(stage, Budget::start(spec().budget).unwrap()),
            native_allowed: AtomicBool::new(true),
        })
    }
}
impl GuardChecks for TestGuard {
    fn check(&self, op: GuardOperation) -> Result<()> {
        self.gate.check(op, || {
            if self.native_allowed.load(Ordering::Acquire) {
                Ok(())
            } else {
                Err(fault(
                    "simulated_native_fact_hold",
                    "Offline peer/session/marker/focus negative; not a native query.",
                ))
            }
        })
    }
    fn hold(&self) {
        self.gate.hold();
    }
    fn deadline(&self) -> Instant {
        self.gate.budget.deadline()
    }
    fn metadata(&self) -> Value {
        json!({"test_only":true,"held":self.gate.held.load(Ordering::Acquire)})
    }
}

#[test]
fn guard_native_fact_loss_is_sticky_and_prevents_delegation() {
    let fixture = rcu_core::fixture::Fixture::default();
    let guard = TestGuard::new(Stage::Model);
    let mut backend = GuardedBackend::new(fixture.clone(), guard.clone());
    backend.observe().unwrap();
    assert_eq!(backend.capabilities()["agent_worker"]["test_only"], true);
    guard.native_allowed.store(false, Ordering::Release);
    assert!(backend.observe().is_err());
    guard.native_allowed.store(true, Ordering::Release);
    assert!(backend.observe().is_err());
    assert_eq!(backend.capabilities()["input"], false);
    assert_eq!(fixture.0.lock().unwrap().calls, 0);
}

#[test]
fn wrapped_frozen_fixture_dispatches_once_and_uncertainty_holds_new_effects() {
    for disconnect in [false, true] {
        let state = tempfile::tempdir().unwrap();
        let fixture = rcu_core::fixture::Fixture::default();
        fixture.0.lock().unwrap().disconnect = disconnect;
        let guard = TestGuard::new(Stage::Model);
        let mut engine = rcu_core::Engine::new(state.path().into()).unwrap();
        engine
            .register(
                "fixture".into(),
                Box::new(GuardedBackend::new(fixture.clone(), guard)),
            )
            .unwrap();
        let frame = engine.observe("fixture").unwrap().frame;
        let lease = engine.claim("fixture", 60_000).unwrap();
        let receipt = engine
            .act_checked(rcu_core::CheckedAction {
                target: "fixture",
                frame: &frame,
                element: "Input",
                lease: &lease,
                request: "worker-once",
                intent: Intent::Text,
                value: Some("worker original model".into()),
            })
            .unwrap();
        assert_eq!(
            receipt.dispatch,
            if disconnect { "uncertain" } else { "returned" }
        );
        assert!(!receipt.goal_verified);
        if disconnect {
            assert!(engine.observe("fixture").is_err());
            assert!(
                engine
                    .act_checked(rcu_core::CheckedAction {
                        target: "fixture",
                        frame: &frame,
                        element: "Input",
                        lease: &lease,
                        request: "new-id",
                        intent: Intent::Text,
                        value: Some("must not happen".into())
                    })
                    .is_err()
            );
        } else {
            let result = engine
                .verify(rcu_core::Verification {
                    target: "fixture",
                    frame: &frame,
                    element: Some("Input".into()),
                    request: Some("worker-once".into()),
                    predicate: Predicate::ElementValue,
                    expected: "worker original model",
                    timeout_ms: 100,
                    stable_samples: 1,
                })
                .unwrap();
            assert_eq!(result["goal_verified"], true);
        }
        engine.release(&lease).unwrap();
        let actual = fixture.0.lock().unwrap();
        assert_eq!(actual.calls, 1);
        assert_eq!(actual.value, "worker original model");
    }
}

#[test]
fn shared_guard_gate_preserves_deadline_and_wrong_stage_hold() {
    let gate = GuardGate::new(Stage::Preflight, Budget::start(spec().budget).unwrap());
    assert!(gate.check(GuardOperation::Read, || Ok(())).is_ok());
    assert!(
        gate.check(GuardOperation::Launch, || panic!(
            "wrong stage must precede native code"
        ))
        .is_err()
    );
    assert!(
        gate.check(GuardOperation::Read, || panic!("HOLD must not resume"))
            .is_err()
    );
    let mut budget = spec().budget;
    budget.maximum_elapsed_ms = 1;
    let gate = GuardGate::new(Stage::Model, Budget::start(budget).unwrap());
    std::thread::sleep(Duration::from_millis(3));
    assert!(
        gate.check(GuardOperation::Input, || panic!(
            "expired budget must precede native code"
        ))
        .is_err()
    );
}

#[test]
fn model_ready_requires_exact_absolute_full_window_and_cannot_renew_late() {
    let now = now_ms();
    let mut guard = spec();
    guard.budget.issued_unix_ms = now - 90_000;
    guard.budget.expires_unix_ms = guard.budget.issued_unix_ms + MODEL_WORKER_MS;
    guard.budget.maximum_elapsed_ms = MODEL_WORKER_MS;
    let hash = "a".repeat(64);
    let ready = ModelReadySpec {
        version: 1,
        task: guard.task.clone(),
        stage: Stage::Model,
        guard_sha256: hash.clone(),
        budget: rcu_native_link::budget::BudgetSpec {
            issued_unix_ms: now - 50_000,
            expires_unix_ms: now - 50_000 + MODEL_USABLE_MS,
            maximum_elapsed_ms: MODEL_USABLE_MS,
            clock_drift_tolerance_ms: 100,
        },
    };
    validate_model_ready(&guard, &hash, &ready, now).unwrap();
    let budget = Budget::start(ready.budget.clone()).unwrap();
    assert!(budget.remaining() <= Duration::from_millis(MODEL_USABLE_MS - 50_000));
    assert!(validate_model_ready(&guard, &hash, &ready, ready.budget.expires_unix_ms).is_err());
    let mut bad = vec![ready.clone(); 8];
    bad[0].task = "other-task".into();
    bad[1].guard_sha256 = "b".repeat(64);
    bad[2].stage = Stage::Activate;
    bad[3].budget.maximum_elapsed_ms -= 1;
    bad[4].budget.expires_unix_ms += 1;
    bad[5].budget.issued_unix_ms = guard.budget.issued_unix_ms - 1;
    bad[5].budget.expires_unix_ms = bad[5].budget.issued_unix_ms + MODEL_USABLE_MS;
    bad[6].budget.clock_drift_tolerance_ms += 1;
    bad[7].budget.issued_unix_ms = now + 50_000;
    bad[7].budget.expires_unix_ms = bad[7].budget.issued_unix_ms + MODEL_USABLE_MS;
    for (i, value) in bad.iter().enumerate() {
        assert!(
            validate_model_ready(&guard, &hash, value, now).is_err(),
            "negative {i}"
        );
    }
    let mut missing = guard.clone();
    missing.model_ready_path = None;
    assert!(validate_spec(&missing, r"C:\private\guard.json").is_err());
    missing.model_ready_path = Some(format!(r"C:\other\ready-{}.json", "4".repeat(64)).into());
    assert!(validate_spec(&missing, r"C:\private\guard.json").is_err());
}

#[test]
fn selected_scope_unknown_holds_before_backend_dispatch_and_never_recovers() {
    fn unknown(
        _: &mut rcu_core::fixture::Fixture,
        _: &Binding,
        _: &Resolved,
    ) -> Result<rcu_windows::agent_contract::SelectedInputProof> {
        Err(fault(
            "simulated_selected_scope_unknown",
            "Pure negative callback, no native observation.",
        ))
    }
    let state = tempfile::tempdir().unwrap();
    let fixture = rcu_core::fixture::Fixture::default();
    let guard = TestGuard::new(Stage::Model);
    let mut engine = rcu_core::Engine::new(state.path().into()).unwrap();
    engine
        .register(
            "fixture".into(),
            Box::new(GuardedBackend::new(fixture.clone(), guard.clone()).with_input_proof(unknown)),
        )
        .unwrap();
    let frame = engine.observe("fixture").unwrap().frame;
    let lease = engine.claim("fixture", 60_000).unwrap();
    let receipt = engine
        .act_checked(rcu_core::CheckedAction {
            target: "fixture",
            frame: &frame,
            element: "Input",
            lease: &lease,
            request: "scope-never-dispatched",
            intent: Intent::Text,
            value: Some("never".into()),
        })
        .unwrap();
    assert!(!receipt.goal_verified);
    assert_eq!(fixture.0.lock().unwrap().calls, 0);
    assert!(guard.gate.held.load(Ordering::Acquire));
    assert!(engine.observe("fixture").is_err());
    engine.release(&lease).unwrap();
}

struct CleanupDriver(Arc<std::sync::atomic::AtomicUsize>);
impl LifecycleDriver for CleanupDriver {
    fn identity(&mut self) -> Result<Identity> {
        Ok(spec().owner.session.native)
    }
    fn verify_application(&mut self, _: &ApplicationIdentity) -> Result<()> {
        Ok(())
    }
    fn verify_file(&mut self, _: &AllowedFile) -> Result<()> {
        Ok(())
    }
    fn inventory(&mut self, _: &[ApplicationIdentity]) -> Result<Inventory> {
        Err(fault("not_tested", "No inventory simulated."))
    }
    fn begin_events(&mut self) -> Result<()> {
        Ok(())
    }
    fn events(&mut self) -> Result<EventBatch> {
        Ok(EventBatch::default())
    }
    fn end_events(&mut self) -> Result<EventBatch> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(EventBatch {
            events: vec![],
            complete: true,
            hooks_released: true,
        })
    }
    fn launch(&mut self, _: &LaunchPermission) -> Result<NativeLaunch> {
        panic!("held launch must not delegate")
    }
    fn foreground(&mut self) -> Result<Option<WindowIdentity>> {
        Ok(None)
    }
    fn user_input_tick(&mut self) -> Result<u64> {
        Ok(0)
    }
    fn check_window(&mut self, _: &WindowIdentity) -> Result<()> {
        Ok(())
    }
    fn activate(&mut self, _: &WindowIdentity) -> Result<bool> {
        panic!("held activation must not delegate")
    }
    fn pause(&mut self, _: u64) {
        panic!("held pause must not delegate")
    }
}
#[test]
fn lifecycle_cleanup_is_delegated_after_hold_without_authorizing_input() {
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let guard = TestGuard::new(Stage::Launch);
    let mut driver = GuardedLifecycleDriver {
        inner: Box::new(CleanupDriver(count.clone())),
        guard: guard.clone(),
    };
    driver.begin_events().unwrap();
    guard.hold();
    assert!(driver.events().is_err());
    assert!(driver.identity().is_err());
    driver.pause(50);
    assert!(driver.end_events().unwrap().hooks_released);
    assert_eq!(count.load(Ordering::SeqCst), 1);
}
