//! Standard MCP adapter over the task-independent device API. No OS FFI.
mod cancellation;
#[cfg(all(test, unix))]
mod cancellation_tests;
#[cfg(all(test, unix))]
mod control_lane_tests;
mod convenience;
mod invocation;
mod numbers;
use numbers::{numeric_inputs, portable_numbers, schema_numbers};
mod live_ui;
mod presence;
mod presentation;
mod progress;
mod repl;
mod routing;
mod shutdown;
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt, model::*, service::RequestContext};
use rwi_core::device::{self, Call, DeviceResult, Request};
use rwi_native_link::device::{AdapterProfile, Client};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::Path,
    sync::{Arc, Mutex},
};

struct Connection {
    token: String,
    instance: String,
    targets: Value,
}
struct Route {
    client: Result<Client, String>,
    context: tokio::sync::Mutex<Option<Connection>>,
}
#[derive(Clone)]
struct Gateway {
    routes: Arc<BTreeMap<String, Arc<Route>>>,
    handles: Arc<Mutex<BTreeMap<String, BTreeSet<String>>>>,
    profile_error: Option<String>,
    cancels: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    inflight: Arc<tokio::sync::Semaphore>,
    shutdown: tokio::sync::watch::Sender<bool>,
    presentation: Arc<Mutex<presentation::Presentation>>,
    repl: Arc<repl::Repl>,
    presence: Arc<presence::Views>,
    instance: String,
}
impl Gateway {
    fn new(profile: io::Result<AdapterProfile>) -> Self {
        let mut routes = BTreeMap::new();
        let mut error = None;
        match profile.and_then(|p| {
            p.validate()?;
            Ok(p)
        }) {
            Ok(profile) => {
                for d in profile.devices {
                    let id = d.device_id.clone();
                    let client = d
                        .identity
                        .load()
                        .and_then(|identity| Client::new(d, identity))
                        .map_err(|e| e.to_string());
                    routes.insert(
                        id,
                        Arc::new(Route {
                            client,
                            context: tokio::sync::Mutex::new(None),
                        }),
                    );
                }
            }
            Err(e) => error = Some(e.to_string()),
        }
        Self {
            routes: Arc::new(routes),
            handles: Arc::new(Mutex::new(BTreeMap::new())),
            profile_error: error,
            cancels: Arc::new(Mutex::new(Vec::new())),
            inflight: Arc::new(tokio::sync::Semaphore::new(64)),
            shutdown: tokio::sync::watch::channel(false).0,
            presentation: Arc::new(Mutex::new(presentation::Presentation::default())),
            repl: Arc::new(repl::Repl::default()),
            presence: Arc::new(presence::Views::default()),
            instance: uuid::Uuid::new_v4().simple().to_string(),
        }
    }
    async fn request(
        &self,
        id: &str,
        token: Option<String>,
        call: Call,
    ) -> Result<device::Reply, DeviceResult> {
        let route = self.routes.get(id).ok_or_else(|| {
            DeviceResult::error(
                "device_unknown",
                "Select a paired device from computer.targets.",
            )
        })?;
        let client = route
            .client
            .as_ref()
            .map_err(|e| DeviceResult::error("device_configuration_invalid", e))?;
        let failure_scope = if matches!(call, Call::Describe) {
            "only a disconnected reused Describe may reconnect once; native input is never retried"
        } else {
            "request was not automatically retried; an action effect may be unknown"
        };
        client
            .call(&Request {
                version: device::PROTOCOL_VERSION,
                id: uuid::Uuid::new_v4().to_string(),
                device: id.into(),
                context: token,
                call,
            })
            .await
            .map_err(|e| {
                DeviceResult::error("device_connection_failed", &format!("{e}; {failure_scope}"))
            })
    }
    async fn targets(&self, selected_device_id: Option<&str>) -> DeviceResult {
        let device_ids = match selected_device_id {
            Some(id) if !device::valid_id(id) => {
                return DeviceResult::error("invalid_arguments", "Use one valid paired device_id.");
            }
            Some(id) if !self.routes.contains_key(id) => {
                return DeviceResult::error(
                    "device_unknown",
                    "The selected device_id is not paired.",
                );
            }
            Some(id) => vec![id.to_owned()],
            None => self.routes.keys().cloned().collect(),
        };
        let mut pending = tokio::task::JoinSet::new();
        for id in device_ids {
            let this = self.clone();
            pending.spawn(async move {
                let d = this.inspect_device(&id).await;
                (id, d)
            });
        }
        let mut devices = Vec::new();
        let mut targets = Vec::new();
        while let Some(result) = pending.join_next().await {
            match result {
                Ok((id, result)) => {
                    if let Some(rows) = result.data["targets"].as_array() {
                        targets.extend(rows.clone());
                    }
                    let transport = self.routes[&id]
                        .client
                        .as_ref()
                        .ok()
                        .map(Client::transport_stats);
                    devices.push(
                        json!({"device_id":id,"is_error":result.is_error,"state":result.data,
                        "adapter_transport":transport}),
                    );
                }
                Err(e) => devices.push(json!({"is_error":true,"error":e.to_string()})),
            }
        }
        DeviceResult::value(
            json!({"devices":devices,"targets":targets,"profile_error":self.profile_error,"service":"healthy","workflow_required":false}),
        )
    }
    async fn inspect_device(&self, id: &str) -> DeviceResult {
        let previous = self.routes[id]
            .context
            .lock()
            .await
            .as_ref()
            .map(|c| c.token.clone());
        let description = match self.request(id, previous.clone(), Call::Describe).await {
            Ok(r) => r,
            Err(e) => return e,
        };
        if description.result.is_error || description.result.data["availability"] != "available" {
            return description.result;
        }
        let route = &self.routes[id];
        let mut context = route.context.lock().await;
        // Describe was sent outside the lock: a newer context may have opened
        // while its response was in flight. Only retire the inspected epoch.
        if context.as_ref().map(|c| &c.token) == previous.as_ref()
            && (description.result.data["context_valid"] == false
                || context
                    .as_ref()
                    .is_some_and(|c| c.instance != description.instance))
        {
            *context = None;
        }
        if context.is_none() {
            let opened = match self.request(id, None, Call::OpenContext).await {
                Ok(r) => r,
                Err(e) => return e,
            };
            if opened.result.is_error {
                return opened.result;
            }
            let Some(token) = opened.result.data["context"].as_str() else {
                return DeviceResult::error(
                    "context_response_invalid",
                    "Device did not supply an authenticated context.",
                );
            };
            *context = Some(Connection {
                token: token.into(),
                instance: opened.instance,
                targets: opened.result.data["targets"].clone(),
            });
        }
        // OpenContext acknowledges its catalog, not current native readiness. A
        // late Describe may also have lost its epoch to another inspector.
        // In either case validate the exact current token under the existing
        // context lock; the normal same-token Describe adds no extra RPC.
        let description = if context.as_ref().map(|c| &c.token) != previous.as_ref() {
            let current = context.as_ref().unwrap();
            let token = current.token.clone();
            let instance = current.instance.clone();
            let fresh = match self.request(id, Some(token), Call::Describe).await {
                Ok(r) => r,
                Err(e) => {
                    *context = None;
                    return e;
                }
            };
            let available = fresh.result.data["availability"] == "available";
            let consistent = fresh.instance == instance
                && fresh.result.data["context_valid"] == true
                && fresh.result.data["targets"].is_array();
            if fresh.result.is_error || !available || !consistent {
                // Only this locked epoch is retired. Preserve actual status and
                // diagnostics, but never publish an unvalidated owner catalog.
                *context = None;
                let mut result = fresh.result;
                if !result.is_error && available {
                    result.is_error = true;
                    result.data["code"] = json!("context_response_invalid");
                    result.data["next_step"] =
                        json!("Inspect a fresh authenticated context; no input was dispatched.");
                }
                if let Some(state) = result.data.as_object_mut() {
                    state.remove("targets");
                }
                return result;
            }
            context.as_mut().unwrap().targets = fresh.result.data["targets"].clone();
            fresh
        } else {
            description
        };
        let mut result = description.result;
        if context.as_ref().map(|c| &c.token) == previous.as_ref()
            && result.data["context_valid"] == true
            && result.data["targets"].is_array()
        {
            context.as_mut().unwrap().targets = result.data["targets"].clone();
        }
        result.data["targets"] = context.as_ref().unwrap().targets.clone();
        self.remember(id, &result.data);
        result
    }
    async fn close(&self) {
        self.repl.reset().await;
        let mut jobs = tokio::task::JoinSet::new();
        if let Ok(mut pending) = self.cancels.lock() {
            for job in pending.drain(..) {
                jobs.spawn(async move {
                    let _ = job.await;
                });
            }
        }
        for (id, route) in self.routes.iter() {
            let gateway = self.clone();
            let id = id.clone();
            let route = route.clone();
            jobs.spawn(async move {
                let token = route.context.lock().await.take().map(|c| c.token);
                if token.is_some() {
                    let _ = gateway.request(&id, token, Call::CloseContext).await;
                }
            });
        }
        if tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while jobs.join_next().await.is_some() {}
        })
        .await
        .is_err()
        {
            jobs.abort_all();
            eprintln!("rwi-mcp: cleanup deadline; native resource release remains unconfirmed");
        }
    }
}
fn mcp_result(mut result: DeviceResult) -> CallToolResult {
    // Older native agents can report a failed dispatch in a successful envelope.
    // Surface the error without rewriting native uncertainty/effect evidence.
    if let Some(failure) = result.data.get("dispatch_error").cloned()
        && failure.is_object()
    {
        result.is_error = true;
        for (key, source) in [("code", "code"), ("message", "next_step")] {
            if result.data.get(key).is_none()
                && let Some(value) = failure.get(source)
            {
                result.data[key] = value.clone();
            }
        }
    }
    portable_numbers(&mut result.data);
    let mut reply = if result.is_error {
        CallToolResult::structured_error(result.data)
    } else {
        CallToolResult::structured(result.data)
    };
    for image in result.images {
        reply.content.push(Content::image(image.base64, image.mime));
    }
    reply
}
fn mcp_result_with_text_projection(result: DeviceResult, text: Option<Value>) -> CallToolResult {
    let mut reply = mcp_result(result);
    if reply.is_error == Some(false)
        && let Some(text) = text
    {
        reply.content[0] = Content::text(serde_json::to_string(&text).expect("JSON value"));
    }
    reply
}
const SERVER_INSTRUCTIONS: &str = "RWI: use computer.targets({device_id}) for a known device; omit only for all peers (offline peers may delay). Add include_timings only if exposed by the tool input schema: adapter wall time, not native CPU; elapsed_ms is pre-cache. Never replay uncertain input. Keep GUI and terminal evidence separate.";

