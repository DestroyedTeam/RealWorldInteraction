use super::*;
#[test]
fn legacy_nested_dispatch_failure_is_explicit_without_rewriting_evidence() {
    let raw = json!({"dispatch":"uncertain","system_accepted":null,"native_dispatch_entered":false,"dispatch_error":{"code":"unsupported_chord","next_step":"Inspect supported keys"},"operation_ref":{"execution_nonce":"original"}});
    let result = mcp_result(DeviceResult::value(raw.clone()));
    assert_eq!(result.is_error, Some(true));
    let data = result.structured_content.unwrap();
    assert_eq!(data["code"], "unsupported_chord");
    assert_eq!(data["message"], "Inspect supported keys");
    for key in [
        "dispatch",
        "dispatch_error",
        "system_accepted",
        "native_dispatch_entered",
        "operation_ref",
    ] {
        assert_eq!(data[key], raw[key]);
    }
    assert_eq!(
        mcp_result(DeviceResult::value(json!({"system_accepted":true}))).is_error,
        Some(false)
    );
}
