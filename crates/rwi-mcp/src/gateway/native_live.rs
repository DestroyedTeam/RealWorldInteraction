//! Explicit native presentation, not a substitute for Host visibility evidence.
use super::*;

pub(super) const TOOL: &str = "computer.show_native_live";
const PRESENTATION: &str = "RWI_Agent_native_nonactivating_panel";

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Args {
    target: String,
    options: rwi_core::LiveOptions,
}

pub(super) fn tool() -> Tool {
    let mut schema = serde_json::to_value(schemars::schema_for!(Args)).unwrap();
    schema_numbers(&mut schema);
    Tool::new(TOOL, "Start one independent native Live overlay for an explicitly selected target and viewer display. options.display is required; bounded duration 1..600s. No MCP Apps, browser tab, Host presence or active-chat inference. Changing chats does not hide it. Native permissions, source/session identity and lifetime remain authoritative; context close/expiry ends it. Use computer.live status/hide/resume/stop, or its native controls. Stop an existing preview before changing modes. No input, focus, automatic restart or fallback; independently verify real frames and controls. Unsupported device backends refuse.", Arc::new(schema.as_object().unwrap().clone()))
        .with_title("Show independent native Live overlay")
        .with_annotations(ToolAnnotations::new().read_only(false).destructive(false))
}

/// Both Live entry points share a catalog boundary, not a visibility policy.
pub(super) fn tools(ui_supported: bool) -> Vec<Tool> {
    let mut tools = vec![tool()];
    if ui_supported {
        tools.extend(live_ui::tools());
    }
    tools
}

fn request(mut args: Value) -> Result<Value, DeviceResult> {
    // Reject even null/false private fields before deserialization erases them.
    if args.get("view").is_some()
        || args.pointer("/options/host_view_id").is_some()
        || args.pointer("/options/host_image_only").is_some()
    {
        return Err(DeviceResult::error(
            "host_view_reserved",
            "Do not supply Host visibility or private presentation fields.",
        ));
    }
    numeric_inputs(&mut args);
    let args: Args = serde_json::from_value(args).map_err(|_| {
        DeviceResult::error("invalid_native_live_start", "Supply an exact target and options with an explicit viewer display; no action or visibility fields.")
    })?;
    if args.target.is_empty() {
        return Err(DeviceResult::error(
            "invalid_native_live_start",
            "Select an exact target first.",
        ));
    }
    args.options
        .validate()
        .map_err(|e| DeviceResult::error(&e.code, &e.next_step))?;
    Ok(json!({"target":args.target,"action":"start","options":args.options}))
}

fn confirmed(mut result: DeviceResult) -> DeviceResult {
    if result.is_error {
        return result;
    }
    if !result.data.is_object() {
        result.is_error = true;
        result.data = json!({"code":"native_live_presentation_unconfirmed","native_start_result":result.data,"message":"Native start returned an unknown shape; inspect status without replaying start."});
        return result;
    }
    let code = if result.data.get("start").is_some() {
        Some("live_presentation_restart_required")
    } else if result.data.get("host_view_binding") != Some(&Value::Null)
        || result.data.get("host_view_lease_ms") != Some(&Value::Null)
        || result.data["presentation"] != PRESENTATION
        || result.data["ownership"] != "this_MCP_device_context"
        || result.data["session_id"]
            .as_str()
            .is_none_or(|id| id.is_empty())
        || !matches!(
            result.data["status"].as_str(),
            Some("running" | "recovering")
        )
    {
        Some("native_live_presentation_unconfirmed")
    } else {
        None
    };
    if let Some(code) = code {
        // Keep the original result: start may already have had an effect.
        result.is_error = true;
        if result.data.get("code").is_none() {
            result.data["code"] = json!(code);
        }
        if result.data.get("message").is_none() {
            result.data["message"] = json!(
                "Independent native presentation was not confirmed. Inspect computer.live status; explicitly stop the prior preview before changing modes. No start/stop/presence was replayed."
            );
        }
    }
    result
}

