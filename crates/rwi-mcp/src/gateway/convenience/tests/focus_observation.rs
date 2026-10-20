use super::*;

fn stamp(window: &str) -> Value {
    json!({"pid":42,"window":window,"process_start":"42:boot:1","native_epoch":"epoch"})
}

fn targets(window: &str, time: u64) -> Value {
    let context = json!({"host":"fixture","session":"s","desktop":"d","seat":"s"});
    json!({"devices":[{"device_id":"mac-host","is_error":false,"state":{
        "instance":"agent","context_valid":true,"native_context":context,
        "live_desktop_facts":{"identity":context,"captured_unix_ms":time,
            "values":{"current_foreground":stamp(window)}},
        "platform":"macos","executor_features":{"click_delivery_options":{"implemented":true}},
        "capabilities":[{"id":"computer.act_checked","implemented":true,"authorized":true}]
    }}],"targets":[{"target":"t","identity":{"binding":"exact"},"capabilities":{
        "window_selected":true,"current_foreground":stamp(window),
        "pointer_click":{"background_public_appkit_experimental":{"implemented":true,"available":true}}
    }}]})
}

fn action() -> DeviceResult {
    DeviceResult::value(
        json!({"dispatch":"returned","request_id":"once","operation_ref":"op",
        "scope":{"pid":42,"window":"cgwindow:2"},"goal_verified":false}),
    )
}

