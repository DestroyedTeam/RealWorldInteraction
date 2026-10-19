use super::*;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct SnapshotArgs {
    pub(super) target: String,
    #[serde(default = "edge")]
    pub(super) max_edge: u32,
    #[serde(default = "limit")]
    pub(super) limit: usize,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ControlsArgs {
    /// Original observation_ref from computer.snapshot (not details_ref).
    pub(super) observation_ref: String,
    pub(super) frame: String,
    /// Case-insensitive substring of name, description or role. Empty lists all.
    #[serde(default)]
    pub(super) query: String,
    #[serde(default)]
    pub(super) offset: usize,
    #[serde(default = "controls_limit")]
    pub(super) limit: usize,
}
pub(super) fn controls_limit() -> usize {
    32
}
pub(super) fn edge() -> u32 {
    2048
}
pub(super) fn limit() -> usize {
    64
}
pub(super) fn take_include_timings(args: &mut Value) -> Result<bool, DeviceResult> {
    let Some(object) = args.as_object_mut() else {
        return Ok(false);
    };
    match object.remove("include_timings") {
        None => Ok(false),
        Some(Value::Bool(value)) => Ok(value),
        Some(_) => Err(invalid("include_timings must be a boolean.")),
    }
}

pub(super) fn expose_timings(name: &str, result: &DeviceResult, requested: bool) -> bool {
    requested
        || result.is_error
        || !(matches!(name, "computer.snapshot" | "computer.controls")
            || compactable_receipt(result)
            || name == "computer.bring_to_front" && bring_to_front_returned(result))
}

pub(super) fn trim_timing_fields(data: &mut Value) {
    if let Some(object) = data.as_object_mut() {
        object.remove("stage_timings");
        object.remove("elapsed_ms");
    }
    trim_known_capture_phases(data);
    compact_snapshot_diagnostics(data);
}

fn trim_known_capture_phases(data: &mut Value) {
    if data["input_domain_fenced"] != false {
        return;
    }
    for key in [
        "target",
        "frame",
        "view_id",
        "observation_ref",
        "details_ref",
    ] {
        if data
            .get(key)
            .and_then(Value::as_str)
            .is_none_or(str::is_empty)
        {
            return;
        }
    }
    if let Some(operation) = data.get("operation")
        && (!operation.is_object()
            || operation["terminal"] != true
            || operation["resources_released"] != true
            || operation["native_call_pending"] != false
            || operation["resource_release_uncertain"] != false)
    {
        return;
    }
    if !data["window"].is_object() || !data["mapping"].is_object() || !data["elements"].is_array() {
        return;
    }
    let Some(phases) = data
        .pointer("/consistency/capture_phases")
        .and_then(Value::as_object)
    else {
        return;
    };
    let brief = phases.len() == 9;
    let details = phases.len() == 12
        && phases.get("cpu_time").and_then(Value::as_bool) == Some(false)
        && phases
            .get("request_intervals_include_callback_setup")
            .and_then(Value::as_bool)
            == Some(true)
        && phases
            .get("phases_do_not_account_for_entire_caller_total")
            .and_then(Value::as_bool)
            == Some(true);
    if phases.get("scope").and_then(Value::as_str) != Some("native_capture_wall")
        || phases.get("units").and_then(Value::as_str) != Some("ms")
        || !(brief || details)
        || [
            "shareable_request_to_callback_ms",
            "screenshot_request_to_callback_ms",
            "bitmap_conversion_ms",
            "unpremultiply_ms",
            "content_digest_ms",
            "png_encode_ms",
            "caller_total_ms",
        ]
        .iter()
        .any(|key| {
            phases.get(*key).is_none_or(|value| {
                !value.is_null() && value.as_f64().is_none_or(|n| !n.is_finite() || n < 0.0)
            })
        })
    {
        return;
    }
    if let Some(consistency) = data.get_mut("consistency").and_then(Value::as_object_mut) {
        consistency.remove("capture_phases");
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ClickTarget {
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
pub(super) struct ClickArgs {
    /// One click (default) or one native double-click sequence; never a retry.
    #[serde(default = "single_click")]
    pub(super) click_count: u8,
    pub(super) target: String,
    pub(super) request_id: String,
    pub(super) at: ClickTarget,
    /// Explicit single pointer-click delivery; omission keeps the native default.
    #[serde(default, deserialize_with = "click_delivery_option")]
    #[schemars(with = "rwi_core::ClickDeliveryOptions")]
    pub(super) delivery: Option<rwi_core::ClickDeliveryOptions>,
}
fn click_delivery_option<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<rwi_core::ClickDeliveryOptions>, D::Error> {
    rwi_core::ClickDeliveryOptions::deserialize(deserializer).map(Some)
}
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum HoverTarget {
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
pub(super) struct HoverArgs {
    pub(super) target: String,
    pub(super) request_id: String,
    pub(super) at: HoverTarget,
    /// Omit for unchanged native default. Explicit routes require native capability;
    /// delta is AppKit-only, a nonzero pair in -128..128, not pointer history.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "hover_options_option"
    )]
    #[schemars(with = "HoverOptionsSchema")]
    pub(super) options: Option<rwi_core::MovePointerOptions>,
}
fn hover_options_option<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<rwi_core::MovePointerOptions>, D::Error> {
    rwi_core::MovePointerOptions::deserialize(deserializer).map(Some)
}

// Schema-only mirror; runtime parsing and native capability checks stay in core.
#[derive(JsonSchema)]
#[serde(tag = "delivery", rename_all = "snake_case", deny_unknown_fields)]
#[allow(dead_code)]
enum HoverOptionsSchema {
    BackgroundWindow {},
    BackgroundSkylightExperimental {},
    BackgroundCooperativeTrackingExperimental {
        phase: rwi_core::TrackingPhase,
        owner_endpoint: String,
        registration_id: String,
    },
    BackgroundAppkitTrackingExperimental {
        phase: rwi_core::TrackingPhase,
        /// Caller-registered public opaque NSInteger; lossless signed decimal string.
        tracking_number: String,
    },
    BackgroundAppkitExperimental {
        /// Optional declared native delta, not inferred motion history; [0,0] rejected.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(with = "[HoverDeltaComponentSchema; 2]")]
        delta: Option<[i32; 2]>,
    },
    /// Explicit system-cursor event requires the exact observed foreground tuple.
    ForegroundSystem {
        expected_foreground: rwi_core::ForegroundState,
    },
    /// Moves the physical cursor to an exposed target without activating it.
    /// Pin the current foreground even when it differs from the selected target.
    VisibleDesktopSystem {
        expected_foreground: rwi_core::ForegroundState,
    },
}
#[derive(JsonSchema)]
#[schemars(transparent)]
#[allow(dead_code)]
struct HoverDeltaComponentSchema(#[schemars(range(min = -128, max = 128))] i32);
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct RevealArgs {
    pub(super) target: String,
    pub(super) frame: String,
    pub(super) element: String,
    pub(super) request_id: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum ScrollTarget {
    Element { frame: String, element: String },
    View { view_id: String, x: f64, y: f64 },
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ScrollArgs {
    pub(super) target: String,
    pub(super) request_id: String,
    pub(super) at: ScrollTarget,
    /// Signed native vertical units: positive up, negative down. macOS pixels
    /// +/-2000; Windows wheel units, multiples of 120, +/-1200. Nonzero only.
    pub(super) delta_y: i32,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct DragArgs {
    pub(super) target: String,
    pub(super) request_id: String,
    pub(super) view_id: String,
    pub(super) from: [f64; 2],
    pub(super) to: [f64; 2],
    #[serde(default = "duration")]
    pub(super) duration_ms: u64,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct DragEndpointArgs {
    pub(super) target: String,
    pub(super) view_id: String,
    pub(super) point: [f64; 2],
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct DragBetweenArgs {
    pub(super) source: DragEndpointArgs,
    pub(super) destination: DragEndpointArgs,
    pub(super) request_id: String,
    #[serde(default = "duration")]
    pub(super) duration_ms: u64,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct TextArgs {
    pub(super) target: String,
    pub(super) frame: String,
    pub(super) element: String,
    pub(super) request_id: String,
    pub(super) text: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct KeyArgs {
    pub(super) target: String,
    pub(super) frame: String,
    pub(super) element: String,
    pub(super) request_id: String,
    pub(super) key: String,
    /// macOS-only opt-in legacy248 key-window records; runtime ABI/effect unverified.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "key_preparation_option"
    )]
    #[schemars(with = "rwi_core::KeyPreparation")]
    pub(super) preparation: Option<rwi_core::KeyPreparation>,
}
fn key_preparation_option<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<rwi_core::KeyPreparation>, D::Error> {
    rwi_core::KeyPreparation::deserialize(d).map(Some)
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PlaceWindowArgs {
    pub(super) target: String,
    pub(super) request_id: String,
    pub(super) display: rwi_core::DisplayTarget,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct BringToFrontArgs {
    pub(super) target: String,
    pub(super) request_id: String,
}
pub(super) fn single_click() -> u8 {
    1
}
pub(super) fn click_intent(count: u8, action: bool) -> Result<&'static str, DeviceResult> {
    match (count, action) {
        (1, _) => Ok("click"),
        (2, false) => Ok("double_click"),
        _ => Err(invalid(
            "click_count must be 1 or 2; native AX actions cannot be double-clicked.",
        )),
    }
}
pub(super) fn view_resolution_args(
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

pub(super) fn compact_action_reply(data: &Value, details_ref: Option<String>) -> Value {
    let mut reply = serde_json::Map::new();
    for key in [
        "request_id",
        "dispatch",
        "deduplicated",
        "operation_ref",
        "system_accepted",
        "text_effect",
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
        "requested_delivery",
        "requested_event_source",
        "requested_mouse_subtype",
    ] {
        // Omit an absent field, never an explicit null: a native unknown (such
        // as system_accepted:null) remains distinct from missing evidence.
        if let Some(value) = data.get(key) {
            reply.insert(key.into(), value.clone());
        }
    }
    if let Some(observation) = data.get("focus_observation") {
        reply.insert(
            "focus_observation".into(),
            click::focus::compact(observation),
        );
    }
    reply.insert("goal_verified".into(), json!(false));
    reply.insert("details_ref".into(), json!(details_ref));
    Value::Object(reply)
}
pub(super) fn convenience_timing_option(
    name: &str,
    args: &mut Value,
) -> Result<bool, DeviceResult> {
    if matches!(
        name,
        "computer.focus_element" | "computer.select_window_local" | "computer.set_text"
    ) {
        // Do not strip an unknown fifth model field before strict deserialization.
        Ok(false)
    } else {
        take_include_timings(args)
    }
}
pub(super) fn duration() -> u64 {
    500
}
pub(super) fn input_schema<T: JsonSchema>() -> Value {
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

pub(super) fn tool<T: JsonSchema>(name: &'static str, description: &'static str) -> Tool {
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
        "computer.bring_to_front" => "Bring selected window to foreground",
        "computer.drag_between" => "Drag between selected windows",
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
        "computer.drag_between" => tool.with_title("Drag between selected windows"),
        "computer.scroll" => tool.with_title("Scroll selected surface"),
        "computer.place_window" => tool.with_title("Place selected window"),
        _ => tool,
    }
}
