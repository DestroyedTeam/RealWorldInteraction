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
        "pointer_feedback",
        "mapping",
        "custom_future_policy",
    ] {
        assert_eq!(caps[key], raw["targets"][0]["capabilities"][key], "{key}");
    }
    let mut background = raw["targets"][0]["capabilities"]["background_input"].clone();
    let keyboard_detail = background["receiver_preparation"]
        .as_object_mut()
        .unwrap()
        .remove("keyboard")
        .unwrap();
    assert_eq!(caps["background_input"], background);
    let reference = out["presentation"]["result_ref"].as_str().unwrap();
    assert_eq!(
        cache
            .read(args(
                reference,
                "/targets/0/capabilities/background_input/receiver_preparation/keyboard",
                0
            ))
            .data["value"],
        keyboard_detail
    );
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

fn targets_known_policy_fixture() -> Value {
    let mut raw = native_targets_with_known_detail();
    let policy = json!({
        "atomicity": "pre/post validation; neither CGEventPost nor property equality proves the business goal",
        "native_semantic_actions": ["AXPress", "AXCancel"],
        "unsupported": ["clipboard", "unattended unlock"],
        "background_input": {
            "foreground_transition_scope": "no explicit system-frontmost request; not target AppKit key/order preservation",
            "receiver_preparation": {
                "keyboard": "legacy AppKit activation when inactive after payload/focus validation"
            },
            "application_effect_attested": false,
            "native_acknowledgement": false
        },
        "local_pixel_grounding": {
            "evidence": "stable textured local bitmap patch plus native input checks"
        },
        "paired_pointer_drag": {
            "delivery": "initiating PID/window addressed; native drag manager owns destination routing",
            "endpoint_authority": "two independently resolved input targets; same native desktop/shared lease"
        },
        "pointer_move": {
            "background_appkit_tracking_experimental": {
                "registration": "caller must establish exact currently registered legacy owner/tag/process/window lifetime",
                "scope": "experimental registered legacy owner calibration only; not generic modern tracking/tooltip",
                "implemented": true,
                "available": false,
                "reason": "registered_owner_required",
                "generic_cold_hover_verified": false,
                "user_data": "NULL only",
                "tracking_number_encoding": "canonical nonzero signed i64 decimal string (opaque NSInteger)",
                "value": {
                    "delivery": "background_appkit_tracking_experimental",
                    "tracking_number": "9007199254740993"
                }
            },
            "background_cooperative_tracking_experimental": {
                "mutual_peer": "kernel PID + audit token/pidversion + native process birth, repeated before execution",
                "registration_lifetime": "owner UI reservation; caller integration must defer close/reparent/remove while busy",
                "route": "native-authenticated cooperative owner; synchronous owner-main-thread NSApplication.sendEvent",
                "implemented": true,
                "available": true,
                "unknown_registration": "fail_closed",
                "ordinary_third_party_or_modern_tracking_verified": false,
                "raw_tag_lane_enabled": false,
                "retry_input": false,
                "value": {
                    "delivery": "background_cooperative_tracking_experimental"
                }
            },
            "foreground_system": {
                "goal_verification": "independent; not guaranteed tracking or tooltip",
                "requires": "fresh grounded reference + shared input lease + exact current foreground + idle HID + existing post/listen permissions",
                "implemented": true,
                "native_acknowledgement": false,
                "value": {
                    "delivery": "foreground_system",
                    "expected_foreground": "fresh exact selected-window tuple from current_foreground"
                }
            },
            "implemented": true,
            "default_delivery": "background_window",
            "event": "mouse_moved",
            "global_fallback": false
        },
        "recovery": "read-only HID + combined session resource state, requiring listen-event preflight and agent idle writer proof",
        "select_window_local": {
            "kind_support": "fresh exact per-kind settable status required; Main is not a Key fallback"
        },
        "semantic_object_identity": {
            "identity": "retained type-checked AXUIElement plus CFEqual window/object identity and non-reused serial; not attribute hash alone"
        },
        "set_text": {
            "readback": "two full retained-object CFString reads; no DOM or typing claim",
            "semantics": "entire current ordinary plain CFString replacement; empty clears",
            "implemented": true,
            "available": false,
            "reason": "selected_window_required",
            "max_utf8_bytes": 16384,
            "keyboard_events": false,
            "append_or_caret": false
        },
        "text_input": "only the exact focused, explicitly classified non-secure AX text element; unknown classifications remain unavailable",
        "visual_only_snapshot": {
            "capture": "exact bound CGWindowID/PID via ScreenCaptureKit",
            "trigger": "exact AXWindows read CannotComplete after bounded settle, explicitly unsupported/no-value, or complete unambiguous AX search with no match",
            "implemented": true,
            "input_reference": false,
            "controls": false
        }
    });
    for (key, value) in policy.as_object().unwrap() {
        raw["targets"][0]["capabilities"][key] = value.clone();
    }
    raw
}

