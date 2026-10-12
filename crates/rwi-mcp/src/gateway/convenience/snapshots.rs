use super::*;

/// Default snapshot display only, after the original reply was retained.
/// No broad allowlist: unknown fields, limits and mismatched shapes survive.
pub(super) fn compact_snapshot_diagnostics(data: &mut Value) {
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
            .is_none_or(|s| s.is_empty())
        {
            return;
        }
    }
    // A future/extended snapshot may carry a completion receipt. Never hide its
    // inline coverage when that receipt is uncertain, partial or malformed.
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
    let Some(coverage) = data.get("coverage").filter(|v| v.is_object()).cloned() else {
        return;
    };
    let Some(consistency) = data.get_mut("consistency").and_then(Value::as_object_mut) else {
        return;
    };
    if consistency.contains_key("ax_coverage_after_ref")
        || consistency.get("ax_coverage_after") != Some(&coverage)
    {
        return;
    }
    consistency.remove("ax_coverage_after");
    consistency.insert("ax_coverage_after_ref".into(), json!("/coverage"));
}

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

#[cfg(test)]
mod r94_diagnostics_tests {
    use super::*;
    fn coverage() -> Value {
        json!({"absence_proven":false,"child_reads_failed":2,"complete":false,
            "described_nodes":50,"descriptions_failed":4,"dropped_queue_nodes":0,
            "input_authority":false,
            "limitations":"no continuous tree/hidden-content attestation; depth-boundary children not read",
            "maximum_depth_seen":12,"pending_nodes":0,
            "reasons":["children_read_unavailable","element_read_unavailable"],
            "scope":"selected_window_AXChildren_at_read_time","unexpanded_depth_nodes":0,
            "visited_nodes":54})
    }
    fn snapshot() -> Value {
        let coverage = coverage();
        json!({"target":"fixture-target","frame":"frame-v1","view_id":"view-v1",
            "window":{"pid":42,"window":"window-v1","bounds":[3,4,5,6]},
            "mapping":{"pixel_to_desktop":[1,0,0,1,3,4],"image_size":[800,600]},
            "observation_ref":"observation-v1","details_ref":"details-v1",
            "elements":[{"id":"a".repeat(64),"name":"Close window","role":"AXButton",
                "actions":["AXPress"],"point":[13,24],"enabled":false,"selected":false,
                "visible":null,"occluded":null,"value_truncated":true}],
            "coverage":coverage,"native_complete":false,"available_controls":28,
            "omitted_controls":27,"reference_max_age_ms":5000,"blocked":null,
            "mutation_ready":false,"input_domain_fenced":true,"input_reference_available":false,
            "visual_available":true,"visual_only":false,"unknown_future":null,
            "consistency":{"ax_coverage_after":coverage,"before_elements":null,
                "pixel_semantic_atomic":false,"window_context_stable":true,
                "input_revalidation_required":true,"future_consistency":{"unknown":null}},
            "stage_timings":[{"tool":"computer.observe","elapsed_ms":12}],"elapsed_ms":17})
    }
    fn default_reply(name: &str, mut result: DeviceResult, requested: bool) -> DeviceResult {
        // Exact existing default-reply seam used after retaining full diagnostics.
        if !expose_timings(name, &result, requested) {
            trim_timing_fields(&mut result.data);
        }
        result
    }
    fn old_default(mut data: Value) -> Value {
        let object = data.as_object_mut().unwrap();
        object.remove("stage_timings");
        object.remove("elapsed_ms");
        data
    }
    #[test]
    fn r94_default_snapshot_replaces_only_exact_coverage_duplicate() {
        let raw = snapshot();
        let result = default_reply("computer.snapshot", DeviceResult::value(&raw), false);
        let mut expected = old_default(raw);
        expected["consistency"]
            .as_object_mut()
            .unwrap()
            .remove("ax_coverage_after");
        expected["consistency"]["ax_coverage_after_ref"] = json!("/coverage");
        assert_eq!(result.data, expected);
        assert!(!result.is_error && result.images.is_empty());
    }
    #[test]
    fn r94_unknown_unequal_or_unretained_snapshot_shapes_pass_through() {
        let mut fixtures = Vec::new();
        for (pointer, value) in [
            ("/details_ref", Value::Null),
            ("/details_ref", json!("")),
            ("/observation_ref", Value::Null),
            ("/frame", Value::Null),
            ("/view_id", Value::Null),
            ("/window", Value::Null),
            ("/mapping", Value::Null),
            ("/elements", json!({})),
            ("/coverage", Value::Null),
            ("/consistency", Value::Null),
            ("/consistency/ax_coverage_after", json!({"complete":true})),
        ] {
            let mut value_fixture = snapshot();
            *value_fixture.pointer_mut(pointer).unwrap() = value;
            fixtures.push(value_fixture);
        }
        for key in [
            "target",
            "details_ref",
            "observation_ref",
            "coverage",
            "consistency",
        ] {
            let mut missing = snapshot();
            missing.as_object_mut().unwrap().remove(key);
            fixtures.push(missing);
        }
        let mut already_ref = snapshot();
        already_ref["consistency"]["ax_coverage_after_ref"] = json!("/future");
        fixtures.push(already_ref);
        let mut controls = json!({"frame":"frame-v1","observation_ref":"o","elements":[],
            "matches":0,"next_offset":null,"native_complete":"unknown","elapsed_ms":17});
        controls["stage_timings"] = json!([]);
        fixtures.push(controls);
        for raw in fixtures {
            let got = default_reply("computer.snapshot", DeviceResult::value(&raw), false);
            assert_eq!(got.data, old_default(raw));
        }
    }
    #[test]
    fn r94_snapshot_operation_uncertainty_keeps_coverage_inline() {
        for operation in [
            Value::Null,
            json!("unknown"),
            json!({}),
            json!({"terminal":true}),
            json!({"terminal":false,"resources_released":true,
                "native_call_pending":false,"resource_release_uncertain":false}),
            json!({"terminal":true,"resources_released":false,
                "native_call_pending":false,"resource_release_uncertain":false}),
            json!({"terminal":true,"resources_released":true,
                "native_call_pending":true,"resource_release_uncertain":false}),
            json!({"terminal":true,"resources_released":true,
                "native_call_pending":false,"resource_release_uncertain":true}),
        ] {
            let mut raw = snapshot();
            raw["operation"] = operation;
            let result = default_reply("computer.snapshot", DeviceResult::value(&raw), false);
            assert_eq!(
                result.data,
                old_default(raw),
                "coverage stays inline; existing timing default may trim"
            );
        }
        let mut raw = snapshot();
        raw["operation"] = json!({"terminal":true,"resources_released":true,
            "native_call_pending":false,"resource_release_uncertain":false,"future_unknown":null});
        let result = default_reply("computer.snapshot", DeviceResult::value(&raw), false);
        let mut expected = old_default(raw);
        expected["consistency"]
            .as_object_mut()
            .unwrap()
            .remove("ax_coverage_after");
        expected["consistency"]["ax_coverage_after_ref"] = json!("/coverage");
        assert_eq!(result.data, expected);
    }
    #[test]
    fn r94_errors_uncertainty_and_explicit_timings_keep_full_metadata() {
        let raw = snapshot();
        let mut error = DeviceResult::value(&raw);
        error.is_error = true;
        assert_eq!(default_reply("computer.snapshot", error, false).data, raw);
        assert_eq!(
            default_reply("computer.snapshot", DeviceResult::value(&raw), true).data,
            raw
        );
        let mut uncertain = raw.clone();
        uncertain["operation"] = json!({"terminal":false,
            "resources_released":false,"resource_release_uncertain":true,"native_call_pending":true});
        assert_eq!(
            default_reply("computer.click", DeviceResult::value(&uncertain), false).data,
            uncertain
        );
    }
    #[test]
    fn r94_original_details_cache_and_hash_are_unchanged() {
        use crate::gateway::presentation::{Presentation, ResultArgs};
        let mut raw = snapshot();
        raw.as_object_mut().unwrap().remove("details_ref");
        let expected_hash = rwi_core::digest(&serde_json::to_vec(&raw).unwrap());
        let mut cache = Presentation::default();
        let details = cache.retain("computer.snapshot", raw.clone()).unwrap();
        let mut displayed = raw.clone();
        displayed["details_ref"] = json!(details);
        let _projected = default_reply("computer.snapshot", DeviceResult::value(displayed), false);
        let read = cache.read(ResultArgs {
            result_ref: details,
            pointer: "/consistency/ax_coverage_after".into(),
            offset: 0,
            limit: 16,
            byte_offset: None,
        });
        assert_eq!(read.data["value"], raw["coverage"]);
        assert_eq!(read.data["metadata_sha256"], expected_hash);
        assert_eq!(read.data["native_dispatched"], false);
    }
    #[test]
    fn r94_recorded_coverage_model_is_443_bytes_smaller_with_pointer() {
        let raw = snapshot();
        let before = old_default(raw.clone());
        let after = default_reply("computer.snapshot", DeviceResult::value(raw), false).data;
        assert_eq!(
            serde_json::to_vec(&before).unwrap().len() - serde_json::to_vec(&after).unwrap().len(),
            443
        );
    }
    #[test]
    fn r94_future_coverage_members_and_unknown_limits_remain_visible() {
        let mut raw = snapshot();
        raw["coverage"]["future_bounds"] = json!({"limit":null,"reason":"unknown"});
        raw["consistency"]["ax_coverage_after"] = raw["coverage"].clone();
        let result = default_reply("computer.snapshot", DeviceResult::value(&raw), false);
        assert_eq!(result.data["coverage"], raw["coverage"]);
        for key in [
            "elements",
            "window",
            "mapping",
            "frame",
            "view_id",
            "target",
            "details_ref",
            "observation_ref",
            "native_complete",
            "mutation_ready",
            "blocked",
            "input_domain_fenced",
            "input_reference_available",
            "available_controls",
            "omitted_controls",
            "unknown_future",
        ] {
            assert_eq!(result.data[key], raw[key], "changed {key}");
        }
    }
}
