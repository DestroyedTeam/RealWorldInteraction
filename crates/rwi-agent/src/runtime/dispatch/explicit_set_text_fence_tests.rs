#[test]
fn explicit_set_text_uncertain_fences_unless_exact_once_ax_no_held_input_evidence() {
    use super::*;
    let mut result = DeviceResult::value(
        json!({"receipt":{"dispatch":"uncertain","replay":"forbidden","goal_verified":false,"dispatch_error":{"code":"set_text_unconfirmed"}}}),
    );
    assert!(needs_execution_input_fence(
        "computer.set_text",
        true,
        true,
        true,
        false,
        &result
    ));
    result.data["receipt"]["dispatch_error"]["diagnostic"] = json!({"version":1,"operation_kind":"set_text","native_setter_attempts":1,"native_dispatch_counter_known":true,"api":"AXUIElementSetAttributeValue","attribute":"AXValue","native_setter_attempted":true,"native_status":-25204,"native_dispatch_count_before":4,"native_dispatch_count_after":5,"held_input_resources":false,"replay_allowed":false});
    assert!(!needs_execution_input_fence(
        "computer.set_text",
        true,
        true,
        true,
        false,
        &result
    ));
    assert!(needs_execution_input_fence(
        "computer.set_text",
        true,
        true,
        true,
        true,
        &result
    ));
    assert!(needs_execution_input_fence(
        "computer.set_text",
        true,
        true,
        false,
        false,
        &result
    ));
    result.data["receipt"]["dispatch_error"]["diagnostic"]["native_dispatch_count_after"] =
        6.into();
    assert!(needs_execution_input_fence(
        "computer.set_text",
        true,
        true,
        true,
        false,
        &result
    ));
}
