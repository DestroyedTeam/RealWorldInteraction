//! U: service/control isolation without native OS access or business manifests.
use rcu_agent::{
    config::{Config, PlatformPolicy, Principal},
    runtime::Runtime,
};
use rcu_core::device::{Call, PROTOCOL_VERSION, Request};
use rcu_link_trust::{IdentityPaths, PeerPin};
use std::sync::Arc;
fn setup(platform: PlatformPolicy) -> (tempfile::TempDir, Arc<Runtime>, PeerPin) {
    let temp = tempfile::tempdir().unwrap();
    let pin = PeerPin::parse(&"a".repeat(64)).unwrap();
    let runtime = Runtime::new(Config {
        version: 1,
        device_id: "test-device".into(),
        bind: "127.0.0.1:0".parse().unwrap(),
        identity: IdentityPaths::in_profile(temp.path()),
        principals: vec![Principal {
            certificate: pin.clone(),
            observe: true,
            input: true,
        }],
        state_root: temp.path().join("state"),
        platform,
    })
    .unwrap();
    (temp, runtime, pin)
}
async fn wire_call(
    r: &Arc<Runtime>,
    p: &PeerPin,
    context: Option<&str>,
    call: Call,
) -> rcu_core::device::DeviceResult {
    r.clone()
        .handle(
            p.clone(),
            Request {
                version: PROTOCOL_VERSION,
                id: uuid::Uuid::new_v4().to_string(),
                device: "test-device".into(),
                context: context.map(str::to_owned),
                call,
            },
        )
        .await
        .result
}
// Test-only ordinary invocation helper; race tests use wire_call directly.
async fn call(
    r: &Arc<Runtime>,
    p: &PeerPin,
    context: Option<&str>,
    call: Call,
) -> rcu_core::device::DeviceResult {
    let execute =
        matches!(&call,Call::Prepare{tool,..} if rcu_core::device::catalog::executable(tool));
    let call = match call {
        Call::Prepare { tool, arguments } if rcu_core::device::catalog::cached_read(&tool) => {
            Call::Cached { tool, arguments }
        }
        other => other,
    };
    let accepted = wire_call(r, p, context, call).await;
    if accepted.is_error || !execute {
        return accepted;
    }
    let operation_ref = serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
    wire_call(r, p, None, Call::Execute { operation_ref }).await
}
#[tokio::test]
async fn service_health_has_no_business_manifest_or_platform_readiness_dependency() {
    let (temp, r, p) = setup(PlatformPolicy::Unsupported {
        platform: "not-yet-implemented".into(),
    });
    std::fs::write(
        temp.path().join("state/approved-task.json"),
        b"not even valid JSON",
    )
    .unwrap();
    let info = call(&r, &p, None, Call::Describe).await;
    assert!(!info.is_error);
    assert_eq!(info.data["service"], "healthy");
    assert_eq!(info.data["availability"], "unavailable");
    assert_eq!(info.data["platform"], "not-yet-implemented");
    let caps = info.data["capabilities"].as_array().unwrap();
    assert_eq!(
        caps.iter().find(|c| c["id"] == "computer.observe").unwrap()["implemented"],
        false
    );
    assert_eq!(
        caps.iter()
            .find(|c| c["id"] == "computer.operation")
            .unwrap()["available"],
        true
    );
    assert!(call(&r, &p, None, Call::OpenContext).await.is_error);
    assert_eq!(
        call(&r, &p, None, Call::Describe).await.data["service"],
        "healthy"
    );
    let foreign = PeerPin::parse(&"b".repeat(64)).unwrap();
    assert!(call(&r, &foreign, None, Call::Describe).await.is_error);
}

