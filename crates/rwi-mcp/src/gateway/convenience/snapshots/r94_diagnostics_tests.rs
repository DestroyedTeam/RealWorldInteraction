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
