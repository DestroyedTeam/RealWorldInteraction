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

// Serving adapter metadata, not the device Agent, the installed file hash,
// or an input grant. A cloned connection keeps its ID; a new one does not.
#[tokio::test]
async fn adapter_runtime_identity_is_compact_and_separate_from_device_identity() {
    let gateway = Gateway::new(Ok(AdapterProfile::default()));
    let first = gateway.targets(None).await;
    let again = gateway.clone().targets(None).await;
    let other = Gateway::new(Ok(AdapterProfile::default()))
        .targets(None)
        .await;
    let identity = &first.data["adapter_runtime"];
    assert_eq!(identity.as_object().unwrap().len(), 3);
    assert_eq!(identity["pid"], std::process::id());
    assert_eq!(identity["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(identity["instance"], gateway.instance);
    assert_eq!(identity, &again.data["adapter_runtime"]);
    assert_ne!(
        identity["instance"],
        other.data["adapter_runtime"]["instance"]
    );
    assert_eq!(first.data["devices"], json!([]));
    assert_eq!(first.data["targets"], json!([]));
    assert_eq!(first.data["workflow_required"], false);
    for claim in [
        "binary_hash",
        "executable",
        "authorized",
        "native_dispatch_entered",
    ] {
        assert!(identity.get(claim).is_none());
    }
}
