//! Optional MCP Apps presenter. Native capture/input authority stays on Agent.
//! UI presence is evidence about the rendered component, not an active-task API.
use super::*;

pub(super) const EXTENSION: &str = "io.modelcontextprotocol/ui";
pub(super) const MIME: &str = "text/html;profile=mcp-app";
pub(super) const URI: &str = "ui://real-world-interaction/live/v3.html";
const LEGACY_URI: &str = "ui://real-world-interaction/live/v1.html";
const PREVIOUS_URI: &str = "ui://real-world-interaction/live/v2.html";
pub(super) const PRESENCE_TOOL: &str = "computer.live_panel";
pub(super) const START_TOOL: &str = "computer.show_live";

fn supports(capabilities: &ClientCapabilities) -> bool {
    capabilities
        .extensions
        .as_ref()
        .and_then(|e| e.get(EXTENSION))
        .and_then(|e| e.get("mimeTypes"))
        .and_then(Value::as_array)
        .is_some_and(|types| types.iter().any(|value| value == MIME))
}
pub(super) fn negotiated(context: &RequestContext<RoleServer>) -> bool {
    context
        .peer
        .peer_info()
        .is_some_and(|info| supports(&info.capabilities))
}
fn meta(value: Value) -> Meta {
    Meta(value.as_object().unwrap().clone())
}
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct StartArgs {
    target: String,
    options: rwi_core::LiveOptions,
}
pub(super) fn start_tool() -> Tool {
    Tool::new(START_TOOL, "Start a bounded Live preview and render its conversation component on a Host that negotiated MCP Apps HTML. Supply an already selected window target and an explicit viewer display in options; native authorization remains authoritative. No independent browser tab required by this route. Use computer.live for status/hide/resume/stop so control calls do not recreate the component. A returned resource is not proof of visible UI or task-switch binding.",
        Arc::new(serde_json::to_value(schemars::schema_for!(StartArgs)).unwrap().as_object().unwrap().clone()))
        .with_title("Show conversation Live preview")
        .with_annotations(ToolAnnotations::new().read_only(false).destructive(false))
        .with_meta(meta(json!({"ui":{"resourceUri":URI}})))
}
pub(super) fn resource() -> Resource {
    RawResource::new(URI, "rwi_live_panel").with_title("RWI Live · 会话内控制")
        .with_description("Optional Host-rendered Live visibility and hide/close controls; no screen pixels, input, credentials or conversation text in this resource.")
        .with_mime_type(MIME).no_annotation()
}
pub(super) fn read(uri: &str) -> Result<ReadResourceResult, ErrorData> {
    if uri != URI && uri != LEGACY_URI && uri != PREVIOUS_URI {
        return Err(ErrorData::invalid_params("Unknown RWI UI resource", None));
    }
    // Declare the mode before loading. Inline cards can be collapsed without
    // mounting, leaving native presence correctly hidden. This is only a Host
    // presentation request: it never supplies visibility or capture authority.
    Ok(ReadResourceResult::new(vec![ResourceContents::text(include_str!("live_ui.html"), uri)
        .with_mime_type(MIME).with_meta(meta(json!({"ui":{"prefersBorder":false,"csp":{"connectDomains":[],"resourceDomains":[],"frameDomains":[]}},"openai/ui":{"availableDisplayModes":["fullscreen"]}}))) ]))
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct PresenceArgs {
    view_key: String,
    page: String,
    sequence: u64,
    visible: bool,
    #[serde(default)]
    close: bool,
}
fn valid_token(token: &str) -> bool {
    token.len() == 32
        && token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl PresenceArgs {
    fn valid(&self) -> bool {
        valid_token(&self.view_key)
            && valid_token(&self.page)
            && self.sequence > 0
            && self.sequence <= 9_007_199_254_740_991
            && !(self.close && self.visible)
    }
}
pub(super) fn presence_tool() -> Tool {
    Tool::new(PRESENCE_TOOL,
        "UI-only owned Live presence/hide/close. Requires negotiated MCP Apps and exact ephemeral view/page capability from tool-result _meta. Cannot select a device/window, capture pixels, send input, focus or grant permission. Expired/replaced/out-of-order events refuse; never retry close after uncertainty.",
        Arc::new(serde_json::to_value(schemars::schema_for!(PresenceArgs)).unwrap().as_object().unwrap().clone()))
        .with_title("Update owned Live panel")
        .with_annotations(ToolAnnotations::new().read_only(false).destructive(false))
        .with_meta(meta(json!({"ui":{"visibility":["app"]}})))
}
impl Gateway {
    pub(super) async fn start_live_app(
        &self,
        args: serde_json::Map<String, Value>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        if !negotiated(&context) {
            return mcp_result(DeviceResult::error(
                "live_ui_not_negotiated",
                "Host has not declared MCP Apps HTML support; use computer.live and its explicit viewer result. No Live was started.",
            ));
        }
        let args = match serde_json::from_value::<StartArgs>(Value::Object(args)) {
            Ok(args) => args,
            Err(_) => {
                return mcp_result(DeviceResult::error(
                    "invalid_live_ui_start",
                    "Supply an already selected target and explicit viewer options, without action or visibility fields.",
                ));
            }
        };
        let _permit = match self.inflight.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                return mcp_result(DeviceResult::error(
                    "adapter_busy",
                    "No Live start was forwarded.",
                ));
            }
        };
        let mut result = self
            .invoke(
                "computer.live",
                json!({"target":args.target,"action":"start","options":args.options}),
                Some(context),
            )
            .await;
        let metadata = self.prepare_live_app(&mut result, &args.target).await;
        let mut reply = mcp_result(result);
        reply.meta = metadata;
        reply
    }
    pub(super) async fn prepare_live_app(
        &self,
        result: &mut DeviceResult,
        target: &str,
    ) -> Option<Meta> {
        if result.is_error {
            return None;
        }
        let Some((key, page, hidden)) = self.presence.app_page(target) else {
            *result = DeviceResult::error(
                "live_view_ended",
                "Live binding ended before UI preparation; no component visibility was inferred.",
            );
            return None;
        };
        // A replacement component invalidates the old page before receiving
        // any tool result. Clear native preview; missing UI must stay hidden.
        let cleared = self.publish_view(target, hidden).await;
        if cleared.is_error {
            let start = result.data.clone();
            self.presence.remove_target(target);
            *result = DeviceResult::error(
                "live_ui_preparation_failed",
                "Native start returned before UI preparation failed. No component was marked visible; inspect retained start and original presence error, do not replay start.",
            );
            result.data["live_start_result"] = start;
            result.data["presence_update_error"] = cleared.data;
            return None;
        }
        result.data["viewer"] = json!({"resource_uri":URI,"open_with":"MCP Apps-capable Host conversation component","binding":"rendered component visibility, not MCP connection or SessionEnd; actual task-switch behavior requires Host acceptance","unavailable_signal":"hide and clear when the component presence lease expires"});
        Some(meta(
            json!({"rwi/live_panel":{"view_key":key,"page":page,"adapter_instance":self.instance}}),
        ))
    }
    // Diagnostic only. This UUID neither selects targets nor grants authority.
    fn rejected_app_event(&self, reason: &str) -> DeviceResult {
        let mut result = DeviceResult::error("live_ui_event_rejected", reason);
        result.data["adapter_instance"] = json!(self.instance);
        result
    }
    pub(super) async fn app_live_presence(
        &self,
        args: serde_json::Map<String, Value>,
        context: &RequestContext<RoleServer>,
    ) -> DeviceResult {
        if !negotiated(context) {
            return DeviceResult::error(
                "live_ui_not_negotiated",
                "Host has not declared MCP Apps HTML support; no presence was published.",
            );
        }
        let args = match serde_json::from_value::<PresenceArgs>(Value::Object(args)) {
            Ok(args) if args.valid() => args,
            _ => {
                return DeviceResult::error(
                    "invalid_live_ui_event",
                    "Use the owned component capability and a positive, monotonic safe-integer sequence; close must be hidden.",
                );
            }
        };
        let _permit = match self.inflight.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                return DeviceResult::error(
                    "adapter_busy",
                    "No presence was forwarded; native presence lease will expire.",
                );
            }
        };
        let (target, update) =
            match self
                .presence
                .app_event(&args.view_key, args.page, args.sequence, args.visible)
            {
                Ok(value) => value,
                Err(reason) => return self.rejected_app_event(reason),
            };
        if args.close {
            // Revoke even if remote stop is uncertain. Old views cannot renew
            // capture. Do not replay stop or turn a receipt into proof of closure.
            self.presence.remove_target(&target);
            let mut result = self
                .invoke_device(
                    "computer.live",
                    json!({"target":target,"action":"stop"}),
                    None,
                )
                .await;
            result.data["view_revoked"] = json!(true);
            return result;
        }
        self.publish_view(&target, update).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn apps_support_is_explicit_and_html_specific() {
        for (value, expected) in [
            (json!({}), false),
            (
                json!({"experimental":{EXTENSION:{"mimeTypes":[MIME]}}}),
                false,
            ),
            (
                json!({"extensions":{EXTENSION:{"mimeTypes":["image/png"]}}}),
                false,
            ),
            (json!({"extensions":{EXTENSION:{"mimeTypes":[MIME]}}}), true),
        ] {
            assert_eq!(supports(&serde_json::from_value(value).unwrap()), expected);
        }
    }
    #[test]
    fn component_events_cannot_select_targets_or_replay_old_pages() {
        let views = presence::Views::default();
        // Allocate only registry state; no Agent, native window or input.
        let (key, page, _) = views.test_app_page("fixture:target");
        let valid = PresenceArgs {
            view_key: key.clone(),
            page: page.clone(),
            sequence: 1,
            visible: true,
            close: false,
        };
        assert!(valid.valid());
        assert!(views.app_event(&key, page.clone(), 1, true).is_ok());
        assert!(views.app_event(&key, page.clone(), 1, false).is_err());
        let (_, newer, hidden) = views.app_page("fixture:target").unwrap();
        assert!(!hidden.visible);
        assert!(views.app_event(&key, page, 2, true).is_err());
        assert!(views.app_event(&key, newer, 1, false).is_ok());
        assert!(serde_json::from_value::<PresenceArgs>(json!({"view_key":key,"page":"a".repeat(32),"sequence":2,"visible":true,"target":"other"})).is_err());
        assert!(
            !PresenceArgs {
                close: true,
                ..valid
            }
            .valid()
        );
    }
    #[test]
    fn component_owner_diagnostics_do_not_delegate_foreign_view_authority() {
        let owner = Gateway::new(Ok(AdapterProfile::default()));
        let other = Gateway::new(Ok(AdapterProfile::default()));
        assert_ne!(owner.instance, other.instance);
        assert_eq!(owner.instance, owner.clone().instance);
        let (key, page, _) = owner.presence.test_app_page("fixture:owned-target");
        assert!(
            owner
                .presence
                .app_event(&key, page.clone(), 1, false)
                .is_ok()
        );
        let reason = other.presence.app_event(&key, page, 1, true).unwrap_err();
        let rejected = other.rejected_app_event(reason);
        assert!(rejected.is_error);
        assert_eq!(rejected.data["code"], "live_ui_event_rejected");
        assert_eq!(rejected.data["message"], "view_ended");
        assert_eq!(rejected.data["adapter_instance"], other.instance);
        assert!(rejected.data.get("view_key").is_none());
        assert!(rejected.data.get("target").is_none());
    }
    #[test]
    fn ui_resource_is_static_closed_network_and_exact_uri() {
        let resource = serde_json::to_value(read(URI).unwrap()).unwrap();
        assert_eq!(resource["contents"][0]["mimeType"], MIME);
        assert_eq!(
            resource["contents"][0]["_meta"]["ui"]["csp"]["connectDomains"],
            json!([])
        );
        assert!(read("file:///etc/passwd").is_err());
        assert_eq!(
            resource["contents"][0]["_meta"]["openai/ui"]["availableDisplayModes"],
            json!(["fullscreen"])
        );
        let legacy = serde_json::to_value(read(LEGACY_URI).unwrap()).unwrap();
        assert_eq!(legacy["contents"][0]["uri"], LEGACY_URI);
        assert_eq!(
            legacy["contents"][0]["_meta"],
            resource["contents"][0]["_meta"]
        );
        let previous = serde_json::to_value(read(PREVIOUS_URI).unwrap()).unwrap();
        assert_eq!(previous["contents"][0]["uri"], PREVIOUS_URI);
        assert_eq!(
            previous["contents"][0]["text"],
            resource["contents"][0]["text"]
        );
        let tool = serde_json::to_value(presence_tool()).unwrap();
        assert_eq!(tool["_meta"]["ui"]["visibility"], json!(["app"]));
        assert!(tool["inputSchema"]["properties"].get("target").is_none());
        let render = serde_json::to_value(start_tool()).unwrap();
        assert_eq!(render["_meta"]["ui"]["resourceUri"], URI);
        assert!(render["inputSchema"]["properties"].get("action").is_none());
    }
}
