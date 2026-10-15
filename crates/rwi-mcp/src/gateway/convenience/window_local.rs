//! Strict explicit app-local window selection; never element focus or typing.
use super::*;
use rwi_core::window_local::WindowLocalRequest;

pub(super) fn window_local_tool() -> Tool {
    let mut schema = serde_json::to_value(schemars::schema_for!(WindowLocalRequest)).unwrap();
    schema_numbers(&mut schema);
    Tool::new(
        "computer.select_window_local",
        "Select one exact ordinary macOS visual window as application-local main or key. Four fields target/frame/kind(main|key)/request_id; fresh native checks and short lease are internal. At most one requested AXMain or window AXFocused setter, or no write if already selected. No AXRaise/frontmost/activation, click, keys, element focus, value write, automatic typing, fallback or retry. Main is not key or element Ready. Returns selected/partial/unknown with raw window/application CF axes and OS foreground separately; unsupported kinds/providers refuse. Typing remains unauthorized.",
        Arc::new(schema.as_object().unwrap().clone()),
    )
    .with_title("Select app-local main/key window")
    .with_annotations(ToolAnnotations::new().read_only(false).destructive(false))
}
pub(super) async fn select_window_local<F, Fut>(args: WindowLocalRequest, call: F) -> DeviceResult
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
        "computer.select_window_local",
        json!({"frame":args.frame,"kind":args.kind}),
        call,
    )
    .await
}
pub(super) fn compact_window_local_reply(data: &Value, details_ref: Option<String>) -> Value {
    // Retain all four raw window axes, nulls, requested/actual budget and OS
    // foreground. A selected Main receipt never turns into element readiness.
    let mut fields = serde_json::Map::new();
    for key in [
        "receipt",
        "window_readback",
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
        json!({"target":"t","frame":"f","kind":"main","request_id":"once"})
    }
    fn result(state: &str, goal: bool) -> Value {
        json!({"receipt":{"request_id":"once","dispatch":"returned","system_accepted":true,"goal_verified":goal,"deduplicated":false,"replay":"forbidden"},
            "window_readback":{"kind":"main","state":state,"requested_write_plan":"set_attribute_true_once","write_plan":"set_attribute_true_once","setter_attempt":"attempted","setter_status":0,"setter_acknowledged":true,
                "window_main":{"native_status":0,"value":true},"application_main_window_cf_match":{"native_status":0,"value":true},
                "window_focused":{"native_status":0,"value":false},"application_focused_window_cf_match":{"native_status":0,"value":false},
                "os_foreground_before":{"pid":1,"window":"a","process_start":"x","native_epoch":"z"},"os_foreground_after":null,"os_foreground_unchanged":null,
                "selected_goal_verified":goal,"typing_authorized":false,"semantic_element_ready":null},
            "grounding_invalidated":{"scope":"same_application","target_count":2},"native_dispatch_entered":true,"operation_ref":"op"})
    }
    #[test]
    fn window_local_model_schema_is_exact_four_fields_and_not_device_lease_schema() {
        let tools = tools();
        let rows: Vec<_> = tools
            .iter()
            .filter(|t| t.name == "computer.select_window_local")
            .collect();
        assert_eq!(rows.len(), 1);
        let schema = serde_json::to_value(&rows[0].input_schema).unwrap();
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["properties"].as_object().unwrap().len(), 4);
        assert_eq!(schema["required"].as_array().unwrap().len(), 4);
        for f in ["target", "frame", "kind", "request_id"] {
            let mut v = request();
            v.as_object_mut().unwrap().remove(f);
            assert!(serde_json::from_value::<WindowLocalRequest>(v).is_err());
        }
        for f in [
            "lease",
            "element",
            "value",
            "options",
            "fallback",
            "include_timings",
            "intent",
            "expected_foreground",
            "operation_id",
        ] {
            let mut v = request();
            v[f] = json!(true);
            assert!(
                serde_json::from_value::<WindowLocalRequest>(v).is_err(),
                "{f}"
            );
            assert!(schema["properties"].get(f).is_none(), "{f}");
        }
        for kind in [json!("main"), json!("key")] {
            let mut v = request();
            v["kind"] = kind;
            assert!(serde_json::from_value::<WindowLocalRequest>(v).is_ok());
        }
        for kind in [
            json!("focused"),
            json!("Main"),
            json!(""),
            Value::Null,
            json!(true),
        ] {
            let mut v = request();
            v["kind"] = kind;
            assert!(serde_json::from_value::<WindowLocalRequest>(v).is_err());
        }
    }
    #[test]
    fn window_local_unknown_timing_field_survives_until_strict_parse() {
        let mut v = request();
        v["include_timings"] = json!(true);
        assert!(!convenience_timing_option("computer.select_window_local", &mut v).unwrap());
        assert_eq!(v["include_timings"], true);
        assert!(serde_json::from_value::<WindowLocalRequest>(v).is_err());
        let mut focus = json!({"target":"t","frame":"f","element":"e","request_id":"once"});
        assert!(!convenience_timing_option("computer.focus_element", &mut focus).unwrap());
        assert!(
            serde_json::from_value::<rwi_core::semantic_focus::FocusElementRequest>(focus).is_ok()
        );
    }
    #[test]
    fn window_local_compact_output_preserves_kind_goal_raw_axes_and_unknowns() {
        for (state, goal) in [("selected", true), ("partial", false), ("unknown", false)] {
            let mut data = result(state, goal);
            if state == "unknown" {
                data["window_readback"]["window_main"] =
                    json!({"native_status":-25202,"value":null});
                data["window_readback"]["setter_status"] = Value::Null;
                data["receipt"]["system_accepted"] = Value::Null;
            }
            let out = compact_window_local_reply(&data, Some("details".into()));
            assert_eq!(out["receipt"], data["receipt"]);
            assert_eq!(out["window_readback"], data["window_readback"]);
            assert_eq!(out["window_readback"]["selected_goal_verified"], goal);
            assert_eq!(out["window_readback"]["typing_authorized"], false);
            assert_eq!(
                out["window_readback"]["semantic_element_ready"],
                Value::Null
            );
            assert!(out.get("focus_readback").is_none());
            assert!(out.get("goal_verified").is_none());
        }
    }
    #[test]
    fn window_local_main_success_does_not_hide_key_or_global_foreground_change() {
        let mut data = result("selected", true);
        data["window_readback"]["os_foreground_unchanged"] = json!(false);
        data["window_readback"]["write_plan"] = json!("no_write_already_selected");
        data["window_readback"]["setter_attempt"] = json!("not_attempted");
        data["window_readback"]["setter_status"] = Value::Null;
        let out = compact_window_local_reply(&data, None);
        assert_eq!(out["window_readback"], data["window_readback"]);
        assert_eq!(out["window_readback"]["window_focused"]["value"], false);
        assert_eq!(out["receipt"]["goal_verified"], true);
        assert_eq!(out["window_readback"]["typing_authorized"], false);
    }
    #[test]
    fn window_local_cleanup_and_native_errors_keep_original_receipts() {
        let v = json!({"code":"input_cleanup_unconfirmed","action":result("partial",false),"cleanup":{"code":"release_uncertain"},"goal_verified":false,"replay_allowed":false});
        let out = compact_window_local_reply(&v, None);
        for key in [
            "code",
            "action",
            "cleanup",
            "goal_verified",
            "replay_allowed",
        ] {
            assert_eq!(out.get(key), v.get(key));
        }
        let v = json!({"code":"window_local_kind_unsupported","diagnostic":{"kind":"key","attribute":"AXFocused","native_status":0,"writable":false},"next_step":"No Main fallback."});
        assert_eq!(
            compact_window_local_reply(&v, None)["diagnostic"],
            v["diagnostic"]
        );
    }
    #[tokio::test]
    async fn window_local_calls_one_mutation_and_release_without_focus_typing_or_fallback() {
        for kind in ["main", "key"] {
            let mut v = request();
            v["kind"] = json!(kind);
            let calls = Arc::new(Mutex::new(Vec::new()));
            let log = calls.clone();
            let selected = result("selected", true);
            let returned = selected.clone();
            let out = select_window_local(
                serde_json::from_value(v).unwrap(),
                move |tool, args, cleanup| {
                    log.lock().unwrap().push((tool, args, cleanup));
                    let selected = returned.clone();
                    async move {
                        match tool {
                            "computer.claim" => DeviceResult::value(json!({"lease":"internal"})),
                            "computer.select_window_local" => DeviceResult::value(selected),
                            "computer.release" => DeviceResult::value(json!({"released":true})),
                            _ => panic!("unexpected implicit action"),
                        }
                    }
                },
            )
            .await;
            assert!(!out.is_error);
            assert_eq!(out.data, selected);
            let calls = calls.lock().unwrap();
            assert_eq!(
                calls.iter().map(|c| c.0).collect::<Vec<_>>(),
                vec![
                    "computer.claim",
                    "computer.select_window_local",
                    "computer.release"
                ]
            );
            assert_eq!(
                calls[1].1,
                json!({"target":"t","frame":"f","kind":kind,"request_id":"once","lease":"internal"})
            );
            assert!(calls[2].2);
        }
    }
    #[tokio::test]
    async fn window_local_invalid_request_never_acquires_lease() {
        for f in ["target", "frame", "request_id"] {
            let mut v = request();
            v[f] = json!("");
            let out = select_window_local(serde_json::from_value(v).unwrap(), |_, _, _| async {
                panic!("invalid request reached RPC")
            })
            .await;
            assert!(out.is_error);
        }
    }
    #[tokio::test]
    async fn window_local_kind_refusal_is_not_retried_or_replaced_by_main() {
        let mut v = request();
        v["kind"] = json!("key");
        let calls = Arc::new(Mutex::new(Vec::new()));
        let log = calls.clone();
        let out = select_window_local(
            serde_json::from_value(v).unwrap(),
            move |tool, args, cleanup| {
                log.lock().unwrap().push((tool, args, cleanup));
                async move {
                    match tool {
                        "computer.claim" => DeviceResult::value(json!({"lease":"internal"})),
                        "computer.select_window_local" => DeviceResult::error(
                            "window_local_kind_unsupported",
                            "Key not writable; no fallback.",
                        ),
                        "computer.release" => DeviceResult::value(json!({"released":true})),
                        _ => panic!("unexpected retry"),
                    }
                }
            },
        )
        .await;
        assert!(out.is_error);
        assert_eq!(out.data["code"], "window_local_kind_unsupported");
        let calls = calls.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[1].1["kind"], "key");
        assert!(calls[2].2);
    }
}