impl Gateway {
    pub(super) async fn independent_native_live(
        &self,
        args: Value,
        context: Option<RequestContext<RoleServer>>,
    ) -> DeviceResult {
        let args = match request(args) {
            Ok(args) => args,
            Err(error) => return error,
        };
        // Shared call_native admission precedes this path. Device invocation
        // still enforces EOF/cancellation, context, authorization and no replay.
        // Deliberately do not register or fabricate a Host presence binding.
        confirmed(self.invoke_device("computer.live", args, context).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args() -> Value {
        json!({"target":"own-window","options":{"display":{"id":4},"duration_ms":60000}})
    }
    fn native() -> Value {
        json!({"session_id":"live:fixture","status":"running","ownership":"this_MCP_device_context","presentation":PRESENTATION,"host_view_binding":null,"host_view_lease_ms":null,"presenter_visible":false,"frames_captured":0,"input_sent":false,"operation_ref":{"retained":true}})
    }
    #[test]
    fn independent_native_live_explicit_display_and_no_private_fields() {
        let r = request(args()).unwrap();
        assert_eq!(r["action"], "start");
        assert_eq!(r["options"]["display"]["id"], 4);
        assert!(r["options"].get("host_view_id").is_none());
        assert!(r["options"].get("host_image_only").is_none());
        assert!(r.get("view").is_none());
        let schema = serde_json::to_value(tool()).unwrap();
        assert!(schema["_meta"].is_null());
        assert!(
            schema["inputSchema"]["$defs"]["LiveOptions"]["required"]
                .as_array()
                .unwrap()
                .contains(&json!("display"))
        );
    }
    #[test]
    fn independent_native_live_rejects_forged_presence_and_unbounded_options() {
        for key in ["host_view_id", "host_image_only"] {
            for value in [Value::Null, json!(false), json!("a".repeat(32))] {
                let mut a = args();
                a["options"][key] = value;
                assert_eq!(request(a).unwrap_err().data["code"], "host_view_reserved");
            }
        }
        for options in [
            json!({}),
            json!({"display":null}),
            json!({"display":{"id":4},"duration_ms":600001}),
            json!({"display":{"id":4},"interval_ms":0}),
        ] {
            assert!(request(json!({"target":"t","options":options})).is_err());
        }
        let mut a = args();
        a["view"] = Value::Null;
        assert!(request(a).is_err());
    }
    #[test]
    fn independent_native_live_confirmation_is_not_pixels_or_goal() {
        let data = native();
        let r = confirmed(DeviceResult::value(data.clone()));
        assert!(!r.is_error);
        assert_eq!(r.data, data);
        assert_eq!(r.data["presenter_visible"], false);
        assert_eq!(r.data["frames_captured"], 0);
    }
    #[test]
    fn independent_native_live_retains_unknown_and_existing_session_results() {
        let mut cases = vec![json!({})];
        for (key, value) in [
            ("host_view_binding", json!("old-view")),
            ("host_view_lease_ms", json!(1500)),
            (
                "presentation",
                json!("Host_component_cached_image; Host_render_unattested"),
            ),
            ("ownership", Value::Null),
            ("session_id", json!("")),
            ("status", json!("stopped")),
        ] {
            let mut data = native();
            data[key] = value;
            cases.push(data);
        }
        for data in cases {
            let r = confirmed(DeviceResult::value(data.clone()));
            assert!(r.is_error);
            for (k, v) in data.as_object().unwrap() {
                assert_eq!(&r.data[k], v);
            }
        }
        let mut data = native();
        data["start"] = json!("already_running; stop before reconfiguring");
        let r = confirmed(DeviceResult::value(data.clone()));
        assert!(r.is_error);
        assert_eq!(r.data["code"], "live_presentation_restart_required");
        assert_eq!(r.data["start"], data["start"]);
        let mut data = native();
        data.as_object_mut().unwrap().remove("host_view_binding");
        assert!(confirmed(DeviceResult::value(data)).is_error);
        let original = DeviceResult::error("original_native_error", "Keep this.");
        let data = original.data.clone();
        assert_eq!(confirmed(original).data, data);
        for data in [Value::Null, json!([]), json!(false), json!("unknown")] {
            let r = confirmed(DeviceResult::value(data.clone()));
            assert!(r.is_error);
            assert_eq!(r.data["native_start_result"], data);
        }
    }
    #[tokio::test]
    async fn independent_native_live_keeps_shutdown_admission() {
        let gateway = Gateway::new(Ok(AdapterProfile::default()));
        gateway.shutdown.send_replace(true);
        let r = gateway.invoke(TOOL, args(), None).await;
        assert!(r.is_error);
        assert_eq!(r.data["code"], "host_disconnected");
    }
}
