use super::*;

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
fn compact_controls_are_bounded_and_keep_windows_controls() {
    let rows = vec![json!({"id":"win","control_type":50000,"name":"a".repeat(20000)}); 512];
    let (v, n) = compact_controls(&rows, 128);
    assert_eq!(n, 512);
    assert!(v.len() < 128);
    assert!(serde_json::to_vec(&v).unwrap().len() < 13 * 1024);
    assert_eq!(v[0]["display_truncated"], true);
    assert_eq!(v[0]["id"], "win");
}

pub(super) fn native_plain_observation_fixture(status: i32, settable: Value) -> Value {
    json!({"frame":"plain-frame","scene":{"assurance":"native-attested",
        "binding":{"scope":{"pid":101,"window":"cgwindow:11"}},
        "elements":[{"id":"plain","name":"Window A text","role":"AXTextField",
            "actions":[],"point":[11.5,20.25],"enabled":false,"visible":null,"occluded":null}]},
        "adapter_status":{"adapter":"rust-macos-native","observation_consistency":{
            "ax_coverage_after":{"editable_focus_availability":{
                "api":"AXUIElementIsAttributeSettable","attribute":"AXFocused",
                "scope":"returned_classified_plain_text_roles_only","probe_limit":4,
                "attempted":1,"budget_omitted":0,"absence_proven":false,"input_authority":false,
                "missing_entry":"unknown_or_not_probed; never false",
                "elements":[{"element":"plain","state":if status==0{"read"}else{"unknown"},
                    "native_status":status,"settable":settable}]}}}}})
}

#[test]
fn native_plain_display_candidate_without_actions_is_visible_in_both_projections() {
    let observation = native_plain_observation_fixture(0, json!(true));
    let original = observation.clone();
    let rows = observation["scene"]["elements"].as_array().unwrap();
    let candidates = native_plain_display_candidates(&observation);
    let (snapshot, total) = compact_controls_with_candidates(rows, 64, &candidates);
    let args = ControlsArgs {
        observation_ref: "original-ref".into(),
        frame: "plain-frame".into(),
        query: "Window A text".into(),
        offset: 0,
        limit: 64,
    };
    let controls = control_page_with_candidates(rows, &args, &candidates);
    assert_eq!(total, 1);
    assert_eq!(
        snapshot,
        controls.data["elements"].as_array().unwrap().clone()
    );
    assert_eq!(snapshot[0]["id"], "plain");
    assert_eq!(snapshot[0]["point"], json!([11.5, 20.25]));
    assert_eq!(snapshot[0]["enabled"], false);
    assert_eq!(snapshot[0]["actions"], json!([]));
    assert_eq!(snapshot[0]["display_candidate"]["input_authority"], false);
    assert!(snapshot[0].get("keyboard_focus").is_none());
    assert_eq!(controls.data["frame_refreshed"], false);
    assert_eq!(controls.data["input_sent"], false);
    assert_eq!(observation, original);
}

#[test]
fn native_plain_display_candidate_false_and_error_never_grant_input() {
    for (status, settable) in [(0, json!(false)), (-25204, Value::Null)] {
        let observation = native_plain_observation_fixture(status, settable.clone());
        let before = observation.clone();
        let candidates = native_plain_display_candidates(&observation);
        assert_eq!(candidates.iter().collect::<Vec<_>>(), vec!["plain"]);
        let (rows, _) = compact_controls_with_candidates(
            observation["scene"]["elements"].as_array().unwrap(),
            64,
            &candidates,
        );
        assert_eq!(rows[0]["display_candidate"]["input_authority"], false);
        for absent in [
            "available",
            "mutation_ready",
            "keyboard_focus",
            "goal_verified",
        ] {
            assert!(rows[0].get(absent).is_none());
        }
        let sidecar = &observation["adapter_status"]["observation_consistency"]["ax_coverage_after"]
            ["editable_focus_availability"];
        assert_eq!(sidecar["elements"][0]["native_status"], status);
        assert_eq!(sidecar["elements"][0]["settable"], settable);
        assert_eq!(observation, before);
    }
}

#[test]
fn known_empty_ax_value_compact_snapshot_and_control_pages_keep_empty() {
    let rows = vec![
        json!({"id":"empty","name":"Plain empty","role":"AXTextField","value":"","actions":["AXConfirm"],"point":[1,2]}),
        json!({"id":"unknown","name":"Plain unknown","role":"AXTextField","actions":["AXConfirm"],"point":[3,4]}),
        json!({"id":"null","name":"Future null","role":"AXTextField","value":null,"actions":["AXConfirm"],"point":[5,6]}),
    ];
    let original = rows.clone();
    let (snapshot, total) = compact_controls(&rows, 64);
    assert_eq!(total, 3);
    assert_eq!(snapshot[0].get("value").and_then(Value::as_str), Some(""));
    assert!(snapshot[1].get("value").is_none());
    assert!(snapshot[2].get("value").is_none());
    let args = ControlsArgs {
        observation_ref: "original".into(),
        frame: "frame".into(),
        query: String::new(),
        offset: 0,
        limit: 64,
    };
    let page = control_page(&rows, &args);
    assert_eq!(page.data["elements"], json!(snapshot));
    assert_eq!(page.data["input_sent"], false);
    assert_eq!(page.data["frame_refreshed"], false);
    assert_eq!(rows, original);
}

#[test]
fn known_empty_ax_value_same_snapshot_detail_pointer_is_one_read_not_empty_inference() {
    let snapshot = json!({"frame":"same-frame","elements":[
        {"id":"empty","name":"Plain empty","value":""},
        {"id":"unknown","name":"Plain unknown"},
        {"id":"null","name":"Future null","value":null}
    ]});
    let mut cache = presentation::Presentation::default();
    let details_ref = cache.retain("computer.snapshot", snapshot.clone()).unwrap();
    let args = |pointer: &str| presentation::ResultArgs {
        result_ref: details_ref.clone(),
        pointer: pointer.into(),
        offset: 0,
        limit: 16,
        byte_offset: None,
    };
    let empty = cache.read(args("/elements/0"));
    assert!(!empty.is_error);
    assert_eq!(empty.data["value"], snapshot["elements"][0]);
    assert_eq!(
        empty.data["value"].get("value").and_then(Value::as_str),
        Some("")
    );
    assert_eq!(empty.data["native_dispatched"], false);
    assert_eq!(cache.read(args("/elements/0/value")).data["value"], "");
    assert_eq!(
        cache.read(args("/elements/1/value")).data["code"],
        "result_pointer_unknown"
    );
    let null = cache.read(args("/elements/2/value"));
    assert!(!null.is_error);
    assert!(null.data["value"].is_null());
    assert_eq!(snapshot["frame"], "same-frame");
}
