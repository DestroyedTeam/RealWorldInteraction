//! Optional MCP Apps presenter. Native capture/input authority stays on Agent.
//! UI presence is evidence about the rendered component, not an active-task API.
use super::*;
mod image;
pub(super) use image::METHOD as IMAGE_TOOL;

pub(super) const EXTENSION: &str = "io.modelcontextprotocol/ui";
pub(super) const MIME: &str = "text/html;profile=mcp-app";
pub(super) const URI: &str = "ui://real-world-interaction/live/v8.html";
const FULLSCREEN_URI: &str = "ui://real-world-interaction/live/v8-fullscreen.html";
const LEGACY_URIS: [&str; 7] = [
    "ui://real-world-interaction/live/v1.html",
    "ui://real-world-interaction/live/v2.html",
    "ui://real-world-interaction/live/v3.html",
    "ui://real-world-interaction/live/v4.html",
    "ui://real-world-interaction/live/v5.html",
    "ui://real-world-interaction/live/v6.html",
    "ui://real-world-interaction/live/v7.html",
];
pub(super) const PRESENCE_TOOL: &str = "computer.live_panel";
pub(super) const START_TOOL: &str = "computer.show_live";
pub(super) const EXPANDED_START_TOOL: &str = "computer.show_live_expanded";
pub(super) fn is_start(name: &str) -> bool {
    matches!(name, START_TOOL | EXPANDED_START_TOOL)
}

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
// Metadata-only observation of the exact already-owned view. This never
// republishes presence, retries a mutation, or reads pixels. Keep it off the
// visible heartbeat path and stop on identity change rather than borrowing
// state from a replacement preview.
fn hidden_capture_state(data: &Value, view_id: &str) -> Value {
    if data["host_view_binding"].as_str() != Some(view_id) {
        return json!({"state":"unknown","reason":"view_binding_changed"});
    }
    let known = data["presentation_suspended"].is_boolean()
        && data["capture_pending"].is_boolean()
        && data["native_call_pending"].is_boolean();
    let suspended = known
        && data["presentation_suspended"] == true
        && data["capture_pending"] == false
        && data["native_call_pending"] == false;
    let mut result =
        json!({"state":if !known {"unknown"} else if suspended {"suspended"} else {"pending"}});
    for field in [
        "session_id",
        "status",
        "presentation_suspended",
        "capture_pending",
        "native_call_pending",
        "worker_done",
        "frames_captured",
    ] {
        if let Some(value) = data.get(field) {
            result[field] = value.clone();
        }
    }
    result
}
async fn observe_hidden_capture<F, Fut>(view_id: &str, mut read: F) -> Value
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = DeviceResult>,
{
    use std::time::Duration;
    let started = tokio::time::Instant::now();
    let deadline = started + Duration::from_millis(500);
    let mut last = json!({"state":"unknown","reason":"observation_timeout"});
    let mut reads = 0;
    for _ in 0..8 {
        reads += 1;
        let reply = match tokio::time::timeout_at(deadline, read()).await {
            Ok(reply) => reply,
            Err(_) => break,
        };
        if reply.is_error {
            last = json!({"state":"unknown","reason":"status_unavailable"});
            break;
        }
        last = hidden_capture_state(&reply.data, view_id);
        if last["state"] != "pending" {
            break;
        }
        if tokio::time::timeout_at(deadline, tokio::time::sleep(Duration::from_millis(50)))
            .await
            .is_err()
        {
            break;
        }
    }
    last["read_only"] = json!(true);
    last["status_reads"] = json!(reads);
    last["elapsed_ms"] = json!(started.elapsed().as_millis());
    last
}
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct StartArgs {
    target: String,
    options: rwi_core::LiveOptions,
    /// Explicit Host expansion request only; never Native visibility authority.
    #[serde(default)]
    presentation: HostPresentation,
}
#[derive(Clone, Copy, Default, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
enum HostPresentation {
    #[default]
    Automatic,
    Fullscreen,
}
impl HostPresentation {
    fn decorate(self, metadata: &mut Option<Meta>) {
        if !matches!(self, Self::Fullscreen) {
            return;
        }
        let Some(metadata) = metadata.as_mut() else {
            return;
        };
        let Some(owner) = metadata
            .0
            .get_mut("rwi/live_panel")
            .and_then(Value::as_object_mut)
        else {
            return;
        };
        owner.insert("presentation".into(), json!("fullscreen"));
        metadata
            .0
            .insert("ui".into(), json!({"resourceUri":FULLSCREEN_URI}));
    }
}
fn start_schema() -> Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(StartArgs)).unwrap();
    // Only this negotiated component entry point permits display omission. Keep
    // computer.live and the shared Native options schema explicit-display-only.
    schema["$defs"]["LiveOptions"]["required"]
        .as_array_mut()
        .expect("LiveOptions required fields")
        .retain(|field| field != "display");
    schema
}
pub(super) fn start_tool() -> Tool {
    let schema = start_schema();
    Tool::new(START_TOOL, "Show a bounded Live preview in this conversation (MCP Apps HTML required). Supply a selected target and options: omit display for conversation-only cached pixels; specify display to also show a native overlay there. Optional presentation=fullscreen requests Host expansion once when offered; omission prefers task-contained inline and never requests PiP; a known Host PiP reply suspends this conversation view. Use independent native Live for a persistent overlay. It is not visibility or render proof. Stop the prior preview before changing presentation. Native authorization remains authoritative. Use computer.live for status/hide/resume/stop; a resource is not proof of rendered pixels or task-switch binding.",
        Arc::new(schema.as_object().unwrap().clone()))
        .with_title("Show conversation Live preview")
        .with_annotations(ToolAnnotations::new().read_only(false).destructive(false))
        .with_meta(meta(json!({"ui":{"resourceUri":URI}})))
}
pub(super) fn expanded_start_tool() -> Tool {
    let mut schema = start_schema();
    schema["properties"]
        .as_object_mut()
        .unwrap()
        .remove("presentation");
    Tool::new(EXPANDED_START_TOOL, "Start the selected read-only Live with a static fullscreen-only MCP App resource. Explicit Host expansion, not a visibility grant or render proof. Same target/options and native authorization as show_live; no default preview changes, input, fallback or retry.", Arc::new(schema.as_object().unwrap().clone()))
        .with_title("Show expanded conversation Live")
        .with_annotations(ToolAnnotations::new().read_only(false).destructive(false))
        .with_meta(meta(json!({"ui":{"resourceUri":FULLSCREEN_URI}})))
}
pub(super) fn resource() -> Resource {
    RawResource::new(URI, "rwi_live_panel").with_title("RWI Live · 会话内预览")
        .with_description("Optional Host-rendered read-only cached Live PNG and hide/close controls. Pixels use the app-only bridge, never model content. No input, credentials or conversation text.")
        .with_mime_type(MIME).no_annotation()
}
pub(super) fn read(uri: &str) -> Result<ReadResourceResult, ErrorData> {
    if uri == FULLSCREEN_URI
        || matches!(
            uri,
            "ui://real-world-interaction/live/v6-fullscreen.html"
                | "ui://real-world-interaction/live/v7-fullscreen.html"
        )
    {
        let html = include_str!("live_ui.html")
            .replace(
                "const defaultPresentation = 'automatic';",
                "const defaultPresentation = 'fullscreen';",
            )
            .replace(
                "availableDisplayModes: ['inline', 'fullscreen']",
                "availableDisplayModes: ['fullscreen']",
            );
        return Ok(ReadResourceResult::new(vec![ResourceContents::text(html, uri)
            .with_mime_type(MIME).with_meta(meta(json!({"ui":{"prefersBorder":false,"csp":{"connectDomains":[],"resourceDomains":[],"frameDomains":[]}},"openai/ui":{"availableDisplayModes":["fullscreen"],"preferredDisplayMode":"fullscreen"}})))]));
    }
    if uri != URI && !LEGACY_URIS.contains(&uri) {
        return Err(ErrorData::invalid_params("Unknown RWI UI resource", None));
    }
    // Prefer a compact inline container without forcing a fullscreen-only view.
    // A collapsed/unmounted card still stays hidden: metadata is not presence.
    Ok(ReadResourceResult::new(vec![ResourceContents::text(include_str!("live_ui.html"), uri)
        .with_mime_type(MIME).with_meta(meta(json!({"ui":{"prefersBorder":false,"csp":{"connectDomains":[],"resourceDomains":[],"frameDomains":[]}},"openai/ui":{"availableDisplayModes":["inline","fullscreen"],"preferredDisplayMode":"inline"}}))) ]))
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
pub(super) fn tools() -> Vec<Tool> {
    vec![
        start_tool(),
        expanded_start_tool(),
        presence_tool(),
        image::tool(),
    ]
}
impl Gateway {
    pub(super) async fn call_live_app(
        &self,
        p: CallToolRequestParams,
        context: &RequestContext<RoleServer>,
    ) -> CallToolResult {
        if p.name == IMAGE_TOOL {
            return self
                .app_live_image(p.arguments.unwrap_or_default(), context)
                .await;
        }
        mcp_result(
            self.app_live_presence(p.arguments.unwrap_or_default(), context)
                .await,
        )
    }
    pub(super) async fn start_live_app(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        if !negotiated(&context) {
            return mcp_result(DeviceResult::error(
                "live_ui_not_negotiated",
                "Host has not declared MCP Apps HTML support; use computer.live and its explicit viewer result. No Live was started.",
            ));
        }
        let mut args = request.arguments.unwrap_or_default();
        if request.name == EXPANDED_START_TOOL {
            if args.contains_key("presentation") {
                return mcp_result(DeviceResult::error(
                    "invalid_live_ui_start",
                    "Expanded Live takes target/options only; its template is explicitly fullscreen.",
                ));
            }
            args.insert("presentation".into(), json!("fullscreen"));
        }
        let args = Value::Object(args);
        // Reject before serde can erase an explicitly supplied false flag.
        if presence::reserved_arguments(&args) {
            return mcp_result(DeviceResult::error(
                "host_view_reserved",
                "Host visibility is published only by the task-owned view, not tool arguments.",
            ));
        }
        let args = match serde_json::from_value::<StartArgs>(args) {
            Ok(args) => args,
            Err(_) => {
                return mcp_result(DeviceResult::error(
                    "invalid_live_ui_start",
                    "Supply a selected target and options, without action or visibility fields; display is optional on this component route.",
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
        let mut result = if args.options.display.is_some() {
            self.invoke(
                "computer.live",
                json!({"target":args.target,"action":"start","options":args.options}),
                Some(context),
            )
            .await
        } else {
            self.host_component_start(&args.target, args.options, context)
                .await
        };
        let mut metadata = self.prepare_live_app(&mut result, &args.target).await;
        args.presentation.decorate(&mut metadata);
        if matches!(args.presentation, HostPresentation::Fullscreen) && metadata.is_some() {
            result.data["viewer"]["resource_uri"] = json!(FULLSCREEN_URI);
        }
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
        let view_id = update.view_id.clone();
        let mut result = self.publish_view(&target, update).await;
        if !args.visible && !result.is_error && result.data["accepted"] == true {
            result.data["native_observation"] = observe_hidden_capture(&view_id, || {
                self.invoke_device(
                    "computer.live",
                    json!({"target":target,"action":"status"}),
                    None,
                )
            })
            .await;
        }
        result
    }
}

#[cfg(test)]
mod tests;
