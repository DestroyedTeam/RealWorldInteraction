//! Standard MCP adapter over the task-independent device API. No OS FFI.
mod cancellation;
#[cfg(all(test, unix))]
mod cancellation_tests;
mod invocation;
mod presentation;
mod repl;
mod routing;
mod shutdown;
use rcu_core::device::{self, Call, DeviceResult, Request};
use rcu_native_link::device::{AdapterProfile, Client};
use rmcp::{ErrorData, RoleServer, ServerHandler, ServiceExt, model::*, service::RequestContext};
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
    async fn targets(&self) -> DeviceResult {
        let mut pending = tokio::task::JoinSet::new();
        for id in self.routes.keys() {
            let this = self.clone();
            let id = id.clone();
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
        let description = match self.request(id, previous, Call::Describe).await {
            Ok(r) => r,
            Err(e) => return e,
        };
        if description.result.is_error || description.result.data["availability"] != "available" {
            return description.result;
        }
        let route = &self.routes[id];
        let mut context = route.context.lock().await;
        if description.result.data["context_valid"] == false
            || context
                .as_ref()
                .is_some_and(|c| c.instance != description.instance)
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
            self.remember(id, &opened.result.data);
            *context = Some(Connection {
                token: token.into(),
                instance: opened.instance,
                targets: opened.result.data["targets"].clone(),
            });
        }
        let mut result = description.result;
        result.data["targets"] = context.as_ref().unwrap().targets.clone();
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
            eprintln!("rcu-mcp: cleanup deadline; native resource release remains unconfirmed");
        }
    }
}
fn numeric_inputs(v: &mut Value) {
    match v {
        Value::Object(o) => {
            for (k, v) in o {
                if matches!(
                    k.as_str(),
                    "process_created" | "creation_filetime_100ns" | "logon_time_100ns"
                ) && let Some(n) = v.as_str().and_then(|s| s.parse::<u64>().ok())
                {
                    *v = json!(n);
                }
                numeric_inputs(v)
            }
        }
        Value::Array(a) => {
            for v in a {
                numeric_inputs(v)
            }
        }
        _ => {}
    }
}
fn portable_numbers(v: &mut Value) {
    match v {
        Value::Number(n) => {
            if let Some(n) = n.as_u64()
                && n > 9_007_199_254_740_991
            {
                *v = json!(n.to_string());
            }
        }
        Value::Object(o) => {
            for v in o.values_mut() {
                portable_numbers(v)
            }
        }
        Value::Array(a) => {
            for v in a {
                portable_numbers(v)
            }
        }
        _ => {}
    }
}
fn schema_numbers(v: &mut Value) {
    match v {
        Value::Object(o) => {
            if let Some(props) = o.get_mut("properties").and_then(Value::as_object_mut) {
                for (k, v) in props {
                    if matches!(
                        k.as_str(),
                        "process_created" | "creation_filetime_100ns" | "logon_time_100ns"
                    ) {
                        *v = json!({"type":"string","pattern":"^[0-9]{1,20}$"});
                    }
                }
            }
            for v in o.values_mut() {
                schema_numbers(v)
            }
        }
        Value::Array(a) => {
            for v in a {
                schema_numbers(v)
            }
        }
        _ => {}
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
impl ServerHandler for Gateway {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(Implementation::new("real-computer-use",env!("CARGO_PKG_VERSION"))).with_instructions("Generic multi-device RCU. Select computer.targets. Never replay uncertain input. GUI and terminal are complementary; report each channel's evidence separately.")
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let mut tools: Vec<_> = device::catalog::definitions()
            .into_iter()
            .map(|mut d| {
                schema_numbers(&mut d.input_schema);
                Tool::new(
                    d.name,
                    d.description,
                    Arc::new(d.input_schema.as_object().unwrap().clone()),
                )
            })
            .collect();
        tools.push(Tool::new("computer.result", "Read retained original metadata from this adapter's result_ref. No device RPC, capture or input. Use a JSON pointer and array pages; byte_offset=0 exports any large value as bounded UTF-8 JSON chunks. References expire independently of native execution; never replay input to recreate them.", Arc::new(serde_json::to_value(schemars::schema_for!(presentation::ResultArgs)).unwrap().as_object().unwrap().clone())));
        tools.extend(repl::tools());
        Ok(ListToolsResult {
            tools,
            ..Default::default()
        })
    }
    async fn call_tool(
        &self,
        p: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
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
        p: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
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
        let mut result = self
            .invoke(
                &p.name,
                Value::Object(p.arguments.unwrap_or_default()),
                Some(context),
            )
            .await;
        portable_numbers(&mut result.data);
        if matches!(p.name.as_ref(), "computer.observe" | "computer.targets")
            && let Ok(mut cache) = self.presentation.lock()
        {
            result = cache.project(&p.name, result);
        }
        Ok(mcp_result(result))
    }
}
pub fn run(path: Option<&Path>) -> io::Result<()> {
    let profile = match path {
        Some(path) => rcu_link_trust::read_private(path, 65536)
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
            tokio::io::duplex(rcu_native_link::device::REQUEST_CAP);
        let shutdown = gateway.shutdown.clone();
        let feed = tokio::spawn(async move {
            let _stop = shutdown::Stop(shutdown);
            let mut input = BufReader::new(tokio::io::stdin());
            while let Some(raw) =
                rcu_native_link::wire::read_frame(&mut input, rcu_native_link::device::REQUEST_CAP)
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
