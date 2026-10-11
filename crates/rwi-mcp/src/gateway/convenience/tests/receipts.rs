use super::*;

#[test]
fn click_count_maps_one_native_intent_and_never_repeats_ax_actions() {
    let base =
        json!({"target":"t","request_id":"r","at":{"kind":"element","frame":"f","element":"e"}});
    let args: ClickArgs = serde_json::from_value(base.clone()).unwrap();
    assert_eq!(args.click_count, 1);
    assert_eq!(click_intent(1, true).unwrap(), "click");
    assert_eq!(click_intent(2, false).unwrap(), "double_click");
    assert!(click_intent(2, true).is_err());
    assert!(click_intent(0, false).is_err());
    assert!(click_intent(3, false).is_err());
    let mut invalid_count = base;
    invalid_count["click_count"] = json!(-1);
    assert!(serde_json::from_value::<ClickArgs>(invalid_count).is_err());
}

#[test]
fn stage_timings_aggregate_repeated_calls_without_logging_payloads() {
    let timings = CallTimings::default();
    assert_eq!(timings.rows(), json!([]));
    timings.record(
        "computer.resolve_view",
        std::time::Duration::from_micros(600),
    );
    timings.record("computer.claim", std::time::Duration::from_millis(2));
    timings.record(
        "computer.resolve_view",
        std::time::Duration::from_micros(600),
    );
    timings.record("computer.release", std::time::Duration::from_millis(3));
    assert_eq!(
        timings.rows(),
        json!([
            {"tool":"computer.resolve_view","calls":2,"elapsed_ms":1},
            {"tool":"computer.claim","calls":1,"elapsed_ms":2},
            {"tool":"computer.release","calls":1,"elapsed_ms":3}
        ])
    );
}

#[test]
fn timing_output_is_opt_in_for_success_but_kept_for_uncertainty() {
    let returned = DeviceResult::value(
        json!({"dispatch":"returned","dispatch_error":null,"input_domain_fenced":false}),
    );
    assert!(!expose_timings("computer.click", &returned, false));
    assert!(expose_timings("computer.click", &returned, true));
    assert!(!expose_timings("computer.snapshot", &returned, false));

    for uncertain in [
        DeviceResult::value(json!({"dispatch":"uncertain"})),
        DeviceResult::value(json!({"dispatch":"refused"})),
        DeviceResult::error("native_action_unconfirmed", "inspect receipt"),
    ] {
        assert!(expose_timings("computer.click", &uncertain, false));
    }
}

#[test]
fn compact_success_timing_opt_in_survives_projection_without_growing_default() {
    let operation_ref = json!({"ref_version":1,"device_id":"fixture",
        "agent_instance":"agent","context_generation":"context",
        "execution_nonce":"once"});
    let ordinary = json!({"request_id":"once","dispatch":"returned",
        "operation_ref":operation_ref,"system_accepted":null,
        "replay_allowed":false,"cleanup":{"resources_released":true},
        "resources_released":true,"input_domain_fenced":false,
        "frame":"fixture-frame","scope":{"window":"fixture-window","pid":1},
        "point":[12.0,34.0],"goal_verified":false,"details_ref":"fixture-details"});
    let mut raw = ordinary.clone();
    raw.as_object_mut().unwrap().remove("details_ref");
    raw["dispatch_error"] = Value::Null;
    raw["stage_timings"] = json!([
        {"tool":"computer.resolve_view","calls":2,"elapsed_ms":3},
        {"tool":"computer.claim","calls":1,"elapsed_ms":2},
        {"tool":"computer.act","calls":1,"elapsed_ms":5},
        {"tool":"computer.release","calls":1,"elapsed_ms":1}
    ]);
    raw["elapsed_ms"] = json!(12);
    let returned = DeviceResult::value(&raw);
    assert!(compactable_receipt(&returned));
    for tool in ["computer.click", "computer.drag"] {
        for requested in [None, Some(false), Some(true)] {
            let mut args = json!({});
            if let Some(requested) = requested {
                args["include_timings"] = json!(requested);
            }
            let requested = take_include_timings(&mut args).unwrap();
            assert_eq!(args, json!({}));
            // Compose the actual production policy + compact projection +
            // final opt-out trim, rather than testing the predicate alone.
            let include = expose_timings(tool, &returned, requested);
            let mut projected = compact_action_reply(&raw, Some("fixture-details".into()));
            if !include {
                trim_timing_fields(&mut projected);
            }
            if requested {
                assert_eq!(projected.get("stage_timings"), raw.get("stage_timings"));
                assert_eq!(projected.get("elapsed_ms"), raw.get("elapsed_ms"));
                trim_timing_fields(&mut projected);
            }
            // Identical default JSON, including coords, the exact operation
            // reference, explicit unknown, no-replay and cleanup evidence.
            assert_eq!(projected, ordinary);
        }
    }
}

