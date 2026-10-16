//! Bounded, already-owned Live control, independent of long native input.
//! Prepare/Execute identities, principal/target validation and cancellation are
//! unchanged. This lane never owns input, creates a preview or resets its writer's
//! dispatch evidence. A missing controller is an error, not a fallback/replay.
use super::{
    dispatch::{Finish, fault_result},
    execution::Execution,
    *,
};
use rwi_core::{LiveAction, device::api::LiveArgs};

impl Runtime {
    pub(super) async fn dispatch_live(
        &self,
        context: Arc<Context>,
        entry: Arc<Execution>,
        finish: Finish,
    ) -> DeviceResult {
        let args: LiveArgs = match serde_json::from_value(entry.arguments.clone()) {
            Ok(args) => args,
            Err(_) => return DeviceResult::error("invalid_arguments", "Use the Live schema."),
        };
        if entry.writes || args.action == LiveAction::Start || args.options.is_some() {
            return DeviceResult::error(
                "invalid_live_options",
                "This lane cannot create or reconfigure a preview.",
            );
        }
        if args.view.is_some() && args.action != LiveAction::Status {
            return DeviceResult::error(
                "invalid_live_view",
                "Visibility updates cannot also control a session.",
            );
        }
        let Ok(permit) = context.live_gate.clone().try_acquire_owned() else {
            return DeviceResult::error(
                "live_control_busy",
                "Only bounded preview control is busy; no input was dispatched.",
            );
        };
        if context.closed.load(Ordering::Acquire) {
            return DeviceResult::error(
                "context_closed",
                "No preview control after context closure.",
            );
        }
        // Exact entries were published under the work lane after start/binding
        // changes. Clone before native readback; never hold this mutex in a callback.
        let controller = match context.live_controls.lock() {
            Ok(controls) => controls.get(&args.target).cloned(),
            Err(_) => {
                return DeviceResult::error(
                    "live_state_unavailable",
                    "Owned preview registry is unavailable.",
                );
            }
        };
        let controller = match controller {
            Some(Ok(controller)) => controller,
            Some(Err(error)) => return fault_result(error),
            None => {
                return DeviceResult::error(
                    "unknown_target",
                    "The preview target was retired; do not reuse it.",
                );
            }
        };
        match tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let operation = finish.0.clone();
            let result = rwi_core::operation::run(&operation, || {
                rwi_core::operation::checkpoint("live_control_dispatch")?;
                if context.closed.load(Ordering::Acquire) {
                    return Err(rwi_core::fault(
                        "context_closed",
                        "No preview control after context closure.",
                    ));
                }
                if let Some(update) = args.view {
                    update.validate()?;
                    controller.view_update(update)
                } else {
                    controller.control(args.action)
                }
            });
            let mut result = match result {
                Ok(value) => DeviceResult::value(value),
                Err(error) => fault_result(error),
            };
            drop(finish);
            result.data["result_kind"] = json!("live_control");
            result.data["native_dispatch_entered"] = json!(false);
            result.data["input_domain_fenced"] =
                json!(context.domain.fenced.load(Ordering::Acquire));
            result
        })
        .await
        {
            Ok(result) => result,
            Err(_) => DeviceResult::error(
                "live_control_worker_failed",
                "Query the original preview operation; no input was sent.",
            ),
        }
    }
}

#[cfg(all(test, feature = "fixture"))]
mod tests {
    #![allow(clippy::await_holding_lock)] // Intentionally hold the input executor to detect coupling.
    use super::*;
    use crate::runtime::dispatch_tests::{invoke, tracked_backend_context, wire};
    use rwi_core::{Backend, Binding, Identity, Intent, Oracle, Predicate, Resolved, Scene, Scope};

