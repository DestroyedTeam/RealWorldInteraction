use super::*;

#[test]
fn visible_desktop_hover_schema_discloses_physical_cursor_and_explicit_identity() {
    let entry = tools()
        .into_iter()
        .find(|t| t.name == "computer.hover")
        .unwrap();
    let schema = serde_json::to_value(entry.input_schema)
        .unwrap()
        .to_string();
    assert!(schema.contains("visible_desktop_system") && schema.contains("expected_foreground"));
    assert!(
        entry
            .description
            .unwrap()
            .contains("physical system cursor")
    );
}

#[tokio::test]
async fn visible_desktop_hover_forwards_once_and_preserves_refusal_without_fallback() {
    let option = json!({"delivery":"visible_desktop_system","expected_foreground":{"pid":42,"window":"cgwindow:7","process_start":"42:1:2","native_epoch":"a".repeat(64)}});
    let args:HoverArgs=serde_json::from_value(json!({"target":"t","request_id":"visible-desktop-once","at":{"kind":"element","frame":"f","element":"e"},"options":option})).unwrap();
    let calls = RefCell::new(vec![]);
    let result = hover(args, |tool, args, cleanup| {
        calls.borrow_mut().push((tool, args, cleanup));
        std::future::ready(match tool {
            "computer.claim" => DeviceResult::value(json!({"lease":"short"})),
            "computer.release" => DeviceResult::value(json!({"released":true})),
            _ => DeviceResult::error("visible_desktop_hit_unproven", "no physical hit; no input"),
        })
    })
    .await;
    assert!(result.is_error);
    let calls = calls.borrow();
    let actions: Vec<_> = calls
        .iter()
        .filter(|c| c.0 == "computer.act_checked")
        .collect();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].1["intent"], "move_pointer");
    assert_eq!(
        serde_json::from_str::<Value>(actions[0].1["value"].as_str().unwrap()).unwrap(),
        option
    );
    assert_eq!(
        calls.iter().filter(|c| c.0 == "computer.release").count(),
        1
    );
    assert!(calls.iter().all(|c| !matches!(
        c.0,
        "computer.focus_window" | "computer.hotkey" | "computer.act"
    )));
}

#[test]
fn hover_contract_typed_tool_is_explicit_nonsemantic_and_bounded() {
    let catalog = tools();
    let hover = catalog
        .iter()
        .find(|tool| tool.name == "computer.hover")
        .expect("typed hover missing");
    assert!(handles("computer.hover"));
    let schema = serde_json::to_value(&hover.input_schema)
        .unwrap()
        .to_string();
    assert!(schema.contains("include_timings"));
    assert!(!schema.contains("AXPress"));
    assert!(
        hover
            .description
            .as_ref()
            .unwrap()
            .contains("not tooltip success")
    );
}

#[tokio::test]
async fn hover_contract_is_one_pointer_intent_without_button_or_semantic_value() {
    for at in [
        json!({"kind":"element","frame":"f","element":"e"}),
        json!({"kind":"view","view_id":"v","x":4,"y":5}),
    ] {
        let args: HoverArgs =
            serde_json::from_value(json!({"target":"t","request_id":"hover-once","at":at}))
                .unwrap();
        let calls = RefCell::new(vec![]);
        let result = hover(args, |tool, args, cleanup| {
            calls.borrow_mut().push((tool, args, cleanup));
            std::future::ready(match tool {
                "computer.resolve_view" => DeviceResult::value(json!({"reference":"exact-ref"})),
                "computer.claim" => DeviceResult::value(json!({"lease":"short-lease"})),
                "computer.release" => DeviceResult::value(json!({"released":true})),
                _ => DeviceResult::value(json!({"dispatch":"acknowledged","goal_verified":false})),
            })
        })
        .await;
        assert!(!result.is_error);
        assert_eq!(result.data["hover_effect"], "unverified");
        assert_eq!(result.data["motion_history"], "unknown");
        let compact = compact_action_reply(&result.data, None);
        assert_eq!(compact["hover_effect"], "unverified");
        assert_eq!(compact["motion_history"], "unknown");
        assert_eq!(compact["goal_verified"], false);
        let calls = calls.borrow();
        let actions: Vec<_> = calls
            .iter()
            .filter(|c| matches!(c.0, "computer.act" | "computer.act_checked"))
            .collect();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].1["intent"], "move_pointer");
        assert_eq!(actions[0].1["request_id"], "hover-once");
        assert!(actions[0].1.get("value").is_none());
        assert!(actions[0].1.get("action").is_none());
        assert_eq!(calls.last().unwrap().0, "computer.release");
        assert!(calls.last().unwrap().2);
        if let Some(resolve) = calls.iter().find(|c| c.0 == "computer.resolve_view") {
            assert!(resolve.1.get("red_marker_cluster").is_none());
        }
    }
}

