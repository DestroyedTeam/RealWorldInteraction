//! Counters belong to test-injected fixtures, not product/business workflows.
use super::*;
use serde_json::Value;
async fn invoke(r: &Arc<Runtime>, p: &Principal, c: &str, tool: &str, args: Value) -> DeviceResult {
    let accepted = r.prepare(&p.certificate, p, Some(c), tool.into(), args);
    assert!(!accepted.is_error, "{:?}", accepted.data);
    let reference = serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
    r.execute(&p.certificate, p, &reference).await
}
async fn tracked_context(
    r: &Arc<Runtime>,
    p: &Principal,
    fixture: &rwi_core::fixture::Fixture,
) -> (String, String) {
    let opened = r.call(&p.certificate, p, None, Call::OpenContext).await;
    let token = opened.data["context"].as_str().unwrap().to_owned();
    let c = r.context(&p.certificate, Some(&token)).unwrap();
    let mut engine = rwi_core::Engine::with_lease_root(
        r.config.state_root.join("test-contexts").join(&token),
        r.config.state_root.join("shared-native"),
    )
    .unwrap();
    engine
        .register(
            c.target.clone(),
            Box::new(crate::observed_backend::Observed {
                inner: Box::new(fixture.clone()),
                entered: c.entered.clone(),
            }),
        )
        .unwrap();
    *c.executor.lock().unwrap() = Executor::new(engine);
    (token, c.target.clone())
}
#[tokio::test]
async fn request_identity_does_not_reset_dispatch_count_at_context_or_agent_epoch() {
    let temp = tempfile::tempdir().unwrap();
    let p = Principal {
        certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
        observe: true,
        input: true,
    };
    let r = Runtime::new(Config {
        version: 1,
        device_id: "device".into(),
        bind: "127.0.0.1:0".parse().unwrap(),
        identity: rwi_link_trust::IdentityPaths::in_profile(temp.path()),
        principals: vec![p.clone()],
        state_root: temp.path().join("state"),
        platform: crate::config::PlatformPolicy::Fixture,
    })
    .unwrap();
    let fixture = rwi_core::fixture::Fixture::default();
    let (c, target) = tracked_context(&r, &p, &fixture).await;
    let lease = invoke(
        &r,
        &p,
        &c,
        "computer.claim",
        json!({"target":target,"ttl_ms":60000}),
    )
    .await;
    let mut refs = Vec::new();
    for request in ["old-read-label", "second-request"] {
        let observation = invoke(
            &r,
            &p,
            &c,
            "computer.observe",
            json!({"target":target,"correlation_id":"old-read-label","inline_image":false}),
        )
        .await;
        let args = json!({"target":target,"frame":observation.data["frame"],"lease":lease.data["lease"],"request_id":request,"correlation_id":"old-read-label","element":"Input","intent":"text","value":request});
        let result = invoke(&r, &p, &c, "computer.act_checked", args.clone()).await;
        assert_eq!(result.data["dispatch"], "returned", "{:?}", result.data);
        refs.push(
            serde_json::from_value::<OperationRef>(result.data["operation_ref"].clone()).unwrap(),
        );
        let mut args = args;
        args["correlation_id"] = json!("different-label");
        assert_eq!(
            invoke(&r, &p, &c, "computer.act_checked", args).await.data["dispatched_again"],
            false
        );
    }
    assert_eq!(fixture.0.lock().unwrap().calls, 2);
    invoke(
        &r,
        &p,
        &c,
        "computer.release",
        json!({"lease":lease.data["lease"]}),
    )
    .await;
    assert!(
        !r.call(&p.certificate, &p, Some(&c), Call::CloseContext)
            .await
            .is_error
    );
    let config = r.config.clone();
    drop(r);
    let r = Runtime::new(config).unwrap();
    let (c, target) = tracked_context(&r, &p, &fixture).await;
    let lease = invoke(
        &r,
        &p,
        &c,
        "computer.claim",
        json!({"target":target,"ttl_ms":60000}),
    )
    .await;
    let observation = invoke(
        &r,
        &p,
        &c,
        "computer.observe",
        json!({"target":target,"inline_image":false}),
    )
    .await;
    let result=invoke(&r,&p,&c,"computer.act_checked",json!({"target":target,"frame":observation.data["frame"],"lease":lease.data["lease"],"request_id":"old-read-label","correlation_id":"old-read-label","element":"Input","intent":"text","value":"must not dispatch"})).await;
    assert_eq!(result.data["code"], "operation_payload_conflict");
    for request in ["old-read-label", "second-request"] {
        assert_eq!(
            r.query(
                &p.certificate,
                &OperationSelector::Request {
                    device_id: "device".into(),
                    request_id: request.into()
                }
            )
            .data["replay_allowed"],
            false
        );
    }
    for reference in refs {
        assert_eq!(
            r.cancel(&p.certificate, &reference).data["code"],
            "execution_ref_expired"
        );
    }
    assert_eq!(
        fixture.0.lock().unwrap().calls,
        2,
        "restart and same-label work cannot replay either original action"
    );
}