#[test]
fn compact_success_timing_preserves_explicit_null_but_not_absent_fields() {
    let absent = compact_action_reply(&json!({"dispatch":"returned"}), None);
    assert!(absent.get("stage_timings").is_none());
    assert!(absent.get("elapsed_ms").is_none());
    let explicit_unknown = compact_action_reply(
        &json!({"dispatch":"returned","stage_timings":null,"elapsed_ms":0}),
        None,
    );
    assert_eq!(explicit_unknown.get("stage_timings"), Some(&Value::Null));
    assert_eq!(explicit_unknown["elapsed_ms"], 0);
}

#[test]
fn trimmed_success_timings_remain_available_from_details_ref() {
    let raw = json!({
        "request_id":"once",
        "dispatch":"returned",
        "stage_timings":[{"tool":"computer.claim","calls":1,"elapsed_ms":2}],
        "elapsed_ms":8
    });
    let mut cache = presentation::Presentation::default();
    let details_ref = cache.retain("computer.click", raw.clone()).unwrap();
    let mut ordinary = raw;
    trim_timing_fields(&mut ordinary);
    assert!(ordinary.get("stage_timings").is_none());
    assert!(ordinary.get("elapsed_ms").is_none());

    let details = cache.read(presentation::ResultArgs {
        result_ref: details_ref,
        pointer: "/stage_timings".into(),
        offset: 0,
        limit: 16,
        byte_offset: None,
    });
    assert_eq!(details.data["value"][0]["tool"], "computer.claim");
}

#[test]
fn timing_opt_in_is_removed_before_strict_tool_argument_parsing() {
    let mut args = json!({"target":"t","include_timings":true});
    assert!(take_include_timings(&mut args).unwrap());
    assert!(args.get("include_timings").is_none());
    assert!(serde_json::from_value::<SnapshotArgs>(args).is_ok());

    let mut malformed = json!({"target":"t","include_timings":"yes"});
    assert_eq!(
        take_include_timings(&mut malformed).unwrap_err().data["code"],
        "invalid_arguments"
    );
}

