use super::*;
use rwi_core::semantic_focus::*;
use rwi_core::{
    Backend, Binding, Identity, Intent, Oracle, Predicate, Resolved, Result as CoreResult, Scene,
    Scope,
};
use std::sync::atomic::AtomicU64;
#[derive(Clone, Copy)]
enum Outcome {
    Ready,
    AckUnknown,
    ReadbackUnknown,
    ExactNoHidError,
    CounterUnknown,
    AlreadyFocused,
}
struct FocusBackend {
    fixture: rwi_core::fixture::Fixture,
    setters: Arc<AtomicU64>,
    outcome: Outcome,
}
impl Backend for FocusBackend {
    fn identity(&self) -> Identity {
        self.fixture.identity()
    }
    fn capabilities(&self) -> Value {
        json!({"adapter":"offline-agent-focus-pipeline","focus_element":{"implemented":true,"available":true}})
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
    fn focus_element_preflight(&mut self, _: &Resolved) -> CoreResult<FocusNativeFacts> {
        let yes = FocusBooleanRead {
            native_status: 0,
            value: Some(true),
        };
        Ok(FocusNativeFacts {
            platform: FocusPlatform::Macos,
            application_focus_custody: ApplicationFocusCustody::NoActiveForeignSibling,
            input_principal: ExactEvidence::Verified,
            accessibility_permission: ExactEvidence::Verified,
            retained_element: ExactEvidence::Verified,
            element_window_cf: ExactEvidence::Verified,
            process_lifetime: ExactEvidence::Verified,
            cg_window_binding: ExactEvidence::Verified,
            native_context: ExactEvidence::Verified,
            editable_class: EditableClass::PlainEditable,
            probe_origin: ProbeOrigin::FreshExactObject,
            value_writable: yes,
            focused_writable: yes,
            focused: FocusBooleanRead {
                native_status: 0,
                value: Some(matches!(self.outcome, Outcome::AlreadyFocused)),
            },
        })
    }
    fn focus_element(
        &mut self,
        _: &Binding,
        _: &Resolved,
        plan: FocusWritePlan,
    ) -> CoreResult<FocusReadback> {
        let attempted = plan == FocusWritePlan::SetAXFocusedTrueOnce;
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
                "semantic_focus_unconfirmed",
                "Synthetic setter acknowledgement failed; no replay.",
            );
            error.diagnostic = Some(Box::new(
                json!({"version":1,"operation_kind":"semantic_focus","api":"AXUIElementSetAttributeValue","attribute":"AXFocused","native_setter_attempted":attempted,"native_setter_attempts":1,"native_status":-25202,"native_dispatch_counter_known":known,"native_dispatch_count_before":if known{Some(before)}else{None},"native_dispatch_count_after":if known{Some(self.setters.load(Ordering::SeqCst))}else{None},"native_call_pending":false,"native_resources_released":true,"held_input_resources":false,"replay_allowed":false}),
            ));
            return Err(error);
        }
        let yes = FocusBooleanRead {
            native_status: 0,
            value: Some(true),
        };
        Ok(FocusReadback {
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
            element_focused: if matches!(self.outcome, Outcome::ReadbackUnknown) {
                FocusBooleanRead {
                    native_status: 0,
                    value: None,
                }
            } else {
                yes
            },
            focused_ui_element_cf_match: yes,
            application_focused_window_cf_match: yes,
            os_foreground_before: None,
            os_foreground_after: None,
            readback_diagnostics: matches!(
                self.outcome,
                Outcome::ReadbackUnknown | Outcome::AckUnknown
            )
            .then(|| {
                let probe = FocusReadbackProbe::known(yes);
                let first = if matches!(self.outcome, Outcome::ReadbackUnknown) {
                    FocusReadbackProbe::known(FocusBooleanRead {
                        native_status: 0,
                        value: Some(false),
                    })
                } else {
                    probe.clone()
                };
                FocusReadbackDiagnostics::new(
                    FocusReadbackAxisPair::new(first, probe.clone()),
                    FocusReadbackAxisPair::new(probe.clone(), probe.clone()),
                    FocusReadbackAxisPair::new(probe.clone(), probe),
                )
            }),
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
async fn semantic_focus_actual_agent_pipeline_preserves_uncertainty_fence_and_no_replay() {
    for (outcome, expected_fence, expected_goal, expected_writes) in [
        (Outcome::Ready, false, true, 1),
        (Outcome::AckUnknown, true, false, 1),
        (Outcome::ReadbackUnknown, false, false, 1),
        (Outcome::ExactNoHidError, false, false, 1),
        (Outcome::CounterUnknown, true, false, 1),
        (Outcome::AlreadyFocused, false, true, 0),
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
            Box::new(FocusBackend {
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
        let arguments = json!({"target":target,"frame":observed.data["frame"],"element":"Input","request_id":"semantic-focus-agent-once","lease":lease.data["lease"]});
        let result = invoke(&runtime, &p, &token, "computer.focus_element", arguments).await;
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
        assert_eq!(result.data["operation"]["terminal"], true);
        assert_eq!(result.data["operation"]["resources_released"], true);
        assert_eq!(setters.load(Ordering::SeqCst), expected_writes);
        if matches!(outcome, Outcome::AckUnknown | Outcome::ReadbackUnknown) {
            assert_eq!(result.data["focus_readback"]["state"], "unknown");
            assert_eq!(
                result.data["focus_readback"]["readback_diagnostics"]["input_authority"],
                false
            );
        } else {
            assert!(
                result.data["focus_readback"]
                    .get("readback_diagnostics")
                    .is_none()
            );
        }
        let status = wire(
            &runtime,
            &p,
            None,
            Call::Operation {
                selector: OperationSelector::Request {
                    device_id: "device".into(),
                    request_id: "semantic-focus-agent-once".into(),
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
            "AGENT_FOCUS_PIPELINE {}",
            json!({"result":result.data,"durable_status":status.data,"duplicate":duplicate.data,"setter_count":setters.load(Ordering::SeqCst),"release":release.data,"native_os_effect":false})
        );
    }
}
