//! Explicit, four-field semantic focus. No synthetic AXAction or implicit input.
use super::*;
use rwi_core::semantic_focus::FocusElementRequest;

pub(super) fn focus_element_tool() -> Tool {
    // The common convenience schema adds include_timings; this exact contract must not.
    let mut schema = serde_json::to_value(schemars::schema_for!(FocusElementRequest)).unwrap();
    schema_numbers(&mut schema);
    Tool::new(
        "computer.focus_element",
        "Explicitly focus one exact plain, nonsecure editable control from target/frame/element; stable request_id. A short lease and fresh native checks are internal. At most one AXFocused=true setter (or no write if already focused). No click/action, value mutation, activation request, keys, fallback or retry. Output preserves ready/partial/unknown, exact element/app focused-element/app focused-window readback and OS foreground separately. Readiness is local focus only, not text insertion or preserved global foreground. Unsupported devices refuse; missing/unknown facts are not false or permission.",
        Arc::new(schema.as_object().unwrap().clone()),
    )
    .with_title("Focus exact editable control")
    .with_annotations(ToolAnnotations::new().read_only(false))
}

pub(super) async fn focus_element<F, Fut>(args: FocusElementRequest, call: F) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    if let Err(error) = args.validate() {
        return DeviceResult::error(&error.code, &error.next_step);
    }
    leased(
        &args.target,
        &args.request_id,
        "computer.focus_element",
        json!({"frame":args.frame,"element":args.element}),
        call,
    )
    .await
}

