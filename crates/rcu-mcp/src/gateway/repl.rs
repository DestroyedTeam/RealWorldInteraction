//! Host-local orchestration; owned interpreter process has no device credentials.
//! GUI/terminal are peers, but only advertised, implemented capabilities route.
mod process;
mod trace;
use super::*;
use rcu_js::protocol::{CALL_CAP, CODE_CAP, Cell, Event, OUTPUT_CAP};
use std::time::Duration;
use tokio::sync::{Mutex as AsyncMutex, watch};

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct JsArgs {
    /// Async function body. Persist objects in state.*, not cell-local let/const.
    code: String,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
    /// Optional generation returned by a previous call; mismatch never runs code.
    #[serde(default)]
    generation: Option<String>,
}
fn default_timeout() -> u64 {
    30_000
}

pub(super) fn tools() -> Vec<Tool> {
    vec![
        Tool::new(
            "computer.js",
            "Run bounded async JS in a persistent owned Rust/QuickJS worker. Available: rcu (RcuClient), state.*, emit(data), emitImage(reply.media[i]). Example: state.targets = await rcu.targets(); emit(state.targets). Use rcu.call(tool,args) for the advertised computer.* tools; rcu.window(exactTarget) provides bind, activate, snapshot, summary, observe, elementsPage, captureVisual and withInput. activate requires a fresh explicit foreground tuple; never silently focus. captureVisual({maxEdge:1024,feedback:\"exclude\"}) is response-only, no input frame; include does not guarantee a visible pointer. Prefer snapshot(): concise controls use act({ref,frame,intent,requestId}) with the exact returned frame. snapshot({preview:true}) adds a mapped 1024px preview; emit data once and media separately. Locals do not persist: use state.window. Await every call. withInput owns its target across wrappers: use its callback observe/snapshot/elementsPage, not outside reads. maxDurationMs stops new scheduling, not in-flight native effects. Cells run serially, max 64 calls / 60s; native authorization, lease, operation refs and no-replay remain authoritative. Errors, cancellation and reset discard JS state; stopping JS does NOT prove native effects stopped. Leases created here are released at cell end, never held across model turns. No Node, shell, filesystem or network API; terminal remains a separate complementary Host channel. Emit only needed data; tool calls are not automatically echoed.",
            Arc::new(
                serde_json::to_value(schemars::schema_for!(JsArgs))
                    .unwrap()
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        ),
        Tool::new(
            "computer.js_reset",
            "Stop the current JS cell and discard only this adapter's JS state. Other MCP calls and device services remain independent. Does not replay input or attest native termination; inspect the original cell's execution receipts.",
            Arc::new(
                serde_json::from_value(
                    json!({"type":"object","properties":{},"additionalProperties":false}),
                )
                .unwrap(),
            ),
        ),
    ]
}

pub(super) struct Repl {
    worker: AsyncMutex<Option<process::Worker>>,
    reset: watch::Sender<u64>,
}
impl Default for Repl {
    fn default() -> Self {
        Self {
            worker: AsyncMutex::new(None),
            reset: watch::channel(0).0,
        }
    }
}

impl Repl {
    pub async fn reset(&self) -> DeviceResult {
        self.reset.send_modify(|n| *n = n.wrapping_add(1));
        let mut slot = self.worker.lock().await;
        if let Some(mut worker) = slot.take() {
            worker.stop().await;
        }
        DeviceResult::value(
            json!({"js_state_discarded":true,"original_action_replayed":false,"native_termination_confirmed":false}),
        )
    }

    pub async fn evaluate(
        &self,
        gateway: &Gateway,
        args: serde_json::Map<String, Value>,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let args = match serde_json::from_value::<JsArgs>(Value::Object(args)) {
            Ok(a) if a.code.len() <= CODE_CAP && (100..=60_000).contains(&a.timeout_ms) => a,
            _ => {
                return mcp_result(DeviceResult::error(
                    "invalid_arguments",
                    "Use <=64KiB code and timeout_ms 100..60000.",
                ));
            }
        };
        let mut reset = self.reset.subscribe();
        let mut shutdown = gateway.shutdown.subscribe();
        let Ok(mut slot) = self.worker.try_lock() else {
            return mcp_result(DeviceResult::error(
                "js_busy",
                "One cell is running; control/status/reset tools remain available. Nothing queued.",
            ));
        };
        if context.ct.is_cancelled() || *shutdown.borrow() {
            return mcp_result(DeviceResult::error(
                "js_cancelled_before_start",
                "No code ran.",
            ));
        }
        if let Some(expected) = &args.generation
            && slot.as_ref().is_none_or(|w| &w.generation != expected)
        {
            return mcp_result(DeviceResult::error(
                "js_generation_expired",
                "JS state changed; inspect receipts and rebuild bindings, never replay input.",
            ));
        }
        if slot.is_none() {
            match process::Worker::start() {
                Ok(worker) => *slot = Some(worker),
                Err(e) => {
                    return mcp_result(DeviceResult::error(
                        "js_worker_start_failed",
                        &e.to_string(),
                    ));
                }
            }
        }
        // Take ownership out of the slot: if the Host drops this future, Drop
        // kills the worker instead of leaving pending JS for a later cell.
        let mut worker = slot.take().expect("started");
        let generation = worker.generation.clone();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(args.timeout_ms);
        let mut trace = trace::Trace::new(gateway.clone());
        let mut error = None;
        let cell = Cell {
            code: args.code,
            timeout_ms: args.timeout_ms,
        };
        let run = async {
            guarded(
                worker.send(&cell),
                &context,
                &mut reset,
                &mut shutdown,
                deadline,
            )
            .await?;
            loop {
                if trace.calls.len() > CALL_CAP {
                    return Err(io::Error::other("js_call_limit"));
                }
                match guarded(worker.read(), &context, &mut reset, &mut shutdown, deadline).await? {
                    Event::Call { tool, arguments } => {
                        if trace.calls.len() >= CALL_CAP {
                            return Err(io::Error::other("js_call_limit"));
                        }
                        // No nested REPL/reset recursion or unadvertised tools.
                        if tool != "computer.result"
                            && !device::catalog::definitions()
                                .iter()
                                .any(|d| d.name == tool)
                        {
                            return Err(io::Error::other("js_unknown_capability"));
                        }
                        let Some(arguments) = arguments.as_object().cloned() else {
                            return Err(io::Error::other("js_arguments_not_object"));
                        };
                        let mut nested = context.clone();
                        nested.ct = context.ct.child_token();
                        let token = nested.ct.clone();
                        let index = trace.begin(&tool, &arguments);
                        let future = gateway.call_native(
                            CallToolRequestParams::new(tool.clone())
                                .with_arguments(arguments.clone()),
                            nested,
                        );
                        tokio::pin!(future);
                        let reply = tokio::select! {
                            biased;
                            _ = context.ct.cancelled() => None,
                            _ = shutdown.changed() => None,
                            _ = reset.changed() => None,
                            _ = tokio::time::sleep_until(deadline) => None,
                            result = &mut future => Some(result),
                        };
                        let stopped = reply.is_none();
                        let reply = if let Some(reply) = reply { reply } else {
                            token.cancel();
                            // Preserve accepted execution reference and cancellation
                            // outcome from the existing gateway path (2s control cap).
                            tokio::time::timeout(Duration::from_secs(3), &mut future).await
                                .unwrap_or_else(|_| Ok(mcp_result(DeviceResult::error("js_dispatch_outcome_unconfirmed", "Inspect original request identity; input was not replayed."))))
                        }.unwrap_or_else(|e| mcp_result(DeviceResult::error("js_tool_error", &e.to_string())));
                        trace.finish(index, &tool, &arguments, &reply);
                        if stopped {
                            return Err(io::Error::other("js_stopped_during_call"));
                        }
                        guarded(
                            worker.send(&reply),
                            &context,
                            &mut reset,
                            &mut shutdown,
                            deadline,
                        )
                        .await?;
                    }
                    Event::Emit { value } => {
                        trace.output_bytes += serde_json::to_vec(&value)?.len();
                        if trace.output_bytes > OUTPUT_CAP || trace.outputs.len() >= 64 {
                            return Err(io::Error::other("js_output_limit"));
                        }
                        trace.outputs.push(value);
                    }
                    Event::Image { data, mime_type } => {
                        trace.image_bytes += data.len();
                        if trace.image_bytes > 8 * 1024 * 1024
                            || trace.images.len() >= 4
                            || !matches!(
                                mime_type.as_str(),
                                "image/png" | "image/jpeg" | "image/webp"
                            )
                        {
                            return Err(io::Error::other("js_image_limit_or_type"));
                        }
                        trace.images.push(Content::image(data, mime_type));
                    }
                    Event::Done { error: js_error } => {
                        if let Some(e) = js_error {
                            return Err(io::Error::other(e));
                        }
                        return Ok::<_, io::Error>(());
                    }
                }
            }
        };
        if let Err(e) = run.await {
            error = Some(e.to_string());
        }
        if error.is_some() {
            worker.stop().await;
        }
        trace.cleanup().await;
        if trace.cleanup_failed() && error.is_none() {
            error = Some("js_lease_cleanup_unconfirmed".into());
            worker.stop().await;
        }
        let retained = error.is_none();
        if retained {
            *slot = Some(worker);
        }
        trace.result(generation, retained, error)
    }
}

// Guard only IPC waits, never drop an accepted native invocation before its
// cancellation path can return the exact reference. Pure JS is killed by owner.
async fn guarded<T>(
    future: impl std::future::Future<Output = io::Result<T>>,
    context: &RequestContext<RoleServer>,
    reset: &mut watch::Receiver<u64>,
    shutdown: &mut watch::Receiver<bool>,
    deadline: tokio::time::Instant,
) -> io::Result<T> {
    tokio::select! {
        biased;
        _ = context.ct.cancelled() => Err(io::Error::other("js_host_cancelled")),
        _ = reset.changed() => Err(io::Error::other("js_reset")),
        _ = shutdown.changed() => Err(io::Error::other("js_host_disconnected")),
        _ = tokio::time::sleep_until(deadline) => Err(io::Error::other("js_deadline")),
        result = future => result,
    }
}
