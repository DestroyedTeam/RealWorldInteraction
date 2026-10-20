//! Offline wire contract. No native window, capture, or input.
use rwi_core::device::catalog;
use serde_json::json;

#[test]
fn explicit_system_hotkey_catalog_has_guarded_separate_route() {
    let definitions = catalog::definitions();
    let tool = definitions
        .iter()
        .find(|tool| tool.name == "computer.hotkey")
        .expect("foreground/system hotkeys need an explicit route, not press_key fallback");
    assert!(tool.writes);
    let properties = tool.input_schema["properties"].as_object().unwrap();
    for field in [
        "target",
        "lease",
        "request_id",
        "expected_foreground",
        "key",
    ] {
        assert!(properties.contains_key(field), "missing {field}");
    }
    assert!(!properties.contains_key("element"));
    assert!(!properties.contains_key("frame"));
    let args = json!({"target":"desktop","lease":"lease","request_id":"cmd-tab-contract",
        "key":"cmd+tab", "expected_foreground":{"pid":42,"window":"cgwindow:7",
        "process_start":"42:1:2","native_epoch":"a".repeat(64)}});
    catalog::validate("computer.hotkey", &args).unwrap();
    for field in [
        "target",
        "lease",
        "request_id",
        "expected_foreground",
        "key",
    ] {
        let mut incomplete = args.clone();
        incomplete.as_object_mut().unwrap().remove(field);
        assert!(catalog::validate("computer.hotkey", &incomplete).is_err());
    }
    let mut unexpected = args;
    unexpected["global_fallback"] = json!(true);
    assert!(catalog::validate("computer.hotkey", &unexpected).is_err());
}
