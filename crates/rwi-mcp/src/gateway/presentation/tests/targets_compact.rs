use super::*;

fn native_targets_with_known_detail() -> Value {
    let context =
        json!({"host":"HOST","guest":null,"session":"SERIAL","seat":"SEAT","desktop":"DESKTOP"});
    let fg = json!({"pid":42,"window":"cgwindow:77","process_start":"PROCESS_START","native_epoch":"EXACT_EPOCH"});
    let runtime = json!({"pid":10070,"uid":501,"euid":501,"status":"available","native_context":context,"lock_state":"unknown","permissions":{"accessibility":true,"listen_event":false,"post_event":null,"screen_capture":true,"requests_permission":false}});
    let capabilities = vec![
        json!({"id":"computer.focus_element","implemented":true,"authorized":true,"available":false,"reason":"selected_window_required"}),
        json!({"id":"computer.launch_application","implemented":false,"authorized":true,"available":false,"reason":"not_implemented_on_this_platform"}),
        json!({"id":"computer.custom","implemented":null,"available":null,"reason":"unknown"}),
    ];
    let caps = json!({"adapter":"rust-macos-native","evidence":"native_api_observed","window_selected":false,"current_foreground":fg,"permissions":runtime["permissions"],"atomicity":"Neither a returned post nor property equality proves the business goal.",
        "background_input":{"native_acknowledgement":false,"application_effect_attested":false,"receiver_preparation":{"keyboard":"legacy AppKit activation when inactive after payload/focus validation","pointer":"none"}},
        "background_operation":"DOC".repeat(16000),"binding_identity":"DOC".repeat(5000),
        "focus_element":{"implemented":true,"available":false,"attribute":"AXFocused","max_setter_attempts":1,"scope":"DOC".repeat(2000),"goal_attested_by_capability":false},
        "pointer_feedback":{"visible":false,"point":[123.5,456.75],"applied_point":null,"display_id":4},
        "custom_future_policy":{"scope":"DO NOT DROP FUTURE UNKNOWN","available":null},
        "mapping":{"point":[900.25,501.5],"native_epoch":"CONTROL_EPOCH"}});
    json!({"devices":[{"device_id":"mac-host","is_error":false,"adapter_transport":{"authenticated_connections":2,"request_writes_started":3,"custom_error":"unknown transport condition"},"state":{"device_id":"mac-host","platform":"macos","instance":"INSTANCE","resource_domain":"DOMAIN","context_valid":true,"native_context":context,"runtime_status":runtime,"service_process_status":runtime,"input_domain_fenced":false,"capabilities":capabilities,"live_desktop_facts":{"identity":context,"values":{"current_foreground":fg}},"recent_operations":[{"request_id":"old-once","dispatch":"uncertain","window":"cgwindow:77"}]}}],"targets":[{"target":"mac-host:INSTANCE:EXACT_TARGET","identity":context,"parent_target":null,"binding":{"window":"cgwindow:77","pid":42,"bounds":[1.5,2.5,800,600]},"capabilities":caps}],"service":"healthy","profile_error":null,"workflow_required":false})
}
#[test]
fn targets_compact_keeps_exact_routing_permission_fence_foreground_and_capability_facts() {
    let raw = native_targets_with_known_detail();
    let mut cache = Presentation::default();
    let out = cache
        .project("computer.targets", DeviceResult::value(&raw))
        .data;
    let old = &raw["devices"][0]["state"];
    let state = &out["devices"][0]["state"];
    for key in [
        "device_id",
        "platform",
        "instance",
        "resource_domain",
        "context_valid",
        "native_context",
        "input_domain_fenced",
        "capabilities",
        "live_desktop_facts",
    ] {
        assert_eq!(&state[key], &old[key], "{key}");
    }
    assert_eq!(state["runtime_status"]["pid"], 10070);
    assert_eq!(
        state["runtime_status"]["permissions"],
        old["runtime_status"]["permissions"]
    );
    assert_eq!(state["runtime_status"]["lock_state"], "unknown");
    assert!(state.get("service_process_status").is_none());
    assert!(state["runtime_status"].get("native_context").is_none());
    for key in ["target", "identity", "parent_target", "binding"] {
        assert_eq!(out["targets"][0][key], raw["targets"][0][key]);
    }
    let caps = &out["targets"][0]["capabilities"];
    for key in [
        "current_foreground",
        "permissions",
        "atomicity",
        "background_input",
        "pointer_feedback",
        "mapping",
        "custom_future_policy",
    ] {
        assert_eq!(caps[key], raw["targets"][0]["capabilities"][key], "{key}");
    }
    assert_eq!(caps["focus_element"]["implemented"], true);
    assert_eq!(caps["focus_element"]["available"], false);
    assert_eq!(caps["focus_element"]["attribute"], "AXFocused");
    assert_eq!(
        out["devices"][0]["adapter_transport"]["custom_error"],
        "unknown transport condition"
    );
    assert!(
        out["devices"][0]["adapter_transport"]
            .get("authenticated_connections")
            .is_none()
    );
    assert!(bytes(&out) <= COMPACT_BYTES);
    assert!(out["presentation"].get("over_budget").is_none());
}
#[test]
fn targets_compact_retains_original_detail_hash_and_byte_exact_cache_data() {
    let raw = native_targets_with_known_detail();
    let mut cache = Presentation::default();
    let out = cache
        .project("computer.targets", DeviceResult::value(&raw))
        .data;
    assert_eq!(cache.entries[0].data, raw);
    assert_eq!(
        out["presentation"]["metadata_sha256"],
        rwi_core::digest(&serde_json::to_vec(&raw).unwrap())
    );
    let id = out["presentation"]["result_ref"].as_str().unwrap();
    let full = cache.read(args(id, "/devices/0/state/recent_operations", 0));
    assert_eq!(
        full.data["value"],
        raw["devices"][0]["state"]["recent_operations"]
    );
    assert_eq!(
        cache
            .read(args(id, "/devices/0/state/service_process_status", 0))
            .data["value"],
        raw["devices"][0]["state"]["service_process_status"]
    );
    assert_eq!(
        cache
            .read(args(id, "/targets/0/capabilities/pointer_feedback", 0))
            .data["value"],
        raw["targets"][0]["capabilities"]["pointer_feedback"]
    );
    assert_eq!(out["presentation"]["source_tool"], "computer.targets");
    assert_eq!(out["presentation"]["details_tool"], "computer.result");
}
#[test]
fn targets_compact_never_hides_conflicting_or_unknown_native_status_context() {
    let mut raw = native_targets_with_known_detail();
    raw["devices"][0]["state"]["service_process_status"]["pid"] = json!(999);
    raw["devices"][0]["state"]["runtime_status"]["native_context"]["session"] = json!("DIFFERENT");
    raw["devices"][0]["state"]["runtime_status"]["permissions"]["accessibility"] = Value::Null;
    let mut cache = Presentation::default();
    let out = cache
        .project("computer.targets", DeviceResult::value(&raw))
        .data;
    assert_eq!(
        out["devices"][0]["state"]["service_process_status"],
        raw["devices"][0]["state"]["service_process_status"]
    );
    assert_eq!(
        out["devices"][0]["state"]["runtime_status"],
        raw["devices"][0]["state"]["runtime_status"]
    );
    assert_eq!(
        out["devices"][0]["state"]["capabilities"][2]["implemented"],
        Value::Null
    );
    assert!(
        out["devices"][0]["state"]["capabilities"][2]
            .get("authorized")
            .is_none()
    );
}
#[test]
fn targets_compact_unknown_adapter_and_oversized_future_data_are_not_silently_discarded() {
    let mut raw = native_targets_with_known_detail();
    raw["targets"][0]["capabilities"]["adapter"] = json!("future-platform");
    let mut cache = Presentation::default();
    let out = cache
        .project("computer.targets", DeviceResult::value(&raw))
        .data;
    assert_eq!(
        out["targets"][0]["capabilities"],
        raw["targets"][0]["capabilities"]
    );
    assert_eq!(out["presentation"]["over_budget"], true);
    assert_eq!(out["presentation"]["source_tool"], "computer.targets");
}
#[test]
fn targets_compact_preserves_null_and_wrong_type_prose_keys_and_does_not_touch_controls() {
    let mut raw = native_targets_with_known_detail();
    raw["targets"][0]["capabilities"]["focus_element"]["scope"] = Value::Null;
    let mut cache = Presentation::default();
    let out = cache
        .project("computer.targets", DeviceResult::value(&raw))
        .data;
    assert_eq!(
        out["targets"][0]["capabilities"]["focus_element"]["scope"],
        Value::Null
    );
    let observed = observation();
    let expected = Presentation::default()
        .project("computer.observe", DeviceResult::value(&observed))
        .data;
    let actual = cache
        .project("computer.observe", DeviceResult::value(observed))
        .data;
    assert_eq!(actual["scene"]["elements"], expected["scene"]["elements"]);
    assert_eq!(actual["operation_ref"], expected["operation_ref"]);
}