    #[derive(Default)]
    struct PreviewState {
        sequence: u64,
        visible: bool,
        stopped: bool,
        calls: u64,
    }
    #[derive(Default)]
    struct Preview(Mutex<PreviewState>);
    impl rwi_core::LiveController for Preview {
        fn control(&self, action: LiveAction) -> rwi_core::Result<serde_json::Value> {
            let mut s = self.0.lock().unwrap();
            s.calls += 1;
            match action {
                LiveAction::Start => {
                    return Err(rwi_core::fault("invalid_start", "Not a constructor."));
                }
                LiveAction::Stop => {
                    s.stopped = true;
                    s.visible = false;
                }
                LiveAction::Hide | LiveAction::Pause => s.visible = false,
                LiveAction::Resume | LiveAction::Status => {}
            }
            Ok(
                json!({"stopped":s.stopped,"visible":s.visible,"sequence":s.sequence,"input_sent":false}),
            )
        }
        fn view_update(
            &self,
            update: rwi_core::LiveViewUpdate,
        ) -> rwi_core::Result<serde_json::Value> {
            update.validate()?;
            if update.view_id != "a".repeat(32) {
                return Err(rwi_core::fault("live_view_mismatch", "Not this preview."));
            }
            let mut s = self.0.lock().unwrap();
            s.calls += 1;
            let accepted = !s.stopped && update.sequence > s.sequence;
            if accepted {
                s.sequence = update.sequence;
                s.visible = update.visible;
            }
            Ok(json!({"accepted":accepted,"visible":s.visible,"input_sent":false}))
        }
    }
    struct PreviewBackend {
        fixture: rwi_core::fixture::Fixture,
        preview: Arc<Preview>,
    }
    impl Backend for PreviewBackend {
        fn identity(&self) -> Identity {
            self.fixture.identity()
        }
        fn capabilities(&self) -> serde_json::Value {
            self.fixture.capabilities()
        }
        fn observe(&mut self) -> rwi_core::Result<Scene> {
            self.fixture.observe()
        }
        fn act(
            &mut self,
            b: &Binding,
            r: &Resolved,
            i: &Intent,
            v: Option<&str>,
        ) -> rwi_core::Result<Option<bool>> {
            self.fixture.act(b, r, i, v)
        }
        fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> rwi_core::Result<Oracle> {
            self.fixture.inspect(s, p, e)
        }
        fn live_controller(&self) -> rwi_core::Result<Arc<dyn rwi_core::LiveController>> {
            Ok(self.preview.clone())
        }
    }
    fn setup() -> (tempfile::TempDir, Arc<Runtime>, Principal, Arc<Preview>) {
        let temp = tempfile::tempdir().unwrap();
        let principal = Principal {
            journal_identity: None,
            certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
            observe: true,
            input: true,
        };
        let runtime = Runtime::new(Config {
            version: 1,
            device_id: "device".into(),
            bind: "127.0.0.1:0".parse().unwrap(),
            identity: rwi_link_trust::IdentityPaths::in_profile(temp.path()),
            principals: vec![principal.clone()],
            state_root: temp.path().join("state"),
            platform: crate::config::PlatformPolicy::Fixture,
        })
        .unwrap();
        (temp, runtime, principal, Arc::new(Preview::default()))
    }
    async fn binding(
        runtime: &Arc<Runtime>,
        principal: &Principal,
        preview: Arc<Preview>,
    ) -> (String, String) {
        tracked_backend_context(
            runtime,
            principal,
            Box::new(PreviewBackend {
                fixture: rwi_core::fixture::Fixture::default(),
                preview,
            }),
        )
        .await
    }
    fn view(target: &str, sequence: u64, visible: bool) -> serde_json::Value {
        json!({"target":target,"action":"status","view":{"view_id":"a".repeat(32),"sequence":sequence,"visible":visible}})
    }

