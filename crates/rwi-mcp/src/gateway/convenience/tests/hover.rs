use super::*;

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
