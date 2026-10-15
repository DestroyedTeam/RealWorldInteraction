//! MCP adapter: tool routing, bounded execution and SDK responses.
//! No native target construction or process bootstrap lives here.
mod parameters;
mod views;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use parameters::*;
use views::{StoredView, make_view};
#[cfg(test)]
mod tests;
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
};
use rwi_core::Verification;
use rwi_core::operation::{CancelOnDrop, Registry};
use rwi_core::{Engine, Locator, Result as CoreResult, fault};
#[cfg(test)]
use rwi_core::{Intent, Predicate};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

const INSTRUCTIONS: &str = "Read computer.targets (opt-in). Prefer act_checked: observe, choose exact element ID, claim, dispatch once, verify, release. It rechecks document context on-server; no pixel/name selectors. Legacy act keeps its 10s TTL. Never loop reobserve to refresh failed actions. Launch/activate need grants+lease; bind never focuses. Partial preflight is not absence. Receipts are not goals/input grants. Never replay uncertainty; read lifecycle_receipt. Cancel cooperatively; await terminal/resources_released.";

#[derive(Clone)]
pub(crate) struct Server {
    engine: Arc<Mutex<Engine>>,
    gate: Arc<tokio::sync::Semaphore>,
    operations: Arc<Registry>,
    views: Arc<Mutex<VecDeque<(String, StoredView)>>>,
    tool_router: ToolRouter<Self>,
}
impl Server {
    pub(crate) fn new(engine: Engine) -> Self {
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
                op: rwi_core::operation::Operation,
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
            rwi_core::operation::run(&worker_op, || {
                rwi_core::operation::checkpoint("worker_started")?;
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
    status: rwi_core::operation::Status,
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
fn error(e: rwi_core::Fault) -> CallToolResult {
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
                let (bytes, _, total) = e.artifact(&aid, 0, rwi_core::INLINE_BYTES)?;
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
        description = "Bind one semantic element or original-image pixel to a specific frame, native window and PID. RedMarkerCluster is explicit opt-in for a compact red-dot cluster; it rechecks marker shape/position while ignoring unrelated non-red animation. Marker move, loss or occlusion still rejects.",
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
        name = "computer.pointer_feedback",
        description = "Visual-only overlay marker for the user: show or hide at a desktop point. Never input, never the real cursor, never focus; acts also update it automatically.",
        annotations(read_only_hint = false)
    )]
    async fn pointer_feedback(
        &self,
        Parameters(a): Parameters<PointerFeedbackArgs>,
    ) -> CallToolResult {
        self.execute_operation(a.operation_id.clone(), move |e| {
            value(e.feedback(&a.target, a.visible, a.point)?)
        })
        .await
    }
    #[tool(
        name = "computer.focus_window",
        description = "Explicit exact-window foreground operation, never background fallback. Requires fresh kind=window reference, lease, expected native foreground tuple and stable request_id. Unsupported backends refuse; inspect independent fresh state, no replay.",
        annotations(read_only_hint = false, destructive_hint = true)
    )]
    async fn focus_window(&self, Parameters(a): Parameters<FocusWindowArgs>) -> CallToolResult {
        self.execute_operation(a.operation_id.clone(), move |e| {
            value(e.focus_window(
                &a.target,
                &a.lease,
                &a.reference,
                &a.request_id,
                &a.expected_foreground,
            )?)
        })
        .await
    }
    #[tool(
        name = "computer.set_text",
        description = "Explicit whole plain AXValue replacement. Five fields target/frame/element/request_id/text; empty clears, 0..4096 UTF-8 bytes. Internal lease, one exact native setter, full AX readback separate from DOM/business goal. No typing/focus/activation/clipboard/append/fallback/replay. Unsupported or secure/specialized fields refuse.",
        annotations(read_only_hint = false, destructive_hint = true)
    )]
    async fn set_text(
        &self,
        Parameters(a): Parameters<rwi_core::set_text::SetTextRequest>,
    ) -> CallToolResult {
        self.execute(move |e| {
            a.validate()?;
            if !rwi_core::device::valid_id(&a.request_id) {
                return Err(fault("invalid_arguments", "Use a stable safe ASCII request_id."));
            }
            let lease = e.claim(&a.target, 10000)?;
            let action = e.set_text(&a, &lease);
            // Explicit cleanup even after action refusal; never rerun the action.
            let cleanup = e.release(&lease);
            if let Err(cleanup) = cleanup {
                return Ok(CallToolResult::structured_error(json!({
                    "code":"input_cleanup_unconfirmed", "action":match action {Ok(v)=>v,Err(err)=>json!({"error":err})},
                    "cleanup":cleanup, "goal_verified":false,"replay_allowed":false
                })));
            }
            value(action?)
        }).await
    }
    #[tool(
        name = "computer.focus_element",
        description = "Explicit semantic focus of one exact plain editable element. Four fields: target/frame/element/request_id. Short lease is internal. At most one AXFocused=true setter; separate exact element/app-window focus and OS foreground readback. No activation request, value mutation, click, key, automatic fallback or replay. Unsupported backends refuse.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    async fn focus_element(
        &self,
        Parameters(a): Parameters<rwi_core::semantic_focus::FocusElementRequest>,
    ) -> CallToolResult {
        self.execute(move |e| {
            a.validate()?;
            if !rwi_core::device::valid_id(&a.request_id) {
                return Err(fault("invalid_arguments", "Use a stable safe ASCII request_id."));
            }
            let lease = e.claim(&a.target, 10000)?;
            let action = e.focus_element(&a, &lease);
            // Explicit cleanup even after action refusal; never rerun the action.
            let cleanup = e.release(&lease);
            if let Err(cleanup) = cleanup {
                return Ok(CallToolResult::structured_error(json!({
                    "code":"input_cleanup_unconfirmed", "action":match action {Ok(v)=>v,Err(err)=>json!({"error":err})},
                    "cleanup":cleanup, "goal_verified":false,"replay_allowed":false
                })));
            }
            value(action?)
        }).await
    }
    #[tool(
        name = "computer.select_window_local",
        description = "Explicit app-local main/key selection of one exact ordinary macOS visual window. Four fields: target/frame/kind/request_id. Short lease is internal; at most one requested AXMain or window AXFocused setter. No raise/frontmost/activation, element focus, keys, click, fallback or replay. Exact window/app main/key CF readback and OS foreground are independent. Main is not key or typing readiness. Unsupported kinds/backends refuse.",
        annotations(read_only_hint = false, destructive_hint = false)
    )]
    async fn select_window_local(
        &self,
        Parameters(a): Parameters<rwi_core::window_local::WindowLocalRequest>,
    ) -> CallToolResult {
        self.execute(move |e| {
            a.validate()?;
            if !rwi_core::device::valid_id(&a.request_id) {
                return Err(fault("invalid_arguments", "Use a stable safe ASCII request_id."));
            }
            let lease = e.claim(&a.target, 10000)?;
            let action = e.select_window_local(&a, &lease);
            // Explicit cleanup even after action refusal; never rerun the action.
            let cleanup = e.release(&lease);
            if let Err(cleanup) = cleanup {
                return Ok(CallToolResult::structured_error(json!({
                    "code":"input_cleanup_unconfirmed", "action":match action {Ok(v)=>v,Err(err)=>json!({"error":err})},
                    "cleanup":cleanup, "goal_verified":false,"replay_allowed":false
                })));
            }
            value(action?)
        }).await
    }
    #[tool(
        name = "computer.move_window",
        description = "Explicitly move the bound window onto a requested display (built_in and/or exact id). Lease-scoped and idempotent: it never presses input, never focuses, never replays; invalidates frames/references on this target. Fails closed on unsupported platforms; verify independently afterward.",
        annotations(read_only_hint = false, destructive_hint = true)
    )]
    async fn move_window(&self, Parameters(a): Parameters<MoveWindowArgs>) -> CallToolResult {
        self.execute_operation(a.operation_id.clone(), move |e| {
            value(e.move_window(&a.target, &a.lease, &a.reference, &a.request_id, &a.display)?)
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
            value(e.act_checked(rwi_core::CheckedAction {
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
                rwi_core::lifecycle::LaunchRequest {
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
                rwi_core::lifecycle::ActivateRequest {
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
                rwi_core::DiscoveryQuery {
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
        name = "computer.attach_window",
        description = "Create an independent window target from a fresh explicit discovery selection. No launch, activation, placement, capture or source rebind; physical desktop lease stays shared. Unsupported backends fail closed.",
        annotations(read_only_hint = true)
    )]
    async fn attach_window(&self, Parameters(a): Parameters<BindArgs>) -> CallToolResult {
        self.execute_operation(a.operation_id, move |e| {
            value(e.attach_window(&a.target, &a.selection)?)
        })
        .await
    }
    #[tool(
        name = "computer.detach_window",
        description = "Retire an attached target, not its native application/window. Release its lease first; old frames expire and durable receipts remain. The original desktop target cannot be detached.",
        annotations(read_only_hint = true)
    )]
    async fn detach_window(
        &self,
        Parameters(a): Parameters<rwi_core::device::api::DetachWindowArgs>,
    ) -> CallToolResult {
        let views = self.views.clone();
        self.execute_operation(a.operation_id, move |e| {
            let result = e.detach_window(&a.target)?;
            views
                .lock()
                .map_err(|_| {
                    fault(
                        "view_registry_unavailable",
                        "Target detached; do not replay.",
                    )
                })?
                .retain(|(_, v)| v.target != a.target);
            value(result)
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
        description = "Map delivered-view pixels to the original frame and desktop. Default red_marker_cluster=false is strict. Opt in only when the exact point is on a visible compact red-dot cluster; native marker position/shape is rechecked, and movement, disappearance or occlusion rejects. Never pass view pixels to computer.resolve.",
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
            let locator = if a.red_marker_cluster {
                Locator::RedMarkerCluster { x, y }
            } else {
                Locator::Pixel { x, y }
            };
            value(e.resolve(&a.target, &v.frame, locator)?)
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
                "real-world-interaction",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(INSTRUCTIONS)
    }
}