#[test]
fn targets_compact_legacy_text_projection_and_unknown_device_shapes_stay_compatible() {
    let raw = f16_targets();
    let mut cache = Presentation::default();
    let result = cache.project("computer.targets", DeviceResult::value(&raw));
    let text = cache
        .targets_text_projection(&result)
        .expect("legacy exact-duplicate text projection");
    assert_eq!(
        result.data["devices"][0]["state"]["runtime_status"],
        raw["devices"][0]["state"]["runtime_status"]
    );
    assert_eq!(
        result.data["devices"][0]["state"]["service_process_status"],
        raw["devices"][0]["state"]["service_process_status"]
    );
    assert_eq!(
        text["presentation"]["text_projection"]["exact_duplicates"]["/devices/0/state/service_process_status"],
        "/devices/0/state/runtime_status"
    );
    let mut unknown = native_targets_with_known_detail();
    unknown["targets"][0]["capabilities"]["adapter"] = json!("future-platform");
    let result = cache.project("computer.targets", DeviceResult::value(&unknown));
    assert_eq!(
        result.data["devices"][0]["state"]["runtime_status"],
        unknown["devices"][0]["state"]["runtime_status"]
    );
    assert_eq!(
        result.data["devices"][0]["state"]["service_process_status"],
        unknown["devices"][0]["state"]["service_process_status"]
    );
    assert_eq!(
        result.data["devices"][0]["adapter_transport"],
        unknown["devices"][0]["adapter_transport"]
    );
}