async fn run(
    explicit: bool,
    before: Value,
    after: DeviceResult,
    action: DeviceResult,
) -> (DeviceResult, Vec<(&'static str, Value, bool)>) {
    let mut args = json!({"target":"t","request_id":"once",
        "at":{"kind":"element","frame":"f","element":"e"}});
    if explicit {
        args["delivery"] = json!({"delivery":"background_public_appkit_experimental"});
    }
    let calls = RefCell::new(Vec::new());
    let reads = RefCell::new(0);
    let result = click(
        serde_json::from_value(args).unwrap(),
        Some("mac-host".into()),
        |tool, args, cleanup| {
            calls.borrow_mut().push((tool, args, cleanup));
            let result = match tool {
                "computer.targets" => {
                    *reads.borrow_mut() += 1;
                    if *reads.borrow() == 1 {
                        DeviceResult::value(before.clone())
                    } else {
                        DeviceResult {
                            is_error: after.is_error,
                            data: after.data.clone(),
                            images: vec![],
                        }
                    }
                }
                "computer.claim" => DeviceResult::value(json!({"lease":"lease"})),
                "computer.release" => DeviceResult::value(json!({"released":true})),
                "computer.act_checked" => DeviceResult {
                    is_error: action.is_error,
                    data: action.data.clone(),
                    images: vec![],
                },
                _ => panic!("unexpected tool {tool}"),
            };
            std::future::ready(result)
        },
    )
    .await;
    (result, calls.into_inner())
}

#[tokio::test]
async fn f256_one_post_read_after_release_reports_different_samples_without_restore() {
    let (result, calls) = run(
        true,
        targets("cgwindow:1", 10),
        DeviceResult::value(targets("cgwindow:2", 20)),
        action(),
    )
    .await;
    assert!(!result.is_error);
    assert_eq!(
        result.data["focus_observation"]["state"],
        "different_at_samples"
    );
    assert_eq!(
        result.data["focus_observation"]["before"],
        stamp("cgwindow:1")
    );
    assert_eq!(
        result.data["focus_observation"]["after"],
        stamp("cgwindow:2")
    );
    assert_eq!(result.data["operation_ref"], "op");
    assert_eq!(result.data["goal_verified"], false);
    assert_eq!(
        calls.iter().map(|c| c.0).collect::<Vec<_>>(),
        [
            "computer.targets",
            "computer.claim",
            "computer.act_checked",
            "computer.release",
            "computer.targets"
        ]
    );
    assert!(calls[3].2);
    assert_eq!(calls[4].1, json!({"device_id":"mac-host"}));
}

#[tokio::test]
async fn f256_same_samples_are_not_continuous_preservation_or_verified_goal() {
    let (result, _) = run(
        true,
        targets("cgwindow:1", 10),
        DeviceResult::value(targets("cgwindow:1", 20)),
        action(),
    )
    .await;
    let full = &result.data["focus_observation"];
    assert_eq!(full["state"], "same_at_samples");
    assert_eq!(full["coverage"], "two_samples_not_continuous_or_causal");
    let projected = compact_action_reply(&result.data, Some("details".into()));
    assert_eq!(
        projected["focus_observation"]["before_window"],
        "cgwindow:1"
    );
    assert!(projected["focus_observation"].get("before").is_none());
    assert_eq!(projected["details_ref"], "details");
    assert_eq!(projected["goal_verified"], false);
    assert_eq!(
        compact_action_reply(&json!({"focus_observation":null}), None)["focus_observation"],
        Value::Null
    );
}

#[tokio::test]
async fn f256_failed_or_stale_post_read_is_unknown_not_action_failure_or_replay() {
    let mut restarted = targets("cgwindow:2", 20);
    restarted["devices"][0]["state"]["instance"] = json!("new-agent");
    let mut rebound = targets("cgwindow:2", 20);
    rebound["targets"][0]["identity"] = json!({"binding":"new"});
    for (after, reason) in [
        (
            DeviceResult::error("offline", "read failed"),
            "focus_read_failed",
        ),
        (
            DeviceResult::value(targets("cgwindow:2", 10)),
            "focus_sample_not_fresh",
        ),
        (DeviceResult::value(restarted), "focus_context_changed"),
        (DeviceResult::value(rebound), "focus_context_changed"),
        (DeviceResult::value(json!({})), "focus_sample_unavailable"),
    ] {
        let (result, calls) = run(true, targets("cgwindow:1", 10), after, action()).await;
        assert!(!result.is_error);
        assert_eq!(result.data["dispatch"], "returned");
        assert_eq!(result.data["focus_observation"]["state"], "unavailable");
        assert_eq!(result.data["focus_observation"]["reason"], reason);
        assert!(result.data["focus_observation"]["after"].is_null());
        assert_eq!(calls.len(), 5);
    }
}

#[tokio::test]
async fn f256_default_cross_pid_and_same_window_have_no_extra_rpc() {
    for (explicit, pid, window) in [
        (false, 42, "cgwindow:2"),
        (true, 43, "cgwindow:2"),
        (true, 42, "cgwindow:1"),
    ] {
        let mut a = action();
        a.data["scope"]["pid"] = json!(pid);
        a.data["scope"]["window"] = json!(window);
        let (result, calls) = run(
            explicit,
            targets("cgwindow:1", 10),
            DeviceResult::value(json!({})),
            a,
        )
        .await;
        assert!(result.data.get("focus_observation").is_none());
        assert_eq!(calls.len(), if explicit { 4 } else { 3 });
    }
}

#[tokio::test]
async fn f256_incomplete_or_mixed_preflight_does_not_invent_a_sample() {
    for (path, value) in [
        ("/devices/0/state/context_valid", json!(false)),
        (
            "/devices/0/state/live_desktop_facts/captured_unix_ms",
            json!(0),
        ),
        ("/devices/0/state/live_desktop_facts/identity", json!({})),
        (
            "/devices/0/state/live_desktop_facts/values/current_foreground/process_start",
            Value::Null,
        ),
        ("/targets/0/identity", json!({})),
        (
            "/targets/0/capabilities/current_foreground/window",
            json!("cgwindow:3"),
        ),
    ] {
        let mut before = targets("cgwindow:1", 10);
        *before.pointer_mut(path).unwrap() = value;
        let (result, calls) = run(true, before, DeviceResult::value(json!({})), action()).await;
        assert!(result.data.get("focus_observation").is_none(), "{path}");
        assert_eq!(calls.len(), 4, "{path}");
    }
}

#[tokio::test]
async fn f256_native_error_fence_and_uncertainty_preserve_original_without_post_read() {
    let mut cases = vec![DeviceResult::error("uncertain", "native failure")];
    for (key, value) in [
        ("input_domain_fenced", json!(true)),
        ("resources_released", json!(false)),
        ("deduplicated", json!(true)),
        ("dispatch_error", json!({"code":"unknown"})),
        (
            "operation",
            json!({"terminal":false,"resources_released":false}),
        ),
        ("dispatch", json!("uncertain")),
    ] {
        let mut a = action();
        a.data[key] = value;
        cases.push(a);
    }
    for a in cases {
        let original = a.data.clone();
        let error = a.is_error;
        let (result, calls) = run(
            true,
            targets("cgwindow:1", 10),
            DeviceResult::value(json!({})),
            a,
        )
        .await;
        assert_eq!(result.is_error, error);
        for (key, value) in original.as_object().unwrap() {
            assert_eq!(&result.data[key], value);
        }
        assert!(result.data.get("focus_observation").is_none());
        assert_eq!(calls.len(), 4);
    }
}
