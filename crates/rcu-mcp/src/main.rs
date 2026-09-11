//! Official MCP SDK + Rust engine. stdout is exclusively MCP transport.
#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(feature = "native-agent-worker")]
mod agent_worker;
mod eof_diagnostics;
#[cfg(feature = "link-fixture")]
mod link_fixture;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rcu_core::Verification;
use rcu_core::operation::{CancelOnDrop, Registry};
use rcu_core::{Engine, Intent, Locator, Predicate, Result as CoreResult, fault};
use rcu_imaging::{Rect, ViewMapping};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, Mutex},
};

const INSTRUCTIONS: &str = "Read computer.targets (opt-in). Prefer act_checked: observe, choose exact element ID, claim, dispatch once, verify, release. It rechecks document context on-server; no pixel/name selectors. Legacy act keeps its 10s TTL. Never loop reobserve to refresh failed actions. Launch/activate need grants+lease; bind never focuses. Partial preflight is not absence. Receipts are not goals/input grants. Never replay uncertainty; read lifecycle_receipt. Cancel cooperatively; await terminal/resources_released.";

#[derive(Clone)]
struct Server {
    engine: Arc<Mutex<Engine>>,
    gate: Arc<tokio::sync::Semaphore>,
    operations: Arc<Registry>,
    views: Arc<Mutex<VecDeque<(String, StoredView)>>>,
    tool_router: ToolRouter<Self>,
}
#[derive(Clone)]
struct StoredView {
    target: String,
    frame: String,
    artifact_id: String,
    mapping: ViewMapping,
}
impl Server {
    fn new(engine: Engine) -> Self {
        Self {
            engine: Arc::new(Mutex::new(engine)),
            gate: Arc::new(tokio::sync::Semaphore::new(1)),
            operations: Arc::new(Registry::default()),
            views: Arc::new(Mutex::new(VecDeque::new())),
            tool_router: Self::tool_router(),
        }
    }
    async fn execute<F>(&self, f: F) -> CallToolResult
    where
        F: FnOnce(&mut Engine) -> CoreResult<CallToolResult> + Send + 'static,
    {
        self.execute_operation(None, f).await
    }
    async fn execute_operation<F>(&self, operation_id: Option<String>, f: F) -> CallToolResult
    where
        F: FnOnce(&mut Engine) -> CoreResult<CallToolResult> + Send + 'static,
    {
        let Ok(permit) = self.gate.clone().try_acquire_owned() else {
            return error(fault(
                "server_busy",
                "Wait for the current bounded operation; never retry uncertain input.",
            ));
        };
        let op = match self.operations.begin(operation_id) {
            Ok(op) => op,
            Err(e) => return error(e),
        };
        let _cancel_on_drop = CancelOnDrop(op.clone());
        // Retain the registered operation outside the worker. An early CoreResult
        // error or JoinError must not lose an automatically assigned query ID.
        let worker_op = op.clone();
        let engine = self.engine.clone();
        let response = match tokio::task::spawn_blocking(move || {
            struct Finish {
                op: rcu_core::operation::Operation,
                permit: Option<tokio::sync::OwnedSemaphorePermit>,
            }
            impl Drop for Finish {
                fn drop(&mut self) {
                    drop(self.permit.take());
                    self.op.finish();
                }
            }
            let _finish = Finish {
                op: worker_op.clone(),
                permit: Some(permit),
            };
            rcu_core::operation::run(&worker_op, || {
                rcu_core::operation::checkpoint("worker_started")?;
                let mut engine = engine.lock().map_err(|_| {
                    fault(
                        "engine_unavailable",
                        "Restart, inspect durable receipts, and do not replay.",
                    )
                })?;
                f(&mut engine)
            })
        })
        .await
        {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => error(e),
            Err(_) => error(fault(
                "worker_failed",
                "Operation may have executed. Do not replay; inspect the target.",
            )),
        };
        // A JoinError can leave a registered but nonterminal operation. Apply
        // the request's cancellation guard before taking the returned snapshot.
        drop(_cancel_on_drop);
        // Observe, never synthesize, the post-join registry state. A cancelled
        // queued worker or retained native guard may still be nonterminal;
        // cleanup uncertainty must remain resources_released=false.
        with_operation(response, op.status())
    }
}
fn with_operation(
    mut response: CallToolResult,
    status: rcu_core::operation::Status,
) -> CallToolResult {
    let metadata = json!({
        "operation_id": status.operation_id,
        "operation": status,
        "cancellation": "cooperative; native preemption unavailable"
    });
    let mut structured = match response.structured_content.take() {
        Some(Value::Object(fields))
            if !metadata
                .as_object()
                .expect("operation metadata object")
                .keys()
                .any(|key| fields.contains_key(key)) =>
        {
            fields
        }
        // Existing object-shaped SDK fields stay at their original locations.
        // Preserve non-object payloads too, rather than discarding their value.
        Some(payload) => serde_json::Map::from_iter([("result".into(), payload)]),
        None => serde_json::Map::new(),
    };
    for (key, value) in metadata.as_object().expect("operation metadata object") {
        structured.insert(key.clone(), value.clone());
    }
    response.structured_content = Some(Value::Object(structured));
    // Keep all original text/image blocks intact and support clients that only
    // expose content rather than structuredContent.
    response.content.push(Content::text(metadata.to_string()));
    response
}
fn error(e: rcu_core::Fault) -> CallToolResult {
    let mut payload = json!({"error":e.code,"next_step":e.next_step});
    if let Some(diagnostic) = e.diagnostic {
        payload["diagnostic"] = *diagnostic;
    }
    CallToolResult::structured_error(payload)
}
fn value<T: serde::Serialize>(v: T) -> CoreResult<CallToolResult> {
    Ok(CallToolResult::structured(
        serde_json::to_value(v)
            .map_err(|_| fault("serialization_error", "Inspect server diagnostics."))?,
    ))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ObserveArgs {
    target: String,
    #[serde(default = "yes")]
    inline_image: bool,
    operation_id: Option<String>,
    region: Option<[u32; 4]>,
}
fn yes() -> bool {
    true
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ResolveArgs {
    target: String,
    frame: String,
    locator: Locator,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClaimArgs {
    target: String,
    #[serde(default = "ttl")]
    ttl_ms: u64,
}
fn ttl() -> u64 {
    30000
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReleaseArgs {
    lease: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ActArgs {
    target: String,
    reference: String,
    lease: String,
    request_id: String,
    intent: Intent,
    value: Option<String>,
    operation_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CheckedActArgs {
    target: String,
    frame: String,
    element: String,
    lease: String,
    request_id: String,
    intent: Intent,
    value: Option<String>,
    operation_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LaunchArgs {
    target: String,
    lease: String,
    authorization_id: String,
    request_id: String,
    #[serde(default = "timeout")]
    timeout_ms: u64,
    operation_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LifecyclePreflightArgs {
    target: String,
    authorization_id: String,
    operation_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ActivateArgs {
    target: String,
    lease: String,
    authorization_id: String,
    request_id: String,
    expected_foreground: Option<rcu_core::lifecycle::WindowIdentity>,
    operation_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LifecycleReceiptArgs {
    target: String,
    kind: String,
    authorization_id: String,
    request_id: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct VerifyArgs {
    target: String,
    frame: String,
    element: Option<String>,
    request_id: Option<String>,
    predicate: Predicate,
    expected: String,
    #[serde(default = "timeout")]
    timeout_ms: u64,
    #[serde(default = "samples")]
    stable_samples: u32,
    operation_id: Option<String>,
}
fn timeout() -> u64 {
    1000
}
fn samples() -> u32 {
    2
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ArtifactArgs {
    artifact_id: String,
    #[serde(default)]
    offset: usize,
    #[serde(default = "chunk")]
    length: usize,
    #[serde(default)]
    as_image: bool,
}
fn chunk() -> usize {
    1024 * 1024
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TargetArgs {
    target: String,
    operation_id: Option<String>,
    cursor: Option<String>,
    pid: Option<u32>,
    title_contains: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct BindArgs {
    target: String,
    selection: rcu_core::WindowSelection,
    operation_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct OperationArgs {
    operation_id: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ImageArgs {
    target: String,
    frame: String,
    artifact_id: String,
    crop: Option<[u32; 4]>,
    #[serde(default = "edge")]
    max_edge: u32,
    operation_id: Option<String>,
}
fn edge() -> u32 {
    1200
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ResolveViewArgs {
    target: String,
    view_id: String,
    x: f64,
    y: f64,
}
fn make_view(
    e: &Engine,
    views: &Mutex<VecDeque<(String, StoredView)>>,
    a: &ImageArgs,
) -> CoreResult<CallToolResult> {
    let (original, scene) = e.original(&a.target, &a.frame, &a.artifact_id)?;
    let crop = a
        .crop
        .map(|[x, y, width, height]| -> CoreResult<Rect> {
            Ok(Rect {
                x: i32::try_from(x).map_err(|_| fault("invalid_crop", "Crop x is too large."))?,
                y: i32::try_from(y).map_err(|_| fault("invalid_crop", "Crop y is too large."))?,
                width,
                height,
            })
        })
        .transpose()?;
    let (png, mapping) = rcu_imaging::view(original, crop, a.max_edge, scene.pixel_to_desktop)?;
    if mapping.original_size != scene.image_size {
        return Err(fault(
            "artifact_dimension_mismatch",
            "Original PNG dimensions do not match this frame metadata; do not resolve its pixels.",
        ));
    }
    let id = rcu_core::digest(
        serde_json::to_string(&(&a.target, &a.frame, &a.artifact_id, &mapping))
            .unwrap()
            .as_bytes(),
    );
    let mut views = views.lock().map_err(|_| {
        fault(
            "view_registry_unavailable",
            "Reobserve in a new connection.",
        )
    })?;
    views.retain(|(key, _)| key != &id);
    views.push_back((
        id.clone(),
        StoredView {
            target: a.target.clone(),
            frame: a.frame.clone(),
            artifact_id: a.artifact_id.clone(),
            mapping: mapping.clone(),
        },
    ));
    while views.len() > 64 {
        views.pop_front();
    }
    let mut r = value(
        json!({"view_id":id,"target":a.target,"frame":a.frame,"original_artifact_id":a.artifact_id,"original_sha256":rcu_core::digest(original),"original_bytes":original.len(),"view_sha256":rcu_core::digest(&png),"view_bytes":png.len(),"mapping":mapping,"original_unchanged":true,"resolve_tool":"computer.resolve_view"}),
    )?;
    r.content
        .push(Content::image(STANDARD.encode(png), "image/png"));
    Ok(r)
}

#[tool_router]
impl Server {
    #[tool(
        name = "computer.targets",
        description = "List explicit targets, pinned identities and actual native capabilities. No discovery or wakeup of virtual machines.",
        annotations(read_only_hint = true)
    )]
    async fn targets(&self) -> CallToolResult {
        self.execute(|e|value(json!({"targets":e.targets(),"implementation":if cfg!(feature="synthetic-fixture"){"synthetic-only-test-build"}else{"rust-native-candidate"},"client_exposed_tools":"unknown; inspect the host allowlist"}))).await
    }
    #[tool(
        name = "computer.observe",
        description = "Capture a native-size window PNG with explicit normalization metadata and a short-lived frame. No input; no resolution changes. Capture permission must already exist.",
        annotations(read_only_hint = true)
    )]
    async fn observe(&self, Parameters(a): Parameters<ObserveArgs>) -> CallToolResult {
        let views = self.views.clone();
        self.execute_operation(a.operation_id.clone(), move |e| {
            let observation = e.observe_with_region(&a.target, a.region)?;
            let mut r = value(&observation)?;
            if a.inline_image
                && let Some(aid) = observation.artifact_id
            {
                let (bytes, _, total) = e.artifact(&aid, 0, rcu_core::INLINE_BYTES)?;
                if bytes.len() == total {
                    r.content
                        .push(Content::image(STANDARD.encode(bytes), "image/png"));
                } else {
                    let view = make_view(
                        e,
                        &views,
                        &ImageArgs {
                            target: a.target,
                            frame: observation.frame,
                            artifact_id: aid,
                            crop: None,
                            max_edge: 1200,
                            operation_id: None,
                        },
                    )?;
                    r.content.extend(view.content);
                }
            }
            Ok(r)
        })
        .await
    }
    #[tool(
        name = "computer.resolve",
        description = "Bind one semantic element or original-image pixel to a specific frame, native window and PID. Ambiguous/unknown mappings are rejected.",
        annotations(read_only_hint = true)
    )]
    async fn resolve(&self, Parameters(a): Parameters<ResolveArgs>) -> CallToolResult {
        self.execute(move |e| value(e.resolve(&a.target, &a.frame, a.locator)?))
            .await
    }
    #[tool(
        name = "computer.claim",
        description = "Acquire a cross-process cooperative input lease for this exact desktop identity. TTL 100..60000 ms. Does not grant OS permissions or focus windows.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    async fn claim(&self, Parameters(a): Parameters<ClaimArgs>) -> CallToolResult {
        self.execute(move |e| {
            value(json!({"lease":e.claim(&a.target,a.ttl_ms)?,"ttl_ms":a.ttl_ms}))
        })
        .await
    }
    #[tool(
        name = "computer.act",
        description = "Dispatch one intent against a fresh native binding, lease and stable request_id. Windows needs an allocated input grant; Text/KeyChord need exact focused UIA element. Text: 1..1024 UTF-16 units, no control chars; Enter/Tab separate. Chords: Ctrl/Alt/Shift+key. Read capabilities for scroll units. No automatic focus or replay; independently verify.",
        annotations(read_only_hint = false, destructive_hint = true)
    )]
    async fn act(&self, Parameters(a): Parameters<ActArgs>) -> CallToolResult {
        self.execute_operation(a.operation_id.clone(), move |e| {
            value(e.act(
                &a.target,
                &a.reference,
                &a.lease,
                &a.request_id,
                a.intent,
                a.value,
            )?)
        })
        .await
    }
    #[tool(
        name = "computer.act_checked",
        description = "Preferred model action: select one exact element ID from a <=120s observed frame, claim a lease, then dispatch once per request_id. The server checks the original window/document context and exact element against fresh native state immediately before single durable dispatch. No pixel/name locator, implicit focus, or replay; unknown/changed context fails closed. Does not relax legacy act's 10s TTL. Do not loop reobserve after an error. Verify independently and release; Text/KeyChord still require exact focused UIA.",
        annotations(read_only_hint = false)
    )]
    async fn act_checked(&self, Parameters(a): Parameters<CheckedActArgs>) -> CallToolResult {
        self.execute_operation(a.operation_id, move |e| {
            value(e.act_checked(rcu_core::CheckedAction {
                target: &a.target,
                frame: &a.frame,
                element: &a.element,
                lease: &a.lease,
                request: &a.request_id,
                intent: a.intent,
                value: a.value,
            })?)
        })
        .await
    }
    #[tool(
        name = "computer.lifecycle_preflight",
        description = "Read bounded native application/window inventory and coverage notes for one explicit lifecycle authorization. No lease or dispatch: never launches, activates, sends input, captures pixels or grants authority. Partial/incomplete coverage is not proof of absence; inspect notes and cleanup status before separately requesting launch.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn lifecycle_preflight(
        &self,
        Parameters(a): Parameters<LifecyclePreflightArgs>,
    ) -> CallToolResult {
        self.execute_operation(a.operation_id.clone(), move |e| {
            value(e.lifecycle_preflight(&a.target, &a.authorization_id)?)
        })
        .await
    }
    #[tool(
        name = "computer.launch_application",
        description = "Launch one exact preauthorized application/argv/file set, once per authorization/run. Requires explicit lifecycle grant and lease. Returns bounded before/after native inventories and events: new_window_observed, reused_existing_window, launched_no_window, ambiguous or uncertain. Never equate PID or API acceptance with window/task success; no input grant. No invented Notepad clean-window switch. Observe 1..5000ms; no retry after reservation.",
        annotations(read_only_hint = false, destructive_hint = true)
    )]
    async fn launch_application(&self, Parameters(a): Parameters<LaunchArgs>) -> CallToolResult {
        self.execute_operation(a.operation_id.clone(), move |e| {
            value(e.launch_application(
                &a.target,
                &a.lease,
                rcu_core::lifecycle::LaunchRequest {
                    authorization_id: &a.authorization_id,
                    request_id: &a.request_id,
                    timeout_ms: a.timeout_ms,
                },
            )?)
        })
        .await
    }
    #[tool(
        name = "computer.activate_window",
        description = "Explicitly activate one exact authorized HWND/PID/birth/epoch from a <=10s native discovery. Requires separate lifecycle grant, lease, stable request_id and expected_foreground tuple. SetForegroundWindow once, verify native foreground, yield on user input/change; no retry, key injection or hidden activation in read-only bind. Focus observation is not an application goal or input authority.",
        annotations(read_only_hint = false, destructive_hint = true)
    )]
    async fn activate_window(&self, Parameters(a): Parameters<ActivateArgs>) -> CallToolResult {
        self.execute_operation(a.operation_id.clone(), move |e| {
            value(e.activate_window(
                &a.target,
                &a.lease,
                rcu_core::lifecycle::ActivateRequest {
                    authorization_id: &a.authorization_id,
                    request_id: &a.request_id,
                    expected_foreground: &a.expected_foreground,
                },
            )?)
        })
        .await
    }
    #[tool(
        name = "computer.lifecycle_receipt",
        description = "Read immutable lifecycle reservation/result for this connection native identity and grant run. kind=launch or activation. Works without an active action lease; does not dispatch, focus or grant input. Interrupted/reserved results remain uncertain and forbidden to replay.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn lifecycle_receipt(
        &self,
        Parameters(a): Parameters<LifecycleReceiptArgs>,
    ) -> CallToolResult {
        self.execute(move |e| {
            value(e.lifecycle_receipt(&a.target, &a.kind, &a.authorization_id, &a.request_id)?)
        })
        .await
    }
    #[tool(
        name = "computer.verify",
        description = "Poll an independent window/PID/element-scoped predicate, not the input acknowledgement. Element values require element id. Optional request_id must match the exact action scope. Equality does not establish causation.",
        annotations(read_only_hint = true)
    )]
    async fn verify(&self, Parameters(a): Parameters<VerifyArgs>) -> CallToolResult {
        self.execute_operation(a.operation_id.clone(), move |e| {
            value(e.verify(Verification {
                target: &a.target,
                frame: &a.frame,
                element: a.element,
                request: a.request_id,
                predicate: a.predicate,
                expected: &a.expected,
                timeout_ms: a.timeout_ms,
                stable_samples: a.stable_samples,
            })?)
        })
        .await
    }
    #[tool(
        name = "computer.release",
        description = "Release this connection's input lease. Never releases another connection's token.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    async fn release(&self, Parameters(a): Parameters<ReleaseArgs>) -> CallToolResult {
        self.execute(move |e| {
            e.release(&a.lease)?;
            value(json!({"released":true}))
        })
        .await
    }
    #[tool(
        name = "computer.artifact",
        description = "Read a bounded raw PNG byte range with SHA-256 and total size. No file writes or clipboard use. as_image requires the complete original PNG in one <=8 MiB range; partial bytes are never labeled as an image.",
        annotations(read_only_hint = true)
    )]
    async fn artifact(&self, Parameters(a): Parameters<ArtifactArgs>) -> CallToolResult {
        self.execute(move|e|{let(bytes,hash,total)=e.artifact(&a.artifact_id,a.offset,a.length)?;if a.as_image&&(a.offset!=0||bytes.len()!=total){return Err(fault("incomplete_image","Read the whole PNG, or reconstruct raw chunks and validate SHA-256."));}let mut r=value(json!({"artifact_id":a.artifact_id,"offset":a.offset,"length":bytes.len(),"total_bytes":total,"sha256":hash,"mime":"image/png","base64":if a.as_image{Value::Null}else{json!(STANDARD.encode(&bytes))}}))?;if a.as_image{r.content.push(Content::image(STANDARD.encode(bytes),"image/png"));}Ok(r)}).await
    }
    #[tool(
        name = "computer.discover",
        description = "Discover windows in the allocated desktop: at most 8 windows / 12 KiB structured data per page. Prefer an exact pid or title_contains filter to avoid model-side long enumeration. Use next_cursor alone for the same cached result; complete=false is not absence. Read-only, no focus or VM wakeup.",
        annotations(read_only_hint = true)
    )]
    async fn discover(&self, Parameters(a): Parameters<TargetArgs>) -> CallToolResult {
        self.execute_operation(a.operation_id, move |e| {
            value(e.discover_with(
                &a.target,
                rcu_core::DiscoveryQuery {
                    cursor: a.cursor,
                    pid: a.pid,
                    title_contains: a.title_contains,
                },
            )?)
        })
        .await
    }
    #[tool(
        name = "computer.bind_window",
        description = "Select an exact recently discovered PID/HWND/process epoch; popup/return transitions require native owner-chain evidence, never similar titles. Invalidates all old frames/references; does not focus or input.",
        annotations(read_only_hint = true, destructive_hint = false)
    )]
    async fn bind_window(&self, Parameters(a): Parameters<BindArgs>) -> CallToolResult {
        self.execute_operation(a.operation_id, move |e| {
            value(e.bind_window(&a.target, &a.selection)?)
        })
        .await
    }
    #[tool(
        name = "computer.image",
        description = "Return a directly viewable mapped PNG preview or native-resolution tile from the preserved original. crop=[x,y,width,height] uses ORIGINAL pixels; max_edge=64..2048. Never labels downsampling as original success.",
        annotations(read_only_hint = true)
    )]
    async fn image(&self, Parameters(a): Parameters<ImageArgs>) -> CallToolResult {
        let views = self.views.clone();
        self.execute_operation(a.operation_id.clone(), move |e| make_view(e, &views, &a))
            .await
    }
    #[tool(
        name = "computer.resolve_view",
        description = "Map pixels from a specific delivered preview/tile back to its original frame and native desktop before creating a fresh reference. Do not pass view pixels to computer.resolve.",
        annotations(read_only_hint = true)
    )]
    async fn resolve_view(&self, Parameters(a): Parameters<ResolveViewArgs>) -> CallToolResult {
        let views = self.views.clone();
        self.execute(move |e| {
            let v = views
                .lock()
                .map_err(|_| fault("view_registry_unavailable", "Reobserve."))?
                .iter()
                .find(|(id, v)| id == &a.view_id && v.target == a.target)
                .map(|(_, v)| v.clone())
                .ok_or_else(|| {
                    fault(
                        "unknown_view",
                        "Request a view of this exact target/frame; registry is bounded.",
                    )
                })?;
            e.original(&a.target, &v.frame, &v.artifact_id)?;
            let [x, y] = v.mapping.original_point([a.x, a.y])?;
            value(e.resolve(&a.target, &v.frame, Locator::Pixel { x, y })?)
        })
        .await
    }
    #[tool(
        name = "computer.cancel",
        description = "Request cooperative cancellation by returned or caller-supplied operation_id, even while the engine is busy. Acknowledgement does NOT mean native worker stopped; inspect terminal/resources_released. Never replay input.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    async fn cancel(&self, Parameters(a): Parameters<OperationArgs>) -> CallToolResult {
        match self.operations.get(&a.operation_id, true) {
            Ok(s) => value(s).unwrap_or_else(error),
            Err(e) => error(e),
        }
    }
    #[tool(
        name = "computer.operation",
        description = "Read bounded operation lifecycle without waiting for a busy engine. native_call_pending and resources_released distinguish cancellation request from completed cleanup; hard native preemption is unavailable.",
        annotations(read_only_hint = true)
    )]
    async fn operation(&self, Parameters(a): Parameters<OperationArgs>) -> CallToolResult {
        match self.operations.get(&a.operation_id, false) {
            Ok(s) => value(s).unwrap_or_else(error),
            Err(e) => error(e),
        }
    }
}
#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "real-computer-use",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }
}