#[test]
fn targets_known_policy_exact_text_is_delayed_but_readiness_syntax_and_binding_stay_exact() {
    let raw = targets_known_policy_fixture();
    let out = Presentation::default()
        .project("computer.targets", DeviceResult::value(&raw))
        .data;
    for pointer in [
        "/atomicity",
        "/background_input/foreground_transition_scope",
        "/background_input/receiver_preparation/keyboard",
        "/local_pixel_grounding/evidence",
        "/paired_pointer_drag/delivery",
        "/paired_pointer_drag/endpoint_authority",
        "/pointer_move/background_appkit_tracking_experimental/registration",
        "/pointer_move/background_appkit_tracking_experimental/scope",
        "/pointer_move/background_cooperative_tracking_experimental/mutual_peer",
        "/pointer_move/background_cooperative_tracking_experimental/registration_lifetime",
        "/pointer_move/background_cooperative_tracking_experimental/route",
        "/pointer_move/foreground_system/goal_verification",
        "/pointer_move/foreground_system/requires",
        "/recovery",
        "/select_window_local/kind_support",
        "/semantic_object_identity/identity",
        "/set_text/readback",
        "/set_text/semantics",
        "/text_input",
        "/visual_only_snapshot/capture",
        "/visual_only_snapshot/trigger",
    ] {
        assert!(
            out["targets"][0]["capabilities"].pointer(pointer).is_none(),
            "{pointer}"
        );
    }
    for key in [
        "device_id",
        "context_valid",
        "native_context",
        "input_domain_fenced",
        "capabilities",
        "live_desktop_facts",
    ] {
        assert_eq!(
            out["devices"][0]["state"][key], raw["devices"][0]["state"][key],
            "{key}"
        );
    }
    for key in ["target", "identity", "parent_target", "binding"] {
        assert_eq!(out["targets"][0][key], raw["targets"][0][key], "{key}");
    }
    for pointer in [
        "/current_foreground",
        "/permissions",
        "/pointer_feedback",
        "/mapping",
        "/custom_future_policy",
        "/native_semantic_actions",
        "/unsupported",
        "/pointer_move/default_delivery",
        "/pointer_move/event",
        "/pointer_move/background_appkit_tracking_experimental/value",
        "/pointer_move/background_appkit_tracking_experimental/tracking_number_encoding",
        "/pointer_move/background_appkit_tracking_experimental/user_data",
        "/pointer_move/background_appkit_tracking_experimental/available",
        "/pointer_move/background_appkit_tracking_experimental/reason",
        "/pointer_move/background_cooperative_tracking_experimental/unknown_registration",
        "/pointer_move/background_cooperative_tracking_experimental/raw_tag_lane_enabled",
        "/pointer_move/foreground_system/value",
        "/set_text/available",
        "/set_text/reason",
        "/set_text/max_utf8_bytes",
    ] {
        assert_eq!(
            out["targets"][0]["capabilities"].pointer(pointer),
            raw["targets"][0]["capabilities"].pointer(pointer),
            "{pointer}"
        );
    }
}

#[test]
fn targets_known_policy_full_cache_and_fingerprint_remain_exact() {
    let raw = targets_known_policy_fixture();
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
    for pointer in [
        "/targets/0/capabilities/recovery",
        "/targets/0/capabilities/set_text/semantics",
        "/targets/0/capabilities/pointer_move/background_cooperative_tracking_experimental/route",
    ] {
        assert_eq!(
            cache.read(args(id, pointer, 0)).data["value"],
            *raw.pointer(pointer).unwrap()
        );
    }
}

#[test]
fn targets_known_policy_new_text_null_wrong_types_and_nested_unknowns_are_not_removed() {
    for replacement in [
        json!(null),
        json!(false),
        json!(17),
        json!({"future":null}),
        json!("future policy wording"),
    ] {
        let mut raw = targets_known_policy_fixture();
        raw["targets"][0]["capabilities"]["recovery"] = replacement.clone();
        raw["targets"][0]["capabilities"]["pointer_move"]["background_cooperative_tracking_experimental"]
            ["route"] = replacement.clone();
        raw["targets"][0]["capabilities"]["custom_future_policy"]["recovery"] = replacement.clone();
        let out = Presentation::default()
            .project("computer.targets", DeviceResult::value(&raw))
            .data;
        assert_eq!(out["targets"][0]["capabilities"]["recovery"], replacement);
        assert_eq!(
            out["targets"][0]["capabilities"]["pointer_move"]["background_cooperative_tracking_experimental"]
                ["route"],
            replacement
        );
        assert_eq!(
            out["targets"][0]["capabilities"]["custom_future_policy"],
            raw["targets"][0]["capabilities"]["custom_future_policy"]
        );
    }
}

#[test]
fn targets_known_policy_unknown_adapter_keeps_all_policy_and_unknown_facts() {
    let mut raw = targets_known_policy_fixture();
    raw["targets"][0]["capabilities"]["adapter"] = json!("future-native-adapter");
    let out = Presentation::default()
        .project("computer.targets", DeviceResult::value(&raw))
        .data;
    assert_eq!(out["targets"][0], raw["targets"][0]);
}

#[test]
fn targets_known_policy_errors_and_non_targets_are_unchanged() {
    let raw = targets_known_policy_fixture();
    let mut error = DeviceResult::value(&raw);
    error.is_error = true;
    assert_eq!(
        Presentation::default()
            .project("computer.targets", error)
            .data,
        raw
    );
    assert_eq!(
        Presentation::default()
            .project("computer.operation", DeviceResult::value(&raw))
            .data,
        raw
    );
}