#[tokio::test]
async fn hover_contract_preserves_explicit_marker_and_never_claims_on_resolve_refusal() {
    let args: HoverArgs = serde_json::from_value(json!({"target":"t","request_id":"hover-once","at":{"kind":"view","view_id":"v","x":4,"y":5,"red_marker_cluster":true}})).unwrap();
    let calls = RefCell::new(vec![]);
    let result = hover(args, |tool, args, cleanup| {
        calls.borrow_mut().push((tool, args, cleanup));
        std::future::ready(DeviceResult::error("stale_reference", "No input issued"))
    })
    .await;
    assert_eq!(result.data["code"], "stale_reference");
    let calls = calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "computer.resolve_view");
    assert_eq!(calls[0].1["red_marker_cluster"], true);
}

#[tokio::test]
async fn hover_contract_releases_on_native_refusal_without_replay() {
    let args: HoverArgs = serde_json::from_value(json!({"target":"t","request_id":"hover-once","at":{"kind":"element","frame":"f","element":"e"}})).unwrap();
    let calls = RefCell::new(vec![]);
    let result = hover(args, |tool, args, cleanup| {
        calls.borrow_mut().push((tool, args, cleanup));
        std::future::ready(match tool {
            "computer.claim" => DeviceResult::value(json!({"lease":"short-lease"})),
            "computer.release" => DeviceResult::value(json!({"released":true})),
            _ => DeviceResult::error("capability_unavailable", "No pointer move backend"),
        })
    })
    .await;
    assert_eq!(result.data["code"], "capability_unavailable");
    let calls = calls.borrow();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].0, "computer.claim");
    assert_eq!(calls[1].0, "computer.act_checked");
    assert_eq!(calls[2].0, "computer.release");
    assert!(calls[2].2);
}

#[tokio::test]
async fn hover_contract_invalid_identity_or_location_never_calls_device() {
    for (target, request, at) in [
        ("", "r", json!({"kind":"element","frame":"f","element":"e"})),
        ("t", "", json!({"kind":"element","frame":"f","element":"e"})),
        ("t", "r", json!({"kind":"element","frame":"","element":"e"})),
        ("t", "r", json!({"kind":"view","view_id":"","x":4,"y":5})),
    ] {
        let args: HoverArgs =
            serde_json::from_value(json!({"target":target,"request_id":request,"at":at})).unwrap();
        let result = hover(args, |_, _, _| {
            std::future::ready({
                panic!("invalid hover must not call device");
                #[allow(unreachable_code)]
                DeviceResult::value(json!({}))
            })
        })
        .await;
        assert_eq!(result.data["code"], "invalid_arguments");
    }
    for at in [
        json!({"kind":"element","frame":"f","element":"e","action":"AXPress"}),
        json!({"kind":"view","view_id":"v","x":4,"y":5,"button":"left"}),
    ] {
        assert!(
            serde_json::from_value::<HoverArgs>(json!({"target":"t","request_id":"r","at":at}))
                .is_err()
        );
    }
    for extra in ["value", "dwell_ms", "click_count", "activate"] {
        let mut args = json!({"target":"t","request_id":"r","at":{"kind":"element","frame":"f","element":"e"}});
        args[extra] = json!(1);
        assert!(serde_json::from_value::<HoverArgs>(args).is_err());
    }
}

#[test]
fn f05_registered_tracking_mcp_schema_and_string_boundary() {
    let schema = serde_json::to_value(schemars::schema_for!(HoverArgs))
        .unwrap()
        .to_string();
    assert!(schema.contains("background_appkit_tracking_experimental"));
    assert!(schema.contains("tracking_number"));
    for phase in ["entered", "exited"] {
        let args: HoverArgs = serde_json::from_value(json!({"target":"t","request_id":"new-registration-carrier",
            "at":{"kind":"element","frame":"f","element":"e"},
            "options":{"delivery":"background_appkit_tracking_experimental","phase":phase,"tracking_number":"9007199254740993"}})).unwrap();
        let options = serde_json::to_string(&args.options.unwrap()).unwrap();
        assert!(rwi_core::MovePointerOptions::parse(Some(&options)).is_ok());
        assert!(options.contains("9007199254740993"));
    }
    for tag in [json!(73), json!(73.0), serde_json::Value::Null] {
        assert!(serde_json::from_value::<HoverArgs>(json!({"target":"t","request_id":"never-dispatch",
            "at":{"kind":"element","frame":"f","element":"e"},
            "options":{"delivery":"background_appkit_tracking_experimental","phase":"entered","tracking_number":tag}})).is_err());
    }
}

#[test]
fn f05_cooperative_tracking_mcp_wire_never_deserializes_authority_or_tag() {
    let options = json!({"delivery":"background_cooperative_tracking_experimental","phase":"exited",
        "owner_endpoint":"/tmp/own-private/owner.sock","registration_id":"owned-1"});
    let arg = json!({"target":"t","request_id":"coop-once","at":{"kind":"element","frame":"f","element":"e"},"options":options});
    let parsed: HoverArgs = serde_json::from_value(arg.clone()).unwrap();
    assert!(
        rwi_core::MovePointerOptions::parse(Some(
            &serde_json::to_string(&parsed.options.unwrap()).unwrap()
        ))
        .is_ok()
    );
    for field in [
        "tracking_number",
        "userData",
        "trusted_witness",
        "expected_peer_pid",
    ] {
        let mut bad = arg.clone();
        bad["options"][field] = json!("foreign");
        assert!(serde_json::from_value::<HoverArgs>(bad).is_err());
    }
}
