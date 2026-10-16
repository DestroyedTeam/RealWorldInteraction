//! Real service acceptance/execution and authenticated context boundaries.
//! Only synthetic windows; no activation, capture or hardware input.
use super::dispatch_tests::{invoke, tracked_backend_context, wire};
use super::*;
use rwi_core::fixture::windows::Desktop;
fn setup(dir: &std::path::Path, input: bool) -> (Arc<Runtime>, Principal, Principal) {
    let policy = Principal {
        journal_identity: None,
        certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
        observe: true,
        input,
    };
    let mut other = policy.clone();
    other.certificate = PeerPin::parse(&"b".repeat(64)).unwrap();
    let r = Runtime::new(Config {
        version: 1,
        device_id: "windows-fixture".into(),
        bind: "127.0.0.1:0".parse().unwrap(),
        identity: rwi_link_trust::IdentityPaths::in_profile(dir),
        state_root: dir.join("state"),
        principals: vec![policy.clone(), other.clone()],
        platform: crate::config::PlatformPolicy::Fixture,
    })
    .unwrap();
    (r, policy, other)
}
#[tokio::test]
async fn attached_handles_are_owned_by_one_principal_context_and_retired_on_detach() {
    let dir = tempfile::tempdir().unwrap();
    let (r, p, other) = setup(dir.path(), true);
    let desktop = Desktop::default();
    let (c, root) = tracked_backend_context(&r, &p, Box::new(desktop.clone())).await;
    let catalog = invoke(&r, &p, &c, "computer.discover", json!({"target":root})).await;
    let selection = catalog.data["windows"][1].clone();
    let attached = invoke(
        &r,
        &p,
        &c,
        "computer.attach_window",
        json!({"target":root,"selection":selection}),
    )
    .await;
    assert!(!attached.is_error, "{:?}", attached.data);
    assert_eq!(attached.data["native_dispatch_entered"], false);
    let target = attached.data["target"].as_str().unwrap();
    let description = wire(&r, &p, Some(&c), Call::Describe).await;
    assert_eq!(description.data["targets"].as_array().unwrap().len(), 2);
    let other_context = wire(&r, &p, None, Call::OpenContext).await;
    let other_token = other_context.data["context"].as_str().unwrap();
    let other_peer_context = wire(&r, &other, None, Call::OpenContext).await;
    let other_peer_token = other_peer_context.data["context"].as_str().unwrap();
    for (policy, token) in [
        (&p, other_token),
        (&other, other_peer_token),
        (&other, c.as_str()),
    ] {
        let rejected = wire(
            &r,
            policy,
            Some(token),
            Call::Prepare {
                tool: "computer.observe".into(),
                arguments: json!({"target":target,"inline_image":false}),
            },
        )
        .await;
        assert!(rejected.is_error, "{:?}", rejected.data);
        assert!(matches!(
            rejected.data["code"].as_str(),
            Some("foreign_target" | "context_unknown")
        ));
    }
    let shot = invoke(
        &r,
        &p,
        &c,
        "computer.observe",
        json!({"target":target,"inline_image":false}),
    )
    .await;
    assert!(!shot.is_error, "{:?}", shot.data);
    let resolved=wire(&r,&p,Some(&c),Call::Cached{tool:"computer.resolve".into(),arguments:json!({"target":target,"frame":shot.data["frame"],"locator":{"kind":"element","id":"Input"}})}).await;
    assert!(!resolved.is_error, "{:?}", resolved.data);
    let lease = invoke(
        &r,
        &p,
        &c,
        "computer.claim",
        json!({"target":target,"ttl_ms":60000}),
    )
    .await;
    let action_args = json!({"target":target,"frame":shot.data["frame"],"element":"Input","lease":lease.data["lease"],"request_id":"attached-input-once","intent":"text","value":"independent-window"});
    let receipt = invoke(&r, &p, &c, "computer.act_checked", action_args.clone()).await;
    assert!(!receipt.is_error, "{:?}", receipt.data);
    assert_eq!(receipt.data["native_dispatch_entered"], true); // Observed wrapper propagated.
    let busy = invoke(
        &r,
        &p,
        &c,
        "computer.detach_window",
        json!({"target":target}),
    )
    .await;
    assert_eq!(busy.data["code"], "target_leased");
    invoke(
        &r,
        &p,
        &c,
        "computer.release",
        json!({"lease":lease.data["lease"]}),
    )
    .await;
    let detached = invoke(
        &r,
        &p,
        &c,
        "computer.detach_window",
        json!({"target":target}),
    )
    .await;
    assert!(!detached.is_error);
    assert_eq!(detached.data["native_dispatch_entered"], false);
    let rejected = wire(
        &r,
        &p,
        Some(&c),
        Call::Prepare {
            tool: "computer.observe".into(),
            arguments: json!({"target":target}),
        },
    )
    .await;
    assert_eq!(rejected.data["code"], "foreign_target");
    let queried = wire(
        &r,
        &p,
        None,
        Call::Operation {
            selector: OperationSelector::Request {
                device_id: r.config.device_id.clone(),
                request_id: "attached-input-once".into(),
            },
        },
    )
    .await;
    assert!(!queried.is_error, "{:?}", queried.data);
    assert_eq!(queried.data["replay_allowed"], false);
    assert_eq!(desktop.windows[1].0.lock().unwrap().calls, 1);
    assert_eq!(desktop.windows[0].0.lock().unwrap().calls, 0);
    let description = wire(&r, &p, Some(&c), Call::Describe).await;
    assert_eq!(description.data["targets"].as_array().unwrap().len(), 1);
    let old_ref: OperationRef =
        serde_json::from_value(attached.data["operation_ref"].clone()).unwrap();
    let repeated = wire(
        &r,
        &p,
        None,
        Call::Execute {
            operation_ref: old_ref.clone(),
        },
    )
    .await;
    assert!(repeated.is_error);
    assert_eq!(description.data["targets"].as_array().unwrap().len(), 1);
    wire(&r, &p, Some(&c), Call::CloseContext).await;
    assert!(
        wire(
            &r,
            &p,
            None,
            Call::Execute {
                operation_ref: old_ref
            }
        )
        .await
        .is_error
    );
}

