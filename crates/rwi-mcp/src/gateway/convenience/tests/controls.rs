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
