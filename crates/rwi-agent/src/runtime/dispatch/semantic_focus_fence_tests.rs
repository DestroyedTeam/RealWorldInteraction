use super::*;
fn uncertain() -> DeviceResult {
    DeviceResult::value(
        json!({"receipt":{"dispatch":"uncertain","replay":"forbidden","goal_verified":false,"system_accepted":null,"dispatch_error":{"code":"focus_setter_uncertain"}},"focus_readback":{"state":"unknown","setter_status":null},"input_domain_fenced":false}),
    )
}
fn exact_proof() -> DeviceResult {
    let mut result = uncertain();
    result.data["receipt"]["dispatch_error"] = json!({"code":"semantic_focus_unconfirmed","diagnostic":{"version":1,"operation_kind":"semantic_focus","native_setter_attempts":1,"native_dispatch_counter_known":true,"native_call_pending":false,"native_resources_released":true,"api":"AXUIElementSetAttributeValue","attribute":"AXFocused","native_setter_attempted":true,"native_status":-25202,"native_dispatch_count_before":4,"native_dispatch_count_after":5,"held_input_resources":false,"replay_allowed":false}});
    result
}
fn fence(result: &DeviceResult, entered: bool, release: bool, pending: bool) -> bool {
    needs_execution_input_fence(
        "computer.focus_element",
        true,
        entered,
        release,
        pending,
        result,
    )
}
#[test]
fn semantic_focus_nested_uncertainty_now_fences_but_old_tool_policy_is_unchanged() {
    let result = uncertain();
    assert!(
        !needs_input_fence(true, true, true, &result),
        "old top-level predicate reproduces the gap"
    );
    assert!(fence(&result, true, true, false));
    assert!(!needs_execution_input_fence(
        "computer.act_checked",
        true,
        true,
        true,
        false,
        &result
    ));
    assert!(fence(&result, false, false, false));
    assert!(fence(&result, false, true, true));
    let mut missing = DeviceResult::value(json!({"focus_readback":null}));
    assert!(fence(&missing, true, true, false));
    missing.data["receipt"] = json!({"dispatch":"rejected_before_dispatch","replay":"forbidden"});
    assert!(!fence(&missing, false, true, false));
}
#[test]
fn semantic_focus_clean_return_or_no_write_is_not_a_goal_or_held_input_uncertainty() {
    for (entered, accepted, goal) in [(true, json!(true), false), (false, Value::Null, true)] {
        let result = DeviceResult::value(
            json!({"receipt":{"dispatch":"returned","replay":"forbidden","system_accepted":accepted,"goal_verified":goal}}),
        );
        assert!(!fence(&result, entered, true, false));
        assert!(fence(&result, entered, false, false));
        assert!(fence(&result, entered, true, true));
    }
}
#[test]
fn semantic_focus_no_hid_exception_requires_complete_exact_native_counter_evidence() {
    let exact = exact_proof();
    assert!(!fence(&exact, true, true, false));
    assert!(fence(&exact, true, false, false));
    assert!(fence(&exact, true, true, true));
    for key in [
        "version",
        "operation_kind",
        "native_setter_attempts",
        "native_dispatch_counter_known",
        "native_call_pending",
        "native_resources_released",
        "api",
        "attribute",
        "native_setter_attempted",
        "native_status",
        "native_dispatch_count_before",
        "native_dispatch_count_after",
        "held_input_resources",
        "replay_allowed",
    ] {
        let mut incomplete = exact.clone();
        incomplete.data["receipt"]["dispatch_error"]["diagnostic"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        assert!(fence(&incomplete, true, true, false), "{key}");
    }
    for (key, value) in [
        ("version", json!(2)),
        ("operation_kind", json!("click")),
        ("native_setter_attempts", json!(2)),
        ("native_dispatch_counter_known", json!(false)),
        ("native_call_pending", json!(true)),
        ("native_resources_released", json!(false)),
        ("api", json!("AXUIElementPerformAction")),
        ("attribute", json!("AXFocusedUIElement")),
        ("native_setter_attempted", json!(false)),
        ("held_input_resources", json!(true)),
        ("replay_allowed", json!(true)),
        ("native_status", json!(i64::from(i32::MAX) + 1)),
        ("native_status", json!(i64::from(i32::MIN) - 1)),
        ("native_status", json!("unknown")),
        ("native_dispatch_count_after", json!(4)),
        ("native_dispatch_count_before", json!(u64::MAX)),
    ] {
        let mut wrong = exact.clone();
        wrong.data["receipt"]["dispatch_error"]["diagnostic"][key] = value;
        assert!(fence(&wrong, true, true, false), "{key}");
    }
    let mut cleanup = exact.clone();
    cleanup.data["resource_release_uncertain"] = json!(true);
    assert!(fence(&cleanup, true, true, false));
    let mut cleanup_unknown = exact.clone();
    cleanup_unknown.data["cleanup"] = json!({"code":"release_uncertain"});
    assert!(fence(&cleanup_unknown, true, true, false));
    let mut replay = exact.clone();
    replay.data["receipt"]["replay"] = json!("allowed");
    assert!(fence(&replay, true, true, false));
    let mut saturated_unknown = exact.clone();
    saturated_unknown.data["receipt"]["dispatch_error"]["diagnostic"]["native_dispatch_count_before"] =
        json!(u64::MAX);
    saturated_unknown.data["receipt"]["dispatch_error"]["diagnostic"]["native_dispatch_count_after"] =
        Value::Null;
    assert!(fence(&saturated_unknown, true, true, false));
    let mut wrong_code = exact;
    wrong_code.data["receipt"]["dispatch_error"]["code"] = json!("native_action_unconfirmed");
    assert!(fence(&wrong_code, true, true, false));
}