    #[tokio::test]
    async fn live_presence_hide_stop_continue_during_owned_native_work_without_touching_input() {
        let (_temp, runtime, principal, preview) = setup();
        let (token, target) = binding(&runtime, &principal, preview.clone()).await;
        let context = runtime.context(principal.identity(), Some(&token)).unwrap();
        let accepted = runtime.prepare(
            principal.identity(),
            &principal,
            Some(&token),
            "computer.observe".into(),
            json!({"target":target}),
        );
        let reference = serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
        let (_, entry) = runtime
            .resolve_execution(principal.identity(), &reference)
            .unwrap();
        let operation = entry.begin(&context.registry).unwrap();
        let scope = rwi_core::operation::run(&operation, || {
            rwi_core::operation::native_call("offline_held_input_fixture").unwrap()
        });
        context.entered.store(true, Ordering::Release);
        let native_gate = context.gate.clone().try_acquire_owned().unwrap();
        let input_gate = context.domain.gate.clone().try_acquire_owned().unwrap();
        let executor = context.executor.lock().unwrap();
        let started = Instant::now();
        for sequence in 1..=4 {
            let result = invoke(
                &runtime,
                &principal,
                &token,
                "computer.live",
                view(&target, sequence, true),
            )
            .await;
            assert!(!result.is_error, "{}", result.data);
            assert_eq!(result.data["accepted"], true);
            assert_eq!(result.data["result_kind"], "live_control");
            assert_eq!(result.data["native_dispatch_entered"], false);
            assert_eq!(result.data["operation"]["resources_released"], true);
            assert_eq!(context.domain.gate.available_permits(), 0);
            assert!(
                context.entered.load(Ordering::Acquire),
                "Live must not clear the active writer's evidence"
            );
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        let ordinary = invoke(
            &runtime,
            &principal,
            &token,
            "computer.observe",
            json!({"target":target}),
        )
        .await;
        assert_eq!(ordinary.data["code"], "context_busy");
        let create = invoke(
            &runtime,
            &principal,
            &token,
            "computer.live",
            json!({"target":target,"action":"start","options":{"display":{"id":1}}}),
        )
        .await;
        assert_eq!(
            create.data["code"], "context_busy",
            "Creation must remain serialized"
        );
        let hidden = invoke(
            &runtime,
            &principal,
            &token,
            "computer.live",
            view(&target, 5, false),
        )
        .await;
        assert_eq!(hidden.data["visible"], false);
        let delayed = invoke(
            &runtime,
            &principal,
            &token,
            "computer.live",
            view(&target, 4, true),
        )
        .await;
        assert_eq!(delayed.data["accepted"], false);
        assert_eq!(delayed.data["visible"], false);
        let status = invoke(
            &runtime,
            &principal,
            &token,
            "computer.live",
            json!({"target":target,"action":"status"}),
        )
        .await;
        assert_eq!(status.data["sequence"], 5);
        let stopped = invoke(
            &runtime,
            &principal,
            &token,
            "computer.live",
            json!({"target":target,"action":"stop"}),
        )
        .await;
        assert_eq!(stopped.data["stopped"], true);
        assert_eq!(
            invoke(
                &runtime,
                &principal,
                &token,
                "computer.live",
                view(&target, 6, true)
            )
            .await
            .data["accepted"],
            false
        );
        assert_eq!(entry.status().data["terminal"], false);
        assert_eq!(
            wire(&runtime, &principal, Some(&token), Call::CloseContext)
                .await
                .data["code"],
            "context_busy"
        );
        assert!(!context.domain.fenced.load(Ordering::Acquire));
        drop(executor);
        drop(input_gate);
        drop(native_gate);
        drop(scope);
        operation.finish();
        assert!(entry.status().data["terminal"].as_bool().unwrap());
        assert!(
            !wire(&runtime, &principal, Some(&token), Call::CloseContext)
                .await
                .is_error
        );
        assert_eq!(preview.0.lock().unwrap().sequence, 5);
    }

    #[tokio::test]
    async fn live_lane_preserves_exact_target_principal_validation_and_pre_dispatch_cancel() {
        let (_temp, runtime, principal, preview) = setup();
        let (token, target) = binding(&runtime, &principal, preview.clone()).await;
        let (other_token, other_target) =
            binding(&runtime, &principal, Arc::new(Preview::default())).await;
        let foreign = wire(
            &runtime,
            &principal,
            Some(&other_token),
            Call::Prepare {
                tool: "computer.live".into(),
                arguments: view(&target, 1, true),
            },
        )
        .await;
        assert_eq!(foreign.data["code"], "foreign_target");
        assert_eq!(preview.0.lock().unwrap().calls, 0);
        let denied = Principal {
            observe: false,
            input: false,
            ..principal.clone()
        };
        assert_eq!(
            runtime
                .prepare(
                    principal.identity(),
                    &denied,
                    Some(&token),
                    "computer.live".into(),
                    view(&target, 1, true)
                )
                .data["code"],
            "permission_denied"
        );
        let accepted = runtime.prepare(
            principal.identity(),
            &principal,
            Some(&token),
            "computer.live".into(),
            view(&target, 1, true),
        );
        let reference = serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
        runtime.cancel(principal.identity(), &reference);
        assert_eq!(
            runtime
                .execute(principal.identity(), &principal, &reference)
                .await
                .data["code"],
            "execution_cancelled_before_dispatch"
        );
        assert_eq!(preview.0.lock().unwrap().calls, 0);
        for args in [
            json!({"target":target,"action":"stop","view":{"view_id":"a".repeat(32),"sequence":1,"visible":false}}),
            json!({"target":target,"action":"status","options":{"display":{"id":1}}}),
        ] {
            let result = invoke(&runtime, &principal, &token, "computer.live", args).await;
            assert!(result.is_error);
            assert_eq!(preview.0.lock().unwrap().calls, 0);
        }
        let wrong = json!({"target":target,"action":"status","view":{"view_id":"b".repeat(32),"sequence":1,"visible":true}});
        assert_eq!(
            invoke(&runtime, &principal, &token, "computer.live", wrong)
                .await
                .data["code"],
            "live_view_mismatch"
        );
        let other = invoke(
            &runtime,
            &principal,
            &other_token,
            "computer.live",
            view(&other_target, 1, true),
        )
        .await;
        assert_eq!(other.data["accepted"], true);
        assert_eq!(preview.0.lock().unwrap().calls, 0);
    }

    #[tokio::test]
    async fn live_lane_is_bounded_and_context_close_cannot_race_its_owner() {
        let (_temp, runtime, principal, preview) = setup();
        let (token, target) = binding(&runtime, &principal, preview.clone()).await;
        let context = runtime.context(principal.identity(), Some(&token)).unwrap();
        let permit = context.live_gate.clone().try_acquire_owned().unwrap();
        let result = invoke(
            &runtime,
            &principal,
            &token,
            "computer.live",
            view(&target, 1, true),
        )
        .await;
        assert_eq!(result.data["code"], "live_control_busy");
        assert_eq!(preview.0.lock().unwrap().calls, 0);
        let ordinary = invoke(
            &runtime,
            &principal,
            &token,
            "computer.observe",
            json!({"target":target}),
        )
        .await;
        assert!(!ordinary.is_error, "{}", ordinary.data);
        assert_eq!(
            wire(&runtime, &principal, Some(&token), Call::CloseContext)
                .await
                .data["code"],
            "context_busy"
        );
        let accepted = runtime.prepare(
            principal.identity(),
            &principal,
            Some(&token),
            "computer.live".into(),
            view(&target, 2, true),
        );
        let reference = serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
        drop(permit);
        assert!(
            !wire(&runtime, &principal, Some(&token), Call::CloseContext)
                .await
                .is_error
        );
        assert_eq!(
            runtime
                .execute(principal.identity(), &principal, &reference)
                .await
                .data["code"],
            "execution_ref_expired"
        );
        assert_eq!(preview.0.lock().unwrap().calls, 0);
    }
}
