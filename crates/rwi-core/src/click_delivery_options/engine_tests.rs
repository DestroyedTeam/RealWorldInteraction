//! Offline transport policy fixtures, never application-effect evidence.
use crate::{fixture::Fixture, *};
use std::sync::{Arc, Mutex};

const EXPLICIT: &str = r#"{"delivery":"background_skylight_experimental"}"#;

#[derive(Default)]
struct Seen {
    reads: usize,
    values: Vec<Option<String>>,
    actions: Vec<Option<String>>,
    elements: Vec<Option<String>>,
}
struct RouteFixture {
    base: Fixture,
    available: Option<bool>,
    semantic_action: bool,
    seen: Arc<Mutex<Seen>>,
}
impl Backend for RouteFixture {
    fn identity(&self) -> Identity {
        self.base.identity()
    }
    fn capabilities(&self) -> serde_json::Value {
        let mut value = serde_json::json!({"checked_context_policy":"native_element"});
        if let Some(available) = self.available {
            value["pointer_click"] = serde_json::json!({"background_skylight_experimental":{
                "implemented":true,"available":available}});
        }
        if self.semantic_action {
            value["native_semantic_actions"] = serde_json::json!(["AXPress"]);
        }
        value
    }
    fn observe(&mut self) -> Result<Scene> {
        self.seen.lock().unwrap().reads += 1;
        let mut scene = self.base.observe()?;
        for element in &mut scene.elements {
            element.enabled = Some(true);
            if self.semantic_action {
                element.actions = vec!["AXPress".into()];
            }
        }
        Ok(scene)
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        Some(self.base.0.lock().unwrap().calls)
    }
    fn act(
        &mut self,
        binding: &Binding,
        target: &Resolved,
        intent: &Intent,
        value: Option<&str>,
    ) -> Result<Option<bool>> {
        let mut seen = self.seen.lock().unwrap();
        seen.values.push(value.map(str::to_owned));
        seen.actions.push(target.native_action.clone());
        seen.elements.push(target.scope.element.clone());
        drop(seen);
        self.base.act(binding, target, intent, value)
    }
    fn inspect(&mut self, scope: &Scope, predicate: &Predicate, expected: &str) -> Result<Oracle> {
        self.base.inspect(scope, predicate, expected)
    }
}

struct Case {
    directory: tempfile::TempDir,
    engine: Engine,
    base: Fixture,
    seen: Arc<Mutex<Seen>>,
    frame: String,
    reference: String,
    lease: String,
}
fn setup(available: Option<bool>, semantic_action: bool) -> Case {
    let directory = tempfile::tempdir().unwrap();
    let base = Fixture::default();
    // A dispatched fixture acknowledgement intentionally proves no app effect.
    base.0.lock().unwrap().ack_without_effect = true;
    let seen = Arc::new(Mutex::new(Seen::default()));
    let mut engine = Engine::new(directory.path().into()).unwrap();
    engine
        .register(
            "t".into(),
            Box::new(RouteFixture {
                base: base.clone(),
                available,
                semantic_action,
                seen: seen.clone(),
            }),
        )
        .unwrap();
    let frame = engine.observe("t").unwrap().frame;
    let reference = engine
        .resolve("t", &frame, Locator::Element { id: "Input".into() })
        .unwrap()
        .reference;
    let lease = engine.claim("t", 60000).unwrap();
    Case {
        directory,
        engine,
        base,
        seen,
        frame,
        reference,
        lease,
    }
}
fn call(
    case: &mut Case,
    checked: bool,
    request: &str,
    intent: Intent,
    value: Option<&str>,
) -> Result<Receipt> {
    if checked {
        case.engine.act_checked(CheckedAction {
            target: "t",
            frame: &case.frame,
            element: "Input",
            lease: &case.lease,
            request,
            intent,
            value: value.map(str::to_owned),
        })
    } else {
        case.engine.act(
            "t",
            &case.reference,
            &case.lease,
            request,
            intent,
            value.map(str::to_owned),
        )
    }
}
fn reservations(case: &Case) -> usize {
    std::fs::read_dir(case.directory.path().join("receipts"))
        .unwrap()
        .count()
}

#[test]
fn r92_click_engine_unadvertised_route_refuses_before_reads_or_reservation() {
    for checked in [false, true] {
        for available in [None, Some(false)] {
            let mut case = setup(available, false);
            let reads = case.seen.lock().unwrap().reads;
            let snapshots = case.engine.snapshot_count();
            let result = call(
                &mut case,
                checked,
                "unsupported",
                Intent::Click,
                Some(EXPLICIT),
            );
            assert_eq!(result.unwrap_err().code, "capability_unavailable");
            assert_eq!(case.base.0.lock().unwrap().calls, 0);
            assert_eq!(case.seen.lock().unwrap().reads, reads);
            assert_eq!(case.engine.snapshot_count(), snapshots);
            assert_eq!(reservations(&case), 0);
            case.engine.release(&case.lease).unwrap();
        }
    }
}

#[test]
fn r92_click_engine_explicit_route_keeps_original_element_and_no_native_action() {
    for checked in [false, true] {
        let mut case = setup(Some(true), false);
        let receipt = call(
            &mut case,
            checked,
            "explicit",
            Intent::Click,
            Some(EXPLICIT),
        )
        .unwrap();
        assert_eq!(receipt.dispatch, "returned");
        assert!(!receipt.goal_verified);
        assert_eq!(receipt.replay, "forbidden");
        assert_eq!(receipt.scope.element.as_deref(), Some("Input"));
        let seen = case.seen.lock().unwrap();
        assert_eq!(seen.values, [Some(EXPLICIT.into())]);
        assert_eq!(seen.actions, [None]);
        assert_eq!(seen.elements, [Some("Input".into())]);
        assert_eq!(case.base.0.lock().unwrap().calls, 1);
        drop(seen);
        case.engine.release(&case.lease).unwrap();
    }
}