#[test]
fn native_failure_receipts_are_not_hidden_by_successful_rpc_envelopes() {
    assert!(compactable_receipt(&DeviceResult::value(
        json!({"dispatch":"returned"})
    )));
    for data in [
        json!({"dispatch":"uncertain","dispatch_error":{"code":"secure_or_unknown_text_field"},"input_domain_fenced":true}),
        json!({"dispatch":"uncertain"}),
        json!({"dispatch":"returned","dispatch_error":{"code":"native_error"}}),
        json!({"dispatch":"returned","input_domain_fenced":true}),
        json!({"dispatch":"refused"}),
        json!({"code":"unknown_reply_shape"}),
    ] {
        assert!(!compactable_receipt(&DeviceResult::value(data)));
    }
    assert!(!compactable_receipt(&DeviceResult::error(
        "failure",
        "keep evidence"
    )));
}
#[test]
fn typed_keyboard_calls_preserve_unicode_and_exact_element_without_focus_fallback() {
    let text = "你好\n🙂\t".to_string();
    let args = keyboard_action("f".into(), "e".into(), text.clone(), "text", 4096).unwrap();
    assert_eq!(args["value"], text);
    assert_eq!(args["element"], "e");
    assert_eq!(args["intent"], "text");
    assert!(args.get("focus").is_none());
    assert!(keyboard_action("f".into(), "e".into(), "x".repeat(4097), "text", 4096).is_err());
    assert!(keyboard_action("f".into(), "".into(), "cmd+s".into(), "key_chord", 128).is_err());
    assert!(
        serde_json::from_value::<KeyArgs>(
            json!({"target":"t","frame":"f","element":"e","request_id":"r","key":"s","shell":"no"})
        )
        .is_err()
    );
}
#[test]
fn default_collapses_noise_but_retains_full_count_and_preferred_order() {
    let rows = vec![
        json!({"id":"noise1","role":"AXRow","name":"","actions":["AXShowAlternateUI"],"visible":false}),
        json!({"id":"normal1","role":"AXButton","name":"","actions":["AXPress"]}),
        json!({"id":"noise2","role":"AXToolbar","actions":["AXShowMenu"],"occluded":true}),
        json!({"id":"normal2","role":"AXRow","name":"Selected font","actions":["AXShowDefaultUI"],"visible":false,"selected":true}),
    ];
    let (out, total) = compact_controls(&rows, 128);
    assert_eq!(total, 4);
    assert_eq!(
        out.iter()
            .map(|e| e["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["normal1", "normal2"]
    );
    assert_eq!(out[1]["selected"], true);
    let (original, _) = compact_controls_in_order(&rows, 128);
    assert_eq!(original.len(), 4);
    assert_eq!(original[2]["actions"], json!(["AXShowMenu"]));
}
#[test]
fn covered_semantic_button_and_unknown_visibility_are_not_dropped_or_demoted() {
    let rows = vec![
        json!({"id":"covered","role":"AXButton","name":"","actions":["AXPress"],"visible":false,"occluded":true,"enabled":false,"point":[4,8]}),
        json!({"id":"unknown","role":"AXRow","name":"","actions":["AXShowDefaultUI"]}),
        json!({"id":"empty-title","role":"AXRow","description":"Font family","actions":["AXShowAlternateUI"],"visible":false}),
    ];
    let (out, total) = compact_controls(&rows, 128);
    assert_eq!(total, 3);
    assert_eq!(out[0]["id"], "covered");
    assert_eq!(out[0]["enabled"], false);
    assert_eq!(out[0]["visible"], false);
    assert_eq!(out[0]["point"], json!([4, 8]));
    assert_eq!(out[1]["id"], "unknown");
    assert_eq!(out[2]["name"], "Font family");
}
#[test]
fn all_container_or_meaningful_state_tree_is_not_silently_emptied() {
    let rows =
        vec![json!({"id":"row","role":"AXRow","actions":["AXShowDefaultUI"],"visible":false})];
    let (out, total) = compact_controls(&rows, 64);
    assert_eq!(total, 1);
    assert_eq!(out[0]["id"], "row");
    let rows = vec![
        json!({"id":"selected","role":"AXRow","actions":["AXShowDefaultUI"],"visible":false,"selected":true}),
        json!({"id":"valued","role":"AXRow","actions":["AXShowDefaultUI"],"occluded":true,"value":"Arial"}),
        json!({"id":"close","role":"AXButton","actions":["AXPress"],"point":[17,19]}),
    ];
    let (out, total) = compact_controls(&rows, 64);
    assert_eq!(total, 3);
    assert_eq!(out.len(), 3);
    assert_eq!(out[0]["selected"], true);
    assert_eq!(out[1]["value"], "Arial");
}
#[tokio::test]
async fn uncertain_action_is_not_replayed_and_cleanup_is_unconditional() {
    let calls = RefCell::new(Vec::new());
    let result = leased(
        "target",
        "one",
        "computer.act_checked",
        json!({"frame":"f","element":"e","intent":"click"}),
        |tool, args, cleanup| {
            calls.borrow_mut().push((tool, args, cleanup));
            std::future::ready(match tool {
                "computer.claim" => DeviceResult::value(json!({"lease":"l"})),
                "computer.release" => DeviceResult::value(json!({"released":true})),
                _ => DeviceResult::error("native_action_unconfirmed", "unknown"),
            })
        },
    )
    .await;
    assert!(result.is_error);
    assert_eq!(result.data["code"], "native_action_unconfirmed");
    let c = calls.borrow();
    assert_eq!(c.len(), 3);
    assert_eq!(c[1].1["request_id"], "one");
    assert_eq!(c[1].1["lease"], "l");
    assert!(c[2].2);
    assert!(!c[1].2);
}
#[tokio::test]
async fn cleanup_failure_keeps_original_action_evidence() {
    let result = leased("t", "r", "computer.act", json!({}), |tool, _, _| {
        std::future::ready(match tool {
            "computer.claim" => DeviceResult::value(json!({"lease":"l"})),
            "computer.release" => DeviceResult::error("offline", "unknown release"),
            _ => DeviceResult::value(
                json!({"dispatch":"returned","operation_ref":{"id":"original"}}),
            ),
        })
    })
    .await;
    assert!(result.is_error);
    assert_eq!(result.data["action"]["operation_ref"]["id"], "original");
    assert_eq!(result.data["replay_allowed"], false);
}
#[tokio::test]
async fn invalid_request_and_failed_claim_never_dispatch() {
    let result = leased("t", "", "computer.act", json!({}), |_, _, _| async {
        panic!("must not call")
    })
    .await;
    assert_eq!(result.data["code"], "invalid_arguments");
    let n = RefCell::new(0);
    let result = leased("t", "r", "computer.act", json!({}), |tool, _, _| {
        *n.borrow_mut() += 1;
        assert_eq!(tool, "computer.claim");
        std::future::ready(DeviceResult::error("busy", "no lease"))
    })
    .await;
    assert!(result.is_error);
    assert_eq!(*n.borrow(), 1);
}
