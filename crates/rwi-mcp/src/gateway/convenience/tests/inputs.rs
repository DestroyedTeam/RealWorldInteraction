use super::*;

#[tokio::test]
async fn reveal_requires_exact_advertised_action_and_dispatches_once_under_lease() {
    let args: RevealArgs = serde_json::from_value(json!({
        "target":"t","frame":"frame-1","element":"element-1","request_id":"reveal-once"
    }))
    .unwrap();
    let calls = RefCell::new(vec![]);
    let result = reveal(args, |tool, args, cleanup| {
        calls.borrow_mut().push((tool, args, cleanup));
        std::future::ready(match tool {
            "computer.targets" => DeviceResult::value(json!({"targets":[{
                "target":"t","capabilities":{"native_semantic_actions":["AXPress","AXScrollToVisible"]}
            }]})),
            "computer.claim" => DeviceResult::value(json!({"lease":"short-lease"})),
            "computer.act_checked" => DeviceResult::value(json!({
                "request_id":"reveal-once","dispatch":"returned","goal_verified":false
            })),
            "computer.release" => DeviceResult::value(json!({"released":true})),
            _ => panic!("unexpected tool {tool}"),
        })
    })
    .await;
    assert_eq!(result.data["operation"], "reveal_to_visible");
    assert_eq!(result.data["native_action"], "AXScrollToVisible");
    let calls = calls.borrow();
    assert_eq!(calls[0].0, "computer.targets");
    assert_eq!(calls[1].0, "computer.claim");
    assert_eq!(calls[2].0, "computer.act_checked");
    assert_eq!(calls[2].1["intent"], "click");
    assert_eq!(calls[2].1["value"], "AXScrollToVisible");
    assert_eq!(calls[2].1["request_id"], "reveal-once");
    assert_eq!(calls[3].0, "computer.release");
    assert!(calls[3].2);
}

#[tokio::test]
async fn reveal_without_native_capability_never_claims_or_dispatches() {
    let args: RevealArgs = serde_json::from_value(json!({
        "target":"t","frame":"frame-1","element":"element-1","request_id":"reveal-once"
    }))
    .unwrap();
    let calls = RefCell::new(vec![]);
    let result = reveal(args, |tool, args, cleanup| {
        calls.borrow_mut().push((tool, args, cleanup));
        std::future::ready(DeviceResult::value(json!({"targets":[{
            "target":"t","capabilities":{"native_semantic_actions":["AXPress"]}
        }]})))
    })
    .await;
    assert!(result.is_error);
    assert_eq!(result.data["code"], "capability_unavailable");
    assert_eq!(calls.borrow().len(), 1);
    assert_eq!(calls.borrow()[0].0, "computer.targets");
}

#[tokio::test]
async fn scroll_is_one_checked_intent_and_releases_without_replaying_a_refusal() {
    for at in [
        json!({"kind":"element","frame":"f","element":"e"}),
        json!({"kind":"view","view_id":"v","x":4,"y":5}),
    ] {
        let args: ScrollArgs = serde_json::from_value(
            json!({"target":"t","request_id":"once","at":at,"delta_y":-600}),
        )
        .unwrap();
        let calls = RefCell::new(vec![]);
        let result = scroll(args, |tool, args, cleanup| {
            calls.borrow_mut().push((tool, args, cleanup));
            std::future::ready(match tool {
                "computer.resolve_view" => DeviceResult::value(json!({"reference":"exact-ref"})),
                "computer.claim" => DeviceResult::value(json!({"lease":"short-lease"})),
                "computer.release" => DeviceResult::value(json!({"released":true})),
                _ => DeviceResult::error("stale_reference", "No input sent"),
            })
        })
        .await;
        assert_eq!(result.data["code"], "stale_reference");
        let calls = calls.borrow();
        let actions: Vec<_> = calls
            .iter()
            .filter(|c| matches!(c.0, "computer.act_checked" | "computer.act"))
            .collect();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].1["intent"], "scroll");
        assert_eq!(actions[0].1["value"], "-600");
        assert_eq!(actions[0].1["request_id"], "once");
        assert_eq!(calls.last().unwrap().0, "computer.release");
        assert!(calls.last().unwrap().2);
    }
}

#[tokio::test]
async fn scroll_rejects_invalid_values_before_native_calls_and_never_claims_on_bad_view() {
    for delta in [0, 2001, -2001, i32::MIN] {
        let args: ScrollArgs = serde_json::from_value(json!({"target":"t","request_id":"once","at":{"kind":"element","frame":"f","element":"e"},"delta_y":delta})).unwrap();
        let result = scroll(args, |_, _, _| {
            std::future::ready({
                panic!("invalid scroll must not call native tools");
                #[allow(unreachable_code)]
                DeviceResult::value(json!({}))
            })
        })
        .await;
        assert_eq!(result.data["code"], "invalid_arguments");
    }
    let args: ScrollArgs = serde_json::from_value(json!({"target":"t","request_id":"once","at":{"kind":"view","view_id":"v","x":4,"y":5},"delta_y":120})).unwrap();
    let result = scroll(args, |tool, _, _| {
        assert_eq!(tool, "computer.resolve_view");
        std::future::ready(DeviceResult::value(json!({})))
    })
    .await;
    assert_eq!(result.data["code"], "invalid_reference_reply");
    assert!(serde_json::from_value::<ScrollArgs>(json!({"target":"t","request_id":"once","at":{"kind":"element","frame":"f","element":"e","action":"AXPress"},"delta_y":120})).is_err());
}

#[tokio::test]
async fn placement_refreshes_locally_dispatches_once_and_releases_even_on_uncertainty() {
    let args:PlaceWindowArgs=serde_json::from_value(json!({"target":"t","request_id":"one","display":{"uuid":"00000000-0000-0000-0000-000000000007"}})).unwrap();
    let calls = RefCell::new(vec![]);
    let result = place_window(args, |tool, args, cleanup| {
        calls.borrow_mut().push((tool, args, cleanup));
        std::future::ready(match tool {
            "computer.observe" => DeviceResult::value(json!({"frame":"f"})),
            "computer.resolve" => DeviceResult::value(json!({"reference":"r"})),
            "computer.claim" => DeviceResult::value(json!({"lease":"l"})),
            "computer.release" => DeviceResult::value(json!({"released":true})),
            _ => DeviceResult::error("placement_rejected", "inspect before a new request"),
        })
    })
    .await;
    assert_eq!(result.data["code"], "placement_rejected");
    let calls = calls.borrow();
    assert_eq!(
        calls.iter().map(|c| c.0).collect::<Vec<_>>(),
        vec![
            "computer.observe",
            "computer.resolve",
            "computer.claim",
            "computer.move_window",
            "computer.release"
        ]
    );
    assert_eq!(calls[3].1["request_id"], "one");
    assert_eq!(calls[3].1["lease"], "l");
    assert_eq!(
        calls[3].1["display"]["uuid"],
        "00000000-0000-0000-0000-000000000007"
    );
    assert!(calls[4].2);
}