#[cfg(feature = "synthetic-fixture")]
fn synthetic_engine(args: &[String]) -> CoreResult<(Engine, Option<String>)> {
    // Deliberately no grant, native selector, pipe server, or default state path.
    if args.len() != 3 || args[0] != "--synthetic-only" || args[1] != "--state-dir" {
        return Err(fault(
            "synthetic_only_cli",
            "Use only --synthetic-only --state-dir ABS. This test build cannot register native targets.",
        ));
    }
    let state = PathBuf::from(&args[2]);
    if !state.is_absolute() {
        return Err(fault(
            "invalid_cli",
            "Synthetic state directory must be absolute.",
        ));
    }
    let mut engine = Engine::new(state.clone())?;
    #[cfg(feature = "link-fixture")]
    engine.register(
        "synthetic-only".into(),
        Box::new(link_fixture::Instrumented::open(state)?),
    )?;
    #[cfg(not(feature = "link-fixture"))]
    engine.register(
        "synthetic-only".into(),
        Box::new(rcu_core::fixture::Fixture::default()),
    )?;
    Ok((engine, None))
}

fn engine_from_args() -> CoreResult<(Engine, Option<String>)> {
    #[cfg(feature = "synthetic-fixture")]
    {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.iter().any(|arg| arg == "--synthetic-only") {
            return synthetic_engine(&args);
        }
    }
    let mut args = std::env::args().skip(1);
    let mut state = None;
    let mut macos = None;
    let mut allow_input = false;
    let mut windows = false;
    let mut windows_grant = None;
    let mut lifecycle_grant_path = None;
    let mut pipe_server = None;
    #[cfg(feature = "native-agent-worker")]
    let mut agent_guard_path = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            #[cfg(feature = "native-agent-worker")]
            "--agent-guard" => {
                if agent_guard_path.is_some() {
                    return Err(fault(
                        "invalid_cli",
                        "--agent-guard must occur exactly once.",
                    ));
                }
                agent_guard_path = Some(PathBuf::from(args.next().ok_or_else(|| {
                    fault(
                        "invalid_cli",
                        "--agent-guard requires an absolute private worker spec.",
                    )
                })?));
            }
            "--state-dir" => {
                state = Some(PathBuf::from(args.next().ok_or_else(|| {
                    fault("invalid_cli", "--state-dir needs an absolute path.")
                })?))
            }
            "--macos-window" => {
                macos =
                    Some(args.next().ok_or_else(|| {
                        fault("invalid_cli", "--macos-window needs PID:CGWindowID.")
                    })?)
            }
            "--allow-input" => allow_input = true,
            "--windows-desktop" => windows = true,
            "--windows-input-grant" => {
                windows_grant = Some(PathBuf::from(args.next().ok_or_else(|| {
                    fault(
                        "invalid_cli",
                        "--windows-input-grant requires an absolute allocation JSON path.",
                    )
                })?));
            }
            "--windows-lifecycle-grant" => {
                lifecycle_grant_path = Some(PathBuf::from(args.next().ok_or_else(|| {
                    fault(
                        "invalid_cli",
                        "--windows-lifecycle-grant needs an absolute JSON path.",
                    )
                })?));
            }
            "--pipe-server" => {
                pipe_server = Some(args.next().ok_or_else(|| {
                    fault("invalid_cli", "--pipe-server requires a local run ID.")
                })?)
            }
            _ => {
                return Err(fault(
                    "invalid_cli",
                    "Use --state-dir PATH, optionally --macos-window PID:ID and --allow-input. Defaults to zero targets.",
                ));
            }
        }
    }
    #[cfg(feature = "native-agent-worker")]
    if let Some(path) = &agent_guard_path {
        agent_worker::validate_guard_cli(path, windows, pipe_server.as_deref(), macos.is_some())?;
        #[cfg(not(windows))]
        return Err(fault(
            "unsupported_platform",
            "The native-agent-worker route must run in the explicitly allocated interactive Windows session.",
        ));
    }
    let state = state
        .or_else(|| std::env::var_os("RCU_STATE_DIR").map(PathBuf::from))
        .unwrap_or_else(|| {
            let base = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("LOCALAPPDATA"))
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            base.join(".local/state/real-computer-use")
        });
    if !state.is_absolute() {
        return Err(fault(
            "invalid_cli",
            "State directory must be absolute and shared by all cooperating RCU servers for this user.",
        ));
    }
    if pipe_server.is_some() && !windows {
        return Err(fault(
            "invalid_cli",
            "--pipe-server requires an explicitly allocated --windows-desktop observation target.",
        ));
    }
    if windows_grant.is_some() && (!windows || !allow_input) {
        return Err(fault(
            "invalid_cli",
            "--windows-input-grant requires --windows-desktop and --allow-input.",
        ));
    }
    if lifecycle_grant_path.is_some() && !windows {
        return Err(fault(
            "invalid_cli",
            "Lifecycle grants require --windows-desktop; they do not implicitly enable input.",
        ));
    }
    if windows && allow_input && windows_grant.is_none() {
        return Err(fault(
            "input_grant_required",
            "Windows input requires an explicit root/window/native-identity/expiry allocation JSON.",
        ));
    }
    // The feature is a separate synthetic-only build, not an extra production
    // target selector. Reject before reading grants or constructing an OS adapter,
    // even when the caller omitted --synthetic-only.
    if cfg!(feature = "synthetic-fixture") && (windows || macos.is_some()) {
        return Err(fault(
            "synthetic_native_forbidden",
            "This synthetic-only test build cannot register any native target.",
        ));
    }
    #[cfg(all(windows, feature = "native-agent-worker"))]
    if let Some(path) = &agent_guard_path {
        agent_worker::install(agent_worker::WindowsGuard::load(path)?)?;
    }
    let mut engine = Engine::new(state)?;
    if windows {
        if macos.is_some() {
            return Err(fault(
                "invalid_cli",
                "Windows target cannot be combined with macOS.",
            ));
        }
        let grant = windows_grant
            .map(|p| {
                if !p.is_absolute() {
                    return Err(fault("invalid_cli", "Input grant path must be absolute."));
                }
                #[cfg(all(windows, feature = "native-agent-worker"))]
                if agent_worker::installed().is_some() {
                    return serde_json::from_slice::<rcu_windows::InputGrant>(
                        &agent_worker::private_grant(p, 16384)?,
                    )
                    .map_err(|_| {
                        fault(
                            "invalid_input_grant",
                            "Use the exact private input grant schema.",
                        )
                    });
                }
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(p)?
                    .take(16385)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > 16384 {
                    return Err(fault("invalid_input_grant", "Input grant exceeds 16 KiB."));
                }
                serde_json::from_slice::<rcu_windows::InputGrant>(&bytes).map_err(|_| {
                    fault(
                        "invalid_input_grant",
                        "Use exact grant schema; unknown fields are rejected.",
                    )
                })
            })
            .transpose()?;
        let lifecycle_grant = lifecycle_grant_path
            .map(|p| {
                if !p.is_absolute() {
                    return Err(fault(
                        "invalid_cli",
                        "Lifecycle grant path must be absolute.",
                    ));
                }
                #[cfg(all(windows, feature = "native-agent-worker"))]
                if agent_worker::installed().is_some() {
                    return serde_json::from_slice::<rcu_core::lifecycle::LifecycleGrant>(
                        &agent_worker::private_grant(p, 65536)?,
                    )
                    .map_err(|_| {
                        fault(
                            "invalid_lifecycle_grant",
                            "Use the exact private lifecycle grant schema.",
                        )
                    });
                }
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(p)?
                    .take(65537)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > 65536 {
                    return Err(fault(
                        "invalid_lifecycle_grant",
                        "Lifecycle grant exceeds 64 KiB.",
                    ));
                }
                serde_json::from_slice::<rcu_core::lifecycle::LifecycleGrant>(&bytes).map_err(
                    |_| {
                        fault(
                            "invalid_lifecycle_grant",
                            "Use exact lifecycle schema; unknown fields are rejected.",
                        )
                    },
                )
            })
            .transpose()?;
        #[cfg(windows)]
        {
            #[cfg(feature = "native-agent-worker")]
            let backend: Box<dyn rcu_core::Backend> = if let Some(guard) = agent_worker::installed()
            {
                let (mut grant, mut lifecycle_grant) = (grant, lifecycle_grant);
                guard.constrain_grants(&mut grant, &mut lifecycle_grant)?;
                Box::new(agent_worker::wrap(
                    rcu_windows::NativeWindows::new_with_grants(grant, lifecycle_grant)?,
                    guard.clone(),
                )?)
            } else {
                Box::new(rcu_windows::NativeWindows::new_with_grants(
                    grant,
                    lifecycle_grant,
                )?)
            };
            #[cfg(not(feature = "native-agent-worker"))]
            let backend: Box<dyn rcu_core::Backend> = Box::new(
                rcu_windows::NativeWindows::new_with_grants(grant, lifecycle_grant)?,
            );
            engine.register("windows-local".into(), backend)?;
        }
        #[cfg(not(windows))]
        {
            let _ = (grant, lifecycle_grant);
            return Err(fault(
                "unsupported_platform",
                "--windows-desktop must run inside the explicitly allocated interactive Windows session.",
            ));
        }
    }
    if let Some(selector) = macos {
        #[cfg(target_os = "macos")]
        {
            let (pid, window) = selector
                .split_once(':')
                .ok_or_else(|| fault("invalid_cli", "Expected PID:CGWindowID."))?;
            let pid = pid
                .parse()
                .map_err(|_| fault("invalid_cli", "PID must be numeric."))?;
            let window = window
                .parse()
                .map_err(|_| fault("invalid_cli", "CGWindowID must be numeric."))?;
            engine.register(
                "macos-local".into(),
                Box::new(rcu_macos::NativeMac::new(pid, window, allow_input)?),
            )?;
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (selector, allow_input);
            return Err(fault(
                "unsupported_platform",
                "macOS native target requested on another OS.",
            ));
        }
    } else if allow_input && !windows {
        return Err(fault(
            "invalid_cli",
            "--allow-input requires an explicitly pinned native target.",
        ));
    }
    Ok((engine, pipe_server))
}
fn main() {
    eof_diagnostics::phase("main_enter");
    let result = (|| -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (engine, pipe_server) = engine_from_args()?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(2)
            .enable_all()
            .build()?;
        eof_diagnostics::phase("runtime_block_on_begin");
        let result = runtime.block_on(async {
            #[cfg(windows)]
            if let Some(id) = pipe_server {
                #[cfg(feature = "native-agent-worker")]
                if let Some(guard) = agent_worker::installed() {
                    let (pipe, peer) = rcu_windows::pipe::accept_owned(
                        &id,
                        guard.owner(),
                        guard.connection_budget(),
                    )
                    .await?;
                    guard.bind_peer(peer)?;
                    let service = Server::new(engine)
                        .serve(eof_diagnostics::transport(pipe))
                        .await?;
                    eof_diagnostics::phase("service_wait_begin");
                    let result = service.waiting().await;
                    eof_diagnostics::phase("service_wait_returned");
                    result?;
                    return Ok::<_, Box<dyn std::error::Error>>(());
                }
                let pipe = rcu_windows::pipe::accept(&id).await?;
                let service = Server::new(engine)
                    .serve(eof_diagnostics::transport(pipe))
                    .await?;
                eof_diagnostics::phase("service_wait_begin");
                let result = service.waiting().await;
                eof_diagnostics::phase("service_wait_returned");
                result?;
                return Ok::<_, Box<dyn std::error::Error>>(());
            }
            #[cfg(not(windows))]
            let _ = pipe_server;
            let service = Server::new(engine)
                .serve(eof_diagnostics::transport(rmcp::transport::stdio()))
                .await?;
            eof_diagnostics::phase("service_wait_begin");
            let result = service.waiting().await;
            eof_diagnostics::phase("service_wait_returned");
            result?;
            Ok::<_, Box<dyn std::error::Error>>(())
        });
        eof_diagnostics::phase("runtime_block_on_returned");
        eof_diagnostics::phase("runtime_drop_begin");
        drop(runtime);
        eof_diagnostics::phase("runtime_drop_returned");
        result
    })();
    if let Err(e) = result {
        // GUI-subsystem Windows services may have no stderr handle. Preserve
        // bounded startup/transport diagnostics only in the explicit state dir.
        use std::io::Write;
        let args: Vec<_> = std::env::args().collect();
        if let Some(path) = args
            .windows(2)
            .find(|p| p[0] == "--state-dir")
            .map(|p| PathBuf::from(&p[1]))
            && path.is_absolute()
            && path.is_dir()
            && let Ok(mut log) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path.join("last-process-error.log"))
        {
            let _ = writeln!(
                log,
                "{}",
                e.to_string().chars().take(2048).collect::<String>()
            );
        }
        let _ = writeln!(std::io::stderr(), "{e}");
        std::process::exit(1);
    }
    eof_diagnostics::phase("main_return");
}

