use super::*;
use std::{collections::VecDeque, future::ready};

type Call = (&'static str, Value, bool);
fn args() -> BringToFrontArgs {
    BringToFrontArgs {
        target: "selected-window".into(),
        request_id: "f19-once".into(),
    }
}
fn foreground() -> Value {
    json!({"pid":7,"window":"opaque-window","process_start":"opaque-birth","native_epoch":"a".repeat(64)})
}
fn observed() -> DeviceResult {
    DeviceResult::value(
        json!({"frame":"fresh-frame","adapter_status":{"current_foreground":foreground()}}),
    )
}
fn reference() -> DeviceResult {
    DeviceResult::value(json!({"reference":"exact-window-ref"}))
}
fn claim() -> DeviceResult {
    DeviceResult::value(json!({"lease":"own-lease"}))
}
fn returned() -> DeviceResult {
    DeviceResult::value(
        json!({"activation":{"state":"already_foreground","goal_verified":false},"receipt":{"request_id":"f19-once","dispatch":"returned","replay":"forbidden"},"input_domain_fenced":false}),
    )
}
fn released() -> DeviceResult {
    DeviceResult::value(json!({"released":true}))
}
async fn run(a: BringToFrontArgs, replies: Vec<DeviceResult>) -> (DeviceResult, Vec<Call>) {
    let mut replies: VecDeque<_> = replies.into();
    let mut calls = Vec::new();
    let result = bring_to_front(a, |name, arguments, cleanup| {
        calls.push((name, arguments, cleanup));
        ready(replies.pop_front().expect("no extra call or replay"))
    })
    .await;
    assert!(
        replies.is_empty(),
        "all planned calls must be accounted for"
    );
    (result, calls)
}

#[tokio::test]
async fn f19_bring_to_front_fresh_exact_foreground_one_request_and_cleanup() {
    let expected = returned().data;
    let (r, calls) = run(
        args(),
        vec![observed(), reference(), claim(), returned(), released()],
    )
    .await;
    assert!(!r.is_error);
    assert_eq!(r.data, expected, "do not upgrade a receipt to a goal");
    assert_eq!(
        calls.iter().map(|c| c.0).collect::<Vec<_>>(),
        [
            "computer.observe",
            "computer.resolve",
            "computer.claim",
            "computer.focus_window",
            "computer.release"
        ]
    );
    assert_eq!(
        calls[0].1,
        json!({"target":"selected-window","inline_image":false})
    );
    assert_eq!(
        calls[1].1,
        json!({"target":"selected-window","frame":"fresh-frame","locator":{"kind":"window"}})
    );
    assert_eq!(
        calls[3].1,
        json!({"target":"selected-window","request_id":"f19-once","lease":"own-lease","reference":"exact-window-ref","expected_foreground":foreground()})
    );
    assert_eq!(calls[4].1, json!({"lease":"own-lease"}));
    assert_eq!(
        calls.iter().map(|c| c.2).collect::<Vec<_>>(),
        [false, false, false, false, true]
    );
}

#[tokio::test]
async fn f19_bring_to_front_invalid_input_never_observes_or_claims() {
    for a in [
        BringToFrontArgs {
            target: String::new(),
            ..args()
        },
        BringToFrontArgs {
            request_id: "bad id".into(),
            ..args()
        },
        BringToFrontArgs {
            request_id: String::new(),
            ..args()
        },
    ] {
        let (r, calls) = run(a, vec![]).await;
        assert_eq!(r.data["code"], "invalid_arguments");
        assert!(calls.is_empty());
    }
}

#[tokio::test]
async fn f19_bring_to_front_missing_frame_or_tuple_is_predispatch() {
    for data in [
        json!({}),
        json!({"frame":""}),
        json!({"frame":"f"}),
        json!({"frame":"f","adapter_status":{"current_foreground":null}}),
        json!({"frame":"f","adapter_status":{"current_foreground":{"pid":7}}}),
    ] {
        let (r, calls) = run(args(), vec![DeviceResult::value(data)]).await;
        assert!(r.is_error);
        assert_eq!(calls.len(), 1);
    }
    for (key, value) in [
        ("pid", json!(0)),
        ("native_epoch", json!("unknown")),
        ("process_start", json!("")),
        ("window", json!("")),
        ("extra", json!(true)),
    ] {
        let mut o = observed();
        o.data["adapter_status"]["current_foreground"][key] = value;
        let (r, calls) = run(args(), vec![o]).await;
        assert_eq!(r.data["code"], "foreground_identity_unavailable");
        assert_eq!(calls.len(), 1);
    }
}

#[tokio::test]
async fn f19_bring_to_front_preserves_early_errors_without_fallback_or_foreign_release() {
    for stage in 0..3 {
        let error = DeviceResult::error("original_refusal", "original bounded diagnostic");
        let expected = error.data.clone();
        let mut prefix = vec![];
        if stage > 0 {
            prefix.push(observed());
        }
        if stage > 1 {
            prefix.push(reference());
        }
        prefix.push(error);
        let (r, calls) = run(args(), prefix).await;
        assert!(r.is_error);
        assert_eq!(r.data, expected);
        assert_eq!(calls.len(), stage + 1);
        assert!(
            !calls
                .iter()
                .any(|c| c.0 == "computer.focus_window" || c.0 == "computer.release")
        );
    }
}

#[tokio::test]
async fn f19_bring_to_front_invalid_reference_and_unusable_lease_never_activate() {
    for value in [json!({}), json!({"reference":""})] {
        let (r, calls) = run(args(), vec![observed(), DeviceResult::value(value)]).await;
        assert_eq!(r.data["code"], "invalid_reference_reply");
        assert_eq!(calls.len(), 2);
    }
    let (r, calls) = run(
        args(),
        vec![
            observed(),
            reference(),
            DeviceResult::value(json!({"ttl_ms":10000})),
        ],
    )
    .await;
    assert_eq!(r.data["code"], "invalid_lease_reply");
    assert_eq!(calls.len(), 3);
}

#[tokio::test]
async fn f19_bring_to_front_refusal_uncertainty_and_native_unsupported_release_once() {
    for action in [
        DeviceResult::error("capability_unavailable", "unsupported device"),
        DeviceResult::error("foreground_changed", "native guard refused"),
        DeviceResult::value(
            json!({"dispatch":"uncertain","input_domain_fenced":true,"replay_allowed":false,"operation_ref":{"execution_nonce":"original"}}),
        ),
    ] {
        let error = action.is_error;
        let expected = action.data.clone();
        let (r, calls) = run(
            args(),
            vec![observed(), reference(), claim(), action, released()],
        )
        .await;
        assert_eq!(r.is_error, error);
        assert_eq!(r.data, expected);
        assert_eq!(calls.len(), 5);
        assert_eq!(
            calls
                .iter()
                .filter(|c| c.0 == "computer.focus_window")
                .count(),
            1
        );
        assert_eq!(calls.last().unwrap().0, "computer.release");
        assert!(calls.last().unwrap().2);
    }
}

#[tokio::test]
async fn f19_bring_to_front_lost_cleanup_retains_original_action_and_no_replay() {
    let action = returned();
    let expected = action.data.clone();
    let (r, calls) = run(
        args(),
        vec![
            observed(),
            reference(),
            claim(),
            action,
            DeviceResult::error("transport_error", "release unknown"),
        ],
    )
    .await;
    assert!(r.is_error);
    assert_eq!(r.data["code"], "input_cleanup_unconfirmed");
    assert_eq!(r.data["action"], expected);
    assert_eq!(r.data["cleanup"]["code"], "transport_error");
    assert_eq!(r.data["replay_allowed"], false);
    assert_eq!(calls.len(), 5);
}

#[test]
fn f19_bring_to_front_strict_two_field_model_schema_and_existing_raw_tool_unchanged() {
    let t = tools()
        .into_iter()
        .find(|t| t.name == "computer.bring_to_front")
        .unwrap();
    assert!(handles("computer.bring_to_front"));
    assert!(!handles("computer.focus_window"));
    assert_eq!(
        t.title.as_deref(),
        Some("Bring selected window to foreground")
    );
    let s = serde_json::to_value(t.input_schema).unwrap();
    let mut required = s["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect::<Vec<_>>();
    required.sort();
    assert_eq!(required, ["request_id", "target"]);
    assert_eq!(s["additionalProperties"], false);
    assert!(s["properties"]["include_timings"].is_object());
    for field in [
        "lease",
        "frame",
        "reference",
        "expected_foreground",
        "key",
        "restore",
        "fallback",
    ] {
        let mut a = json!({"target":"selected-window","request_id":"f19-once"});
        a[field] = json!(true);
        assert!(serde_json::from_value::<BringToFrontArgs>(a).is_err());
    }
}

fn safe_returned() -> DeviceResult {
    let mut result = returned();
    result.data["activation"]["foreground"] = foreground();
    result.data["receipt"]["system_accepted"] = Value::Null;
    result.data["operation_ref"] = json!({"execution_nonce":"original"});
    result.data["operation"] = json!({"terminal":true,"native_call_pending":false,"resources_released":true,"resource_release_uncertain":false});
    result.data["stage_timings"] = json!([]);
    result.data["elapsed_ms"] = json!(10);
    result
}

#[test]
fn f19_bring_to_front_compact_success_preserves_foreground_identity_unknown_and_original() {
    let result = safe_returned();
    let original = result.data.clone();
    let compact = compact_bring_to_front_reply(&result, Some("details".into()));
    assert_eq!(result.data, original);
    assert_eq!(compact["activation"], original["activation"]);
    assert_eq!(compact["operation_ref"], original["operation_ref"]);
    assert_eq!(compact["system_accepted"], Value::Null);
    assert!(compact.get("system_accepted").is_some());
    assert_eq!(compact["replay"], "forbidden");
    assert_eq!(compact["goal_verified"], false);
    assert_eq!(compact["details_ref"], "details");
    assert!(!expose_timings("computer.bring_to_front", &result, false));
    assert!(expose_timings("computer.bring_to_front", &result, true));
}

#[test]
fn f19_bring_to_front_unknown_refusal_or_cleanup_facts_never_disappear() {
    for (pointer, value) in [
        ("/activation/state", json!("unknown")),
        ("/activation/foreground", Value::Null),
        ("/activation/foreground/native_epoch", json!("bad")),
        ("/receipt/dispatch", json!("uncertain")),
        ("/input_domain_fenced", json!(true)),
        ("/operation/terminal", Value::Null),
        ("/operation/native_call_pending", json!(true)),
        ("/operation/resources_released", json!(false)),
        ("/operation/resource_release_uncertain", json!(true)),
    ] {
        let mut result = safe_returned();
        *result.data.pointer_mut(pointer).unwrap() = value;
        assert!(!bring_to_front_returned(&result));
        let compact = compact_bring_to_front_reply(&result, Some("details".into()));
        for (key, value) in result.data.as_object().unwrap() {
            assert_eq!(&compact[key], value, "preserve {pointer}/{key}");
        }
        assert!(expose_timings("computer.bring_to_front", &result, false));
    }
}
