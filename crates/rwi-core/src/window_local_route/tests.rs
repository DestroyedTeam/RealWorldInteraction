use crate::fixture::Fixture;
use crate::semantic_focus::*;
use crate::window_local::*;
use crate::{
    Backend, Binding, Engine, Identity, Intent, Oracle, Predicate, Resolved, Result, Scene, Scope,
    fault,
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
#[derive(Default)]
struct Counts {
    probes: AtomicU64,
    methods: AtomicU64,
    setters: AtomicU64,
    generic: AtomicU64,
    observations: AtomicU64,
}
struct WindowMock {
    app: Fixture,
    case: Value,
    count: Arc<Counts>,
}
impl Backend for WindowMock {
    fn identity(&self) -> Identity {
        self.app.identity()
    }
    fn capabilities(&self) -> Value {
        json!({"adapter":"source4-mocked-window-local-only","select_window_local":{"implemented":self.case["mode"]!="capability_missing","available":self.case["mode"]!="capability_missing"}})
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        match self.case["counter"].as_str().unwrap_or("") {
            "missing" => None,
            "saturated" => Some(u64::MAX),
            _ => Some(self.count.setters.load(Ordering::SeqCst)),
        }
    }
    fn observe(&mut self) -> Result<Scene> {
        let n = self.count.observations.fetch_add(1, Ordering::SeqCst);
        let mut s = self.app.observe()?;
        for e in &mut s.elements {
            e.enabled = Some(true);
            if e.id == "Input" {
                e.role = Some("AXTextField".into());
            }
        }
        if n > 0 {
            match self.case["change"].as_str().unwrap_or("") {
                "sibling_window" => {
                    s.binding.as_mut().unwrap().scope.window = "other-window".into()
                }
                "session" => s.identity.session = "other-session".into(),
                "modal_unknown" => s.blocked = None,
                "modal_true" => s.blocked = Some(true),
                "layout" => s.binding.as_mut().unwrap().layout = "other-layout".into(),
                "native_epoch" => s.binding.as_mut().unwrap().native_epoch = "b".repeat(64),
                "mutation_context" => s.binding.as_mut().unwrap().mutation_context = None,
                "content_only" => s.binding.as_mut().unwrap().content = "new-pixels".into(),
                "element_disabled" => {
                    s.elements
                        .iter_mut()
                        .find(|e| e.id == "Input")
                        .unwrap()
                        .enabled = Some(false)
                }
                "element_hidden" => {
                    s.elements
                        .iter_mut()
                        .find(|e| e.id == "Input")
                        .unwrap()
                        .visible = Some(false)
                }
                _ => {}
            }
        }
        Ok(s)
    }
    fn select_window_local_preflight(
        &mut self,
        r: &Resolved,
        kind: WindowLocalKind,
    ) -> Result<WindowLocalNativeFacts> {
        self.count.probes.fetch_add(1, Ordering::SeqCst);
        if self.case["mode"] == "default_probe" {
            return self.app.select_window_local_preflight(r, kind);
        }
        let b = |value| FocusBooleanRead {
            native_status: 0,
            value: Some(value),
        };
        let writable = if self.case["probe_error"] == true {
            FocusBooleanRead {
                native_status: -25204,
                value: Some(true),
            }
        } else {
            b(self.case["not_writable"] != true)
        };
        let selected = b(self.case["already_selected"] == true);
        Ok(WindowLocalNativeFacts {
            kind,
            platform: if self.case["platform"] == "windows" {
                FocusPlatform::Windows
            } else {
                FocusPlatform::Macos
            },
            application_focus_custody: match self.case["app_custody"].as_str() {
                Some("sibling") => ApplicationFocusCustody::ActiveForeignSibling,
                Some("unknown") => ApplicationFocusCustody::Unknown,
                _ => ApplicationFocusCustody::NoActiveForeignSibling,
            },
            input_principal: ExactEvidence::Verified,
            accessibility_permission: ExactEvidence::Verified,
            retained_window: if self.case["object_cf"] == "different" {
                ExactEvidence::Different
            } else {
                ExactEvidence::Verified
            },
            provider_lifetime: if self.case["provider_birth"] == "different" {
                ExactEvidence::Different
            } else {
                ExactEvidence::Verified
            },
            process_lifetime: ExactEvidence::Verified,
            cg_window_binding: ExactEvidence::Verified,
            native_context: ExactEvidence::Verified,
            probe_origin: if self.case["sidecar"] == true {
                ProbeOrigin::ObservationSidecar
            } else {
                ProbeOrigin::FreshExactObject
            },
            main_writable: writable,
            focused_writable: writable,
            window_main: selected,
            application_main_window_cf_match: selected,
            window_focused: selected,
            application_focused_window_cf_match: selected,
        })
    }
    fn select_window_local(
        &mut self,
        b: &Binding,
        r: &Resolved,
        kind: WindowLocalKind,
        plan: WindowLocalWritePlan,
    ) -> Result<WindowLocalReadback> {
        self.count.methods.fetch_add(1, Ordering::SeqCst);
        if self.case["mode"] == "default_setter" {
            return self.app.select_window_local(b, r, kind, plan);
        }
        if self.case["mode"] == "reject_before_setter" {
            return Err(fault(
                "mock_native_refused",
                "Mock refused before its setter counter.",
            ));
        }
        let attempted = (plan == WindowLocalWritePlan::SetAttributeTrueOnce
            || self.case["mode"] == "escalate_zero_budget")
            && self.case["mode"] != "refine_one_budget_to_no_write";
        if attempted {
            self.count.setters.fetch_add(
                if self.case["mode"] == "double_setter" {
                    2
                } else {
                    1
                },
                Ordering::SeqCst,
            );
        }
        if self.case["mode"] == "error_after_setter" {
            return Err(fault(
                "mock_setter_uncertain",
                "Mock attempted one setter then error; no replay.",
            ));
        }
        let yes = FocusBooleanRead {
            native_status: 0,
            value: Some(true),
        };
        let no = FocusBooleanRead {
            native_status: 0,
            value: Some(false),
        };
        let app = match self.case["mode"].as_str() {
            Some("partial") => no,
            Some("readback_unknown") => FocusBooleanRead {
                native_status: -25204,
                value: Some(true),
            },
            _ => yes,
        };
        let fg = |pid| crate::ForegroundState {
            pid,
            window: "mock-global".into(),
            process_start: "mock-start".into(),
            native_epoch: "c".repeat(64),
        };
        Ok(WindowLocalReadback {
            kind: if self.case["mode"] == "wrong_kind" {
                WindowLocalKind::Key
            } else {
                kind
            },
            write_plan: if attempted {
                WindowLocalWritePlan::SetAttributeTrueOnce
            } else {
                WindowLocalWritePlan::NoWriteAlreadySelected
            },
            setter_attempt: if attempted {
                FocusSetterAttempt::Attempted
            } else {
                FocusSetterAttempt::NotAttempted
            },
            setter_status: if !attempted || self.case["mode"] == "ack_unknown" {
                None
            } else {
                Some(0)
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
            os_foreground_before: Some(fg(7)),
            os_foreground_after: Some(fg(if self.case["foreground_changed"] == true {
                8
            } else {
                7
            })),
            public_application_custody: None,
        })
    }
    fn act(
        &mut self,
        _: &Binding,
        _: &Resolved,
        _: &Intent,
        _: Option<&str>,
    ) -> Result<Option<bool>> {
        self.count.generic.fetch_add(1, Ordering::SeqCst);
        Err(fault(
            "mock_wrong_route",
            "Focus must not call generic click/key/text route.",
        ))
    }
    fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> Result<Oracle> {
        self.app.inspect(s, p, e)
    }
}
pub(crate) fn run_new_window_route(case: &Value) -> Value {
    let dir = tempfile::tempdir().unwrap();
    let count = Arc::new(Counts::default());
    let app = Fixture::default();
    let mut e = Engine::new(dir.path().join("state")).unwrap();
    e.register(
        "window-fixture".into(),
        Box::new(WindowMock {
            app: app.clone(),
            case: case.clone(),
            count: count.clone(),
        }),
    )
    .unwrap();
    let observed = e.observe("window-fixture").unwrap();
    let mut req = WindowLocalRequest {
        target: "window-fixture".into(),
        frame: observed.frame,
        kind: if case["kind"] == "key" {
            WindowLocalKind::Key
        } else {
            WindowLocalKind::Main
        },
        request_id: "same-stable-window-local-request".into(),
    };
    if case["mode"] == "expired_frame" {
        e.snapshots
            .iter_mut()
            .find(|(f, _)| f == &req.frame)
            .unwrap()
            .1
            .at -= std::time::Duration::from_secs(121);
    }
    let mut lease = e.claim("window-fixture", 60000).unwrap();
    if case["mode"] == "invalid_lease" {
        lease = "not-a-real-lease".into();
    }
    if case["mode"] == "same_app_alias" {
        e.register(
            "same-app-alias".into(),
            Box::new(WindowMock {
                app,
                case: json!({}),
                count: Arc::new(Counts::default()),
            }),
        )
        .unwrap();
        e.observe("same-app-alias").unwrap();
    }
    let mut request_value = serde_json::to_value(&req).unwrap();
    if case["mode"] == "unknown_request_field" {
        request_value["value"] = json!("AXPress");
    }
    if let Some(kind) = case.get("invalid_kind") {
        request_value["kind"] = kind.clone();
    }
    if matches!(
        case["mode"].as_str(),
        Some("incomplete_journal" | "pending_journal")
    ) {
        let key = e.identity(&req.target).unwrap().key();
        let path = e.journal_path(&key, &req.request_id);
        if case["mode"] == "incomplete_journal" {
            std::fs::write(path, b"incomplete-without-newline").unwrap();
        } else {
            let fingerprint =
                crate::digest(&serde_json::to_vec(&("select_window_local_v1", &req)).unwrap());
            let receipt = crate::Receipt {
                request_id: req.request_id.clone(),
                frame: req.frame.clone(),
                scope: Scope {
                    window: "fixture-window".into(),
                    pid: 42,
                    element: None,
                },
                dispatch: "uncertain".into(),
                dispatch_error: None,
                system_accepted: None,
                text_effect: None,
                goal_verified: false,
                replay: "forbidden".into(),
                deduplicated: false,
            };
            let mut bytes =
                serde_json::to_vec(&json!({"fingerprint":fingerprint,"receipt":receipt})).unwrap();
            bytes.push(b'\n');
            std::fs::write(path, bytes).unwrap();
        }
    }
    let decoded = serde_json::from_value::<WindowLocalRequest>(request_value);
    let first = match decoded {
        Ok(v) => e.select_window_local(&v, &lease),
        Err(error) => Err(fault("strict_serde_rejection", &error.to_string())),
    };
    let mut secondary = Value::Null;
    if case["mode"] == "duplicate"
        || case["mode"] == "error_after_setter"
        || case["mode"] == "ack_unknown"
    {
        secondary = match e.select_window_local(&req, &lease) {
            Ok(v) => v,
            Err(error) => json!({"error":error.code}),
        };
    }
    if case["mode"] == "conflict" {
        req.kind = WindowLocalKind::Key;
        secondary = match e.select_window_local(&req, &lease) {
            Ok(v) => v,
            Err(error) => json!({"error":error.code}),
        };
    }
    let counts = json!({"mock_probe_calls":count.probes.load(Ordering::SeqCst),"mock_backend_method_calls":count.methods.load(Ordering::SeqCst),"mock_setter_calls":count.setters.load(Ordering::SeqCst),"generic_click_key_text_calls":count.generic.load(Ordering::SeqCst)});
    let result = match first {
        Ok(v) => json!({"value":v}),
        Err(error) => json!({"error":error.code,"diagnostic":error.diagnostic}),
    };
    let receipt_files = std::fs::read_dir(dir.path().join("state/receipts"))
        .unwrap()
        .count();
    json!({"result":result,"secondary":secondary,"counts":counts,"durable_file_count":receipt_files,
        "snapshots_remaining":e.snapshots.len(),"refs_remaining":e.refs.len(),"scope":"compiled Core route with synthetic backend only; not Native evidence"})
}

fn result(case: Value) -> Value {
    run_new_window_route(&case)
}
#[test]
fn core_route_writes_once_and_keeps_durable_duplicate() {
    let r = result(json!({"mode":"duplicate"}));
    assert_eq!(r["counts"]["mock_setter_calls"], 1);
    assert_eq!(r["secondary"]["receipt"]["deduplicated"], true);
    assert_eq!(r["result"]["value"]["receipt"]["goal_verified"], true);
    assert_eq!(r["counts"]["generic_click_key_text_calls"], 0);
}
#[test]
fn core_window_route_zero_write_independently_reads_requested_window_axes() {
    let r = result(json!({"already_selected":true}));
    assert_eq!(r["counts"]["mock_setter_calls"], 0);
    assert_eq!(r["result"]["value"]["window_readback"]["state"], "selected");
    let r = result(json!({"already_selected":true,"mode":"partial"}));
    assert_eq!(r["result"]["value"]["receipt"]["goal_verified"], false);
}
#[test]
fn core_route_native_one_budget_may_refine_to_zero_without_false_ack() {
    let r = result(json!({"mode":"refine_one_budget_to_no_write"}));
    assert_eq!(r["counts"]["mock_setter_calls"], 0);
    assert_eq!(
        r["result"]["value"]["window_readback"]["requested_write_plan"],
        "set_attribute_true_once"
    );
    assert_eq!(
        r["result"]["value"]["window_readback"]["write_plan"],
        "no_write_already_selected"
    );
    assert_eq!(
        r["result"]["value"]["receipt"]["system_accepted"],
        Value::Null
    );
    assert_eq!(r["result"]["value"]["receipt"]["goal_verified"], true);
}
#[test]
fn core_route_neutral_defaults_are_real_unsupported() {
    for mode in ["default_probe", "default_setter"] {
        let r = result(json!({"mode":mode}));
        assert_eq!(r["counts"]["mock_setter_calls"], 0);
        assert_eq!(r["counts"]["generic_click_key_text_calls"], 0);
        if mode == "default_probe" {
            assert_eq!(r["result"]["error"], "capability_unavailable");
        } else {
            assert_eq!(
                r["result"]["value"]["receipt"]["dispatch_error"]["code"],
                "capability_unavailable"
            );
        }
    }
}
#[test]
fn core_route_exact_lease_and_request_identity() {
    let r = result(json!({"mode":"invalid_lease"}));
    assert_eq!(r["result"]["error"], "lease_required");
    assert_eq!(r["counts"]["mock_backend_method_calls"], 0);
    let r = result(json!({"mode":"conflict"}));
    assert_eq!(r["secondary"]["error"], "idempotency_conflict");
    assert_eq!(r["counts"]["mock_setter_calls"], 1);
}
#[test]
fn core_route_pending_and_partial_journals_never_replay() {
    for mode in ["incomplete_journal", "pending_journal"] {
        let r = result(json!({"mode":mode}));
        assert_eq!(r["counts"]["mock_backend_method_calls"], 0);
        assert_eq!(r["counts"]["mock_setter_calls"], 0);
    }
}
#[test]
fn core_route_native_custody_and_sidecar_are_not_tickets() {
    for case in [
        json!({"app_custody":"sibling"}),
        json!({"app_custody":"unknown"}),
        json!({"object_cf":"different"}),
        json!({"sidecar":true}),
        json!({"probe_error":true}),
        json!({"not_writable":true}),
    ] {
        let r = result(case);
        assert_eq!(
            r["result"]["value"]["receipt"]["dispatch"],
            "rejected_before_dispatch"
        );
        assert_eq!(r["counts"]["mock_setter_calls"], 0);
    }
}
#[test]
fn core_route_unknown_ack_is_durable_not_automatic_retry() {
    let r = result(json!({"mode":"ack_unknown"}));
    assert_eq!(r["counts"]["mock_setter_calls"], 1);
    assert_eq!(r["result"]["value"]["receipt"]["dispatch"], "uncertain");
    assert_eq!(r["secondary"]["receipt"]["deduplicated"], true);
    assert_eq!(r["secondary"]["receipt"]["goal_verified"], false);
}
#[test]
fn core_route_missing_dispatch_counter_not_a_predispatch_proof() {
    for counter in ["missing", "saturated"] {
        let r = result(json!({"mode":"reject_before_setter","counter":counter}));
        assert_eq!(r["result"]["value"]["receipt"]["dispatch"], "uncertain");
        assert_eq!(r["counts"]["mock_setter_calls"], 0);
    }
}
#[test]
fn core_route_foreground_axis_preserved_and_aliases_retired() {
    let r = result(json!({"foreground_changed":true}));
    assert_eq!(
        r["result"]["value"]["window_readback"]["os_foreground_unchanged"],
        false
    );
    let r = result(json!({"mode":"same_app_alias"}));
    assert_eq!(
        r["result"]["value"]["grounding_invalidated"]["target_count"],
        2
    );
    assert_eq!(r["snapshots_remaining"], 0);
    assert_eq!(r["refs_remaining"], 0);
}
#[test]
fn core_route_zero_write_budget_cannot_be_escalated() {
    let r = result(json!({"mode":"escalate_zero_budget","already_selected":true}));
    assert_eq!(r["result"]["value"]["receipt"]["goal_verified"], false);
    assert_eq!(r["result"]["value"]["receipt"]["dispatch"], "uncertain");
}
#[test]
fn main_and_key_goals_are_distinct_no_typing_authority() {
    for kind in ["main", "key"] {
        let r = result(json!({"kind":kind}));
        assert_eq!(r["result"]["value"]["window_readback"]["kind"], kind);
        assert_eq!(
            r["result"]["value"]["window_readback"]["typing_authorized"],
            false
        );
        assert_eq!(
            r["result"]["value"]["window_readback"]["semantic_element_ready"],
            Value::Null
        );
        assert_eq!(r["counts"]["generic_click_key_text_calls"], 0);
    }
}
#[test]
fn provider_unsupported_and_raw_probe_error_refuse_before_write() {
    for kind in ["main", "key"] {
        for opts in [
            json!({"not_writable":true}),
            json!({"probe_error":true}),
            json!({"provider_birth":"different"}),
        ] {
            let mut c = opts;
            c["kind"] = json!(kind);
            let r = result(c);
            assert_eq!(r["counts"]["mock_setter_calls"], 0);
            assert_eq!(
                r["result"]["value"]["receipt"]["dispatch"],
                "rejected_before_dispatch"
            );
        }
    }
}
#[test]
fn wrong_kind_and_double_setter_deny_goal_without_fallback() {
    for mode in ["wrong_kind", "double_setter"] {
        let r = result(json!({"mode":mode}));
        assert_eq!(r["result"]["value"]["receipt"]["goal_verified"], false);
        assert_eq!(r["result"]["value"]["receipt"]["dispatch"], "uncertain");
        assert_eq!(r["counts"]["generic_click_key_text_calls"], 0);
    }
}