#[cfg(test)]
mod instruction_contract {
    #[test]
    fn timing_guidance_is_schema_scoped_and_keeps_no_replay() {
        let instructions = super::SERVER_INSTRUCTIONS;
        assert!(
            instructions.contains("Add include_timings only if exposed by the tool input schema")
        );
        assert!(!instructions.contains("include_timings=true adds"));
        assert!(instructions.contains("Never replay uncertain input"));
        assert!(instructions.contains("elapsed_ms is pre-cache"));
    }
}

impl ServerHandler for Gateway {
    fn get_info(&self) -> ServerInfo {
        let mut capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()
            .build();
        capabilities.extensions = Some(BTreeMap::from([(
            live_ui::EXTENSION.into(),
            json!({"mimeTypes":[live_ui::MIME]})
                .as_object()
                .unwrap()
                .clone(),
        )]));
        ServerInfo::new(capabilities)
            .with_server_info(Implementation::new(
                "real-world-interaction",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(SERVER_INSTRUCTIONS)
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let ui_supported = live_ui::negotiated(&context);
        let mut tools: Vec<_> = device::catalog::definitions()
            .into_iter()
            // The model route owns a four-field schema and internal lease.
            .filter(|d| {
                !matches!(
                    d.name,
                    "computer.focus_element" | "computer.select_window_local" | "computer.set_text"
                )
            })
            .map(|mut d| {
                schema_numbers(&mut d.input_schema);
                if d.name == "computer.targets" {
                    presentation::targets_text_schema(&mut d.input_schema);
                }
                let tool = Tool::new(
                    d.name,
                    d.description,
                    Arc::new(d.input_schema.as_object().unwrap().clone()),
                );
                match d.name {
                    "computer.live" => tool.with_title("Control task Live preview"),
                    "computer.hotkey" => tool.with_title("Send foreground/system shortcut"),
                    "computer.attach_window" => tool.with_title("Attach independent window"),
                    "computer.detach_window" => tool.with_title("Detach window binding"),
                    "computer.targets" => tool.with_title("Inspect paired devices"),
                    "computer.discover" => tool.with_title("Find native windows"),
                    "computer.bind_window" => tool.with_title("Select exact native window"),
                    "computer.observe" => tool.with_title("Observe selected window"),
                    "computer.capture_visual" => tool.with_title("Capture read-only window visual"),
                    "computer.act_checked" => tool.with_title("Act on exact observed control"),
                    "computer.operation" => tool.with_title("Query original operation"),
                    "computer.cancel" => tool.with_title("Cancel pending operation"),
                    "computer.verify" => tool.with_title("Verify native state"),
                    _ => tool,
                }
            })
            .collect();
        tools.push(Tool::new("computer.result", "Read retained original metadata from this adapter's result_ref. No device RPC, capture or input. Use a JSON pointer and array pages; byte_offset=0 exports any large value as bounded UTF-8 JSON chunks. References expire independently of native execution; never replay input to recreate them.", Arc::new(serde_json::to_value(schemars::schema_for!(presentation::ResultArgs)).unwrap().as_object().unwrap().clone())));
        tools.extend(repl::tools());
        tools.extend(convenience::tools());
        if ui_supported {
            tools.push(live_ui::start_tool());
            tools.push(live_ui::presence_tool());
        }
        Ok(ListToolsResult {
            tools,
            ..Default::default()
        })
    }
    async fn list_resources(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        Ok(ListResourcesResult {
            resources: vec![live_ui::resource()],
            ..Default::default()
        })
    }
    async fn read_resource(
        &self,
        p: ReadResourceRequestParams,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, ErrorData> {
        live_ui::read(&p.uri)
    }
    async fn call_tool(
        &self,
        p: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        if p.name == live_ui::PRESENCE_TOOL {
            return Ok(mcp_result(
                self.app_live_presence(p.arguments.unwrap_or_default(), &context)
                    .await,
            ));
        }
        if p.name == live_ui::START_TOOL {
            return Ok(self
                .start_live_app(p.arguments.unwrap_or_default(), context)
                .await);
        }
        if p.name == "computer.js" {
            return Ok(self
                .repl
                .evaluate(self, p.arguments.unwrap_or_default(), context)
                .await);
        }
        if p.name == "computer.js_reset" {
            if !p.arguments.unwrap_or_default().is_empty() {
                return Ok(mcp_result(DeviceResult::error(
                    "invalid_arguments",
                    "Reset takes no arguments.",
                )));
            }
            return Ok(mcp_result(self.repl.reset().await));
        }
        self.call_native(p, context).await
    }
}
impl Gateway {
    /// Shared admission and presentation for direct MCP and adapter callers.
    async fn call_native(
        &self,
        mut p: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let requested_text_projection = if p.name == "computer.targets" {
            match presentation::take_targets_text_projection(
                p.arguments.get_or_insert_with(Default::default),
            ) {
                Ok(requested) => requested,
                Err(error) => return Ok(mcp_result(error)),
            }
        } else {
            false
        };
        if p.name == "computer.result" {
            let args = serde_json::from_value::<presentation::ResultArgs>(Value::Object(
                p.arguments.unwrap_or_default(),
            ));
            let result = match args {
                Ok(args) => match self.presentation.lock() {
                    Ok(mut cache) => cache.read(args),
                    Err(_) => DeviceResult::error(
                        "presentation_unavailable",
                        "Metadata cache unavailable; native execution state is unchanged.",
                    ),
                },
                Err(e) => DeviceResult::error("invalid_arguments", &e.to_string()),
            };
            return Ok(mcp_result(result));
        }
        let _permit = if matches!(
            p.name.as_ref(),
            "computer.operation" | "computer.cancel" | "computer.targets"
        ) {
            None
        } else {
            match self.inflight.clone().try_acquire_owned() {
                Ok(p) => Some(p),
                Err(_) => {
                    return Ok(mcp_result(DeviceResult::error(
                        "adapter_busy",
                        "No operation was forwarded; control/status remains available.",
                    )));
                }
            }
        };
        if convenience::handles(&p.name) {
            return Ok(mcp_result(
                self.convenience(
                    &p.name,
                    Value::Object(p.arguments.unwrap_or_default()),
                    context,
                )
                .await,
            ));
        }
        let mut result = self
            .invoke(
                &p.name,
                Value::Object(p.arguments.unwrap_or_default()),
                Some(context),
            )
            .await;
        portable_numbers(&mut result.data);
        let mut text_projection = None;
        if matches!(p.name.as_ref(), "computer.observe" | "computer.targets")
            && let Ok(mut cache) = self.presentation.lock()
        {
            result = cache.project(&p.name, result);
            if requested_text_projection {
                text_projection = cache.targets_text_projection(&result);
            }
        }
        Ok(mcp_result_with_text_projection(result, text_projection))
    }
}
pub fn run(path: Option<&Path>) -> io::Result<()> {
    let profile = match path {
        Some(path) => rwi_link_trust::read_private(path, 65536)
            .and_then(|b| serde_json::from_slice(&b).map_err(io::Error::other)),
        None => Ok(AdapterProfile::default()),
    };
    let gateway = Gateway::new(profile);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()?;
    let result = runtime.block_on(async {
        use tokio::io::{AsyncWriteExt, BufReader};
        let (mut bounded_write, bounded_read) =
            tokio::io::duplex(rwi_native_link::device::REQUEST_CAP);
        let shutdown = gateway.shutdown.clone();
        let feed = tokio::spawn(async move {
            let _stop = shutdown::Stop(shutdown);
            let mut input = BufReader::new(tokio::io::stdin());
            while let Some(raw) =
                rwi_native_link::wire::read_frame(&mut input, rwi_native_link::device::REQUEST_CAP)
                    .await?
            {
                bounded_write.write_all(&raw).await?;
            }
            Ok::<_, io::Error>(())
        });
        struct Abort(tokio::task::AbortHandle);
        impl Drop for Abort {
            fn drop(&mut self) {
                self.0.abort();
            }
        }
        let _abort = Abort(feed.abort_handle());
        let service = gateway
            .clone()
            .serve((bounded_read, tokio::io::stdout()))
            .await
            .map_err(io::Error::other)?;
        let result = service.waiting().await.map_err(io::Error::other);
        gateway.close().await;
        result.map(|_| ())
    });
    runtime.shutdown_timeout(std::time::Duration::from_secs(2));
    result
}

#[cfg(test)]
mod result_tests {
    use super::*;
    #[test]
    fn legacy_nested_dispatch_failure_is_explicit_without_rewriting_evidence() {
        let raw = json!({"dispatch":"uncertain","system_accepted":null,"native_dispatch_entered":false,"dispatch_error":{"code":"unsupported_chord","next_step":"Inspect supported keys"},"operation_ref":{"execution_nonce":"original"}});
        let result = mcp_result(DeviceResult::value(raw.clone()));
        assert_eq!(result.is_error, Some(true));
        let data = result.structured_content.unwrap();
        assert_eq!(data["code"], "unsupported_chord");
        assert_eq!(data["message"], "Inspect supported keys");
        for key in [
            "dispatch",
            "dispatch_error",
            "system_accepted",
            "native_dispatch_entered",
            "operation_ref",
        ] {
            assert_eq!(data[key], raw[key]);
        }
        assert_eq!(
            mcp_result(DeviceResult::value(json!({"system_accepted":true}))).is_error,
            Some(false)
        );
    }
}