#[test]
fn r92_click_engine_bare_legacy_click_does_not_require_private_route_capability() {
    for checked in [false, true] {
        let mut case = setup(None, false);
        assert!(call(&mut case, checked, "bare", Intent::Click, None).is_ok());
        assert_eq!(case.base.0.lock().unwrap().calls, 1);
        assert_eq!(case.seen.lock().unwrap().values, [None]);
        case.engine.release(&case.lease).unwrap();
    }
}

#[test]
fn r92_click_engine_encoded_delivery_is_fingerprinted_and_never_replayed() {
    for checked in [false, true] {
        let mut case = setup(Some(true), false);
        let result = call(&mut case, checked, "once", Intent::Click, Some(EXPLICIT)).unwrap();
        assert!(!result.deduplicated);
        assert!(
            call(&mut case, checked, "once", Intent::Click, Some(EXPLICIT))
                .unwrap()
                .deduplicated
        );
        for changed in [
            None,
            Some(r#"{ "delivery": "background_skylight_experimental" }"#),
        ] {
            assert_eq!(
                call(&mut case, checked, "once", Intent::Click, changed)
                    .unwrap_err()
                    .code,
                "idempotency_conflict"
            );
        }
        assert_eq!(case.base.0.lock().unwrap().calls, 1);
        case.engine.release(&case.lease).unwrap();
    }
}

#[test]
fn r92_click_engine_invalid_json_and_unsupported_pointer_values_never_reserve() {
    for checked in [false, true] {
        for (intent, value) in [
            (Intent::Click, "{"),
            (Intent::Click, "null"),
            (
                Intent::Click,
                r#"{"delivery":"background_skylight_experimental","action":"AXPress"}"#,
            ),
            (Intent::DoubleClick, EXPLICIT),
            (Intent::ContextMenu, EXPLICIT),
        ] {
            let mut case = setup(Some(true), false);
            assert_eq!(
                call(&mut case, checked, "invalid", intent, Some(value))
                    .unwrap_err()
                    .code,
                "invalid_input_value"
            );
            assert_eq!(case.base.0.lock().unwrap().calls, 0);
            assert_eq!(reservations(&case), 0);
            case.engine.release(&case.lease).unwrap();
        }
    }
    assert!(Intent::Text.validate(Some(EXPLICIT)).is_ok());
    assert!(Intent::MovePointer.validate(Some(EXPLICIT)).is_ok());
    let mut literal = setup(None, false);
    literal.base.0.lock().unwrap().ack_without_effect = false;
    let receipt = call(
        &mut literal,
        true,
        "literal-json-text",
        Intent::Text,
        Some(EXPLICIT),
    )
    .unwrap();
    assert_eq!(literal.base.0.lock().unwrap().value, EXPLICIT);
    assert!(!receipt.goal_verified);
    literal.engine.release(&literal.lease).unwrap();
}

#[test]
fn r92_click_engine_native_action_reference_cannot_switch_to_delivery_route() {
    let mut case = setup(Some(true), true);
    case.reference = case
        .engine
        .resolve_action(
            "t",
            &case.frame,
            Locator::Element { id: "Input".into() },
            Some("AXPress"),
        )
        .unwrap()
        .reference;
    assert_eq!(
        call(
            &mut case,
            false,
            "no-promotion",
            Intent::Click,
            Some(EXPLICIT)
        )
        .unwrap_err()
        .code,
        "native_action_mismatch"
    );
    assert_eq!(case.base.0.lock().unwrap().calls, 0);
    assert_eq!(reservations(&case), 0);
    case.engine.release(&case.lease).unwrap();
}

#[test]
fn r92_click_engine_pointer_reference_cannot_be_promoted_by_plain_action_value() {
    for value in ["AXPress", "AXCancel", "AXUnknown", "unknown_action"] {
        let mut case = setup(Some(true), false);
        assert_eq!(
            call(
                &mut case,
                false,
                "no-native-promotion",
                Intent::Click,
                Some(value)
            )
            .unwrap_err()
            .code,
            "native_action_mismatch"
        );
        assert_eq!(case.base.0.lock().unwrap().calls, 0);
        assert_eq!(reservations(&case), 0);
        case.engine.release(&case.lease).unwrap();
    }
}

#[test]
fn r92_click_catalog_explains_own_capability_without_changing_legacy_value_schema() {
    let definitions = device::catalog::definitions();
    for name in ["computer.act", "computer.act_checked"] {
        let definition = definitions.iter().find(|tool| tool.name == name).unwrap();
        assert!(
            definition
                .description
                .contains("pointer_click.background_skylight_experimental")
        );
        let mut arguments = serde_json::json!({"target":"t","lease":"l","request_id":"r92-schema",
            "intent":"click","value":EXPLICIT});
        if name == "computer.act" {
            arguments["reference"] = serde_json::json!("exact-reference");
        } else {
            arguments["frame"] = serde_json::json!("exact-frame");
            arguments["element"] = serde_json::json!("exact-element");
        }
        device::catalog::validate(name, &arguments).unwrap();
        arguments["value"] = serde_json::json!({"delivery":"background_skylight_experimental"});
        assert!(device::catalog::validate(name, &arguments).is_err());
    }
}