#[cfg(test)]
mod tests {
    use super::*;
    struct RasterBackend(Vec<u8>);
    impl rcu_core::Backend for RasterBackend {
        fn identity(&self) -> rcu_core::Identity {
            rcu_core::Identity {
                host: "offline-image-test".into(),
                guest: None,
                session: "fixture".into(),
                seat: "fixture".into(),
                desktop: "fixture".into(),
            }
        }
        fn capabilities(&self) -> Value {
            json!({"backend":"synthetic-image-fixture","input":false})
        }
        fn observe(&mut self) -> CoreResult<rcu_core::Scene> {
            Ok(rcu_core::Scene {
                identity: self.identity(),
                assurance: "synthetic-fixture".into(),
                binding: Some(rcu_core::Binding {
                    scope: rcu_core::Scope {
                        window: "fixture".into(),
                        pid: 42,
                        element: None,
                    },
                    focus_window: "fixture".into(),
                    layout: "physical-fixture".into(),
                    content: rcu_core::digest(&self.0),
                    native_epoch: "fixture".into(),
                    mutation_context: None,
                }),
                mutation_ready: false,
                blocked: Some(false),
                image_size: [200, 100],
                pixel_to_desktop: Some([1., 0., 0., 1., -300., -100.]),
                elements: vec![],
                captured_unix_ms: 0,
                atomicity: "fixture".into(),
                image_normalization: "fixture RGB".into(),
                png: self.0.clone(),
            })
        }
        fn act(
            &mut self,
            _: &rcu_core::Binding,
            _: &rcu_core::Resolved,
            _: &Intent,
            _: Option<&str>,
        ) -> CoreResult<Option<bool>> {
            Err(fault("fixture_read_only", "No input exists."))
        }
        fn inspect(
            &mut self,
            _: &rcu_core::Scope,
            _: &Predicate,
            _: &str,
        ) -> CoreResult<rcu_core::Oracle> {
            Err(fault("fixture_read_only", "Not an oracle fixture."))
        }
    }
    #[tokio::test]
    async fn image_tool_emits_viewable_png_and_pixel_mapping_is_exact_frame_scoped() {
        let state = tempfile::tempdir().unwrap();
        let mut e = Engine::new(state.path().into()).unwrap();
        let original = rcu_imaging::encode_rgb([200, 100], &vec![77; 200 * 100 * 3]).unwrap();
        e.register("raster".into(), Box::new(RasterBackend(original.clone())))
            .unwrap();
        let one = e.observe("raster").unwrap();
        let two = e.observe("raster").unwrap();
        let aid = one.artifact_id.unwrap();
        assert!(e.original("raster", &two.frame, &aid).is_err());
        let server = Server::new(e);
        let image = server
            .image(Parameters(ImageArgs {
                target: "raster".into(),
                frame: one.frame.clone(),
                artifact_id: aid.clone(),
                crop: Some([20, 10, 100, 80]),
                max_edge: 1200,
                operation_id: None,
            }))
            .await;
        let wire = serde_json::to_value(&image).unwrap();
        let content = wire["content"].as_array().unwrap();
        let png = content.iter().find(|c| c["type"] == "image").unwrap();
        assert_eq!(png["mimeType"], "image/png");
        let bytes = STANDARD.decode(png["data"].as_str().unwrap()).unwrap();
        assert_eq!(
            rcu_imaging::decode_original(&bytes).unwrap().size,
            [100, 80]
        );
        let view_id = wire["structuredContent"]["view_id"]
            .as_str()
            .unwrap()
            .to_string();
        let resolved = server
            .resolve_view(Parameters(ResolveViewArgs {
                target: "raster".into(),
                view_id: view_id.clone(),
                x: 4.,
                y: 5.,
            }))
            .await;
        assert_eq!(
            resolved.structured_content.unwrap()["point"],
            json!([-276., -85.])
        );
        let wrong = server
            .resolve_view(Parameters(ResolveViewArgs {
                target: "other".into(),
                view_id,
                x: 4.,
                y: 5.,
            }))
            .await;
        assert_eq!(wrong.structured_content.unwrap()["error"], "unknown_view");
        let engine = server.engine.lock().unwrap();
        assert_eq!(
            engine.original("raster", &one.frame, &aid).unwrap().0,
            original
        );
    }
    #[test]
    fn instructions_fit_first_512_characters() {
        assert!(INSTRUCTIONS.len() <= 512);
    }
    #[test]
    fn schema_rejects_unknown_action_arguments() {
        assert!(serde_json::from_value::<ActArgs>(json!({"target":"a","reference":"b","lease":"c","request_id":"d","intent":"click","args":[]})).is_err());
    }
    #[test]
    fn checked_schema_accepts_only_exact_element_and_no_locator_overrides() {
        let valid = json!({"target":"a","frame":"b","element":"exact-id","lease":"c","request_id":"d","intent":"click"});
        assert!(serde_json::from_value::<CheckedActArgs>(valid.clone()).is_ok());
        for field in ["locator", "reference", "name", "x", "y", "args"] {
            let mut args = valid.clone();
            args[field] = json!({"kind":"pixel","x":1,"y":2});
            assert!(serde_json::from_value::<CheckedActArgs>(args).is_err());
        }
        for element in [
            json!({"kind":"semantic","name":"Input"}),
            json!([1, 2]),
            json!(true),
            Value::Null,
        ] {
            let mut args = valid.clone();
            args["element"] = element;
            assert!(serde_json::from_value::<CheckedActArgs>(args).is_err());
        }
    }
    #[tokio::test]
    async fn checked_endpoint_dispatches_fixture_once_and_preserves_operation_receipt() {
        let state = tempfile::tempdir().unwrap();
        let fixture = rcu_core::fixture::Fixture::default();
        let mut engine = Engine::new(state.path().into()).unwrap();
        engine
            .register("fixture".into(), Box::new(fixture.clone()))
            .unwrap();
        let frame = engine.observe("fixture").unwrap().frame;
        let lease = engine.claim("fixture", 60_000).unwrap();
        let server = Server::new(engine);
        let args = || CheckedActArgs {
            target: "fixture".into(),
            frame: frame.clone(),
            element: "Input".into(),
            lease: lease.clone(),
            request_id: "checked-once".into(),
            intent: Intent::Text,
            value: Some("checked fixture value".into()),
            operation_id: None,
        };
        let result = server.act_checked(Parameters(args())).await;
        assert_ne!(result.is_error, Some(true), "{result:?}");
        let receipt = result.structured_content.as_ref().unwrap();
        assert_eq!(receipt["request_id"], "checked-once");
        assert_eq!(receipt["scope"]["element"], "Input");
        assert_eq!(receipt["dispatch"], "returned");
        assert_eq!(receipt["goal_verified"], false);
        assert_eq!(receipt["replay"], "forbidden");
        query_response_operation(&server, &result).await;
        let replay = server.act_checked(Parameters(args())).await;
        assert_ne!(replay.is_error, Some(true), "{replay:?}");
        assert_eq!(
            replay.structured_content.as_ref().unwrap()["deduplicated"],
            true
        );
        query_response_operation(&server, &replay).await;
        let app = fixture.0.lock().unwrap();
        assert_eq!(app.calls, 1);
        assert_eq!(app.value, "checked fixture value");
    }
    #[tokio::test]
    async fn checked_endpoint_rejects_changed_document_without_dispatch() {
        let state = tempfile::tempdir().unwrap();
        let fixture = rcu_core::fixture::Fixture::default();
        let mut engine = Engine::new(state.path().into()).unwrap();
        engine
            .register("fixture".into(), Box::new(fixture.clone()))
            .unwrap();
        let frame = engine.observe("fixture").unwrap().frame;
        let lease = engine.claim("fixture", 60_000).unwrap();
        fixture.0.lock().unwrap().value = "document changed after observation".into();
        let server = Server::new(engine);
        let response = server
            .act_checked(Parameters(CheckedActArgs {
                target: "fixture".into(),
                frame,
                element: "Toggle test option".into(),
                lease,
                request_id: "changed-document".into(),
                intent: Intent::Click,
                value: None,
                operation_id: Some("checked-context-error".into()),
            }))
            .await;
        assert_eq!(response.is_error, Some(true));
        assert_eq!(
            response.structured_content.as_ref().unwrap()["error"],
            "checked_precondition_changed"
        );
        let op = query_response_operation(&server, &response).await;
        assert_eq!(op["operation_id"], "checked-context-error");
        assert_eq!(op["terminal"], true);
        assert_eq!(op["resources_released"], true);
        assert_eq!(fixture.0.lock().unwrap().calls, 0);
        assert!(
            state
                .path()
                .join("receipts")
                .read_dir()
                .unwrap()
                .next()
                .is_none()
        );
    }
    #[test]
    fn boolean_is_not_a_coordinate() {
        assert!(
            serde_json::from_value::<ResolveArgs>(
                json!({"target":"a","frame":"b","locator":{"kind":"pixel","x":true,"y":0}})
            )
            .is_err()
        );
    }
    async fn query_response_operation(server: &Server, response: &CallToolResult) -> Value {
        let data = response.structured_content.as_ref().unwrap();
        let operation_id = data["operation_id"].as_str().unwrap();
        assert!(!operation_id.is_empty());
        assert_eq!(data["operation"]["operation_id"], operation_id);
        let query = server
            .operation(Parameters(OperationArgs {
                operation_id: operation_id.into(),
            }))
            .await;
        assert_ne!(query.is_error, Some(true));
        let snapshot = query.structured_content.unwrap();
        assert_eq!(data["operation"], snapshot);
        let wire = serde_json::to_value(response).unwrap();
        let last = wire["content"].as_array().unwrap().last().unwrap();
        let metadata: Value = serde_json::from_str(last["text"].as_str().unwrap()).unwrap();
        assert_eq!(metadata["operation_id"], operation_id);
        assert_eq!(metadata["operation"], snapshot);
        snapshot
    }
    #[tokio::test]
    async fn early_native_pin_error_keeps_diagnostic_through_mcp_worker_and_text_blocks() {
        let state = tempfile::tempdir().unwrap();
        let server = Server::new(Engine::new(state.path().into()).unwrap());
        let diagnostic = json!({"schema":"rcu.windows.pin.v1","api":"CreateFileW","stage":"open",
            "request":{"authorized_file":"C:\\Protected\\app.exe","path":"C:\\Protected","desired_access":32,"share_mode":1,"flags":0x0220_0000},
            "native_error":{"hresult":"0x80070005","win32_code":5},
            "verification_context":{"phase":"launch_preflight","role":"selected_launcher"},
            "effective_token":{"status":"unknown","reason":"offline fixture"}});
        let mut failure = fault(
            "application_directory_pin_failed",
            "Original native failure; no dispatch.",
        );
        failure.diagnostic = Some(Box::new(diagnostic.clone()));
        let response = server.execute(move |_| Err(failure)).await;
        let payload = response.structured_content.as_ref().unwrap();
        assert_eq!(response.is_error, Some(true));
        assert_eq!(payload["error"], "application_directory_pin_failed");
        assert_eq!(payload["diagnostic"], diagnostic);
        let wire = serde_json::to_value(&response).unwrap();
        let first_text: Value =
            serde_json::from_str(wire["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(first_text["diagnostic"], diagnostic);
        let status = query_response_operation(&server, &response).await;
        assert_eq!(status["terminal"], true);
        assert_eq!(status["resources_released"], true);
        assert_eq!(status["native_call_depth"], 0);
        let old = error(fault("legacy", "unchanged"));
        assert_eq!(
            old.structured_content.unwrap(),
            json!({"error":"legacy","next_step":"unchanged"})
        );
    }
    #[tokio::test]
    async fn auto_operation_id_and_actual_snapshot_survive_success_and_core_error() {
        let state = tempfile::tempdir().unwrap();
        let server = Server::new(Engine::new(state.path().into()).unwrap());
        let successful = server
            .execute(|_| value(json!({"existing_field": 7})))
            .await;
        assert_eq!(
            successful.structured_content.as_ref().unwrap()["existing_field"],
            7
        );
        let snapshot = query_response_operation(&server, &successful).await;
        assert_eq!(snapshot["terminal"], true);
        assert_eq!(snapshot["resources_released"], true);
        let failed = server
            .execute(|_| {
                Err(fault(
                    "offline_failure",
                    "The original next step is preserved.",
                ))
            })
            .await;
        assert_eq!(failed.is_error, Some(true));
        assert_eq!(
            failed.structured_content.as_ref().unwrap()["error"],
            "offline_failure"
        );
        assert_eq!(
            failed.structured_content.as_ref().unwrap()["next_step"],
            "The original next step is preserved."
        );
        let snapshot = query_response_operation(&server, &failed).await;
        assert_eq!(snapshot["terminal"], true);
        assert_eq!(snapshot["resources_released"], true);
        assert_ne!(
            successful.structured_content.unwrap()["operation_id"],
            failed.structured_content.unwrap()["operation_id"]
        );
    }
    #[tokio::test]
    async fn worker_panic_returns_queryable_auto_id_and_cleanup_uncertainty() {
        let state = tempfile::tempdir().unwrap();
        let server = Server::new(Engine::new(state.path().into()).unwrap());
        let failed = server
            .execute(|_| {
                struct Cleanup;
                impl Drop for Cleanup {
                    fn drop(&mut self) {
                        rcu_core::operation::resource_release_uncertain();
                    }
                }
                let _cleanup = Cleanup;
                panic!("offline worker-panic fixture");
            })
            .await;
        assert_eq!(failed.is_error, Some(true));
        assert_eq!(
            failed.structured_content.as_ref().unwrap()["error"],
            "worker_failed"
        );
        let snapshot = query_response_operation(&server, &failed).await;
        assert_eq!(snapshot["terminal"], true);
        assert_eq!(snapshot["resource_release_uncertain"], true);
        assert_eq!(snapshot["resources_released"], false);
        assert_eq!(snapshot["native_call_pending"], false);
    }
    #[tokio::test]
    async fn core_error_does_not_hide_failed_resource_cleanup() {
        let state = tempfile::tempdir().unwrap();
        let server = Server::new(Engine::new(state.path().into()).unwrap());
        let failed = server
            .execute_operation(Some("cleanup-uncertain".into()), |_| {
                rcu_core::operation::resource_release_uncertain();
                Err(fault(
                    "cleanup_fixture_failed",
                    "Do not claim resources were released.",
                ))
            })
            .await;
        let snapshot = query_response_operation(&server, &failed).await;
        assert_eq!(snapshot["operation_id"], "cleanup-uncertain");
        assert_eq!(snapshot["terminal"], true);
        assert_eq!(snapshot["resource_release_uncertain"], true);
        assert_eq!(snapshot["resources_released"], false);
    }
    #[tokio::test]
    async fn completed_join_cannot_fabricate_terminal_with_a_live_native_guard() {
        let state = tempfile::tempdir().unwrap();
        let server = Server::new(Engine::new(state.path().into()).unwrap());
        let (tx, rx) = std::sync::mpsc::channel();
        let response = server
            .execute(move |_| {
                // This only exercises Rust lifecycle counters; it invokes no OS API.
                tx.send(rcu_core::operation::native_call("retained_offline_guard")?)
                    .unwrap();
                value(json!({"worker_returned": true}))
            })
            .await;
        let snapshot = query_response_operation(&server, &response).await;
        assert_eq!(snapshot["finish_requested"], true);
        assert_eq!(snapshot["native_call_depth"], 1);
        assert_eq!(snapshot["native_call_pending"], true);
        assert_eq!(snapshot["terminal"], false);
        assert_eq!(snapshot["resources_released"], false);
        drop(rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap());
        let query = server
            .operation(Parameters(OperationArgs {
                operation_id: snapshot["operation_id"].as_str().unwrap().into(),
            }))
            .await;
        let completed = query.structured_content.unwrap();
        assert_eq!(completed["terminal"], true);
        assert_eq!(completed["resources_released"], true);
        assert_eq!(completed["native_call_depth"], 0);
        // The earlier response remains a truthful earlier snapshot.
        assert_eq!(
            response.structured_content.unwrap()["operation"]["terminal"],
            false
        );
    }
    #[tokio::test]
    async fn non_object_or_reserved_payload_is_preserved_without_spoofing_operation() {
        let state = tempfile::tempdir().unwrap();
        let server = Server::new(Engine::new(state.path().into()).unwrap());
        for original in [
            json!([1, 2]),
            json!({"operation_id":"not-the-registered-id","payload":9}),
        ] {
            let payload = original.clone();
            let response = server.execute(move |_| value(payload)).await;
            assert_eq!(
                response.structured_content.as_ref().unwrap()["result"],
                original
            );
            let snapshot = query_response_operation(&server, &response).await;
            assert_ne!(snapshot["operation_id"], "not-the-registered-id");
        }
    }
    #[test]
    fn lifecycle_preflight_schema_does_not_accept_dispatch_fields() {
        let good = json!({"target":"explicit","authorization_id":"launch","operation_id":null});
        assert!(serde_json::from_value::<LifecyclePreflightArgs>(good.clone()).is_ok());
        for key in ["lease", "arguments", "activate", "allow_input"] {
            let mut bad = good.clone();
            bad[key] = true.into();
            assert!(serde_json::from_value::<LifecyclePreflightArgs>(bad).is_err());
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancel_endpoint_remains_available_but_native_slot_is_held_until_cleanup() {
        let state = tempfile::tempdir().unwrap();
        let server = Server::new(Engine::new(state.path().into()).unwrap());
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel();
        let worker = server.clone();
        let request = tokio::spawn(async move {
            worker
                .execute_operation(Some("native-pending".into()), move |_| {
                    let native =
                        rcu_core::operation::native_call("synthetic_blocking_native_call")?;
                    started_tx.send(()).unwrap();
                    finish_rx
                        .recv_timeout(std::time::Duration::from_secs(3))
                        .unwrap();
                    drop(native);
                    rcu_core::operation::checkpoint("after_native")?;
                    value(json!({"done":true}))
                })
                .await
        });
        started_rx
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        let cancelled = server
            .cancel(Parameters(OperationArgs {
                operation_id: "native-pending".into(),
            }))
            .await;
        let s = cancelled.structured_content.unwrap();
        assert_eq!(s["native_call_pending"], true);
        assert_eq!(s["resources_released"], false);
        assert!(server.gate.clone().try_acquire_owned().is_err());
        finish_tx.send(()).unwrap();
        let result = request.await.unwrap();
        assert_eq!(
            result.structured_content.unwrap()["error"],
            "operation_cancelled"
        );
        let s = server.operations.get("native-pending", false).unwrap();
        assert!(s.terminal && s.resources_released && !s.native_call_pending);
        assert!(server.gate.clone().try_acquire_owned().is_ok());
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dropped_async_request_cancels_without_detaching_a_second_native_worker() {
        let state = tempfile::tempdir().unwrap();
        let server = Server::new(Engine::new(state.path().into()).unwrap());
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel();
        let worker = server.clone();
        let request = tokio::spawn(async move {
            worker
                .execute_operation(Some("dropped".into()), move |_| {
                    let _native = rcu_core::operation::native_call("synthetic_pending")?;
                    started_tx.send(()).unwrap();
                    finish_rx
                        .recv_timeout(std::time::Duration::from_secs(3))
                        .unwrap();
                    value(json!({}))
                })
                .await
        });
        started_rx
            .recv_timeout(std::time::Duration::from_secs(3))
            .unwrap();
        request.abort();
        assert!(request.await.unwrap_err().is_cancelled());
        let s = server.operations.get("dropped", false).unwrap();
        assert!(s.cancel_requested && !s.terminal && s.native_call_pending);
        assert!(server.gate.clone().try_acquire_owned().is_err());
        finish_tx.send(()).unwrap();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while !server.operations.get("dropped", false).unwrap().terminal {
            assert!(std::time::Instant::now() < until);
            tokio::task::yield_now().await;
        }
    }
}
