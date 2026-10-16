use super::*;

/// Snapshot display only, after the original reply was retained. Timing output
/// does not require a second identical coverage object; errors stay unmodified.
pub(super) fn compact_snapshot_reply_diagnostics(name: &str, result: &mut DeviceResult) {
    if name == "computer.snapshot" && !result.is_error {
        compact_snapshot_diagnostics(&mut result.data);
    }
}

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
mod r94_diagnostics_tests;

// Fixed input and real Presentation/cache seams; fixture metadata is not native timing evidence.
#[cfg(test)]
mod f16_capture_phase_optin_copy_oracle {
    use super::*;
    use crate::gateway::presentation::{Presentation, ResultArgs};
    fn phases() -> Value {
        json!({"scope":"native_capture_wall","units":"ms",
            "shareable_request_to_callback_ms":3.0,"screenshot_request_to_callback_ms":5.0,
            "bitmap_conversion_ms":1.0,"unpremultiply_ms":0.0,"content_digest_ms":0.25,
            "png_encode_ms":2.0,"caller_total_ms":null})
    }
    fn displayed(mut data: Value, requested: bool, error: bool) -> Value {
        let mut result = DeviceResult::value(data.take());
        result.is_error = error;
        if !expose_timings("computer.snapshot", &result, requested) {
            trim_timing_fields(&mut result.data);
        }
        result.data
    }
    fn legacy(mut data: Value) -> Value {
        data.as_object_mut().unwrap().remove("stage_timings");
        data.as_object_mut().unwrap().remove("elapsed_ms");
        compact_snapshot_diagnostics(&mut data);
        data
    }
    fn cached(cache: &mut Presentation, reference: &str, pointer: &str) -> Value {
        cache
            .read(ResultArgs {
                result_ref: reference.into(),
                pointer: pointer.into(),
                offset: 0,
                limit: 64,
                byte_offset: None,
            })
            .data
    }
    #[test]
    fn retained_native_wall_box_is_only_inline_when_requested_or_uncertain() {
        let original = json!({"adapter_status":{"observation_consistency":{
            "capture_phases":phases(),"scope":"last_observe"}},"scene":{"elements":[]}});
        let hash = rwi_core::digest(&serde_json::to_vec(&original).unwrap());
        let mut cache = Presentation::default();
        let observation = cache.retain("computer.observe", original.clone()).unwrap();
        let mut raw = json!({"target":"fixture-target","frame":"fixture-frame","view_id":"fixture-view",
            "window":{"pid":42,"window":"cgwindow:42","title":"Owned fixture"},
            "mapping":{"view_size":[1320,1104],"original_to_desktop":[0.5,0,0,0.5,3036,504]},
            "elements":[{"id":"a".repeat(64),"name":"Close window","role":"AXButton",
                "actions":["AXPress"],"point":[3052,520],"enabled":false,"visible":null}],
            "coverage":{"complete":false,"absence_proven":false,"limits":null},
            "native_complete":false,"mutation_ready":false,"input_domain_fenced":false,
            "blocked":null,"available_controls":1,"omitted_controls":0,"unknown_future":null,
            "observation_ref":observation,"consistency":original["adapter_status"]["observation_consistency"],
            "stage_timings":[{"tool":"computer.observe","elapsed_ms":18}],"elapsed_ms":20});
        let retained = raw.clone();
        let retained_hash = rwi_core::digest(&serde_json::to_vec(&retained).unwrap());
        let details = cache.retain("computer.snapshot", retained.clone()).unwrap();
        raw["details_ref"] = json!(details);
        let full = displayed(raw.clone(), true, false);
        assert_eq!(full, raw);
        assert_eq!(displayed(raw.clone(), false, true), raw);
        for operation in [
            Value::Null,
            json!({}),
            json!({"terminal":false,"resources_released":false,
            "native_call_pending":true,"resource_release_uncertain":true}),
        ] {
            let mut uncertain = raw.clone();
            uncertain["operation"] = operation;
            assert_eq!(
                displayed(uncertain.clone(), false, false),
                legacy(uncertain)
            );
        }
        for value in [json!(true), Value::Null] {
            let mut fenced = raw.clone();
            fenced["input_domain_fenced"] = json!(value);
            assert_eq!(displayed(fenced.clone(), false, false), legacy(fenced));
        }
        for key in [
            "target",
            "frame",
            "view_id",
            "details_ref",
            "observation_ref",
        ] {
            for value in [Value::Null, json!("")] {
                let mut unavailable = raw.clone();
                unavailable[key] = value;
                assert_eq!(
                    displayed(unavailable.clone(), false, false),
                    legacy(unavailable)
                );
            }
        }
        for key in ["window", "mapping", "elements"] {
            let mut unknown = raw.clone();
            unknown[key] = Value::Null;
            assert_eq!(displayed(unknown.clone(), false, false), legacy(unknown));
        }
        let mut invalid_boxes = vec![Value::Null, json!({}), json!("unknown")];
        for (pointer, value) in [
            ("scope", json!("future")),
            ("units", json!("ns")),
            ("png_encode_ms", json!("unknown")),
            ("png_encode_ms", json!(-1)),
            ("future_phase", Value::Null),
        ] {
            let mut value_box = phases();
            value_box[pointer] = value;
            invalid_boxes.push(value_box);
        }
        let mut missing = phases();
        missing.as_object_mut().unwrap().remove("caller_total_ms");
        invalid_boxes.push(missing);
        for value_box in invalid_boxes {
            let mut future = raw.clone();
            future["consistency"]["capture_phases"] = value_box;
            assert_eq!(displayed(future.clone(), false, false), legacy(future));
        }
        let original_read = cached(
            &mut cache,
            &observation,
            "/adapter_status/observation_consistency/capture_phases",
        );
        let details_read = cached(&mut cache, &details, "/consistency/capture_phases");
        assert_eq!(original_read["value"], phases());
        assert_eq!(original_read["metadata_sha256"], hash);
        assert_eq!(details_read["value"], phases());
        assert_eq!(details_read["metadata_sha256"], retained_hash);
        assert_eq!(original_read["native_dispatched"], false);
        assert_eq!(details_read["native_dispatched"], false);
        let default = displayed(raw.clone(), false, false);
        let mut expected = legacy(raw.clone());
        expected["consistency"]
            .as_object_mut()
            .unwrap()
            .remove("capture_phases");
        let default_ok = default == expected;
        let mut details_box = phases();
        details_box["cpu_time"] = json!(false);
        details_box["request_intervals_include_callback_setup"] = json!(true);
        details_box["phases_do_not_account_for_entire_caller_total"] = json!(true);
        let mut detailed = raw.clone();
        detailed["consistency"]["capture_phases"] = details_box;
        let mut expected_detailed = legacy(detailed.clone());
        expected_detailed["consistency"]
            .as_object_mut()
            .unwrap()
            .remove("capture_phases");
        let detailed_ok = displayed(detailed.clone(), false, false) == expected_detailed;
        for key in [
            "cpu_time",
            "request_intervals_include_callback_setup",
            "phases_do_not_account_for_entire_caller_total",
        ] {
            let mut malformed = detailed.clone();
            malformed["consistency"]["capture_phases"][key] = Value::Null;
            assert_eq!(
                displayed(malformed.clone(), false, false),
                legacy(malformed)
            );
        }
        let mut completed = raw.clone();
        completed["operation"] = json!({"terminal":true,"resources_released":true,
            "native_call_pending":false,"resource_release_uncertain":false});
        let mut expected_completed = expected.clone();
        expected_completed["operation"] = completed["operation"].clone();
        // Baseline must emit the fixed receipt before the final semantic assertion.
        let completed_ok = displayed(completed, false, false) == expected_completed;
        assert_eq!(displayed(detailed.clone(), true, false), detailed);
        println!(
            "F16_CAPTURE_PHASE_PROJECTION_RECEIPT={}",
            json!({
            "scope":"real_MCP_default_projection_and_Presentation_cache_unit_seam_not_native_capture_or_MCP_stdio",
            "default_only_known_box_removed":default_ok,"details_known_box_removed":detailed_ok,
            "completed_operation_known_box_removed":completed_ok,
            "explicit_true_and_error_full":true,"fenced_uncertain_unknown_unretained_preserved":true,
            "observe_and_snapshot_original_cache_hash_and_values_preserved":true,
            "UTF8_default_bytes":serde_json::to_vec(&default).unwrap().len(),
            "UTF8_legacy_bytes":serde_json::to_vec(&legacy(raw)).unwrap().len(),
            "UTF8_explicit_true_bytes":serde_json::to_vec(&full).unwrap().len(),
            "tokens_CPU_native_latency_benefit_attested":false,"GUI_or_native_input":false})
        );
        assert!(
            default_ok && detailed_ok && completed_ok,
            "Expected retained known capture phases hidden only by default; full cache and explicit timings stay intact."
        );
    }
}
