//! App-only cached-image envelope; pixels never enter model content or result cache.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rwi_core::live_image::{LiveImageCursor, LiveImageQuery, MAX_LIVE_IMAGE_BYTES};
pub(in crate::gateway) const METHOD: &str = "computer.live_panel_image";
const AGENT_METHOD: &str = "computer.live_image";
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct ImageArgs {
    view_key: String,
    page: String,
    sequence: u64,
    #[serde(default)]
    after: Option<LiveImageCursor>,
}
impl ImageArgs {
    fn valid(&self) -> bool {
        valid_token(&self.view_key)
            && valid_token(&self.page)
            && self.sequence > 0
            && self.sequence <= 9_007_199_254_740_991
            && LiveImageQuery {
                view_id: self.view_key.clone(),
                after: self.after,
            }
            .validate()
            .is_ok()
    }
}
pub(super) fn tool() -> Tool {
    Tool::new(METHOD,"UI-only read of the owned visible page's already-cached Live PNG. Exact ephemeral page capability, monotonic image request sequence and negotiated MCP Apps required. No capture, input, presence renewal, target selection or execution-image retention. Pixels are component-only _meta, never model content.",Arc::new(serde_json::to_value(schemars::schema_for!(ImageArgs)).unwrap().as_object().unwrap().clone()))
        .with_title("Read owned Live preview image")
        .with_annotations(ToolAnnotations::new().read_only(true).destructive(false))
        .with_meta(meta(json!({"ui":{"visibility":["app"]}})))
}
fn failure(code: &str) -> CallToolResult {
    mcp_result(DeviceResult::error(
        code,
        "No pixels returned. Only the current visible component can read its cache; no capture, presence or input was started.",
    ))
}
fn component_reply(result: DeviceResult, sequence: u64) -> CallToolResult {
    if result.is_error {
        let code = result.data["code"]
            .as_str()
            .filter(|s| {
                s.len() <= 64
                    && s.bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            })
            .unwrap_or("live_image_unavailable");
        return failure(code);
    }
    let cursor: LiveImageCursor = match serde_json::from_value(result.data["cursor"].clone()) {
        Ok(v) => v,
        Err(_) => return failure("live_image_invalid_reply"),
    };
    if (LiveImageQuery {
        view_id: "0".repeat(32),
        after: Some(cursor),
    })
    .validate()
    .is_err()
    {
        return failure("live_image_invalid_reply");
    }
    let state = result.data["state"].as_str().unwrap_or("");
    let mut image = json!({"state":state,"cursor":cursor,"request_sequence":sequence});
    match state {
        "unchanged" if result.images.is_empty() => {}
        "ready" if result.images.len() == 1 => {
            let v = &result.images[0];
            if v.mime != "image/png" || v.base64.len() > MAX_LIVE_IMAGE_BYTES.div_ceil(3) * 4 {
                return failure("live_image_bounds");
            }
            let png = match STANDARD.decode(&v.base64) {
                Ok(v) if v.len() >= 24 && v.len() <= MAX_LIVE_IMAGE_BYTES => v,
                _ => return failure("live_image_bounds"),
            };
            let size: [u32; 2] = match serde_json::from_value(result.data["image_size"].clone()) {
                Ok(v) => v,
                Err(_) => return failure("live_image_bounds"),
            };
            let [w, h] = size;
            if !png.starts_with(b"\x89PNG\r\n\x1a\n")
                || &png[12..16] != b"IHDR"
                || w == 0
                || h == 0
                || w > 2048
                || h > 2048
                || u32::from_be_bytes(png[16..20].try_into().unwrap()) != w
                || u32::from_be_bytes(png[20..24].try_into().unwrap()) != h
                || result.data["png_bytes"].as_u64() != Some(png.len() as u64)
                || result.data["png_sha256"] != rwi_core::digest(&png)
            {
                return failure("live_image_bounds");
            }
            image["mime"] = json!("image/png");
            image["base64"] = json!(v.base64);
            image["image_size"] = json!(size);
            image["png_bytes"] = json!(png.len());
            image["png_sha256"] = result.data["png_sha256"].clone();
            if let Some(ms) = result.data["captured_unix_ms"]
                .as_u64()
                .filter(|v| *v <= 9_007_199_254_740_991)
            {
                image["captured_unix_ms"] = json!(ms);
            }
        }
        _ => return failure("live_image_invalid_reply"),
    }
    let mut reply = mcp_result(DeviceResult::value(
        json!({"state":state,"image_in_component_only":state == "ready","capture_on_read":false,"input_sent":false}),
    ));
    reply.meta = Some(meta(json!({"rwi/live_image":image})));
    reply
}
impl Gateway {
    pub(in crate::gateway) async fn app_live_image(
        &self,
        args: serde_json::Map<String, Value>,
        context: &RequestContext<RoleServer>,
    ) -> CallToolResult {
        if !negotiated(context) {
            return failure("live_ui_not_negotiated");
        }
        let args = match serde_json::from_value::<ImageArgs>(Value::Object(args)) {
            Ok(v) if v.valid() => v,
            _ => return failure("invalid_live_image_request"),
        };
        let ticket = match self
            .presence
            .image_read(&args.view_key, &args.page, args.sequence)
        {
            Ok(v) => v,
            Err(e) => return failure(e),
        };
        let id = match self.route_id("computer.live", &json!({"target":ticket.target})) {
            Ok(v) => v,
            Err(_) => return failure("live_image_binding_retired"),
        };
        let Some(route) = self.routes.get(&id) else {
            return failure("device_unknown");
        };
        let (token, instance) = match route.context.try_lock() {
            Ok(v) => match v.as_ref() {
                Some(v) => (v.token.clone(), v.instance.clone()),
                None => return failure("context_unknown"),
            },
            Err(_) => return failure("live_image_context_busy"),
        };
        let mut shutdown = self.shutdown.subscribe();
        if *shutdown.borrow() || context.ct.is_cancelled() {
            return failure("live_image_cancelled");
        }
        let call = Call::Cached {
            tool: AGENT_METHOD.into(),
            arguments: json!({"target":ticket.target,"query":{"view_id":ticket.binding,"after":args.after}}),
        };
        let reply = tokio::select! {
            biased;
            _ = shutdown.changed() => return failure("host_disconnected"),
            _ = context.ct.cancelled() => return failure("live_image_cancelled"),
            value = tokio::time::timeout(std::time::Duration::from_secs(2),self.request(&id,Some(token.clone()),call)) => match value { Ok(Ok(v)) => v, Ok(Err(_)) => return failure("live_image_transport_unavailable"), Err(_) => return failure("live_image_timeout") },
        };
        if !ticket.current()
            || reply.instance != instance
            || !route.context.try_lock().is_ok_and(|v| {
                v.as_ref()
                    .is_some_and(|v| v.token == token && v.instance == instance)
            })
        {
            return failure("live_image_response_revoked");
        }
        let response = component_reply(reply.result, args.sequence);
        if !ticket.current()
            || *shutdown.borrow()
            || context.ct.is_cancelled()
            || !route.context.try_lock().is_ok_and(|v| {
                v.as_ref()
                    .is_some_and(|v| v.token == token && v.instance == instance)
            })
        {
            return failure("live_image_response_revoked");
        }
        response
    }
}
#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;
