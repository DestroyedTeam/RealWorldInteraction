//! Actual child/TLS protocol tests; synthetic descriptions only, never real GUI.
use super::*;

fn descriptor() -> Value {
    json!({
        "availability":"available", "service":"healthy", "context_valid":true,
        "platform":"macos", "input_domain_fenced":false,
        "runtime_status":{"pid":42,"permissions":{"screen_capture":false,"listen_event":null}},
        "capabilities":[{"id":"computer.click","available":false,"authorized":true,"reason":"selected_window_required"}],
        "executor_features":{"description":"fixture documentation ".repeat(120)},
        "targets":[{"target":TARGET,"identity":{"host":"fixture-host","session":"fixture-session"},"parent_target":null,
            "capabilities":{"fixture_policy":"fixture policy details ".repeat(240)},
            "controls":[{"id":"synthetic-x","name":"X close","bounds":[12,20,44,44]}]}],
        "future":{"unknown":null}
    })
}
fn text(reply: &Value) -> Value {
    serde_json::from_str(reply["content"][0]["text"].as_str().unwrap()).unwrap()
}
fn same_nonvolatile_data(before: &Value, after: &Value) {
    let (mut before, mut after) = (before.clone(), after.clone());
    // These are separate RPCs. Preserve/check the changing transport counters
    // rather than pretending their values or per-call result handles are equal.
    for key in [
        "authenticated_connections",
        "keepalive_connections",
        "replies_validated",
        "request_writes_started",
        "reused_checkouts",
    ] {
        let pointer = format!("/devices/0/adapter_transport/{key}");
        let a = before.pointer_mut(&pointer).unwrap();
        let b = after.pointer_mut(&pointer).unwrap();
        assert!(b.as_u64().unwrap() >= a.as_u64().unwrap(), "{key}");
        *a = Value::Null;
        *b = Value::Null;
    }
    for key in ["result_ref", "metadata_sha256", "original_metadata_bytes"] {
        for result in [&mut before, &mut after] {
            let value = result["presentation"]
                .as_object_mut()
                .unwrap()
                .remove(key)
                .unwrap();
            if key == "original_metadata_bytes" {
                assert!(value.as_u64().unwrap() > 0);
            } else {
                let s = value.as_str().unwrap();
                assert_eq!(s.len(), if key == "result_ref" { 32 } else { 64 });
                assert!(s.bytes().all(|b| b.is_ascii_hexdigit()));
            }
        }
    }
    assert!(
        before == after,
        "a nonvolatile structured field changed between requests"
    );
}
#[tokio::test]
async fn f45_summary_real_mcp_preserves_full_structured_and_current_service_shape() {
    let p = Peer::new().await;
    *p.state.targets_fixture.lock().unwrap() = Some(descriptor());
    let mut h = Host::start(&p.profile, false).await;
    h.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    let list = h.receive(2).await;
    let schema = &list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "computer.targets")
        .unwrap()["inputSchema"];
    assert_eq!(
        schema["properties"]["text_projection"]["enum"],
        json!(["exact_duplicates_v1", "structured_summary_v1"])
    );
    let full = h
        .call(3, "computer.targets", json!({"device_id":"image-device"}))
        .await;
    assert_eq!(text(&full), full["structuredContent"]);
    let summary = h
        .call(
            4,
            "computer.targets",
            json!({"device_id":"image-device","text_projection":"structured_summary_v1"}),
        )
        .await;
    assert_ne!(summary["isError"], true, "{summary}");
    let plain = text(&summary);
    assert_eq!(
        plain["_text_projection"]["version"],
        "targets_structured_summary_v1"
    );
    same_nonvolatile_data(&full["structuredContent"], &summary["structuredContent"]);
    assert_eq!(plain["targets"][0]["target"], TARGET);
    assert_eq!(
        plain["targets"][0]["controls"],
        full["structuredContent"]["targets"][0]["controls"]
    );
    assert_eq!(plain["devices"][0]["state"]["service"], "healthy");
    assert_eq!(
        plain["devices"][0]["state"]["runtime_status"],
        full["structuredContent"]["devices"][0]["state"]["runtime_status"]
    );
    let old = serde_json::to_vec(&full).unwrap().len();
    let new = serde_json::to_vec(&summary).unwrap().len();
    assert!(new * 10 < old * 7, "{new} not below 70% of {old}");
    println!(
        "F45_SUMMARY_REAL_MCP_SYNTHETIC full_result_bytes={old} summary_result_bytes={new} nonvolatile_structured_equal=true"
    );
    let legacy = h
        .call(
            5,
            "computer.targets",
            json!({"device_id":"image-device","text_projection":"exact_duplicates_v1"}),
        )
        .await;
    assert_eq!(
        text(&legacy),
        legacy["structuredContent"],
        "current service string must not become an invented legacy alias"
    );
    h.stop().await;
    assert!(p.state.calls.lock().unwrap().iter().all(|r| matches!(
        r.call,
        Call::Describe | Call::OpenContext | Call::CloseContext
    )));
    p.stop().await;
}

#[tokio::test]
async fn f45_summary_real_mcp_invalid_mode_and_unknown_device_do_not_reach_peer() {
    let p = Peer::new().await;
    let mut h = Host::start(&p.profile, false).await;
    for (id, value) in [(2, json!(null)), (3, json!(true)), (4, json!("future_v2"))] {
        let r = h
            .call(
                id,
                "computer.targets",
                json!({"device_id":"image-device","text_projection":value}),
            )
            .await;
        assert_eq!(r["isError"], true);
        assert_eq!(r["structuredContent"]["code"], "invalid_arguments");
        assert_eq!(text(&r), r["structuredContent"]);
    }
    let r = h
        .call(
            5,
            "computer.targets",
            json!({"device_id":"not-paired","text_projection":"structured_summary_v1"}),
        )
        .await;
    assert_eq!(r["isError"], true);
    assert_eq!(r["structuredContent"]["code"], "device_unknown");
    assert_eq!(text(&r), r["structuredContent"]);
    assert!(p.state.calls.lock().unwrap().is_empty());
    h.stop().await;
    p.stop().await;
}
