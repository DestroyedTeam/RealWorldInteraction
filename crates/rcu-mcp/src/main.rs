//! Official MCP SDK + Rust engine. stdout is exclusively MCP transport.
#![cfg_attr(windows, windows_subsystem = "windows")]
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

const INSTRUCTIONS: &str = "Read computer.targets capabilities. Targets are opt-in. Windows input needs a timed native-window grant and lease. Discover/bind, observe, resolve; keyboard actions require the exact focused UIA element. Use image/resolve_view for preview pixels. Popup binding needs native owner evidence. Act once with stable request_id; independently verify and release. Never replay uncertainty. Cancel is cooperative; await terminal/resources_released before resuming.";

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
        let engine = self.engine.clone();
        match tokio::task::spawn_blocking(move || {
            struct Finish {op:rcu_core::operation::Operation,permit:Option<tokio::sync::OwnedSemaphorePermit>}
            impl Drop for Finish {fn drop(&mut self){drop(self.permit.take());self.op.finish();}}
            let _finish=Finish{op:op.clone(),permit:Some(permit)};
            rcu_core::operation::run(&op,|| {
            rcu_core::operation::checkpoint("worker_started")?;
            let mut engine = engine.lock().map_err(|_| {
                fault(
                    "engine_unavailable",
                    "Restart, inspect durable receipts, and do not replay.",
                )
            })?;
            let mut result=f(&mut engine)?;
            result.content.push(Content::text(serde_json::json!({"operation_id":op.status().operation_id,"cancellation":"cooperative; native preemption unavailable"}).to_string()));
            Ok(result)
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
        }
    }
}
fn error(e: rcu_core::Fault) -> CallToolResult {
    CallToolResult::structured_error(json!({"error":e.code,"next_step":e.next_step}))
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
        self.execute(|e|value(json!({"targets":e.targets(),"implementation":"rust-native-candidate","client_exposed_tools":"unknown; inspect the host allowlist"}))).await
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
        description = "Request cooperative cancellation by caller-supplied operation_id, even while the engine is busy. Acknowledgement does NOT mean native worker stopped; inspect terminal/resources_released. Never replay input.",
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

fn engine_from_args() -> CoreResult<(Engine, Option<String>)> {
    let mut args = std::env::args().skip(1);
    let mut state = None;
    let mut macos = None;
    let mut allow_input = false;
    let mut windows = false;
    let mut windows_grant = None;
    let mut pipe_server = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
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
    let mut engine = Engine::new(state)?;
    if windows {
        if macos.is_some() {
            return Err(fault(
                "invalid_cli",
                "Windows target cannot be combined with macOS.",
            ));
        }
        if allow_input && windows_grant.is_none() {
            return Err(fault(
                "input_grant_required",
                "Windows input requires an explicit root/window/native-identity/expiry allocation JSON.",
            ));
        }
        let grant = windows_grant
            .map(|p| {
                if !p.is_absolute() {
                    return Err(fault("invalid_cli", "Input grant path must be absolute."));
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
        #[cfg(windows)]
        {
            engine.register(
                "windows-local".into(),
                Box::new(rcu_windows::NativeWindows::new_with_grant(grant)?),
            )?;
        }
        #[cfg(not(windows))]
        {
            let _ = grant;
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
    let result = (|| -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (engine, pipe_server) = engine_from_args()?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(2)
            .enable_all()
            .build()?;
        runtime.block_on(async {
            #[cfg(windows)]
            if let Some(id) = pipe_server {
                let pipe = rcu_windows::pipe::accept(&id).await?;
                let service = Server::new(engine).serve(pipe).await?;
                service.waiting().await?;
                return Ok::<_, Box<dyn std::error::Error>>(());
            }
            #[cfg(not(windows))]
            let _ = pipe_server;
            let service = Server::new(engine).serve(rmcp::transport::stdio()).await?;
            service.waiting().await?;
            Ok::<_, Box<dyn std::error::Error>>(())
        })
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
    fn boolean_is_not_a_coordinate() {
        assert!(
            serde_json::from_value::<ResolveArgs>(
                json!({"target":"a","frame":"b","locator":{"kind":"pixel","x":true,"y":0}})
            )
            .is_err()
        );
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