#[cfg(feature = "fixture")]
#[tokio::test]
async fn principal_receipts_are_isolated_but_desktop_leases_are_shared() {
    use serde_json::json;
    let (_temp, initial, p) = setup(PlatformPolicy::Fixture);
    let q = PeerPin::parse(&"b".repeat(64)).unwrap();
    let readonly = PeerPin::parse(&"c".repeat(64)).unwrap();
    let mut config = initial.config.clone();
    drop(initial);
    config.principals.extend([
        Principal {
            certificate: q.clone(),
            observe: true,
            input: true,
        },
        Principal {
            certificate: readonly.clone(),
            observe: true,
            input: false,
        },
    ]);
    let r = Runtime::new(config).unwrap();
    let caps = call(&r, &readonly, None, Call::Describe).await;
    let claim = caps.data["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "computer.claim")
        .unwrap();
    assert_eq!(claim["implemented"], true);
    assert_eq!(claim["authorized"], false);
    assert_eq!(claim["available"], false);
    let a = call(&r, &p, None, Call::OpenContext).await;
    let b = call(&r, &q, None, Call::OpenContext).await;
    let invoke = |tool: &str, arguments| Call::Prepare {
        tool: tool.into(),
        arguments,
    };
    for (index, peer, opened) in [(0, &p, &a), (1, &q, &b)] {
        let context = opened.data["context"].as_str();
        let target = &opened.data["targets"][0]["target"];
        let observed = call(
            &r,
            peer,
            context,
            invoke(
                "computer.observe",
                json!({"target":target,"inline_image":false}),
            ),
        )
        .await;
        let lease = call(
            &r,
            peer,
            context,
            invoke("computer.claim", json!({"target":target,"ttl_ms":60000})),
        )
        .await;
        assert!(!lease.is_error, "{:?}", lease.data);
        if index == 0 {
            let blocked = call(
                &r,
                &q,
                b.data["context"].as_str(),
                invoke(
                    "computer.claim",
                    json!({"target":b.data["targets"][0]["target"],"ttl_ms":1000}),
                ),
            )
            .await;
            assert!(
                blocked.is_error,
                "different principals must share the same desktop lock"
            );
        }
        let result = call(&r,peer,context,invoke("computer.act_checked",json!({"target":target,"frame":observed.data["frame"],"element":"Input","lease":lease.data["lease"],"request_id":"same-id","operation_id":"same-monitor","intent":"text","value":format!("principal {index}")}))).await;
        assert!(!result.is_error, "principal {index}: {:?}", result.data);
        assert_eq!(result.data["dispatch"], "returned");
        assert!(
            !call(
                &r,
                peer,
                context,
                invoke("computer.release", json!({"lease":lease.data["lease"]}))
            )
            .await
            .is_error
        );
    }
}

#[cfg(feature = "fixture")]
#[tokio::test]
async fn cancellation_after_acceptance_never_dispatches_and_recovery_is_independent() {
    use serde_json::json;
    let (_temp, r, p) = setup(PlatformPolicy::Fixture);
    let opened = call(&r, &p, None, Call::OpenContext).await;
    let context = opened.data["context"].as_str();
    let target = &opened.data["targets"][0]["target"];
    let observe = || Call::Prepare {
        tool: "computer.observe".into(),
        arguments: json!({"target":target,"correlation_id":"reused","inline_image":false}),
    };
    let accepted = wire_call(&r, &p, context, observe()).await;
    assert_eq!(accepted.data["native_dispatch_started"], false);
    let operation_ref: rcu_core::device::OperationRef =
        serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
    let cancelled = wire_call(
        &r,
        &p,
        None,
        Call::Cancel {
            operation_ref: operation_ref.clone(),
        },
    )
    .await;
    assert_eq!(cancelled.data["terminal"], true);
    assert_eq!(cancelled.data["dispatch_started"], false);
    assert_eq!(
        wire_call(&r, &p, None, Call::Execute { operation_ref })
            .await
            .data["code"],
        "execution_cancelled_before_dispatch"
    );
    assert!(
        !call(&r, &p, context, observe()).await.is_error,
        "correlation label does not poison later work"
    );
    let domain = call(&r, &p, None, Call::Describe).await.data["resource_domain"].clone();
    for (key, ok) in [(json!("other-domain"), false), (domain, true)] {
        let result = call(
            &r,
            &p,
            context,
            Call::Prepare {
                tool: "computer.recover_input".into(),
                arguments: json!({"device_id":"test-device","resource_domain":key}),
            },
        )
        .await;
        assert_eq!(!result.is_error, ok, "{:?}", result.data);
        if ok {
            assert_eq!(result.data["old_operations_replayable"], false);
        }
    }
}

#[cfg(feature = "fixture")]
#[tokio::test]
async fn separate_contexts_foreign_handles_bad_actions_and_new_sessions() {
    use serde_json::{Value, json};
    let (_temp, r, p) = setup(PlatformPolicy::Fixture);
    let a = call(&r, &p, None, Call::OpenContext).await;
    let b = call(&r, &p, None, Call::OpenContext).await;
    let ca = a.data["context"].as_str().unwrap();
    let cb = b.data["context"].as_str().unwrap();
    assert_ne!(ca, cb);
    let ta = a.data["targets"][0]["target"].as_str().unwrap();
    let tb = b.data["targets"][0]["target"].as_str().unwrap();
    let invoke = |tool: &str, args: Value| Call::Prepare {
        tool: tool.into(),
        arguments: args,
    };
    assert!(
        call(
            &r,
            &p,
            Some(cb),
            invoke(
                "computer.observe",
                json!({"target":ta,"inline_image":false})
            )
        )
        .await
        .is_error
    );
    let bad=call(&r,&p,Some(ca),invoke("computer.act_checked",json!({"target":ta,"frame":"missing","element":"Input","lease":"missing","request_id":"bad-one","intent":"click"}))).await;
    assert!(bad.is_error);
    assert_eq!(
        call(&r, &p, None, Call::Describe).await.data["input_domain_fenced"],
        false
    );
    assert!(
        !call(
            &r,
            &p,
            Some(cb),
            invoke(
                "computer.observe",
                json!({"target":tb,"inline_image":false})
            )
        )
        .await
        .is_error
    );
    assert!(!call(&r, &p, Some(ca), Call::CloseContext).await.is_error);
    assert!(!call(&r, &p, None, Call::OpenContext).await.is_error);
}

#[cfg(feature = "fixture")]
#[tokio::test]
async fn lease_is_shared_and_repeated_operation_does_not_dispatch() {
    use serde_json::{Value, json};
    let (_temp, r, p) = setup(PlatformPolicy::Fixture);
    let a = call(&r, &p, None, Call::OpenContext).await;
    let b = call(&r, &p, None, Call::OpenContext).await;
    let ca = a.data["context"].as_str().unwrap();
    let cb = b.data["context"].as_str().unwrap();
    let ta = &a.data["targets"][0]["target"];
    let tb = &b.data["targets"][0]["target"];
    let invoke = |tool: &str, args: Value| Call::Prepare {
        tool: tool.into(),
        arguments: args,
    };
    let observed = call(
        &r,
        &p,
        Some(ca),
        invoke(
            "computer.observe",
            json!({"target":ta,"inline_image":false}),
        ),
    )
    .await;
    let lease = call(
        &r,
        &p,
        Some(ca),
        invoke("computer.claim", json!({"target":ta,"ttl_ms":60000})),
    )
    .await;
    assert!(!lease.is_error);
    assert!(
        call(
            &r,
            &p,
            Some(cb),
            invoke("computer.claim", json!({"target":tb,"ttl_ms":60000}))
        )
        .await
        .is_error
    );
    let args = json!({"target":ta,"frame":observed.data["frame"],"element":"Input","lease":lease.data["lease"],"request_id":"stable-action","operation_id":"monitor-action","intent":"text","value":"test via generic capability"});
    let result = call(
        &r,
        &p,
        Some(ca),
        invoke("computer.act_checked", args.clone()),
    )
    .await;
    assert!(!result.is_error, "{:?}", result.data);
    assert_eq!(result.data["dispatch"], "returned");
    let repeated = call(&r, &p, Some(ca), invoke("computer.act_checked", args)).await;
    assert_eq!(repeated.data["dispatched_again"], false);
    let queried = call(
        &r,
        &p,
        None,
        Call::Operation {
            selector: rcu_core::device::OperationSelector::Request {
                device_id: "test-device".into(),
                request_id: "stable-action".into(),
            },
        },
    )
    .await;
    assert!(!queried.is_error);
    assert_eq!(
        queried.data["result"]["data"]["correlation_id"],
        "monitor-action"
    );
    assert!(
        !call(
            &r,
            &p,
            Some(ca),
            invoke("computer.release", json!({"lease":lease.data["lease"]}))
        )
        .await
        .is_error
    );
    assert!(
        !call(
            &r,
            &p,
            Some(cb),
            invoke("computer.claim", json!({"target":tb,"ttl_ms":1000}))
        )
        .await
        .is_error
    );
}
