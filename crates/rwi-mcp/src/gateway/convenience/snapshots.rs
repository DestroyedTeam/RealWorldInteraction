use super::*;

pub(super) fn snapshot_mutation_ready(data: &Value) -> Value {
    if data["input_domain_fenced"] == true {
        json!(false)
    } else {
        data["scene"]["mutation_ready"].clone()
    }
}

pub(super) fn snapshot_native_complete(data: &Value) -> Value {
    data["adapter_status"]["observation_consistency"]["ax_coverage_after"]["complete"]
        .as_bool()
        .map_or_else(|| json!("unknown"), |complete| json!(complete))
}

#[test]
pub(super) fn snapshot_coverage_preserves_unknown_for_old_or_visual_only_agents() {
    assert_eq!(snapshot_native_complete(&json!({})), "unknown");
    for complete in [true, false] {
        let evidence = json!({"adapter_status":{"observation_consistency":{"ax_coverage_after":{"complete":complete}}}});
        assert_eq!(snapshot_native_complete(&evidence), complete);
    }
}

pub(super) fn snapshot_window(data: &Value) -> Value {
    let binding = &data["scene"]["binding"];
    if !binding["scope"]["window"].is_string() || !binding["scope"]["pid"].is_u64() {
        return Value::Null;
    }
    json!({"window":binding["scope"]["window"],"pid":binding["scope"]["pid"],"title":binding["mutation_context"]["window_title"]})
}

#[test]
pub(super) fn snapshot_window_uses_observed_binding_not_an_inferred_title_or_rendering_window() {
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
pub(super) fn completed_native_read(result: &DeviceResult) -> bool {
    let d = &result.data;
    d["native_dispatch_entered"] == false
        && d["operation"]["terminal"] == true
        && d["operation"]["native_call_pending"] == false
        && d["operation"]["resources_released"] == true
        && d["operation"]["resource_release_uncertain"] == false
}

pub(super) fn degraded_ax_snapshot(
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

pub(super) fn read_only_ax_visual_candidate(observed: &DeviceResult) -> bool {
    let d = &observed.data;
    observed.is_error
        && d["code"] == "ax_attribute_unavailable"
        && d["diagnostic"]["api"] == "AXUIElementCopyAttributeValue"
        && d["diagnostic"]["attribute"] == "AXWindows"
        && d["diagnostic"]["ax_error"] == -25204
        && d["diagnostic"]["operation_kind"] == "read"
        && completed_native_read(observed)
}

pub(super) fn snapshot_input_view(scene: &Value, image: &Value) -> (Value, bool) {
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
pub(super) fn compactable_receipt(result: &DeviceResult) -> bool {
    !result.is_error
        && result.data["dispatch"] == "returned"
        && result.data["dispatch_error"].is_null()
        && result.data["input_domain_fenced"] != true
}

pub(super) fn keyboard_action(
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