pub(super) fn compact_focus_reply(data: &Value, details_ref: Option<String>) -> Value {
    // Keep the native three axes/statuses as one bounded summary. Never run the
    // generic action projection which intentionally forces goal_verified=false.
    // In particular, explicit null remains unknown; never infer true from setter ack.
    let mut fields = serde_json::Map::new();
    for key in [
        "receipt",
        "focus_readback",
        "grounding_invalidated",
        "deduplicated",
        "operation_ref",
        "operation_id",
        "operation",
        "receipt_ref",
        "native_dispatch_entered",
        "input_domain_fenced",
        "resources_released",
        "code",
        "error",
        "next_step",
        "diagnostic",
        "goal_verified",
        "system_accepted",
        "replay",
        "replay_allowed",
        "cleanup",
        "action",
        "stage_timings",
        "elapsed_ms",
    ] {
        if let Some(value) = data.get(key) {
            fields.insert(key.into(), value.clone());
        }
    }
    fields.insert("details_ref".into(), json!(details_ref));
    Value::Object(fields)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    fn request() -> Value {
        json!({"target":"t", "frame":"f", "element":"e", "request_id":"once"})
    }
    fn result(state: &str, goal: bool) -> Value {
        json!({
            "receipt":{"request_id":"once","dispatch":"returned","system_accepted":true,"goal_verified":goal,"deduplicated":false,"replay":"forbidden"},
            "focus_readback":{"state":state,"requested_write_plan":"set_ax_focused_true_once","write_plan":"set_ax_focused_true_once","setter_attempt":"attempted","setter_acknowledged":true,"setter_status":0,
                "element_focused":{"native_status":0,"value":true},"focused_ui_element_cf_match":{"native_status":0,"value":true},"application_focused_window_cf_match":{"native_status":0,"value":true},
                "os_foreground_before":{"pid":1,"window":"a","process_start":"x","native_epoch":"z"},"os_foreground_after":null,"os_foreground_unchanged":null,"goal_verified":goal},
            "grounding_invalidated":{"scope":"same_application","target_count":2},"operation_ref":"op","native_dispatch_entered":true
        })
    }
    #[test]
    fn semantic_focus_model_schema_is_strict_four_fields_without_hidden_lease() {
        let tools = tools();
        let rows: Vec<_> = tools
            .iter()
            .filter(|t| t.name == "computer.focus_element")
            .collect();
        assert_eq!(rows.len(), 1);
        let schema = serde_json::to_value(&rows[0].input_schema).unwrap();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"].as_object().unwrap().len(), 4);
        assert_eq!(schema["required"].as_array().unwrap().len(), 4);
        assert!(serde_json::from_value::<FocusElementRequest>(request()).is_ok());
        for field in [
            "lease",
            "value",
            "options",
            "fallback",
            "include_timings",
            "intent",
            "operation_id",
        ] {
            let mut extra = request();
            extra[field] = json!(true);
            assert!(
                serde_json::from_value::<FocusElementRequest>(extra).is_err(),
                "{field}"
            );
            assert!(schema["properties"].get(field).is_none(), "{field}");
        }
    }
    #[test]
    fn semantic_focus_timing_seam_accepts_exact_request_and_preserves_other_tools() {
        let mut exact = request();
        assert!(!convenience_timing_option("computer.focus_element", &mut exact).unwrap());
        assert!(serde_json::from_value::<FocusElementRequest>(exact).is_ok());
        let mut other = json!({"include_timings":true});
        assert!(convenience_timing_option("computer.click", &mut other).unwrap());
        assert!(other.get("include_timings").is_none());
    }
    #[test]
    fn semantic_focus_unknown_timing_field_is_not_removed_before_parse() {
        let mut extra = request();
        extra["include_timings"] = json!(true);
        assert!(!convenience_timing_option("computer.focus_element", &mut extra).unwrap());
        assert_eq!(extra["include_timings"], true);
        assert!(serde_json::from_value::<FocusElementRequest>(extra).is_err());
    }
    #[test]
    fn semantic_focus_compact_output_preserves_ready_partial_and_raw_unknown() {
        for (state, goal) in [("ready", true), ("partial", false), ("unknown", false)] {
            let mut data = result(state, goal);
            if state == "unknown" {
                data["focus_readback"]["element_focused"] =
                    json!({"native_status":-25202,"value":null});
                data["focus_readback"]["setter_status"] = Value::Null;
                data["receipt"]["system_accepted"] = Value::Null;
            }
            let projected = compact_focus_reply(&data, Some("details".into()));
            for key in [
                "receipt",
                "focus_readback",
                "grounding_invalidated",
                "operation_ref",
                "native_dispatch_entered",
            ] {
                assert_eq!(projected.get(key), data.get(key), "{state} {key}");
            }
            assert_eq!(projected["receipt"]["goal_verified"], goal);
            assert_eq!(projected["focus_readback"]["state"], state);
            assert!(
                projected.get("goal_verified").is_none(),
                "must not invent duplicate goal"
            );
        }
    }
    #[test]
    fn semantic_focus_unknown_sample_diagnostics_pass_through_compact_unchanged() {
        let mut data = result("unknown", false);
        data["focus_readback"]["element_focused"] = json!({"native_status":0,"value":null});
        data["focus_readback"]["readback_diagnostics"] = json!({
            "version":1,"input_authority":false,"non_atomic_samples":true,
            "element_focused":{"first":{"native_status":0,"value":false},"second":{"native_status":0,"value":true},
                "native_status_changed":false,"value_changed":true},
            "focused_ui_element_cf_match":{"first":{"native_status":0,"value":null,"unknown_cause":"not_ax_element",
                "validation_error_code":"focus_native_type_unknown"},"second":{"native_status":0,"value":null,
                "unknown_cause":"provider_validation_failed","validation_error_code":"delegate_unverified"},
                "native_status_changed":false,"value_changed":false},
            "application_focused_window_cf_match":{"first":{"native_status":-25204,"value":null,
                "unknown_cause":"copy_attribute_failed"},"second":{"native_status":0,"value":null,
                "unknown_cause":"copy_returned_null"},"native_status_changed":true,"value_changed":false}
        });
        let projected = compact_focus_reply(&data, None);
        assert_eq!(projected["focus_readback"], data["focus_readback"]);
        assert_eq!(projected["receipt"]["goal_verified"], false);
        assert!(projected.get("readback_diagnostics").is_none());
        assert_eq!(
            serde_json::to_vec(&projected["focus_readback"]).unwrap(),
            serde_json::to_vec(&data["focus_readback"]).unwrap()
        );
    }
    #[tokio::test]
    async fn semantic_focus_unknown_diagnostics_do_not_add_rpc_or_request_fields() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let log = calls.clone();
        let mut unknown = result("unknown", false);
        unknown["focus_readback"]["readback_diagnostics"] = json!({"version":1,"input_authority":false,
            "non_atomic_samples":true,"element_focused":{"first":{"native_status":0,"value":false},
            "second":{"native_status":0,"value":true},"native_status_changed":false,"value_changed":true}});
        let reply = unknown.clone();
        let output = focus_element(
            serde_json::from_value(request()).unwrap(),
            move |tool, args, cleanup| {
                log.lock().unwrap().push((tool, args, cleanup));
                let reply = reply.clone();
                async move {
                    match tool {
                        "computer.claim" => DeviceResult::value(json!({"lease":"internal"})),
                        "computer.focus_element" => DeviceResult::value(reply),
                        "computer.release" => DeviceResult::value(json!({"released":true})),
                        _ => panic!("diagnostics must not dispatch follow-up input"),
                    }
                }
            },
        )
        .await;
        assert_eq!(output.data, unknown);
        let calls = calls.lock().unwrap();
        assert_eq!(
            calls.iter().map(|r| r.0).collect::<Vec<_>>(),
            [
                "computer.claim",
                "computer.focus_element",
                "computer.release"
            ]
        );
        assert_eq!(
            calls[1].1,
            json!({"target":"t","frame":"f","element":"e","request_id":"once","lease":"internal"})
        );
        assert!(calls[2].2);
        let mut extra = request();
        extra["readback_diagnostics"] = json!({});
        assert!(serde_json::from_value::<FocusElementRequest>(extra).is_err());
    }
    #[test]
    fn semantic_focus_no_write_and_foreground_axis_are_not_laundered() {
        let mut data = result("ready", true);
        data["focus_readback"]["write_plan"] = json!("no_write_already_focused");
        data["focus_readback"]["setter_attempt"] = json!("not_attempted");
        data["focus_readback"]["setter_acknowledged"] = Value::Null;
        data["focus_readback"]["setter_status"] = Value::Null;
        data["focus_readback"]["os_foreground_unchanged"] = json!(false);
        let output = compact_focus_reply(&data, None);
        assert_eq!(output["focus_readback"], data["focus_readback"]);
        assert_eq!(output["receipt"]["goal_verified"], true);
    }
    #[test]
    fn semantic_focus_cleanup_errors_and_native_diagnostics_remain_visible() {
        let data = json!({"code":"input_cleanup_unconfirmed","action":result("partial",false),"cleanup":{"code":"release_uncertain"},"goal_verified":false,"replay_allowed":false});
        let projected = compact_focus_reply(&data, None);
        for key in [
            "code",
            "action",
            "cleanup",
            "goal_verified",
            "replay_allowed",
        ] {
            assert_eq!(projected.get(key), data.get(key));
        }
        let error = json!({"code":"capability_unavailable","diagnostic":{"api":"AXUIElementSetAttributeValue","attribute":"AXFocused","native_status":-25202,"held_input_resources":false},"next_step":"Do not replay."});
        let output = compact_focus_reply(&error, None);
        assert_eq!(output["diagnostic"], error["diagnostic"]);
        assert!(output.get("focus_readback").is_none());
    }
    #[tokio::test]
    async fn semantic_focus_calls_one_device_mutation_and_releases_without_followup_input() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let log = calls.clone();
        let ready = result("ready", true);
        let returned = ready.clone();
        let output = focus_element(
            serde_json::from_value(request()).unwrap(),
            move |tool, args, cleanup| {
                log.lock().unwrap().push((tool, args, cleanup));
                let ready = returned.clone();
                async move {
                    match tool {
                        "computer.claim" => DeviceResult::value(json!({"lease":"internal"})),
                        "computer.focus_element" => DeviceResult::value(ready),
                        "computer.release" => DeviceResult::value(json!({"released":true})),
                        _ => panic!("unexpected route"),
                    }
                }
            },
        )
        .await;
        assert!(!output.is_error);
        assert_eq!(output.data, ready);
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(
            calls.iter().map(|r| r.0).collect::<Vec<_>>(),
            vec![
                "computer.claim",
                "computer.focus_element",
                "computer.release"
            ]
        );
        assert_eq!(
            calls[1].1,
            json!({"target":"t","frame":"f","element":"e","request_id":"once","lease":"internal"})
        );
        assert!(calls[2].2);
    }
    #[tokio::test]
    async fn semantic_focus_invalid_request_does_not_acquire_a_lease() {
        for field in ["target", "frame", "element", "request_id"] {
            let mut input = request();
            input[field] = json!("");
            let output = focus_element(serde_json::from_value(input).unwrap(), |_, _, _| async {
                panic!("invalid request reached RPC")
            })
            .await;
            assert!(output.is_error, "{field}");
        }
    }
    #[tokio::test]
    async fn semantic_focus_device_refusal_is_not_retried_and_release_still_runs() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let log = calls.clone();
        let output = focus_element(
            serde_json::from_value(request()).unwrap(),
            move |tool, _, cleanup| {
                log.lock().unwrap().push((tool, cleanup));
                async move {
                    match tool {
                        "computer.claim" => DeviceResult::value(json!({"lease":"internal"})),
                        "computer.focus_element" => DeviceResult::error(
                            "capability_unavailable",
                            "No native support; no replay.",
                        ),
                        "computer.release" => DeviceResult::value(json!({"released":true})),
                        _ => panic!("unexpected route"),
                    }
                }
            },
        )
        .await;
        assert!(output.is_error);
        assert_eq!(output.data["code"], "capability_unavailable");
        assert_eq!(
            calls
                .lock()
                .unwrap()
                .iter()
                .map(|r| r.0)
                .collect::<Vec<_>>(),
            vec![
                "computer.claim",
                "computer.focus_element",
                "computer.release"
            ]
        );
    }
}
