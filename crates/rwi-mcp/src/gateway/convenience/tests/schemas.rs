use super::*;

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
