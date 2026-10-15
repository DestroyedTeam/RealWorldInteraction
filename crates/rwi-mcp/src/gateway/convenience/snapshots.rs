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
        // Exact production reply seam used after retaining full diagnostics.
        compact_snapshot_reply_diagnostics(name, &mut result);
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
    fn r94_errors_uncertainty_keep_full_metadata_and_timings_keep_all_distinct_fields() {
        let raw = snapshot();
        let mut error = DeviceResult::value(&raw);
        error.is_error = true;
        assert_eq!(default_reply("computer.snapshot", error, false).data, raw);
        let mut timed_expected = raw.clone();
        timed_expected["consistency"]
            .as_object_mut()
            .unwrap()
            .remove("ax_coverage_after");
        timed_expected["consistency"]["ax_coverage_after_ref"] = json!("/coverage");
        assert_eq!(
            default_reply("computer.snapshot", DeviceResult::value(&raw), true).data,
            timed_expected
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
    #[test]
    fn source21_snapshot_coverage_explicit_timings_preserves_every_distinct_field() {
        let raw = snapshot();
        let mut expected = raw.clone();
        expected["consistency"]
            .as_object_mut()
            .unwrap()
            .remove("ax_coverage_after");
        expected["consistency"]["ax_coverage_after_ref"] = json!("/coverage");
        let after = default_reply("computer.snapshot", DeviceResult::value(&raw), true).data;
        assert_eq!(after, expected);
        assert_eq!(after["stage_timings"], raw["stage_timings"]);
        assert_eq!(after["elapsed_ms"], raw["elapsed_ms"]);
        assert_eq!(after["elements"], raw["elements"]);
        assert_eq!(after["coverage"], raw["coverage"]);
    }

    #[test]
    fn source21_snapshot_coverage_recorded_projection_is_reversible_and_smaller() {
        // Root's matching gate supplies one frozen own-fixture reply, never UI.
        // Ordinary offline CI uses the existing synthetic snapshot above.
        let (raw, scope) = match std::env::var_os("RWI_S21_SNAPSHOT_FIXTURE") {
            Some(path) => (
                serde_json::from_slice::<Value>(&std::fs::read(path).unwrap()).unwrap(),
                "frozen_recorded_reply",
            ),
            None => (snapshot(), "synthetic_reply"),
        };
        let before_bytes = serde_json::to_vec(&raw).unwrap().len();
        let mut after = default_reply("computer.snapshot", DeviceResult::value(&raw), true).data;
        let after_bytes = serde_json::to_vec(&after).unwrap().len();
        assert!(after_bytes < before_bytes);
        assert_eq!(after["consistency"]["ax_coverage_after_ref"], "/coverage");
        let coverage = after.pointer("/coverage").unwrap().clone();
        let consistency = after["consistency"].as_object_mut().unwrap();
        consistency.remove("ax_coverage_after_ref");
        consistency.insert("ax_coverage_after".into(), coverage);
        assert_eq!(
            after, raw,
            "expanding the same-reply pointer must recover every value"
        );
        println!(
            "{}",
            json!({"probe":"source21_snapshot_coverage","scope":scope,
            "before_utf8_bytes":before_bytes,"after_utf8_bytes":after_bytes,
            "saved_utf8_bytes":before_bytes-after_bytes,"semantic_reconstruction_equal":true,
            "Native_GUI_executed":false,"CPU_or_token_savings_measured":false})
        );
    }

    #[test]
    fn source21_snapshot_coverage_unknown_and_future_members_are_not_false_or_omitted() {
        let mut raw = snapshot();
        raw["coverage"]["complete"] = Value::Null;
        raw["coverage"]["future_coverage"] = json!({"limit":null,"status":"unknown"});
        raw["consistency"]["ax_coverage_after"] = raw["coverage"].clone();
        raw["native_complete"] = json!("unknown");
        raw["elements"][0]["value"] = json!("");
        let after = default_reply("computer.snapshot", DeviceResult::value(&raw), true).data;
        assert_eq!(after["coverage"], raw["coverage"]);
        assert!(after["coverage"]["complete"].is_null());
        assert!(after["coverage"]["future_coverage"]["limit"].is_null());
        assert_eq!(after["native_complete"], "unknown");
        assert_eq!(after["elements"], raw["elements"]);
        assert_eq!(after["consistency"]["before_elements"], Value::Null);
        assert_eq!(
            after["consistency"]["future_consistency"],
            raw["consistency"]["future_consistency"]
        );
    }

    #[test]
    fn source21_snapshot_coverage_unequal_error_pending_and_unretained_pass_through() {
        let mut bad = Vec::new();
        for (pointer, value) in [
            ("/details_ref", Value::Null),
            ("/frame", json!("")),
            ("/coverage", Value::Null),
            ("/consistency/ax_coverage_after", json!({"complete":null})),
        ] {
            let mut raw = snapshot();
            *raw.pointer_mut(pointer).unwrap() = value;
            bad.push(raw);
        }
        let mut pending = snapshot();
        pending["operation"] = json!({"terminal":false,"resources_released":false,
            "native_call_pending":true,"resource_release_uncertain":true});
        bad.push(pending);
        for raw in bad {
            assert_eq!(
                default_reply("computer.snapshot", DeviceResult::value(&raw), true).data,
                raw
            );
        }
        let raw = snapshot();
        let mut error = DeviceResult::value(&raw);
        error.is_error = true;
        assert_eq!(default_reply("computer.snapshot", error, true).data, raw);
        assert_eq!(
            default_reply("computer.controls", DeviceResult::value(&raw), true).data,
            raw
        );
        assert_eq!(
            default_reply("computer.click", DeviceResult::value(&raw), true).data,
            raw
        );
    }

    #[test]
    fn source21_snapshot_coverage_retained_details_hash_and_original_pointer_are_unchanged() {
        use crate::gateway::presentation::{Presentation, ResultArgs};
        let mut raw = snapshot();
        raw.as_object_mut().unwrap().remove("details_ref");
        let original_bytes = serde_json::to_vec(&raw).unwrap();
        let expected_hash = rwi_core::digest(&original_bytes);
        let mut cache = Presentation::default();
        let details = cache.retain("computer.snapshot", raw.clone()).unwrap();
        let mut displayed = raw.clone();
        displayed["details_ref"] = json!(details);
        let after = default_reply("computer.snapshot", DeviceResult::value(displayed), true).data;
        assert_eq!(after["details_ref"], details);
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
        assert_eq!(serde_json::to_vec(&raw).unwrap(), original_bytes);
    }
}

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
