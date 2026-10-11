//! Typed, platform-neutral convenience calls. No JS and no native input replay.
use super::*;
use schemars::JsonSchema;
use serde::Deserialize;
use std::{future::Future, time::Instant};

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SnapshotArgs {
    target: String,
    #[serde(default = "edge")]
    max_edge: u32,
    #[serde(default = "limit")]
    limit: usize,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ControlsArgs {
    /// Original observation_ref from computer.snapshot (not details_ref).
    observation_ref: String,
    frame: String,
    /// Case-insensitive substring of name, description or role. Empty lists all.
    #[serde(default)]
    query: String,
    #[serde(default)]
    offset: usize,
    #[serde(default = "controls_limit")]
    limit: usize,
}
fn controls_limit() -> usize {
    32
}
fn edge() -> u32 {
    2048
}
fn limit() -> usize {
    64
}
fn take_include_timings(args: &mut Value) -> Result<bool, DeviceResult> {
    let Some(object) = args.as_object_mut() else {
        return Ok(false);
    };
    match object.remove("include_timings") {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(value),
        Some(_) => Err(invalid("include_timings must be a boolean.")),
    }
}

fn expose_timings(name: &str, result: &DeviceResult, requested: bool) -> bool {
    requested
        || result.is_error
        || (!matches!(name, "computer.snapshot" | "computer.controls")
            && !compactable_receipt(result))
}

fn trim_timing_fields(data: &mut Value) {
    if let Some(object) = data.as_object_mut() {
        object.remove("stage_timings");
        object.remove("elapsed_ms");
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ClickTarget {
    /// Exact native element from snapshot. action is optional AXPress/AXOpen etc.
    Element {
        frame: String,
        element: String,
        action: Option<String>,
    },
    /// Coordinates in the delivered view, not desktop or original image pixels.
    View {
        view_id: String,
        x: f64,
        y: f64,
        /// Explicit opt-in for a stable compact red-dot cluster on an animated canvas.
        #[serde(default)]
        red_marker_cluster: bool,
    },
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ClickArgs {
    /// One click (default) or one native double-click sequence; never a retry.
    #[serde(default = "single_click")]
    click_count: u8,
    target: String,
    request_id: String,
    at: ClickTarget,
}
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum HoverTarget {
    /// Exact observed element used only for pointer location, never a native action.
    Element { frame: String, element: String },
    /// Coordinates in the delivered view, not desktop or original-image pixels.
    View {
        view_id: String,
        x: f64,
        y: f64,
        #[serde(default)]
        red_marker_cluster: bool,
    },
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct HoverArgs {
    target: String,
    request_id: String,
    at: HoverTarget,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RevealArgs {
    target: String,
    frame: String,
    element: String,
    request_id: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum ScrollTarget {
    Element { frame: String, element: String },
    View { view_id: String, x: f64, y: f64 },
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ScrollArgs {
    target: String,
    request_id: String,
    at: ScrollTarget,
    /// Signed native vertical units: positive up, negative down. macOS pixels
    /// +/-2000; Windows wheel units, multiples of 120, +/-1200. Nonzero only.
    delta_y: i32,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DragArgs {
    target: String,
    request_id: String,
    view_id: String,
    from: [f64; 2],
    to: [f64; 2],
    #[serde(default = "duration")]
    duration_ms: u64,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TextArgs {
    target: String,
    frame: String,
    element: String,
    request_id: String,
    text: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct KeyArgs {
    target: String,
    frame: String,
    element: String,
    request_id: String,
    key: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlaceWindowArgs {
    target: String,
    request_id: String,
    display: rwi_core::DisplayTarget,
}
fn single_click() -> u8 {
    1
}
fn click_intent(count: u8, action: bool) -> Result<&'static str, DeviceResult> {
    match (count, action) {
        (1, _) => Ok("click"),
        (2, false) => Ok("double_click"),
        _ => Err(invalid(
            "click_count must be 1 or 2; native AX actions cannot be double-clicked.",
        )),
    }
}
fn view_resolution_args(
    target: &str,
    view_id: &str,
    x: f64,
    y: f64,
    red_marker_cluster: bool,
) -> Value {
    let mut args = json!({"target":target,"view_id":view_id,"x":x,"y":y});
    // Older Agents reject unknown fields even when false. Absence selects the
    // existing strict pixel path; a requested opt-in must remain explicit so an
    // unsupported peer refuses it rather than silently downgrading the action.
    if red_marker_cluster {
        args["red_marker_cluster"] = json!(true);
    }
    args
}

fn compact_action_reply(data: &Value, details_ref: Option<String>) -> Value {
    let mut reply = serde_json::Map::new();
    for key in [
        "request_id",
        "dispatch",
        "deduplicated",
        "operation_ref",
        "system_accepted",
        "replay",
        "replay_allowed",
        "cleanup",
        "resources_released",
        "input_domain_fenced",
        "frame",
        "scope",
        "point",
        "stage_timings",
        "elapsed_ms",
        "hover_effect",
        "motion_history",
    ] {
        // Omit an absent field, never an explicit null: a native unknown (such
        // as system_accepted:null) remains distinct from missing evidence.
        if let Some(value) = data.get(key) {
            reply.insert(key.into(), value.clone());
        }
    }
    reply.insert("goal_verified".into(), json!(false));
    reply.insert("details_ref".into(), json!(details_ref));
    Value::Object(reply)
}
fn duration() -> u64 {
    500
}
fn input_schema<T: JsonSchema>() -> Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(T)).unwrap();
    schema_numbers(&mut schema);
    schema
        .as_object_mut()
        .unwrap()
        .entry("properties")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .unwrap()
        .insert("include_timings".into(), json!({"type":"boolean"}));
    schema
}

fn tool<T: JsonSchema>(name: &'static str, description: &'static str) -> Tool {
    let schema = input_schema::<T>();
    let tool = Tool::new(
        name,
        description,
        Arc::new(schema.as_object().unwrap().clone()),
    )
    .with_title(match name {
        "computer.snapshot" => "Inspect window",
        "computer.click" => "Click control or point",
        "computer.hover" => "Move pointer to control or point",
        "computer.controls" => "Find or page controls",
        "computer.reveal" => "Reveal observed control",
        "computer.type_text" => "Type into focused control",
        "computer.press_key" => "Press key or shortcut",
        "computer.place_window" => "Move window to display",
        _ => "Drag in screenshot",
    })
    .with_annotations(
        ToolAnnotations::new().read_only(matches!(name, "computer.snapshot" | "computer.controls")),
    );
    match name {
        "computer.snapshot" => tool.with_title("Inspect selected window"),
        "computer.controls" => tool.with_title("Find observed controls"),
        "computer.reveal" => tool.with_title("Reveal selected control"),
        "computer.click" => tool.with_title("Click selected control"),
        "computer.hover" => tool.with_title("Hover within selected window"),
        "computer.type_text" => tool.with_title("Type in focused field"),
        "computer.press_key" => tool.with_title("Press key in focused control"),
        "computer.drag" => tool.with_title("Drag within selected window"),
        "computer.scroll" => tool.with_title("Scroll selected surface"),
        "computer.place_window" => tool.with_title("Place selected window"),
        _ => tool,
    }
}
pub(super) fn tools() -> Vec<Tool> {
    vec![
        tool::<SnapshotArgs>(
            "computer.snapshot",
            "Inspect selected window: compact controls and screenshot in one call. Default max_edge=2048 for text detail; use computer.image with an original-pixel crop for a readable tile. AX-incompatible macOS windows may return visual_only=true with image but no controls, input binding, or resolvable view_id; computer.image remains read-only. Semantic-only dialogs have no image or pixel mapping. Terminal, released AXWindows read errors may include a separate exact-current-bound-window visual read; the AX error stays explicit and no frame, view or input authority is created. Full metadata via computer.result. No JS, activation or permission grant.",
        ),
        tool::<ControlsArgs>(
            "computer.controls",
            "Find or page compact controls from an existing snapshot: observation_ref + frame, optional query and offset. No JS, JSON pointers, device RPC, screenshot or input. Returns original element IDs for computer.click. Preserves ambiguity and unknown states; does not refresh frame lifetime. Use next_offset to continue.",
        ),
        tool::<ClickArgs>(
            "computer.click",
            "Click one exact snapshot element or delivered-view coordinate. No JS or manual lease management. Supply stable request_id; click_count=1 (default) or 2 for a native double-click. On a visible compact red-dot cluster only, red_marker_cluster=true opts into marker-shape revalidation while ignoring unrelated non-red animation; marker move/loss/occlusion still refuses. Default false is strict. Returns receipt, not goal success; no retry, focus or fallback. Observe afterward.",
        ),
        tool::<HoverArgs>(
            "computer.hover",
            "Send one checked mouse-move to an exact snapshot element or delivered-view coordinate; no button press/release or AX action. Requires pointer_move.implemented on the selected device (Windows currently unsupported). Stable request_id; short lease and release handled internally. No JS, dwell, activation, real-cursor warp, global fallback or retry. Motion history is unknown. Returns a dispatch receipt, not tooltip success; independently inspect application state afterward.",
        ),
        tool::<RevealArgs>(
            "computer.reveal",
            "Ask one exact observed AX element to make itself visible using its advertised AXScrollToVisible action. This is an explicit native UI action (it may scroll the selected app's content), not a click, focus, activation, or pointer fallback. The device must advertise AXScrollToVisible and the element must advertise that exact action; otherwise no lease/input is issued. Supply the original frame/element and a stable request_id. One durable action only; no retry. Returns a dispatch receipt, not proof of visibility; inspect a fresh computer.snapshot afterward.",
        ),
        tool::<TextArgs>(
            "computer.type_text",
            "Type Unicode into one exact currently focused editable element (target/frame/element). Stable request_id; text 1..4096 UTF-8 bytes. Uses native checked text input, not setValue or clipboard. App selection determines insertion/replacement. No auto-focus, secure-field bypass, global keyboard fallback or retry; snapshot afterward.",
        ),
        tool::<KeyArgs>(
            "computer.press_key",
            "Send one key or shortcut to the exact currently focused native element (target/frame/element). Stable request_id; key e.g. escape or cmd+s on macOS, ctrl+s on Windows. Uses existing platform chord parser; unsupported keys are rejected. No auto-focus, shell, global fallback or retry; snapshot afterward.",
        ),
        tool::<DragArgs>(
            "computer.drag",
            "Drag from/to pixels in one delivered view_id; native mapping and short lease handled internally. No JS or desktop-coordinate arithmetic. duration_ms 16..5000, default 500. Stable request_id required. No replay or activation; inspect computer.snapshot afterward.",
        ),
        tool::<ScrollArgs>(
            "computer.scroll",
            "Scroll once at one exact snapshot element or delivered-view coordinate. delta_y is signed native vertical units: positive up/negative down; macOS pixels +/-2000, Windows wheel units multiples of 120 +/-1200; nonzero. This is not a cross-platform pixel conversion. Short lease, native guards and release handled internally, no JS, activation, fallback, replay or automatic screenshot. Read back application state afterward.",
        ),
        tool::<PlaceWindowArgs>(
            "computer.place_window",
            "Move the already selected window to one explicit display (prefer macOS UUID). Fresh observation, window reference, short lease and release are handled in one call, avoiding model-side timing gaps. Supply stable request_id. Uses computer.move_window capability, returns native placement confirmation and durable receipt. Does not bind or activate another window, resize it, send keyboard/mouse input or replay a move.",
        ),
    ]
}
pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "computer.snapshot"
            | "computer.click"
            | "computer.hover"
            | "computer.reveal"
            | "computer.drag"
            | "computer.scroll"
            | "computer.controls"
            | "computer.type_text"
            | "computer.press_key"
            | "computer.place_window"
    )
}
fn invalid(message: &str) -> DeviceResult {
    DeviceResult::error("invalid_arguments", message)
}

// Cleanup ignores Host cancellation, but never repeats an action. Native lease
// expiry remains authoritative if transport/shutdown prevents explicit release.
async fn leased<F, Fut>(
    target: &str,
    request: &str,
    tool: &'static str,
    mut args: Value,
    mut call: F,
) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    if !device::valid_id(request) {
        return invalid("Supply a stable request_id (1..128 safe ASCII characters).");
    }
    let claim = call(
        "computer.claim",
        json!({"target":target,"ttl_ms":10000}),
        false,
    )
    .await;
    if claim.is_error {
        return claim;
    }
    let Some(lease) = claim.data["lease"].as_str() else {
        return DeviceResult::error(
            "invalid_lease_reply",
            "Claim returned no lease; no input was sent.",
        );
    };
    args["target"] = json!(target);
    args["request_id"] = json!(request);
    args["lease"] = json!(lease);
    let action = call(tool, args, false).await;
    let release = call("computer.release", json!({"lease":lease}), true).await;
    if release.is_error {
        return DeviceResult {
            is_error: true,
            data: json!({"code":"input_cleanup_unconfirmed","action":action.data,"cleanup":release.data,"goal_verified":false,"replay_allowed":false}),
            images: vec![],
        };
    }
    action
}
async fn place_window<F, Fut>(args: PlaceWindowArgs, mut call: F) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    if !device::valid_id(&args.request_id) || args.display.validate().is_err() {
        return invalid("Use a stable request_id and an explicit valid display selector.");
    }
    let observed = call(
        "computer.observe",
        json!({"target":args.target,"inline_image":false}),
        false,
    )
    .await;
    if observed.is_error {
        return observed;
    }
    let Some(frame) = observed.data["frame"].as_str() else {
        return DeviceResult::error(
            "invalid_observation_reply",
            "No frame returned; nothing moved.",
        );
    };
    let reference = call(
        "computer.resolve",
        json!({"target":args.target,"frame":frame,"locator":{"kind":"window"}}),
        false,
    )
    .await;
    if reference.is_error {
        return reference;
    }
    let Some(reference) = reference.data["reference"].as_str() else {
        return DeviceResult::error(
            "invalid_reference_reply",
            "No window reference returned; nothing moved.",
        );
    };
    leased(
        &args.target,
        &args.request_id,
        "computer.move_window",
        json!({"reference":reference,"display":args.display}),
        call,
    )
    .await
}
async fn hover<F, Fut>(args: HoverArgs, mut call: F) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    if args.target.is_empty() || !device::valid_id(&args.request_id) {
        return invalid("Supply an exact target and stable request_id.");
    }
    let (tool, action) = match args.at {
        HoverTarget::Element { frame, element } => {
            if frame.is_empty() || element.is_empty() {
                return invalid("Use an exact observed frame and element.");
            }
            (
                "computer.act_checked",
                json!({"frame":frame,"element":element,"intent":"move_pointer"}),
            )
        }
        HoverTarget::View {
            view_id,
            x,
            y,
            red_marker_cluster,
        } => {
            if view_id.is_empty() || !x.is_finite() || !y.is_finite() {
                return invalid("Use an exact delivered view and finite view coordinates.");
            }
            let reference = call(
                "computer.resolve_view",
                view_resolution_args(&args.target, &view_id, x, y, red_marker_cluster),
                false,
            )
            .await;
            if reference.is_error {
                return reference;
            }
            let Some(reference) = reference.data["reference"].as_str() else {
                return DeviceResult::error(
                    "invalid_reference_reply",
                    "No reference returned; no input issued.",
                );
            };
            (
                "computer.act",
                json!({"reference":reference,"intent":"move_pointer"}),
            )
        }
    };
    let mut result = leased(&args.target, &args.request_id, tool, action, call).await;
    // These describe the contract's limits, not native goal/effect evidence.
    result.data["hover_effect"] = json!("unverified");
    result.data["motion_history"] = json!("unknown");
    result
}
async fn scroll<F, Fut>(args: ScrollArgs, mut call: F) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    if !device::valid_id(&args.request_id)
        || args.delta_y == 0
        || args.delta_y.unsigned_abs() > 2000
    {
        return invalid(
            "Use a stable request_id and nonzero bounded native delta_y; platform limits still apply.",
        );
    }
    let value = args.delta_y.to_string();
    let (tool, action) = match args.at {
        ScrollTarget::Element { frame, element } => {
            if frame.is_empty() || element.is_empty() {
                return invalid("Use an exact observed frame and element.");
            }
            (
                "computer.act_checked",
                json!({"frame":frame,"element":element,"intent":"scroll","value":value}),
            )
        }
        ScrollTarget::View { view_id, x, y } => {
            if view_id.is_empty() || !x.is_finite() || !y.is_finite() {
                return invalid("Use an exact delivered view and finite view coordinates.");
            }
            let result = call(
                "computer.resolve_view",
                json!({"target":args.target,"view_id":view_id,"x":x,"y":y}),
                false,
            )
            .await;
            if result.is_error {
                return result;
            }
            let Some(reference) = result.data["reference"].as_str().filter(|r| !r.is_empty())
            else {
                return DeviceResult::error(
                    "invalid_reference_reply",
                    "No reference returned; no scroll sent.",
                );
            };
            (
                "computer.act",
                json!({"reference":reference,"intent":"scroll","value":value}),
            )
        }
    };
    leased(&args.target, &args.request_id, tool, action, call).await
}
async fn reveal<F, Fut>(args: RevealArgs, mut call: F) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    if !device::valid_id(&args.request_id) || args.frame.is_empty() || args.element.is_empty() {
        return invalid("Supply a stable request_id and exact observed frame/element.");
    }
    // Never let an unsupported backend reinterpret reveal as a normal click.
    // Capability discovery is read-only and precedes the lease.
    let catalog = call("computer.targets", json!({}), false).await;
    if catalog.is_error {
        return catalog;
    }
    let advertised = catalog.data["targets"].as_array().is_some_and(|targets| {
        targets.iter().any(|entry| {
            entry["target"] == args.target
                && entry["capabilities"]["native_semantic_actions"]
                    .as_array()
                    .is_some_and(|actions| {
                        actions.iter().any(|action| action == "AXScrollToVisible")
                    })
        })
    });
    if !advertised {
        return DeviceResult::error(
            "capability_unavailable",
            "The exact target does not advertise AXScrollToVisible; no lease or input was issued.",
        );
    }
    let mut result = leased(
        &args.target,
        &args.request_id,
        "computer.act_checked",
        json!({"frame":args.frame,"element":args.element,"intent":"click","value":"AXScrollToVisible"}),
        call,
    )
    .await;
    result.data["operation"] = json!("reveal_to_visible");
    result.data["native_action"] = json!("AXScrollToVisible");
    result
}
impl Gateway {
    pub(super) async fn convenience(
        &self,
        name: &str,
        mut args: Value,
        context: RequestContext<RoleServer>,
    ) -> DeviceResult {
        let started = Instant::now();
        let include_timings = match take_include_timings(&mut args) {
            Ok(value) => value,
            Err(error) => return error,
        };
        let progress = progress::Progress::new(context.clone());
        let timings = CallTimings::default();
        progress.emit("Request accepted").await;
        let mut result = self
            .convenience_inner(name, args, context, &progress, &timings)
            .await;
        progress
            .emit(if result.is_error {
                "Request ended with an error; inspect receipt"
            } else {
                "Request returned; verify application state"
            })
            .await;
        let timing_rows = timings.rows();
        result.data["stage_timings"] = timing_rows;
        result.data["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
        let include_timing_output = expose_timings(name, &result, include_timings);
        // Retain full diagnostics before shaping the default reply. Successful
        // callers can request inline timing; errors/uncertainty keep it by default.
        if let Ok(mut cache) = self.presentation.lock() {
            let reference = cache.retain(name, result.data.clone());
            if compactable_receipt(&result)
                && !matches!(name, "computer.snapshot" | "computer.controls")
            {
                result.data = compact_action_reply(&result.data, reference);
            } else {
                result.data["details_ref"] = json!(reference);
            }
        }
        if !include_timing_output {
            trim_timing_fields(&mut result.data);
        }
        result
    }
    async fn convenience_inner(
        &self,
        name: &str,
        args: Value,
        context: RequestContext<RoleServer>,
        progress: &progress::Progress,
        timings: &CallTimings,
    ) -> DeviceResult {
        let call = |tool, args, cleanup: bool| {
            let context = context.clone();
            async move {
                if !cleanup {
                    progress.emit(progress::stage(tool)).await;
                }
                let began = Instant::now();
                let result = self
                    .invoke(tool, args, if cleanup { None } else { Some(context) })
                    .await;
                timings.record(tool, began.elapsed());
                if cleanup {
                    progress
                        .emit("Lease release request returned; inspect receipt")
                        .await;
                }
                result
            }
        };
        match name {
            "computer.hover" => {
                let args = match serde_json::from_value::<HoverArgs>(args) {
                    Ok(args) => args,
                    Err(error) => return invalid(&error.to_string()),
                };
                hover(args, call).await
            }
            "computer.scroll" => {
                let args = match serde_json::from_value::<ScrollArgs>(args) {
                    Ok(args) => args,
                    Err(error) => return invalid(&error.to_string()),
                };
                scroll(args, call).await
            }
            "computer.place_window" => {
                let args = match serde_json::from_value::<PlaceWindowArgs>(args) {
                    Ok(args) => args,
                    Err(error) => return invalid(&error.to_string()),
                };
                place_window(args, call).await
            }
            "computer.controls" => {
                let a = match serde_json::from_value::<ControlsArgs>(args) {
                    Ok(a)
                        if a.query.len() <= 512
                            && (1..=128).contains(&a.limit)
                            && a.offset <= 65536 =>
                    {
                        a
                    }
                    _ => return invalid("Use query <=512 bytes, limit 1..128 and bounded offset."),
                };
                let mut cache = match self.presentation.lock() {
                    Ok(cache) => cache,
                    Err(_) => {
                        return DeviceResult::error(
                            "presentation_unavailable",
                            "Retained observation cache unavailable.",
                        );
                    }
                };
                let rows = match cache.observation_rows(&a.observation_ref, &a.frame) {
                    Ok(rows) => rows,
                    Err(error) => return error,
                };
                control_page(rows, &a)
            }
            "computer.snapshot" => {
                let a = match serde_json::from_value::<SnapshotArgs>(args) {
                    Ok(a) if (64..=2048).contains(&a.max_edge) && (1..=128).contains(&a.limit) => a,
                    _ => return invalid("Use max_edge 64..2048 and limit 1..128."),
                };
                let observed = call(
                    "computer.observe",
                    json!({"target":a.target,"inline_image":false}),
                    false,
                )
                .await;
                if observed.is_error {
                    if !read_only_ax_visual_candidate(&observed) {
                        return observed;
                    }
                    // A separate exact-current-binding visual read. It neither
                    // repairs AX, retries input, nor creates a usable input frame.
                    let visual = call(
                        "computer.capture_visual",
                        json!({"target":a.target,"max_edge":a.max_edge,"feedback":"exclude"}),
                        false,
                    )
                    .await;
                    return degraded_ax_snapshot(&a.target, observed, visual);
                }
                let d = &observed.data;
                let details = self
                    .presentation
                    .lock()
                    .ok()
                    .and_then(|mut c| c.retain("computer.observe", d.clone()));
                let image = if d["artifact_id"].is_null()
                    && d["scene"]["image_size"] == json!([0, 0])
                {
                    DeviceResult::value(
                        json!({"view_id":null,"mapping":null,"input_reference_available":false}),
                    )
                } else {
                    call("computer.image", json!({"target":a.target,"frame":d["frame"],"artifact_id":d["artifact_id"],"max_edge":a.max_edge}), false).await
                };
                if image.is_error {
                    return DeviceResult {
                        is_error: true,
                        data: json!({"code":"snapshot_preview_failed","observation_ref":details,"frame":d["frame"],"preview_error":image.data,"goal_verified":false}),
                        images: vec![],
                    };
                }
                let rows = d["scene"]["elements"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let (elements, total) = compact_controls(&rows, a.limit);
                let omitted = total.saturating_sub(elements.len());
                let (view_id, input_reference_available) =
                    snapshot_input_view(&d["scene"], &image.data);
                DeviceResult {
                    is_error: false,
                    data: json!({"target":a.target,"window":snapshot_window(d),"consistency":d["adapter_status"]["observation_consistency"],"frame":d["frame"],"view_id":view_id,"mapping":image.data["mapping"],"visual_available":!d["artifact_id"].is_null(),"visual_only":d["scene"]["binding"].is_null() && !d["artifact_id"].is_null(),"assurance":d["scene"]["assurance"],"input_reference_available":input_reference_available,"elements":elements,"available_controls":total,"omitted_controls":omitted,"control_projection":"preferred controls; unnamed unavailable containers may be collapsed; computer.controls keeps full original pages","coverage":d["adapter_status"]["observation_consistency"]["ax_coverage_after"],"native_complete":snapshot_native_complete(d),"reference_max_age_ms":d["adapter_status"]["input_proposal_max_age_ms"],"observation_ref":details,"blocked":d["scene"]["blocked"],"mutation_ready":snapshot_mutation_ready(d),"input_domain_fenced":d["input_domain_fenced"]}),
                    images: image.images,
                }
            }
            "computer.click" => {
                let a = match serde_json::from_value::<ClickArgs>(args) {
                    Ok(a) => a,
                    Err(e) => return invalid(&e.to_string()),
                };
                let has_action = matches!(
                    &a.at,
                    ClickTarget::Element {
                        action: Some(_),
                        ..
                    }
                );
                let intent = match click_intent(a.click_count, has_action) {
                    Ok(intent) => intent,
                    Err(error) => return error,
                };
                let (tool, args) = match a.at {
                    ClickTarget::Element {
                        frame,
                        element,
                        action,
                    } => (
                        "computer.act_checked",
                        json!({"frame":frame,"element":element,"intent":intent,"value":action}),
                    ),
                    ClickTarget::View {
                        view_id,
                        x,
                        y,
                        red_marker_cluster,
                    } => {
                        if !x.is_finite() || !y.is_finite() {
                            return invalid("Coordinates must be finite.");
                        }
                        let r = call(
                            "computer.resolve_view",
                            view_resolution_args(&a.target, &view_id, x, y, red_marker_cluster),
                            false,
                        )
                        .await;
                        if r.is_error {
                            return r;
                        }
                        (
                            "computer.act",
                            json!({"reference":r.data["reference"],"intent":intent}),
                        )
                    }
                };
                leased(&a.target, &a.request_id, tool, args, call).await
            }
            "computer.reveal" => {
                let a = match serde_json::from_value::<RevealArgs>(args) {
                    Ok(a) => a,
                    Err(e) => return invalid(&e.to_string()),
                };
                reveal(a, call).await
            }
            "computer.type_text" => {
                let a = match serde_json::from_value::<TextArgs>(args) {
                    Ok(a) => a,
                    Err(e) => return invalid(&e.to_string()),
                };
                let args = match keyboard_action(a.frame, a.element, a.text, "text", 4096) {
                    Ok(args) => args,
                    Err(e) => return e,
                };
                leased(&a.target, &a.request_id, "computer.act_checked", args, call).await
            }
            "computer.press_key" => {
                let a = match serde_json::from_value::<KeyArgs>(args) {
                    Ok(a) => a,
                    Err(e) => return invalid(&e.to_string()),
                };
                let args = match keyboard_action(a.frame, a.element, a.key, "key_chord", 128) {
                    Ok(args) => args,
                    Err(e) => return e,
                };
                leased(&a.target, &a.request_id, "computer.act_checked", args, call).await
            }
            "computer.drag" => {
                let a = match serde_json::from_value::<DragArgs>(args) {
                    Ok(a) => a,
                    Err(e) => return invalid(&e.to_string()),
                };
                if !(16..=5000).contains(&a.duration_ms)
                    || !a.from.iter().chain(a.to.iter()).all(|n| n.is_finite())
                {
                    return invalid("Use finite view coordinates and duration_ms 16..5000.");
                }
                let mut points = Vec::new();
                let mut reference = Value::Null;
                for point in [a.from, a.to] {
                    let r = call(
                        "computer.resolve_view",
                        json!({"target":a.target,"view_id":a.view_id,"x":point[0],"y":point[1]}),
                        false,
                    )
                    .await;
                    if r.is_error {
                        return r;
                    }
                    let p = match serde_json::from_value::<[f64; 2]>(r.data["point"].clone()) {
                        Ok(p) if p.iter().all(|n| n.is_finite()) => p,
                        _ => {
                            return DeviceResult::error(
                                "invalid_mapping_reply",
                                "No input dispatched.",
                            );
                        }
                    };
                    if points.is_empty() {
                        reference = r.data["reference"].clone();
                    }
                    points.push(p);
                }
                let offset = [points[1][0] - points[0][0], points[1][1] - points[0][1]];
                leased(&a.target,&a.request_id,"computer.act",json!({"reference":reference,"intent":{"drag":{"waypoints":[{"at_ms":0,"offset":[0,0]},{"at_ms":a.duration_ms,"offset":offset}]}}}),call).await
            }
            _ => invalid("Unknown convenience tool."),
        }
    }
}

// Invocation-local bounded labels, never arguments/text or a global history.
// This is adapter wall time around invoke, not native CPU/capture time.
#[derive(Default)]
struct CallTimings(Mutex<Vec<(&'static str, u64, std::time::Duration)>>);
impl CallTimings {
    fn record(&self, tool: &'static str, elapsed: std::time::Duration) {
        if let Ok(mut rows) = self.0.lock() {
            if let Some(row) = rows.iter_mut().find(|r| r.0 == tool) {
                row.1 += 1;
                row.2 += elapsed;
            } else {
                rows.push((tool, 1, elapsed));
            }
        }
    }
    fn rows(&self) -> Value {
        match self.0.lock() {
            Ok(rows) => {
                json!(rows.iter().map(|(tool, calls, elapsed)| {
                json!({"tool":tool,"calls":calls,"elapsed_ms":elapsed.as_millis() as u64})
            }).collect::<Vec<_>>())
            }
            Err(_) => Value::Null,
        }
    }
}

fn snapshot_mutation_ready(data: &Value) -> Value {
    if data["input_domain_fenced"] == true {
        json!(false)
    } else {
        data["scene"]["mutation_ready"].clone()
    }
}

fn snapshot_native_complete(data: &Value) -> Value {
    data["adapter_status"]["observation_consistency"]["ax_coverage_after"]["complete"]
        .as_bool()
        .map_or_else(|| json!("unknown"), |complete| json!(complete))
}

#[test]
fn snapshot_coverage_preserves_unknown_for_old_or_visual_only_agents() {
    assert_eq!(snapshot_native_complete(&json!({})), "unknown");
    for complete in [true, false] {
        let evidence = json!({"adapter_status":{"observation_consistency":{"ax_coverage_after":{"complete":complete}}}});
        assert_eq!(snapshot_native_complete(&evidence), complete);
    }
}

fn snapshot_window(data: &Value) -> Value {
    let binding = &data["scene"]["binding"];
    if !binding["scope"]["window"].is_string() || !binding["scope"]["pid"].is_u64() {
        return Value::Null;
    }
    json!({"window":binding["scope"]["window"],"pid":binding["scope"]["pid"],"title":binding["mutation_context"]["window_title"]})
}

#[test]
fn snapshot_window_uses_observed_binding_not_an_inferred_title_or_rendering_window() {
    let native = json!({"scene":{"binding":{"scope":{"window":"cgwindow:42","pid":7},"mutation_context":{"window_title":"Native dialog","document_state":null}}}});
    assert_eq!(
        snapshot_window(&native),
        json!({"window":"cgwindow:42","pid":7,"title":"Native dialog"})
    );
    let no_title = json!({"scene":{"binding":{"scope":{"window":"hwnd:42","pid":7}}}});
    assert!(snapshot_window(&no_title)["title"].is_null());
    assert!(snapshot_window(&json!({"scene":{"binding":null}})).is_null());
}

/// Protect the Host-facing snapshot even when talking to an older Agent that
/// still returns an opaque image view for a visual-only scene. A screenshot
/// alone cannot supply an input binding or a native coordinate transform.
fn completed_native_read(result: &DeviceResult) -> bool {
    let d = &result.data;
    d["native_dispatch_entered"] == false
        && d["operation"]["terminal"] == true
        && d["operation"]["native_call_pending"] == false
        && d["operation"]["resources_released"] == true
        && d["operation"]["resource_release_uncertain"] == false
}

fn degraded_ax_snapshot(
    target: &str,
    observed: DeviceResult,
    visual: DeviceResult,
) -> DeviceResult {
    let available = !visual.is_error
        && completed_native_read(&visual)
        && visual.data["frame_created"] == false
        && visual.data["input_authority_granted"] == false
        && !visual.images.is_empty();
    DeviceResult {
        is_error: true,
        data: json!({"target":target,"code":observed.data["code"],"message":"AX controls remain unavailable; a separate exact-current-bound-window read-only visual was attempted. No input frame or control absence is inferred.","observation_error":observed.data,"visual_read":visual.data,"visual_available":available,"visual_only":available,"frame":null,"view_id":null,"mapping":null,"input_reference_available":false,"input_authority_granted":false,"mutation_ready":false,"native_complete":false,"elements":[],"available_controls":null,"coverage":{"complete":false,"absence_proven":false,"input_authority":false,"reasons":["AXWindows_read_unavailable"]},"goal_verified":false,"input_replayed":false}),
        images: if available { visual.images } else { vec![] },
    }
}

fn read_only_ax_visual_candidate(observed: &DeviceResult) -> bool {
    let d = &observed.data;
    observed.is_error
        && d["code"] == "ax_attribute_unavailable"
        && d["diagnostic"]["api"] == "AXUIElementCopyAttributeValue"
        && d["diagnostic"]["attribute"] == "AXWindows"
        && d["diagnostic"]["ax_error"] == -25204
        && d["diagnostic"]["operation_kind"] == "read"
        && completed_native_read(observed)
}

fn snapshot_input_view(scene: &Value, image: &Value) -> (Value, bool) {
    let grounded = scene["binding"].is_object() && scene["pixel_to_desktop"].is_array();
    let view_id = if grounded {
        image["view_id"].clone()
    } else {
        Value::Null
    };
    let available = grounded && view_id.is_string() && image["input_reference_available"] != false;
    (view_id, available)
}

// DeviceResult::is_error describes the RPC envelope, not native dispatch success.
// Keep uncertainty, refusal and cleanup/fence evidence visible in the first reply.
fn compactable_receipt(result: &DeviceResult) -> bool {
    !result.is_error
        && result.data["dispatch"] == "returned"
        && result.data["dispatch_error"].is_null()
        && result.data["input_domain_fenced"] != true
}

fn keyboard_action(
    frame: String,
    element: String,
    value: String,
    intent: &str,
    max_bytes: usize,
) -> Result<Value, DeviceResult> {
    if frame.is_empty() || element.is_empty() || value.is_empty() || value.len() > max_bytes {
        return Err(invalid(
            "Supply an exact frame/element and a nonempty bounded text or key value.",
        ));
    }
    Ok(json!({"frame":frame,"element":element,"intent":intent,"value":value}))
}
fn eligible_control(e: &Value) -> bool {
    e["actions"]
        .as_array()
        .is_some_and(|a| a.iter().any(standard_action))
        || !e["control_type"].is_null()
}
fn control_page(rows: &[Value], args: &ControlsArgs) -> DeviceResult {
    let query = args.query.to_lowercase();
    let matched: Vec<_> = rows
        .iter()
        .filter(|e| {
            eligible_control(e)
                && (query.is_empty()
                    || ["name", "description", "role"].iter().any(|key| {
                        e[*key]
                            .as_str()
                            .is_some_and(|s| s.to_lowercase().contains(&query))
                    }))
        })
        .collect();
    let page: Vec<_> = matched
        .iter()
        .skip(args.offset)
        .take(args.limit)
        .map(|e| (*e).clone())
        .collect();
    let (elements, _) = compact_controls_in_order(&page, args.limit);
    if elements.is_empty() && !page.is_empty() {
        return DeviceResult::error(
            "control_display_budget",
            "A control exceeds display budget; inspect the original observation with computer.result.",
        );
    }
    let end = args.offset.saturating_add(elements.len());
    let next = (end < matched.len()).then_some(end);
    DeviceResult::value(
        json!({"frame":args.frame,"observation_ref":args.observation_ref,
        "query":args.query,"elements":elements,"matches":matched.len(),"next_offset":next,
        "frame_refreshed":false,"input_sent":false,"native_complete":"unknown"}),
    )
}

fn standard_action(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| s.starts_with("AX") && s.bytes().all(|c| c.is_ascii_alphanumeric()))
}
// Stable default display ranking only. Every native row remains in the
// retained observation and explicit control pages keep their original order.
fn compact_controls(rows: &[Value], limit: usize) -> (Vec<Value>, usize) {
    let eligible: Vec<_> = rows.iter().filter(|e| eligible_control(e)).collect();
    let preferred: Vec<_> = eligible
        .iter()
        .filter(|e| !low_priority_unnamed_container(e))
        .map(|e| (*e).clone())
        .collect();
    // Collapse display-only noise when meaningful controls are available. Full
    // metadata and explicit pages are untouched; an all-container tree retains
    // its rows rather than pretending there are no controls.
    if preferred.is_empty() {
        return compact_controls_in_order(rows, limit);
    }
    let (out, _) = compact_controls_in_order(&preferred, limit);
    (out, eligible.len())
}
fn low_priority_unnamed_container(e: &Value) -> bool {
    let unnamed = ["name", "description"]
        .iter()
        .all(|key| e[*key].as_str().is_none_or(|s| s.trim().is_empty()));
    let unavailable_point = e["visible"] == false || e["occluded"] == true;
    // Ordinary buttons (including anonymous close/minimize/maximize clusters),
    // named controls and unknown visibility are never demoted by this heuristic.
    // Covered semantic buttons remain useful and retain their exact coordinates.
    let meaningful_state =
        e["selected"] == true || e["value"].as_str().is_some_and(|v| !v.is_empty());
    unnamed
        && !meaningful_state
        && unavailable_point
        && matches!(
            e["role"].as_str(),
            Some("AXRow" | "AXColumn" | "AXGroup" | "AXToolbar")
        )
}
// Bounded display projection, never truncation of native authority or evidence.
fn compact_controls_in_order(rows: &[Value], limit: usize) -> (Vec<Value>, usize) {
    let eligible: Vec<_> = rows.iter().filter(|e| eligible_control(e)).collect();
    let mut out = Vec::new();
    let mut used = 0;
    for e in eligible.iter().take(limit) {
        let mut row = serde_json::Map::new();
        for key in [
            "id",
            "name",
            "description",
            "role",
            "control_type",
            "value",
            "value_truncated",
            "actions",
            "point",
            "enabled",
            "selected",
            "visible",
            "occluded",
        ] {
            if let Some(v) = e.get(key)
                && !v.is_null()
            {
                let mut v = v.clone();
                if let Some(text) = v.as_str()
                    && text.chars().count() > 256
                    && key != "id"
                {
                    v = json!(text.chars().take(256).collect::<String>());
                    row.insert("display_truncated".into(), json!(true));
                }
                row.insert(key.into(), v);
            }
        }
        if let Some(actions) = row.get_mut("actions").and_then(Value::as_array_mut) {
            actions.retain(standard_action);
        }
        let name = row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if name.is_empty() {
            if let Some(description) = row.remove("description") {
                row.insert("name".into(), description);
            }
        } else if row.get("description").and_then(Value::as_str) == Some(name.as_str()) {
            row.remove("description");
        }
        for (key, redundant) in [("enabled", true), ("visible", true), ("occluded", false)] {
            if row.get(key).and_then(Value::as_bool) == Some(redundant) {
                row.remove(key);
            }
        }
        let v = Value::Object(row);
        let bytes = serde_json::to_vec(&v).unwrap().len();
        if used + bytes > 12 * 1024 {
            break;
        }
        used += bytes;
        out.push(v);
    }
    (out, eligible.len())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LegacyResolveView {
        target: String,
        view_id: String,
        x: f64,
        y: f64,
    }

    #[test]
    fn mcp_compat_default_view_click_omits_disabled_anchor_for_legacy_agent() {
        for opt_in in [None, Some(false)] {
            let mut input = json!({"target":"exact-window-target","request_id":"once","at":{"kind":"view","view_id":"original-view","x":12.5,"y":6.0}});
            if let Some(enabled) = opt_in {
                input["at"]["red_marker_cluster"] = json!(enabled);
            }
            let args: ClickArgs = serde_json::from_value(input).unwrap();
            let ClickTarget::View {
                view_id,
                x,
                y,
                red_marker_cluster,
            } = args.at
            else {
                panic!("expected view target");
            };
            let payload = view_resolution_args(&args.target, &view_id, x, y, red_marker_cluster);
            println!("legacy input opt_in={opt_in:?}; payload={payload}");
            let legacy = serde_json::from_value::<LegacyResolveView>(payload.clone());
            assert!(
                legacy.is_ok(),
                "default click rejected by legacy agent: {legacy:?}"
            );
            let legacy = legacy.unwrap();
            assert_eq!(legacy.target, "exact-window-target");
            assert_eq!(legacy.view_id, "original-view");
            assert_eq!([legacy.x, legacy.y], [12.5, 6.0]);
            assert!(payload.get("red_marker_cluster").is_none());
        }
    }

    #[test]
    fn mcp_compat_explicit_anchor_stays_explicit_and_is_not_silently_downgraded() {
        let payload = view_resolution_args("t", "original-view", 12.5, 6.0, true);
        assert_eq!(payload["red_marker_cluster"], true);
        assert_eq!(payload["view_id"], "original-view");
        assert_eq!(payload["x"], 12.5);
        assert_eq!(payload["y"], 6.0);
        let legacy_error = serde_json::from_value::<LegacyResolveView>(payload).unwrap_err();
        assert!(legacy_error.to_string().contains("red_marker_cluster"));
        for value in [Value::Null, json!("true"), json!(1)] {
            let input = json!({"target":"t","request_id":"once","at":{"kind":"view","view_id":"v","x":1,"y":2,"red_marker_cluster":value}});
            assert!(serde_json::from_value::<ClickArgs>(input).is_err());
        }
    }

    #[test]
    fn mcp_compat_compact_reply_omits_absent_keys_but_keeps_explicit_unknown_and_safety() {
        let minimal = json!({"request_id":"once","dispatch":"returned"});
        let projected = compact_action_reply(&minimal, Some("details".into()));
        for absent in ["deduplicated", "operation_ref"] {
            assert!(
                projected.get(absent).is_none(),
                "invented missing key {absent}"
            );
        }
        assert_eq!(projected["goal_verified"], false);
        assert_eq!(projected["details_ref"], "details");
        let old = json!({"request_id":"once","dispatch":"returned","deduplicated":null,"operation_ref":null,"goal_verified":false,"details_ref":"details"});
        let old_bytes = serde_json::to_vec(&old).unwrap().len();
        let new_bytes = serde_json::to_vec(&projected).unwrap().len();
        println!("fixture compact JSON bytes: baseline={old_bytes}, modified={new_bytes}");
        assert!(new_bytes < old_bytes);

        let original = json!({"request_id":"once","dispatch":"returned","deduplicated":false,"operation_ref":null,"system_accepted":null,"replay":"forbidden","replay_allowed":false,"cleanup":{"resources_released":true},"point":[12.5,6.0]});
        let projected = compact_action_reply(&original, Some("details".into()));
        println!("explicit native unknown/safety projection={projected}");
        for key in [
            "request_id",
            "dispatch",
            "deduplicated",
            "operation_ref",
            "system_accepted",
            "replay",
            "replay_allowed",
            "cleanup",
            "point",
        ] {
            assert_eq!(
                projected.get(key),
                original.get(key),
                "lost observed key {key}"
            );
        }
    }
    #[tokio::test]
    async fn reveal_requires_exact_advertised_action_and_dispatches_once_under_lease() {
        let args: RevealArgs = serde_json::from_value(json!({
            "target":"t","frame":"frame-1","element":"element-1","request_id":"reveal-once"
        }))
        .unwrap();
        let calls = RefCell::new(vec![]);
        let result = reveal(args, |tool, args, cleanup| {
            calls.borrow_mut().push((tool, args, cleanup));
            std::future::ready(match tool {
                "computer.targets" => DeviceResult::value(json!({"targets":[{
                    "target":"t","capabilities":{"native_semantic_actions":["AXPress","AXScrollToVisible"]}
                }]})),
                "computer.claim" => DeviceResult::value(json!({"lease":"short-lease"})),
                "computer.act_checked" => DeviceResult::value(json!({
                    "request_id":"reveal-once","dispatch":"returned","goal_verified":false
                })),
                "computer.release" => DeviceResult::value(json!({"released":true})),
                _ => panic!("unexpected tool {tool}"),
            })
        })
        .await;
        assert_eq!(result.data["operation"], "reveal_to_visible");
        assert_eq!(result.data["native_action"], "AXScrollToVisible");
        let calls = calls.borrow();
        assert_eq!(calls[0].0, "computer.targets");
        assert_eq!(calls[1].0, "computer.claim");
        assert_eq!(calls[2].0, "computer.act_checked");
        assert_eq!(calls[2].1["intent"], "click");
        assert_eq!(calls[2].1["value"], "AXScrollToVisible");
        assert_eq!(calls[2].1["request_id"], "reveal-once");
        assert_eq!(calls[3].0, "computer.release");
        assert!(calls[3].2);
    }

    #[tokio::test]
    async fn reveal_without_native_capability_never_claims_or_dispatches() {
        let args: RevealArgs = serde_json::from_value(json!({
            "target":"t","frame":"frame-1","element":"element-1","request_id":"reveal-once"
        }))
        .unwrap();
        let calls = RefCell::new(vec![]);
        let result = reveal(args, |tool, args, cleanup| {
            calls.borrow_mut().push((tool, args, cleanup));
            std::future::ready(DeviceResult::value(json!({"targets":[{
                "target":"t","capabilities":{"native_semantic_actions":["AXPress"]}
            }]})))
        })
        .await;
        assert!(result.is_error);
        assert_eq!(result.data["code"], "capability_unavailable");
        assert_eq!(calls.borrow().len(), 1);
        assert_eq!(calls.borrow()[0].0, "computer.targets");
    }

    #[tokio::test]
    async fn scroll_is_one_checked_intent_and_releases_without_replaying_a_refusal() {
        for at in [
            json!({"kind":"element","frame":"f","element":"e"}),
            json!({"kind":"view","view_id":"v","x":4,"y":5}),
        ] {
            let args: ScrollArgs = serde_json::from_value(
                json!({"target":"t","request_id":"once","at":at,"delta_y":-600}),
            )
            .unwrap();
            let calls = RefCell::new(vec![]);
            let result = scroll(args, |tool, args, cleanup| {
                calls.borrow_mut().push((tool, args, cleanup));
                std::future::ready(match tool {
                    "computer.resolve_view" => {
                        DeviceResult::value(json!({"reference":"exact-ref"}))
                    }
                    "computer.claim" => DeviceResult::value(json!({"lease":"short-lease"})),
                    "computer.release" => DeviceResult::value(json!({"released":true})),
                    _ => DeviceResult::error("stale_reference", "No input sent"),
                })
            })
            .await;
            assert_eq!(result.data["code"], "stale_reference");
            let calls = calls.borrow();
            let actions: Vec<_> = calls
                .iter()
                .filter(|c| matches!(c.0, "computer.act_checked" | "computer.act"))
                .collect();
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].1["intent"], "scroll");
            assert_eq!(actions[0].1["value"], "-600");
            assert_eq!(actions[0].1["request_id"], "once");
            assert_eq!(calls.last().unwrap().0, "computer.release");
            assert!(calls.last().unwrap().2);
        }
    }

    #[tokio::test]
    async fn scroll_rejects_invalid_values_before_native_calls_and_never_claims_on_bad_view() {
        for delta in [0, 2001, -2001, i32::MIN] {
            let args: ScrollArgs = serde_json::from_value(json!({"target":"t","request_id":"once","at":{"kind":"element","frame":"f","element":"e"},"delta_y":delta})).unwrap();
            let result = scroll(args, |_, _, _| {
                std::future::ready({
                    panic!("invalid scroll must not call native tools");
                    #[allow(unreachable_code)]
                    DeviceResult::value(json!({}))
                })
            })
            .await;
            assert_eq!(result.data["code"], "invalid_arguments");
        }
        let args: ScrollArgs = serde_json::from_value(json!({"target":"t","request_id":"once","at":{"kind":"view","view_id":"v","x":4,"y":5},"delta_y":120})).unwrap();
        let result = scroll(args, |tool, _, _| {
            assert_eq!(tool, "computer.resolve_view");
            std::future::ready(DeviceResult::value(json!({})))
        })
        .await;
        assert_eq!(result.data["code"], "invalid_reference_reply");
        assert!(serde_json::from_value::<ScrollArgs>(json!({"target":"t","request_id":"once","at":{"kind":"element","frame":"f","element":"e","action":"AXPress"},"delta_y":120})).is_err());
    }

    #[tokio::test]
    async fn placement_refreshes_locally_dispatches_once_and_releases_even_on_uncertainty() {
        let args:PlaceWindowArgs=serde_json::from_value(json!({"target":"t","request_id":"one","display":{"uuid":"00000000-0000-0000-0000-000000000007"}})).unwrap();
        let calls = RefCell::new(vec![]);
        let result = place_window(args, |tool, args, cleanup| {
            calls.borrow_mut().push((tool, args, cleanup));
            std::future::ready(match tool {
                "computer.observe" => DeviceResult::value(json!({"frame":"f"})),
                "computer.resolve" => DeviceResult::value(json!({"reference":"r"})),
                "computer.claim" => DeviceResult::value(json!({"lease":"l"})),
                "computer.release" => DeviceResult::value(json!({"released":true})),
                _ => DeviceResult::error("placement_rejected", "inspect before a new request"),
            })
        })
        .await;
        assert_eq!(result.data["code"], "placement_rejected");
        let calls = calls.borrow();
        assert_eq!(
            calls.iter().map(|c| c.0).collect::<Vec<_>>(),
            vec![
                "computer.observe",
                "computer.resolve",
                "computer.claim",
                "computer.move_window",
                "computer.release"
            ]
        );
        assert_eq!(calls[3].1["request_id"], "one");
        assert_eq!(calls[3].1["lease"], "l");
        assert_eq!(
            calls[3].1["display"]["uuid"],
            "00000000-0000-0000-0000-000000000007"
        );
        assert!(calls[4].2);
    }
    #[test]
    fn click_count_maps_one_native_intent_and_never_repeats_ax_actions() {
        let base = json!({"target":"t","request_id":"r","at":{"kind":"element","frame":"f","element":"e"}});
        let args: ClickArgs = serde_json::from_value(base.clone()).unwrap();
        assert_eq!(args.click_count, 1);
        assert_eq!(click_intent(1, true).unwrap(), "click");
        assert_eq!(click_intent(2, false).unwrap(), "double_click");
        assert!(click_intent(2, true).is_err());
        assert!(click_intent(0, false).is_err());
        assert!(click_intent(3, false).is_err());
        let mut invalid_count = base;
        invalid_count["click_count"] = json!(-1);
        assert!(serde_json::from_value::<ClickArgs>(invalid_count).is_err());
    }

    #[test]
    fn stage_timings_aggregate_repeated_calls_without_logging_payloads() {
        let timings = CallTimings::default();
        assert_eq!(timings.rows(), json!([]));
        timings.record(
            "computer.resolve_view",
            std::time::Duration::from_micros(600),
        );
        timings.record("computer.claim", std::time::Duration::from_millis(2));
        timings.record(
            "computer.resolve_view",
            std::time::Duration::from_micros(600),
        );
        timings.record("computer.release", std::time::Duration::from_millis(3));
        assert_eq!(
            timings.rows(),
            json!([
                {"tool":"computer.resolve_view","calls":2,"elapsed_ms":1},
                {"tool":"computer.claim","calls":1,"elapsed_ms":2},
                {"tool":"computer.release","calls":1,"elapsed_ms":3}
            ])
        );
    }

    #[test]
    fn timing_output_is_opt_in_for_success_but_kept_for_uncertainty() {
        let returned = DeviceResult::value(
            json!({"dispatch":"returned","dispatch_error":null,"input_domain_fenced":false}),
        );
        assert!(!expose_timings("computer.click", &returned, false));
        assert!(expose_timings("computer.click", &returned, true));
        assert!(!expose_timings("computer.snapshot", &returned, false));

        for uncertain in [
            DeviceResult::value(json!({"dispatch":"uncertain"})),
            DeviceResult::value(json!({"dispatch":"refused"})),
            DeviceResult::error("native_action_unconfirmed", "inspect receipt"),
        ] {
            assert!(expose_timings("computer.click", &uncertain, false));
        }
    }

    #[test]
    fn compact_success_timing_opt_in_survives_projection_without_growing_default() {
        let operation_ref = json!({"ref_version":1,"device_id":"fixture",
            "agent_instance":"agent","context_generation":"context",
            "execution_nonce":"once"});
        let ordinary = json!({"request_id":"once","dispatch":"returned",
            "operation_ref":operation_ref,"system_accepted":null,
            "replay_allowed":false,"cleanup":{"resources_released":true},
            "resources_released":true,"input_domain_fenced":false,
            "frame":"fixture-frame","scope":{"window":"fixture-window","pid":1},
            "point":[12.0,34.0],"goal_verified":false,"details_ref":"fixture-details"});
        let mut raw = ordinary.clone();
        raw.as_object_mut().unwrap().remove("details_ref");
        raw["dispatch_error"] = Value::Null;
        raw["stage_timings"] = json!([
            {"tool":"computer.resolve_view","calls":2,"elapsed_ms":3},
            {"tool":"computer.claim","calls":1,"elapsed_ms":2},
            {"tool":"computer.act","calls":1,"elapsed_ms":5},
            {"tool":"computer.release","calls":1,"elapsed_ms":1}
        ]);
        raw["elapsed_ms"] = json!(12);
        let returned = DeviceResult::value(&raw);
        assert!(compactable_receipt(&returned));
        for tool in ["computer.click", "computer.drag"] {
            for requested in [None, Some(false), Some(true)] {
                let mut args = json!({});
                if let Some(requested) = requested {
                    args["include_timings"] = json!(requested);
                }
                let requested = take_include_timings(&mut args).unwrap();
                assert_eq!(args, json!({}));
                // Compose the actual production policy + compact projection +
                // final opt-out trim, rather than testing the predicate alone.
                let include = expose_timings(tool, &returned, requested);
                let mut projected = compact_action_reply(&raw, Some("fixture-details".into()));
                if !include {
                    trim_timing_fields(&mut projected);
                }
                if requested {
                    assert_eq!(projected.get("stage_timings"), raw.get("stage_timings"));
                    assert_eq!(projected.get("elapsed_ms"), raw.get("elapsed_ms"));
                    trim_timing_fields(&mut projected);
                }
                // Identical default JSON, including coords, the exact operation
                // reference, explicit unknown, no-replay and cleanup evidence.
                assert_eq!(projected, ordinary);
            }
        }
    }

    #[test]
    fn compact_success_timing_preserves_explicit_null_but_not_absent_fields() {
        let absent = compact_action_reply(&json!({"dispatch":"returned"}), None);
        assert!(absent.get("stage_timings").is_none());
        assert!(absent.get("elapsed_ms").is_none());
        let explicit_unknown = compact_action_reply(
            &json!({"dispatch":"returned","stage_timings":null,"elapsed_ms":0}),
            None,
        );
        assert_eq!(explicit_unknown.get("stage_timings"), Some(&Value::Null));
        assert_eq!(explicit_unknown["elapsed_ms"], 0);
    }

    #[test]
    fn trimmed_success_timings_remain_available_from_details_ref() {
        let raw = json!({
            "request_id":"once",
            "dispatch":"returned",
            "stage_timings":[{"tool":"computer.claim","calls":1,"elapsed_ms":2}],
            "elapsed_ms":8
        });
        let mut cache = presentation::Presentation::default();
        let details_ref = cache.retain("computer.click", raw.clone()).unwrap();
        let mut ordinary = raw;
        trim_timing_fields(&mut ordinary);
        assert!(ordinary.get("stage_timings").is_none());
        assert!(ordinary.get("elapsed_ms").is_none());

        let details = cache.read(presentation::ResultArgs {
            result_ref: details_ref,
            pointer: "/stage_timings".into(),
            offset: 0,
            limit: 16,
            byte_offset: None,
        });
        assert_eq!(details.data["value"][0]["tool"], "computer.claim");
    }

    #[test]
    fn timing_opt_in_is_removed_before_strict_tool_argument_parsing() {
        let mut args = json!({"target":"t","include_timings":true});
        assert!(take_include_timings(&mut args).unwrap());
        assert!(args.get("include_timings").is_none());
        assert!(serde_json::from_value::<SnapshotArgs>(args).is_ok());

        let mut malformed = json!({"target":"t","include_timings":"yes"});
        assert_eq!(
            take_include_timings(&mut malformed).unwrap_err().data["code"],
            "invalid_arguments"
        );
    }

    #[test]
    fn duplicate_control_description_is_removed_but_distinct_labels_survive() {
        let rows = vec![
            json!({"id":"close","name":"Close","description":"Close","point":[12.5,6.0],"actions":["AXPress"]}),
            json!({"id":"close-window","name":"Close","description":"Close window","point":[20.0,8.0],"actions":["AXPress"]}),
            json!({"id":"x-close","name":"","description":"X Close","point":[80.0,9.0],"actions":["AXPress"]}),
        ];
        let (out, total) = compact_controls(&rows, 3);
        assert_eq!(total, 3);
        assert_eq!(out[0]["name"], "Close");
        assert_eq!(out[0]["point"], json!([12.5, 6.0]));
        assert!(out[0].get("description").is_none());
        assert_eq!(out[0]["actions"], json!(["AXPress"]));
        assert_eq!(out[1]["description"], "Close window");
        assert_eq!(out[1]["point"], json!([20.0, 8.0]));
        assert_eq!(out[2]["name"], "X Close");
        assert_eq!(out[2]["point"], json!([80.0, 9.0]));
        assert!(out[2].get("description").is_none());
    }

    #[test]
    fn fenced_snapshot_is_never_advertised_as_mutation_ready() {
        assert_eq!(
            snapshot_mutation_ready(
                &json!({"input_domain_fenced":true,"scene":{"mutation_ready":true}})
            ),
            false
        );
        assert_eq!(
            snapshot_mutation_ready(
                &json!({"input_domain_fenced":false,"scene":{"mutation_ready":true}})
            ),
            true
        );
        assert!(snapshot_mutation_ready(&json!({})).is_null());
    }

    #[test]
    fn visual_only_snapshot_hides_even_a_legacy_agent_view_id() {
        let legacy_image = json!({"view_id":"old-agent-view"});
        let visual_only = json!({"binding":null,"pixel_to_desktop":null});
        assert_eq!(
            snapshot_input_view(&visual_only, &legacy_image),
            (Value::Null, false)
        );
        let semantic_only = json!({"binding":{"scope":{}},"pixel_to_desktop":null});
        assert_eq!(
            snapshot_input_view(&semantic_only, &legacy_image),
            (Value::Null, false)
        );
        let grounded = json!({"binding":{"scope":{}},"pixel_to_desktop":[1,0,0,1,0,0]});
        assert_eq!(
            snapshot_input_view(&grounded, &legacy_image),
            (json!("old-agent-view"), true)
        );
        assert_eq!(
            snapshot_input_view(
                &grounded,
                &json!({"view_id":null,"input_reference_available":false})
            ),
            (Value::Null, false)
        );
    }

    #[test]
    fn native_failure_receipts_are_not_hidden_by_successful_rpc_envelopes() {
        assert!(compactable_receipt(&DeviceResult::value(
            json!({"dispatch":"returned"})
        )));
        for data in [
            json!({"dispatch":"uncertain","dispatch_error":{"code":"secure_or_unknown_text_field"},"input_domain_fenced":true}),
            json!({"dispatch":"uncertain"}),
            json!({"dispatch":"returned","dispatch_error":{"code":"native_error"}}),
            json!({"dispatch":"returned","input_domain_fenced":true}),
            json!({"dispatch":"refused"}),
            json!({"code":"unknown_reply_shape"}),
        ] {
            assert!(!compactable_receipt(&DeviceResult::value(data)));
        }
        assert!(!compactable_receipt(&DeviceResult::error(
            "failure",
            "keep evidence"
        )));
    }
    #[test]
    fn typed_keyboard_calls_preserve_unicode_and_exact_element_without_focus_fallback() {
        let text = "你好\n🙂\t".to_string();
        let args = keyboard_action("f".into(), "e".into(), text.clone(), "text", 4096).unwrap();
        assert_eq!(args["value"], text);
        assert_eq!(args["element"], "e");
        assert_eq!(args["intent"], "text");
        assert!(args.get("focus").is_none());
        assert!(keyboard_action("f".into(), "e".into(), "x".repeat(4097), "text", 4096).is_err());
        assert!(keyboard_action("f".into(), "".into(), "cmd+s".into(), "key_chord", 128).is_err());
        assert!(serde_json::from_value::<KeyArgs>(json!({"target":"t","frame":"f","element":"e","request_id":"r","key":"s","shell":"no"})).is_err());
    }
    #[test]
    fn oversized_control_cannot_return_a_non_advancing_cursor() {
        let rows = vec![json!({"id":"x".repeat(13000),"actions":["AXPress"]})];
        let a = ControlsArgs {
            observation_ref: "o".into(),
            frame: "f".into(),
            query: String::new(),
            offset: 0,
            limit: 1,
        };
        let result = control_page(&rows, &a);
        assert!(result.is_error);
        assert_eq!(result.data["code"], "control_display_budget");
    }
    #[test]
    fn control_query_pages_all_matches_without_auto_selection() {
        let rows = vec![
            json!({"id":"a","description":"Undo","actions":["AXPress"]}),
            json!({"id":"b","name":"UNDO","actions":["AXPress"]}),
            json!({"id":"c","name":"Redo","actions":["AXPress"]}),
        ];
        let mut a = ControlsArgs {
            observation_ref: "o".into(),
            frame: "f".into(),
            query: "undo".into(),
            offset: 0,
            limit: 1,
        };
        let first = control_page(&rows, &a);
        assert_eq!(first.data["matches"], 2);
        assert_eq!(first.data["next_offset"], 1);
        assert_eq!(first.data["elements"][0]["id"], "a");
        a.offset = 1;
        let second = control_page(&rows, &a);
        assert!(second.data["next_offset"].is_null());
        assert_eq!(second.data["elements"][0]["id"], "b");
        a.query = "missing".into();
        a.offset = 0;
        assert_eq!(control_page(&rows, &a).data["matches"], 0);
    }
    #[test]
    fn retained_controls_require_matching_original_frame_and_source() {
        let mut cache = presentation::Presentation::default();
        let data = json!({"frame":"f","scene":{"elements":[{"id":"e"}]}});
        let reference = cache.retain("computer.observe", data.clone()).unwrap();
        assert_eq!(cache.observation_rows(&reference, "f").unwrap().len(), 1);
        assert_eq!(
            cache
                .observation_rows(&reference, "other")
                .unwrap_err()
                .data["code"],
            "observation_frame_mismatch"
        );
        let foreign = cache.retain("computer.targets", data).unwrap();
        assert!(cache.observation_rows(&foreign, "f").is_err());
        assert!(cache.observation_rows("unknown", "f").is_err());
    }
    #[test]
    fn snapshot_budget_keeps_anonymous_close_ahead_of_hidden_row_noise() {
        let rows = vec![
            json!({"id":"offscreen-row","role":"AXRow","name":"","actions":["AXShowDefaultUI"],"point":[10,10000],"visible":false,"occluded":true}),
            json!({"id":"close","role":"AXButton","name":"","actions":["AXPress"],"point":[17,19],"enabled":true}),
        ];
        let (out, total) = compact_controls(&rows, 1);
        assert_eq!(total, 2);
        assert_eq!(out[0]["id"], "close");
        assert_eq!(out[0]["point"], json!([17, 19]));
        assert_eq!(out[0]["actions"], json!(["AXPress"]));
    }
    #[test]
    fn default_collapses_noise_but_retains_full_count_and_preferred_order() {
        let rows = vec![
            json!({"id":"noise1","role":"AXRow","name":"","actions":["AXShowAlternateUI"],"visible":false}),
            json!({"id":"normal1","role":"AXButton","name":"","actions":["AXPress"]}),
            json!({"id":"noise2","role":"AXToolbar","actions":["AXShowMenu"],"occluded":true}),
            json!({"id":"normal2","role":"AXRow","name":"Selected font","actions":["AXShowDefaultUI"],"visible":false,"selected":true}),
        ];
        let (out, total) = compact_controls(&rows, 128);
        assert_eq!(total, 4);
        assert_eq!(
            out.iter()
                .map(|e| e["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["normal1", "normal2"]
        );
        assert_eq!(out[1]["selected"], true);
        let (original, _) = compact_controls_in_order(&rows, 128);
        assert_eq!(original.len(), 4);
        assert_eq!(original[2]["actions"], json!(["AXShowMenu"]));
    }
    #[test]
    fn covered_semantic_button_and_unknown_visibility_are_not_dropped_or_demoted() {
        let rows = vec![
            json!({"id":"covered","role":"AXButton","name":"","actions":["AXPress"],"visible":false,"occluded":true,"enabled":false,"point":[4,8]}),
            json!({"id":"unknown","role":"AXRow","name":"","actions":["AXShowDefaultUI"]}),
            json!({"id":"empty-title","role":"AXRow","description":"Font family","actions":["AXShowAlternateUI"],"visible":false}),
        ];
        let (out, total) = compact_controls(&rows, 128);
        assert_eq!(total, 3);
        assert_eq!(out[0]["id"], "covered");
        assert_eq!(out[0]["enabled"], false);
        assert_eq!(out[0]["visible"], false);
        assert_eq!(out[0]["point"], json!([4, 8]));
        assert_eq!(out[1]["id"], "unknown");
        assert_eq!(out[2]["name"], "Font family");
    }
    #[test]
    fn all_container_or_meaningful_state_tree_is_not_silently_emptied() {
        let rows =
            vec![json!({"id":"row","role":"AXRow","actions":["AXShowDefaultUI"],"visible":false})];
        let (out, total) = compact_controls(&rows, 64);
        assert_eq!(total, 1);
        assert_eq!(out[0]["id"], "row");
        let rows = vec![
            json!({"id":"selected","role":"AXRow","actions":["AXShowDefaultUI"],"visible":false,"selected":true}),
            json!({"id":"valued","role":"AXRow","actions":["AXShowDefaultUI"],"occluded":true,"value":"Arial"}),
            json!({"id":"close","role":"AXButton","actions":["AXPress"],"point":[17,19]}),
        ];
        let (out, total) = compact_controls(&rows, 64);
        assert_eq!(total, 3);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0]["selected"], true);
        assert_eq!(out[1]["value"], "Arial");
    }
    #[test]
    fn explicit_control_pages_preserve_original_order_offsets_and_metadata() {
        let rows = vec![
            json!({"id":"row","name":"","role":"AXRow","actions":["AXShowDefaultUI"],"visible":false}),
            json!({"id":"close","name":"","role":"AXButton","actions":["AXPress"],"point":[17,19]}),
        ];
        let args = |offset| ControlsArgs {
            observation_ref: "observation".into(),
            frame: "frame".into(),
            query: "".into(),
            offset,
            limit: 1,
        };
        let first = control_page(&rows, &args(0));
        let second = control_page(&rows, &args(1));
        assert_eq!(first.data["elements"][0]["id"], "row");
        assert_eq!(first.data["next_offset"], 1);
        assert_eq!(second.data["elements"][0]["id"], "close");
        assert_eq!(second.data["elements"][0]["point"], json!([17, 19]));
        assert!(second.data["next_offset"].is_null());
        assert_eq!(first.data["matches"], 2);
        assert_eq!(first.data["input_sent"], false);
    }
    #[test]
    fn snapshot_removes_non_native_action_noise_without_inventing_state() {
        let rows = vec![
            json!({"id":"x","name":"","description":"Undo","actions":["AXPress","Name:Move next\nTarget:0x0\nSelector:(null)"],"visible":true,"enabled":false,"occluded":false}),
            json!({"id":"noise","actions":["Name:Move next"]}),
        ];
        let (out, total) = compact_controls(&rows, 64);
        assert_eq!(total, 1);
        assert_eq!(out[0]["name"], "Undo");
        assert_eq!(out[0]["actions"], json!(["AXPress"]));
        assert_eq!(out[0]["enabled"], false);
        assert!(out[0].get("visible").is_none());
        assert!(out[0].get("selected").is_none());
    }
    #[tokio::test]
    async fn uncertain_action_is_not_replayed_and_cleanup_is_unconditional() {
        let calls = RefCell::new(Vec::new());
        let result = leased(
            "target",
            "one",
            "computer.act_checked",
            json!({"frame":"f","element":"e","intent":"click"}),
            |tool, args, cleanup| {
                calls.borrow_mut().push((tool, args, cleanup));
                std::future::ready(match tool {
                    "computer.claim" => DeviceResult::value(json!({"lease":"l"})),
                    "computer.release" => DeviceResult::value(json!({"released":true})),
                    _ => DeviceResult::error("native_action_unconfirmed", "unknown"),
                })
            },
        )
        .await;
        assert!(result.is_error);
        assert_eq!(result.data["code"], "native_action_unconfirmed");
        let c = calls.borrow();
        assert_eq!(c.len(), 3);
        assert_eq!(c[1].1["request_id"], "one");
        assert_eq!(c[1].1["lease"], "l");
        assert!(c[2].2);
        assert!(!c[1].2);
    }
    #[tokio::test]
    async fn cleanup_failure_keeps_original_action_evidence() {
        let result = leased("t", "r", "computer.act", json!({}), |tool, _, _| {
            std::future::ready(match tool {
                "computer.claim" => DeviceResult::value(json!({"lease":"l"})),
                "computer.release" => DeviceResult::error("offline", "unknown release"),
                _ => DeviceResult::value(
                    json!({"dispatch":"returned","operation_ref":{"id":"original"}}),
                ),
            })
        })
        .await;
        assert!(result.is_error);
        assert_eq!(result.data["action"]["operation_ref"]["id"], "original");
        assert_eq!(result.data["replay_allowed"], false);
    }
    #[tokio::test]
    async fn invalid_request_and_failed_claim_never_dispatch() {
        let result = leased("t", "", "computer.act", json!({}), |_, _, _| async {
            panic!("must not call")
        })
        .await;
        assert_eq!(result.data["code"], "invalid_arguments");
        let n = RefCell::new(0);
        let result = leased("t", "r", "computer.act", json!({}), |tool, _, _| {
            *n.borrow_mut() += 1;
            assert_eq!(tool, "computer.claim");
            std::future::ready(DeviceResult::error("busy", "no lease"))
        })
        .await;
        assert!(result.is_error);
        assert_eq!(*n.borrow(), 1);
    }
    #[test]
    fn compact_controls_are_bounded_and_keep_windows_controls() {
        let rows = vec![json!({"id":"win","control_type":50000,"name":"a".repeat(20000)}); 512];
        let (v, n) = compact_controls(&rows, 128);
        assert_eq!(n, 512);
        assert!(v.len() < 128);
        assert!(serde_json::to_vec(&v).unwrap().len() < 13 * 1024);
        assert_eq!(v[0]["display_truncated"], true);
        assert_eq!(v[0]["id"], "win");
    }
    #[test]
    fn schemas_are_titled_and_reject_unknown_or_ambiguous_arguments() {
        for t in tools() {
            assert!(t.title.is_some());
        }
        let schema = input_schema::<SnapshotArgs>();
        assert_eq!(schema["properties"]["include_timings"]["type"], "boolean");
        assert!(
            !schema["required"]
                .as_array()
                .is_some_and(|required| required.iter().any(|field| field == "include_timings"))
        );
        assert!(serde_json::from_value::<ClickArgs>(json!({"target":"t","request_id":"r","at":{"kind":"view","view_id":"v","x":1,"y":2,"element":"e"}})).is_err());
        assert!(serde_json::from_value::<DragArgs>(json!({"target":"t","request_id":"r","view_id":"v","from":[0,0],"to":[1,1],"shell":"no"})).is_err());
    }

    #[test]
    fn hover_contract_typed_tool_is_explicit_nonsemantic_and_bounded() {
        let catalog = tools();
        let hover = catalog
            .iter()
            .find(|tool| tool.name == "computer.hover")
            .expect("typed hover missing");
        assert!(handles("computer.hover"));
        let schema = serde_json::to_value(&hover.input_schema)
            .unwrap()
            .to_string();
        assert!(schema.contains("include_timings"));
        assert!(!schema.contains("AXPress"));
        assert!(
            hover
                .description
                .as_ref()
                .unwrap()
                .contains("not tooltip success")
        );
    }

    #[tokio::test]
    async fn hover_contract_is_one_pointer_intent_without_button_or_semantic_value() {
        for at in [
            json!({"kind":"element","frame":"f","element":"e"}),
            json!({"kind":"view","view_id":"v","x":4,"y":5}),
        ] {
            let args: HoverArgs =
                serde_json::from_value(json!({"target":"t","request_id":"hover-once","at":at}))
                    .unwrap();
            let calls = RefCell::new(vec![]);
            let result = hover(args, |tool, args, cleanup| {
                calls.borrow_mut().push((tool, args, cleanup));
                std::future::ready(match tool {
                    "computer.resolve_view" => {
                        DeviceResult::value(json!({"reference":"exact-ref"}))
                    }
                    "computer.claim" => DeviceResult::value(json!({"lease":"short-lease"})),
                    "computer.release" => DeviceResult::value(json!({"released":true})),
                    _ => DeviceResult::value(
                        json!({"dispatch":"acknowledged","goal_verified":false}),
                    ),
                })
            })
            .await;
            assert!(!result.is_error);
            assert_eq!(result.data["hover_effect"], "unverified");
            assert_eq!(result.data["motion_history"], "unknown");
            let compact = compact_action_reply(&result.data, None);
            assert_eq!(compact["hover_effect"], "unverified");
            assert_eq!(compact["motion_history"], "unknown");
            assert_eq!(compact["goal_verified"], false);
            let calls = calls.borrow();
            let actions: Vec<_> = calls
                .iter()
                .filter(|c| matches!(c.0, "computer.act" | "computer.act_checked"))
                .collect();
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].1["intent"], "move_pointer");
            assert_eq!(actions[0].1["request_id"], "hover-once");
            assert!(actions[0].1.get("value").is_none());
            assert!(actions[0].1.get("action").is_none());
            assert_eq!(calls.last().unwrap().0, "computer.release");
            assert!(calls.last().unwrap().2);
            if let Some(resolve) = calls.iter().find(|c| c.0 == "computer.resolve_view") {
                assert!(resolve.1.get("red_marker_cluster").is_none());
            }
        }
    }

    #[tokio::test]
    async fn hover_contract_preserves_explicit_marker_and_never_claims_on_resolve_refusal() {
        let args: HoverArgs = serde_json::from_value(json!({"target":"t","request_id":"hover-once","at":{"kind":"view","view_id":"v","x":4,"y":5,"red_marker_cluster":true}})).unwrap();
        let calls = RefCell::new(vec![]);
        let result = hover(args, |tool, args, cleanup| {
            calls.borrow_mut().push((tool, args, cleanup));
            std::future::ready(DeviceResult::error("stale_reference", "No input issued"))
        })
        .await;
        assert_eq!(result.data["code"], "stale_reference");
        let calls = calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "computer.resolve_view");
        assert_eq!(calls[0].1["red_marker_cluster"], true);
    }

    #[tokio::test]
    async fn hover_contract_releases_on_native_refusal_without_replay() {
        let args: HoverArgs = serde_json::from_value(json!({"target":"t","request_id":"hover-once","at":{"kind":"element","frame":"f","element":"e"}})).unwrap();
        let calls = RefCell::new(vec![]);
        let result = hover(args, |tool, args, cleanup| {
            calls.borrow_mut().push((tool, args, cleanup));
            std::future::ready(match tool {
                "computer.claim" => DeviceResult::value(json!({"lease":"short-lease"})),
                "computer.release" => DeviceResult::value(json!({"released":true})),
                _ => DeviceResult::error("capability_unavailable", "No pointer move backend"),
            })
        })
        .await;
        assert_eq!(result.data["code"], "capability_unavailable");
        let calls = calls.borrow();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].0, "computer.claim");
        assert_eq!(calls[1].0, "computer.act_checked");
        assert_eq!(calls[2].0, "computer.release");
        assert!(calls[2].2);
    }

    #[tokio::test]
    async fn hover_contract_invalid_identity_or_location_never_calls_device() {
        for (target, request, at) in [
            ("", "r", json!({"kind":"element","frame":"f","element":"e"})),
            ("t", "", json!({"kind":"element","frame":"f","element":"e"})),
            ("t", "r", json!({"kind":"element","frame":"","element":"e"})),
            ("t", "r", json!({"kind":"view","view_id":"","x":4,"y":5})),
        ] {
            let args: HoverArgs =
                serde_json::from_value(json!({"target":target,"request_id":request,"at":at}))
                    .unwrap();
            let result = hover(args, |_, _, _| {
                std::future::ready({
                    panic!("invalid hover must not call device");
                    #[allow(unreachable_code)]
                    DeviceResult::value(json!({}))
                })
            })
            .await;
            assert_eq!(result.data["code"], "invalid_arguments");
        }
        for at in [
            json!({"kind":"element","frame":"f","element":"e","action":"AXPress"}),
            json!({"kind":"view","view_id":"v","x":4,"y":5,"button":"left"}),
        ] {
            assert!(
                serde_json::from_value::<HoverArgs>(json!({"target":"t","request_id":"r","at":at}))
                    .is_err()
            );
        }
        for extra in ["value", "dwell_ms", "click_count", "activate"] {
            let mut args = json!({"target":"t","request_id":"r","at":{"kind":"element","frame":"f","element":"e"}});
            args[extra] = json!(1);
            assert!(serde_json::from_value::<HoverArgs>(args).is_err());
        }
    }
}

