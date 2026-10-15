use super::*;
use rwi_core::semantic_focus::*;
use rwi_core::window_local::*;
use rwi_core::{
    Backend, Binding, Identity, Intent, Oracle, Predicate, Resolved, Result as CoreResult, Scene,
    Scope,
};
use std::sync::atomic::AtomicU64;
#[derive(Clone, Copy)]
enum Outcome {
    Selected,
    Partial,
    AckUnknown,
    ExactNoHidError,
    CounterUnknown,
    AlreadySelected,
}
struct WindowBackend {
    fixture: rwi_core::fixture::Fixture,
    setters: Arc<AtomicU64>,
    outcome: Outcome,
}
impl Backend for WindowBackend {
    fn identity(&self) -> Identity {
        self.fixture.identity()
    }
    fn capabilities(&self) -> Value {
        json!({"adapter":"offline-agent-window-local-pipeline","select_window_local":{"implemented":true,"available":true}})
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        if matches!(self.outcome, Outcome::CounterUnknown) {
            None
        } else {
            Some(self.setters.load(Ordering::SeqCst))
        }
    }
    fn observe(&mut self) -> CoreResult<Scene> {
        let mut scene = self.fixture.observe()?;
        for element in &mut scene.elements {
            element.enabled = Some(true);
            if element.id == "Input" {
                element.role = Some("AXTextField".into());
            }
        }
        Ok(scene)
    }
    fn select_window_local_preflight(
        &mut self,
        _: &Resolved,
        kind: WindowLocalKind,
    ) -> CoreResult<WindowLocalNativeFacts> {
        let yes = FocusBooleanRead {
            native_status: 0,
            value: Some(true),
        };
        let selected = FocusBooleanRead {
            native_status: 0,
            value: Some(matches!(self.outcome, Outcome::AlreadySelected)),
        };
        Ok(WindowLocalNativeFacts {
            kind,
            platform: FocusPlatform::Macos,
            application_focus_custody: ApplicationFocusCustody::NoActiveForeignSibling,
            input_principal: ExactEvidence::Verified,
            accessibility_permission: ExactEvidence::Verified,
            retained_window: ExactEvidence::Verified,
            provider_lifetime: ExactEvidence::Verified,
            process_lifetime: ExactEvidence::Verified,
            cg_window_binding: ExactEvidence::Verified,
            native_context: ExactEvidence::Verified,
            probe_origin: ProbeOrigin::FreshExactObject,
            main_writable: yes,
            focused_writable: yes,
            window_main: selected,
            application_main_window_cf_match: selected,
            window_focused: selected,
            application_focused_window_cf_match: selected,
        })
    }
    fn select_window_local(
        &mut self,
        _: &Binding,
        _: &Resolved,
        kind: WindowLocalKind,
        plan: WindowLocalWritePlan,
    ) -> CoreResult<WindowLocalReadback> {
        let attempted = plan == WindowLocalWritePlan::SetAttributeTrueOnce;
        let before = self.setters.load(Ordering::SeqCst);
        if attempted {
            self.setters.fetch_add(1, Ordering::SeqCst);
        }
        if matches!(
            self.outcome,
            Outcome::ExactNoHidError | Outcome::CounterUnknown
        ) {
            let known = !matches!(self.outcome, Outcome::CounterUnknown);
            let mut error = rwi_core::fault(
                "window_local_selection_unconfirmed",
                "Synthetic window setter acknowledgement failed; no replay.",
            );
            error.diagnostic = Some(Box::new(
                json!({"version":1,"operation_kind":"window_local_selection","kind":kind,"native_window_role":"AXWindow","api":"AXUIElementSetAttributeValue","attribute":kind.attribute(),"native_setter_attempted":attempted,"native_setter_attempts":1,"native_status":-25202,"native_dispatch_counter_known":known,"native_dispatch_count_before":if known{Some(before)}else{None},"native_dispatch_count_after":if known{Some(self.setters.load(Ordering::SeqCst))}else{None},"native_call_pending":false,"native_resources_released":true,"held_input_resources":false,"replay_allowed":false}),
            ));
            return Err(error);
        }
        let yes = FocusBooleanRead {
            native_status: 0,
            value: Some(true),
        };
        let no = FocusBooleanRead {
            native_status: 0,
            value: Some(false),
        };
        let app = if matches!(self.outcome, Outcome::Partial) {
            no
        } else {
            yes
        };
        Ok(WindowLocalReadback {
            kind,
            write_plan: plan,
            setter_attempt: if attempted {
                FocusSetterAttempt::Attempted
            } else {
                FocusSetterAttempt::NotAttempted
            },
            setter_status: if attempted && !matches!(self.outcome, Outcome::AckUnknown) {
                Some(0)
            } else {
                None
            },
            window_main: if kind == WindowLocalKind::Main {
                yes
            } else {
                no
            },
            application_main_window_cf_match: if kind == WindowLocalKind::Main {
                app
            } else {
                no
            },
            window_focused: if kind == WindowLocalKind::Key {
                yes
            } else {
                no
            },
            application_focused_window_cf_match: if kind == WindowLocalKind::Key {
                app
            } else {
                no
            },
            os_foreground_before: None,
            os_foreground_after: None,
            public_application_custody: None,
        })
    }
    fn act(
        &mut self,
        b: &Binding,
        r: &Resolved,
        i: &Intent,
        v: Option<&str>,
    ) -> CoreResult<Option<bool>> {
        self.fixture.act(b, r, i, v)
    }
    fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> CoreResult<Oracle> {
        self.fixture.inspect(s, p, e)
    }
}
#[tokio::test]
async fn window_local_actual_agent_pipeline_preserves_scope_fence_and_no_replay() {
    for kind in [WindowLocalKind::Main, WindowLocalKind::Key] {
        for (outcome, expected_fence, expected_goal, expected_writes) in [
            (Outcome::Selected, false, true, 1),
            (Outcome::Partial, false, false, 1),
            (Outcome::AckUnknown, true, false, 1),
            (Outcome::ExactNoHidError, false, false, 1),
            (Outcome::CounterUnknown, true, false, 1),
            (Outcome::AlreadySelected, false, true, 0),
        ] {
            let temp = tempfile::tempdir().unwrap();
            let p = Principal {
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
                principals: vec![p.clone()],
                state_root: temp.path().join("state"),
                platform: crate::config::PlatformPolicy::Fixture,
            })
            .unwrap();
            let setters = Arc::new(AtomicU64::new(0));
            let (token, target) = tracked_backend_context(
                &runtime,
                &p,
                Box::new(WindowBackend {
                    fixture: rwi_core::fixture::Fixture::default(),
                    setters: setters.clone(),
                    outcome,
                }),
            )
            .await;
            let observed = invoke(
                &runtime,
                &p,
                &token,
                "computer.observe",
                json!({"target":target,"inline_image":false}),
            )
            .await;
            let lease = invoke(
                &runtime,
                &p,
                &token,
                "computer.claim",
                json!({"target":target,"ttl_ms":60000}),
            )
            .await;
            let arguments = json!({"target":target,"frame":observed.data["frame"],"kind":kind,"request_id":"window-local-agent-once","lease":lease.data["lease"]});
            let result = invoke(
                &runtime,
                &p,
                &token,
                "computer.select_window_local",
                arguments,
            )
            .await;
            assert_eq!(
                result.data["input_domain_fenced"], expected_fence,
                "{}",
                result.data
            );
            assert_eq!(
                result.data["receipt"]["goal_verified"], expected_goal,
                "{}",
                result.data
            );
            assert_eq!(result.data["receipt"]["replay"], "forbidden");
            if let Some(readback) = result.data.get("window_readback").filter(|r| !r.is_null()) {
                assert_eq!(readback["typing_authorized"], false);
                assert_eq!(readback["semantic_element_ready"], Value::Null);
                assert_eq!(readback["kind"], serde_json::to_value(kind).unwrap());
            }
            assert!(result.data.get("focus_readback").is_none());
            assert_eq!(result.data["operation"]["terminal"], true);
            assert_eq!(result.data["operation"]["resources_released"], true);
            assert_eq!(setters.load(Ordering::SeqCst), expected_writes);
            let status = wire(
                &runtime,
                &p,
                None,
                Call::Operation {
                    selector: OperationSelector::Request {
                        device_id: "device".into(),
                        request_id: "window-local-agent-once".into(),
                    },
                },
            )
            .await;
            assert_eq!(status.data["replay_allowed"], false);
            let reference = serde_json::from_value(result.data["operation_ref"].clone()).unwrap();
            let duplicate = wire(
                &runtime,
                &p,
                None,
                Call::Execute {
                    operation_ref: reference,
                },
            )
            .await;
            assert_eq!(duplicate.data["code"], "execution_already_consumed");
            assert_eq!(
                setters.load(Ordering::SeqCst),
                expected_writes,
                "query/duplicate Execute must not replay"
            );
            let release = invoke(
                &runtime,
                &p,
                &token,
                "computer.release",
                json!({"lease":lease.data["lease"]}),
            )
            .await;
            assert!(
                !release.is_error,
                "cleanup must remain possible under input fence"
            );
            println!(
                "AGENT_WINDOW_LOCAL_PIPELINE {}",
                json!({"result":result.data,"durable_status":status.data,"duplicate":duplicate.data,"setter_count":setters.load(Ordering::SeqCst),"release":release.data,"native_os_effect":false})
            );
        }
    }
}