#[tokio::test]
async fn observation_only_principal_can_attach_without_input_or_implicit_authority() {
    let dir = tempfile::tempdir().unwrap();
    let (r, p, _) = setup(dir.path(), false);
    let desktop = Desktop::default();
    let (c, root) = tracked_backend_context(&r, &p, Box::new(desktop.clone())).await;
    let catalog = invoke(&r, &p, &c, "computer.discover", json!({"target":root})).await;
    let attached = invoke(
        &r,
        &p,
        &c,
        "computer.attach_window",
        json!({"target":root,"selection":catalog.data["windows"][0]}),
    )
    .await;
    assert!(!attached.is_error);
    assert_eq!(attached.data["native_dispatch_entered"], false);
    let target = &attached.data["target"];
    let denied = wire(
        &r,
        &p,
        Some(&c),
        Call::Prepare {
            tool: "computer.claim".into(),
            arguments: json!({"target":target,"ttl_ms":1000}),
        },
    )
    .await;
    assert_eq!(denied.data["code"], "permission_denied");
    assert!(
        !invoke(
            &r,
            &p,
            &c,
            "computer.observe",
            json!({"target":target,"inline_image":false})
        )
        .await
        .is_error
    );
    assert!(
        !invoke(
            &r,
            &p,
            &c,
            "computer.detach_window",
            json!({"target":target})
        )
        .await
        .is_error
    );
    assert_eq!(
        desktop
            .windows
            .iter()
            .map(|w| w.0.lock().unwrap().calls)
            .sum::<u64>(),
        0
    );
}
#[tokio::test]
async fn cancelled_or_wrong_instance_attachment_execution_cannot_create_an_alias() {
    let dir = tempfile::tempdir().unwrap();
    let (r, p, _) = setup(dir.path(), false);
    let desktop = Desktop::default();
    let (c, root) = tracked_backend_context(&r, &p, Box::new(desktop.clone())).await;
    let catalog = invoke(&r, &p, &c, "computer.discover", json!({"target":root})).await;
    let accepted = wire(
        &r,
        &p,
        Some(&c),
        Call::Prepare {
            tool: "computer.attach_window".into(),
            arguments: json!({"target":root,"selection":catalog.data["windows"][0]}),
        },
    )
    .await;
    assert!(!accepted.is_error);
    let reference: OperationRef =
        serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
    let mut foreign = reference.clone();
    foreign.agent_instance = "different-agent-instance".into();
    assert!(
        wire(
            &r,
            &p,
            None,
            Call::Execute {
                operation_ref: foreign
            }
        )
        .await
        .is_error
    );
    wire(
        &r,
        &p,
        None,
        Call::Cancel {
            operation_ref: reference.clone(),
        },
    )
    .await;
    let result = wire(
        &r,
        &p,
        None,
        Call::Execute {
            operation_ref: reference,
        },
    )
    .await;
    assert!(result.is_error);
    assert_eq!(
        wire(&r, &p, Some(&c), Call::Describe).await.data["targets"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        desktop
            .windows
            .iter()
            .map(|w| w.0.lock().unwrap().calls)
            .sum::<u64>(),
        0
    );
}