#[cfg(test)]
mod r20_degraded_observation_tests {
    use super::*;
    fn observed() -> DeviceResult {
        let mut r = DeviceResult::error("ax_attribute_unavailable", "AXWindows cannotComplete");
        r.data["diagnostic"] = json!({"api":"AXUIElementCopyAttributeValue","attribute":"AXWindows","ax_error":-25204,"operation_kind":"read"});
        r.data["native_dispatch_entered"] = json!(false);
        r.data["operation"] = json!({"terminal":true,"native_call_pending":false,"resources_released":true,"resource_release_uncertain":false});
        r
    }
    fn visual() -> DeviceResult {
        DeviceResult {
            is_error: false,
            data: json!({"frame_created":false,"input_authority_granted":false,"native_dispatch_entered":false,"operation":{"terminal":true,"native_call_pending":false,"resources_released":true,"resource_release_uncertain":false},"capture":{"window":{"window":"cgwindow:77","pid":7}}}),
            images: vec![rwi_core::device::DeviceImage {
                mime: "image/png".into(),
                base64: "fixture-only".into(),
            }],
        }
    }
    #[test]
    fn exact_terminal_released_ax_read_is_the_only_candidate() {
        assert!(read_only_ax_visual_candidate(&observed()));
        for (ptr, value) in [
            ("/code", json!("screen_capture_permission_missing")),
            ("/diagnostic/api", json!("AXUIElementPerformAction")),
            ("/diagnostic/attribute", json!("AXValue")),
            ("/diagnostic/ax_error", json!(-25205)),
            ("/diagnostic/operation_kind", json!("input")),
            ("/native_dispatch_entered", json!(true)),
            ("/operation/terminal", json!(false)),
            ("/operation/native_call_pending", json!(true)),
            ("/operation/resources_released", json!(false)),
            ("/operation/resource_release_uncertain", json!(true)),
        ] {
            let mut r = observed();
            *r.data.pointer_mut(ptr).unwrap() = value;
            assert!(!read_only_ax_visual_candidate(&r), "unsafe {ptr}");
        }
        let mut r = observed();
        r.is_error = false;
        assert!(!read_only_ax_visual_candidate(&r));
    }
    #[test]
    fn missing_cleanup_facts_do_not_become_false_or_authorized() {
        for name in [
            "terminal",
            "native_call_pending",
            "resources_released",
            "resource_release_uncertain",
        ] {
            let mut r = observed();
            r.data["operation"].as_object_mut().unwrap().remove(name);
            assert!(!read_only_ax_visual_candidate(&r));
        }
    }
    #[test]
    fn good_visual_still_keeps_ax_failure_unknown_controls_and_no_authority() {
        let old = observed();
        let data = old.data.clone();
        let r = degraded_ax_snapshot("same-target", old, visual());
        assert!(r.is_error);
        assert_eq!(r.data["observation_error"], data);
        assert_eq!(r.images.len(), 1);
        assert_eq!(r.data["visual_available"], true);
        assert!(
            r.data["frame"].is_null()
                && r.data["view_id"].is_null()
                && r.data["available_controls"].is_null()
        );
        for key in [
            "input_reference_available",
            "input_authority_granted",
            "mutation_ready",
            "native_complete",
            "goal_verified",
            "input_replayed",
        ] {
            assert_eq!(r.data[key], false);
        }
        assert_eq!(r.data["coverage"]["absence_proven"], false);
    }
    #[test]
    fn failed_or_unsettled_visual_cannot_leak_even_attached_media() {
        for (ptr, value) in [
            ("/native_dispatch_entered", json!(true)),
            ("/operation/native_call_pending", json!(true)),
            ("/operation/resources_released", json!(false)),
            ("/operation/resource_release_uncertain", json!(true)),
            ("/frame_created", json!(true)),
            ("/input_authority_granted", json!(true)),
        ] {
            let mut v = visual();
            *v.data.pointer_mut(ptr).unwrap() = value;
            let r = degraded_ax_snapshot("t", observed(), v);
            assert!(r.images.is_empty(), "bad {ptr}");
            assert_eq!(r.data["visual_available"], false);
        }
        let mut v = visual();
        v.is_error = true;
        let r = degraded_ax_snapshot("t", observed(), v);
        assert!(r.images.is_empty());
    }
}
