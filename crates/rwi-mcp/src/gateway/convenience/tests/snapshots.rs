use super::*;

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

#[test]
fn native_plain_display_candidate_missing_secure_unknown_and_foreign_context_stay_unknown() {
    let source = super::controls::native_plain_observation_fixture(0, json!(true));
    for (pointer, value) in [
        ("/adapter_status/adapter", json!("rust-windows-native")),
        ("/scene/binding", Value::Null),
        ("/scene/assurance", json!("unknown")),
        (
            "/adapter_status/observation_consistency/ax_coverage_after/editable_focus_availability/scope",
            json!("unknown"),
        ),
        (
            "/adapter_status/observation_consistency/ax_coverage_after/editable_focus_availability/input_authority",
            Value::Null,
        ),
        (
            "/adapter_status/observation_consistency/ax_coverage_after/editable_focus_availability/elements/0/element",
            json!("secure-or-unclassified-id"),
        ),
        (
            "/adapter_status/observation_consistency/ax_coverage_after/editable_focus_availability/elements/0/native_status",
            Value::Null,
        ),
    ] {
        let mut unknown = source.clone();
        *unknown.pointer_mut(pointer).unwrap() = value;
        assert!(native_plain_display_candidates(&unknown).is_empty());
    }
    let mut missing = source.clone();
    missing["adapter_status"]["observation_consistency"]["ax_coverage_after"]
        .as_object_mut()
        .unwrap()
        .remove("editable_focus_availability");
    assert!(native_plain_display_candidates(&missing).is_empty());
    let mut omitted = source.clone();
    omitted["adapter_status"]["observation_consistency"]["ax_coverage_after"]["editable_focus_availability"]
        ["elements"] = json!([]);
    assert!(native_plain_display_candidates(&omitted).is_empty());
    // An unlisted secure/unknown row with the same generic AXTextField role
    // never obtains this display eligibility from the role alone.
    assert!(!eligible_control(&source["scene"]["elements"][0]));
}

#[test]
fn native_plain_display_candidate_legacy_pages_and_budget_omissions_are_preserved() {
    let mut observation = super::controls::native_plain_observation_fixture(0, json!(true));
    let field = observation["scene"]["elements"][0].clone();
    let rows = observation["scene"]["elements"].as_array_mut().unwrap();
    rows.push(
        json!({"id":"AX","name":"Undo","actions":["AXPress"],"point":[1,2],"future_state":null}),
    );
    rows.push(json!({"id":"UIA","name":"Windows button","control_type":50000}));
    let mut unprobed = field;
    unprobed["id"] = json!("budget-omitted-plain");
    rows.push(unprobed);
    observation["adapter_status"]["observation_consistency"]["ax_coverage_after"]["editable_focus_availability"]
        ["budget_omitted"] = json!(1);
    let original = observation.clone();
    let candidates = native_plain_display_candidates(&observation);
    assert_eq!(candidates.len(), 1);
    let rows = observation["scene"]["elements"].as_array().unwrap();
    let (legacy, count) = compact_controls(rows, 64);
    assert_eq!(count, 2);
    let (unchanged, n) = compact_controls_with_candidates(rows, 64, &Default::default());
    assert_eq!((unchanged, n), (legacy, count));
    let args = |offset| ControlsArgs {
        observation_ref: "same-ref".into(),
        frame: "plain-frame".into(),
        query: String::new(),
        offset,
        limit: 1,
    };
    for (offset, id) in ["plain", "AX", "UIA"].into_iter().enumerate() {
        let page = control_page_with_candidates(rows, &args(offset), &candidates);
        assert_eq!(page.data["elements"][0]["id"], id);
        assert_eq!(page.data["matches"], 3);
        assert_eq!(page.data["frame_refreshed"], false);
    }
    assert_eq!(observation, original);
}
