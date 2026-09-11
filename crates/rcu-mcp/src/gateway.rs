//! Standard MCP adapter over the task-independent device API. No OS FFI.
mod cancellation;
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
        client.call(&Request{version:device::PROTOCOL_VERSION,id:uuid::Uuid::new_v4().to_string(),device:id.into(),context:token,call}).await.map_err(|e|DeviceResult::error("device_connection_failed",&format!("{e}; request was not automatically retried; an action effect may be unknown")))
    }
    fn remember(&self, id: &str, value: &Value) {
        fn visit(map: &mut BTreeMap<String, BTreeSet<String>>, id: &str, v: &Value) {
            match v {
                Value::Object(o) => {
                    for (k, v) in o {
                        if matches!(
                            k.as_str(),
                            "target"
                                | "frame"
                                | "artifact_id"
                                | "lease"
                                | "view_id"
                                | "operation_id"
                        ) && let Some(s) = v.as_str()
                        {
                            map.entry(format!("{k}:{s}")).or_default().insert(id.into());
                        }
                        visit(map, id, v)
                    }
                }
                Value::Array(a) => {
                    for v in a {
                        visit(map, id, v)
                    }
                }
                _ => {}
            }
        }
        if let Ok(mut handles) = self.handles.lock() {
            if handles.len() > 8192 {
                handles.retain(|_, devices| {
                    devices.remove(id);
                    !devices.is_empty()
                });
            }
            visit(&mut handles, id, value);
            while handles.len() > 8192 {
                handles.pop_first();
            }
        }
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
                    devices.push(
                        json!({"device_id":id,"is_error":result.is_error,"state":result.data}),
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
    fn route_id(&self, tool: &str, args: &Value) -> Result<String, DeviceResult> {
        let map = self
            .handles
            .lock()
            .map_err(|_| DeviceResult::error("handle_registry_unavailable", "Re-query targets."))?;
        let mut candidates = args["device_id"]
            .as_str()
            .map(|id| BTreeSet::from([id.to_owned()]));
        for kind in [
            "target",
            "artifact_id",
            "lease",
            "view_id",
            "frame",
            "operation_id",
        ] {
            if kind == "operation_id" && !matches!(tool, "computer.operation" | "computer.cancel") {
                continue;
            }
            if let Some(value) = args[kind].as_str()
                && let Some(devices) = map.get(&format!("{kind}:{value}"))
            {
                candidates = Some(match candidates {
                    Some(set) => set.intersection(devices).cloned().collect(),
                    None => devices.clone(),
                });
            }
        }
        let Some(set) = candidates else {
            return Err(DeviceResult::error(
                "resource_unknown",
                "Use a returned handle, or explicit device_id for durable operation queries.",
            ));
        };
        match set.len() {
            1 => Ok(set.into_iter().next().unwrap()),
            0 => Err(DeviceResult::error(
                "foreign_resource",
                "Handles belong to different devices.",
            )),
            _ => Err(DeviceResult::error(
                "ambiguous_resource",
                "This handle exists on multiple devices. Supply device_id; no device was selected.",
            )),
        }
    }
    async fn invoke(
        &self,
        tool: &str,
        mut args: Value,
        context: Option<RequestContext<RoleServer>>,
    ) -> DeviceResult {
        let mut shutdown = self.shutdown.subscribe();
        if *shutdown.borrow() {
            return DeviceResult::error(
                "host_disconnected",
                "No new operation is forwarded after Host EOF.",
            );
        }
        if tool == "computer.targets" {
            return self.targets().await;
        }
        if context.as_ref().is_some_and(|c| c.ct.is_cancelled()) {
            return DeviceResult::error(
                "operation_cancelled_before_dispatch",
                "The Host cancelled before forwarding.",
            );
        }
        let supports_operation = device::catalog::definitions()
            .iter()
            .any(|d| d.name == tool && d.input_schema["properties"].get("operation_id").is_some());
        let cancellable =
            supports_operation && !matches!(tool, "computer.cancel" | "computer.operation");
        if cancellable && !args["operation_id"].is_string() {
            args["operation_id"] = json!(uuid::Uuid::new_v4().to_string());
        }
        let operation = args["operation_id"].as_str().map(str::to_owned);
        numeric_inputs(&mut args);
        if let Err(e) = device::catalog::validate(tool, &args) {
            return DeviceResult::error("invalid_arguments", &e.to_string());
        }
        let id = match self.route_id(tool, &args) {
            Ok(id) => id,
            Err(e) => return e,
        };
        let Some(route) = self.routes.get(&id) else {
            return DeviceResult::error("device_unknown", "Device is not paired.");
        };
        let token = route.context.lock().await.as_ref().map(|c| c.token.clone());
        let mut cancellation = if cancellable {
            operation.clone().map(|op| {
                self.remember(&id, &json!({"operation_id":op}));
                cancellation::CancelDispatch::new(self.clone(), id.clone(), token.clone(), op)
            })
        } else {
            None
        };
        let call = if tool == "computer.recover_input" {
            Call::RecoverInput {
                resource_domain: args["resource_domain"].as_str().unwrap_or_default().into(),
            }
        } else if tool == "computer.operation" {
            Call::Operation {
                operation_id: args["operation_id"].as_str().unwrap_or_default().into(),
            }
        } else if tool == "computer.cancel" {
            Call::Cancel {
                operation_id: args["operation_id"].as_str().unwrap_or_default().into(),
            }
        } else {
            Call::Invoke {
                tool: tool.into(),
                arguments: args,
            }
        };
        let request = self.request(&id, token, call);
        let host_cancelled = async {
            if let Some(context) = context {
                context.ct.cancelled().await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        let response = tokio::select! {
            result=request=>result,
            _=host_cancelled=>{
                return match cancellation.as_mut(){Some(cancel)=>cancel.request().await,None=>DeviceResult::error("operation_cancelled","No native cancellation/termination was inferred.")};
            },
            _=shutdown.changed()=>{
                return match cancellation.as_mut(){Some(cancel)=>cancel.request().await,None=>DeviceResult::error("host_disconnected","Native completion remains unconfirmed.")};
            }
        };
        let reply = match response {
            Ok(reply) => reply,
            Err(e) => return e,
        };
        if let Some(cancel) = &mut cancellation {
            cancel.disarm();
        }
        if reply.result.data["code"] == "context_unknown" {
            *route.context.lock().await = None;
        }
        self.remember(&id, &reply.result.data);
        reply.result
    }
    async fn close(&self) {
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
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(Implementation::new("real-computer-use",env!("CARGO_PKG_VERSION"))).with_instructions("Generic multi-device RCU. Read computer.targets, choose the exact returned target, discover/bind a fresh window, observe, claim, act once, verify, release. Services do not depend on tasks/windows. Reconnect and reobserve are allowed; never automatically replay an uncertain action. Unavailable devices/capabilities do not disable the tool catalog.")
    }
    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = device::catalog::definitions()
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
        Ok(mcp_result(
            self.invoke(
                &p.name,
                Value::Object(p.arguments.unwrap_or_default()),
                Some(context),
            )
            .await,
        ))
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
            struct Stop(tokio::sync::watch::Sender<bool>);
            impl Drop for Stop {
                fn drop(&mut self) {
                    let _ = self.0.send(true);
                }
            }
            let _stop = Stop(shutdown);
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
