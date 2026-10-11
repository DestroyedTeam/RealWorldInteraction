//! Counters belong to test-injected fixtures, not product/business workflows.
use super::*;
use serde_json::Value;

#[tokio::test]
async fn volatile_describe_drops_cached_foreground_without_waiting_for_executor() {
    // Synthetic backend only: no native windows, AX, capture or input.
    let temp = tempfile::tempdir().unwrap();
    let principal = Principal {
        journal_identity: None,
        certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
        observe: true,
        input: true,
    };
    let runtime = Runtime::new(Config {
        version: 1,
        device_id: "device".into(),
        bind: "127.0.0.1:0".parse().unwrap(),
        identity: rwi_link_trust::IdentityPaths::in_profile(temp.path()),
        principals: vec![principal.clone()],
        state_root: temp.path().join("state"),
        platform: crate::config::PlatformPolicy::Fixture,
    })
    .unwrap();
    let opened = wire(&runtime, &principal, None, Call::OpenContext).await;
    let token = opened.data["context"].as_str().unwrap();
    let context = runtime.context(principal.identity(), Some(token)).unwrap();
    let cached_foreground = json!({"pid":7,"window":"cgwindow:77",
        "process_start":"old-process","native_epoch":"old-foreground"});
    {
        let mut cache = context.target_catalog.lock().unwrap();
        cache[0]["capabilities"]["current_foreground"] = cached_foreground.clone();
        cache[0]["capabilities"]["user_input_recent_seconds"] = json!(99.0);
        cache[0]["capabilities"]["binding_marker"] = json!("preserve-owner-catalog");
    }
    // Mark the native lane busy; Describe must still use its independent probe
    // and metadata-only catalogue, never acquire the executor to refresh facts.
    let owned_executor = context.executor.clone();
    let (held_tx, held_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let holder = std::thread::spawn(move || {
        let _guard = owned_executor.lock().unwrap();
        held_tx.send(()).unwrap();
        let _ = release_rx.recv_timeout(std::time::Duration::from_secs(3));
    });
    held_rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap();
    let started = std::time::Instant::now();
    let described = wire(&runtime, &principal, Some(token), Call::Describe).await;
    release_tx.send(()).unwrap();
    holder.join().unwrap();
    assert!(started.elapsed() < std::time::Duration::from_secs(1));
    assert!(
        described.data["targets"][0]["capabilities"]["current_foreground"].is_null(),
        "cached foreground must not masquerade as a live probe: {}",
        described.data
    );
    assert!(described.data["targets"][0]["capabilities"]["user_input_recent_seconds"].is_null());
    assert_eq!(
        described.data["targets"][0]["capabilities"]["binding_marker"],
        "preserve-owner-catalog"
    );
    assert_eq!(
        described.data["targets"][0]["target"],
        opened.data["targets"][0]["target"]
    );
    assert_eq!(
        context.target_catalog.lock().unwrap()[0]["capabilities"]["current_foreground"],
        cached_foreground
    );
}

pub(super) async fn wire(
    r: &Arc<Runtime>,
    p: &Principal,
    c: Option<&str>,
    call: Call,
) -> DeviceResult {
    r.clone()
        .handle(
            p.certificate.clone(),
            Request {
                version: PROTOCOL_VERSION,
                id: uuid::Uuid::new_v4().to_string(),
                device: r.config.device_id.clone(),
                context: c.map(str::to_owned),
                call,
            },
        )
        .await
        .result
}
pub(super) async fn invoke(
    r: &Arc<Runtime>,
    p: &Principal,
    c: &str,
    tool: &str,
    args: Value,
) -> DeviceResult {
    let accepted = wire(
        r,
        p,
        Some(c),
        Call::Prepare {
            tool: tool.into(),
            arguments: args,
        },
    )
    .await;
    assert!(!accepted.is_error, "{:?}", accepted.data);
    let reference = serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
    wire(
        r,
        p,
        None,
        Call::Execute {
            operation_ref: reference,
        },
    )
    .await
}
async fn tracked_context(
    r: &Arc<Runtime>,
    p: &Principal,
    fixture: &rwi_core::fixture::Fixture,
) -> (String, String) {
    tracked_backend_context(r, p, Box::new(fixture.clone())).await
}
pub(super) async fn tracked_backend_context(
    r: &Arc<Runtime>,
    p: &Principal,
    backend: Box<dyn rwi_core::Backend>,
) -> (String, String) {
    let opened = wire(r, p, None, Call::OpenContext).await;
    let token = opened.data["context"].as_str().unwrap().to_owned();
    let c = r.context(p.identity(), Some(&token)).unwrap();
    let mut engine = rwi_core::Engine::with_lease_root(
        r.config.state_root.join("test-contexts").join(&token),
        r.config.state_root.join("shared-native"),
    )
    .unwrap();
    engine
        .register(
            c.target.clone(),
            Box::new(crate::observed_backend::Observed {
                inner: backend,
                entered: c.entered.clone(),
            }),
        )
        .unwrap();
    let mut executor = Executor::new(engine);
    *c.target_catalog.lock().unwrap() = executor.engine.targets();
    executor.target_catalog = c.target_catalog.clone();
    *c.live_controls.lock().unwrap() = executor.engine.live_controllers();
    executor.live_controls = c.live_controls.clone();
    *c.executor.lock().unwrap() = executor;
    (token, c.target.clone())
}

// A synthetic native backend posts receiver preparation, then stops before the
// requested input. No AX/screen/OS input or running Agent is touched by this test.
#[derive(Clone)]
struct PreparationRejected {
    fixture: rwi_core::fixture::Fixture,
    effects: Arc<std::sync::atomic::AtomicU64>,
    requested_input_events: u64,
}
impl rwi_core::Backend for PreparationRejected {
    fn identity(&self) -> rwi_core::Identity {
        self.fixture.identity()
    }
    fn capabilities(&self) -> Value {
        self.fixture.capabilities()
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        Some(self.effects.load(Ordering::Acquire))
    }
    fn observe(&mut self) -> rwi_core::Result<rwi_core::Scene> {
        self.fixture.observe()
    }
    fn inspect(
        &mut self,
        scope: &rwi_core::Scope,
        predicate: &rwi_core::Predicate,
        expected: &str,
    ) -> rwi_core::Result<rwi_core::Oracle> {
        self.fixture.inspect(scope, predicate, expected)
    }
    fn act(
        &mut self,
        _: &rwi_core::Binding,
        _: &rwi_core::Resolved,
        _: &rwi_core::Intent,
        _: Option<&str>,
    ) -> rwi_core::Result<Option<bool>> {
        self.effects
            .fetch_add(1 + self.requested_input_events, Ordering::AcqRel);
        let mut error = rwi_core::fault("stale_reference", "Synthetic final input guard rejected.");
        error.diagnostic = Some(Box::new(json!({"dispatch_evidence": {
            "version": 1,
            "kind": "macos_background_preparation_only",
            "api": "CGEventPostToPid",
            "preparation_event": "NSApplicationActivatedEventType",
            "preparation_events": 1,
            "requested_input_events": self.requested_input_events,
            "held_input_resources": false,
            "replay_allowed": false
        }})));
        Err(error)
    }
}

#[tokio::test]
async fn preparation_only_receipt_stays_uncertain_and_nonreplayable_without_a_restart_fence() {
    for requested_input_events in [0, 1] {
        let temp = tempfile::tempdir().unwrap();
        let p = Principal {
            journal_identity: None,
            certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
            observe: true,
            input: true,
        };
        let config = Config {
            version: 1,
            device_id: "device".into(),
            bind: "127.0.0.1:0".parse().unwrap(),
            identity: rwi_link_trust::IdentityPaths::in_profile(temp.path()),
            principals: vec![p.clone()],
            state_root: temp.path().join("state"),
            platform: crate::config::PlatformPolicy::Fixture,
        };
        let r = Runtime::new(config.clone()).unwrap();
        let effects = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let backend = PreparationRejected {
            fixture: rwi_core::fixture::Fixture::default(),
            effects: effects.clone(),
            requested_input_events,
        };
        let (c, target) = tracked_backend_context(&r, &p, Box::new(backend)).await;
        let domain = r
            .context(p.identity(), Some(&c))
            .unwrap()
            .domain
            .key
            .clone();
        let lease = invoke(
            &r,
            &p,
            &c,
            "computer.claim",
            json!({"target": target, "ttl_ms": 60000}),
        )
        .await;
        let observation = invoke(
            &r,
            &p,
            &c,
            "computer.observe",
            json!({"target": target, "inline_image": false}),
        )
        .await;
        let args = json!({"target": target, "frame": observation.data["frame"], "lease": lease.data["lease"], "request_id": "preparation-only", "element": "Input", "intent": "click"});
        let result = invoke(&r, &p, &c, "computer.act_checked", args.clone()).await;
        assert_eq!(result.data["dispatch"], "uncertain", "{:?}", result.data);
        assert_eq!(result.data["native_dispatch_entered"], true);
        assert_eq!(
            result.data["input_domain_fenced"],
            requested_input_events != 0
        );
        assert_eq!(result.data["goal_verified"], false);
        assert_eq!(result.data["replay"], "forbidden");
        assert_eq!(result.data["operation"]["resources_released"], true);
        assert_eq!(
            result.data["dispatch_error"]["diagnostic"]["dispatch_evidence"]["requested_input_events"],
            requested_input_events
        );
        let attempts = effects.load(Ordering::Acquire);
        invoke(&r, &p, &c, "computer.act_checked", args).await;
        assert_eq!(
            effects.load(Ordering::Acquire),
            attempts,
            "same request never posts again"
        );
        invoke(
            &r,
            &p,
            &c,
            "computer.release",
            json!({"lease": lease.data["lease"]}),
        )
        .await;
        wire(&r, &p, Some(&c), Call::CloseContext).await;
        drop(r);

        let r = Runtime::new(config).unwrap();
        assert_eq!(
            r.is_fenced(&domain),
            requested_input_events != 0,
            "durable fence classification survives restart"
        );
        let retained = wire(
            &r,
            &p,
            None,
            Call::Operation {
                selector: OperationSelector::Request {
                    device_id: "device".into(),
                    request_id: "preparation-only".into(),
                },
            },
        )
        .await;
        assert_eq!(retained.data["replay_allowed"], false);
        assert_eq!(retained.data["result"]["data"]["dispatch"], "uncertain");
        assert_eq!(
            retained.data["result"]["data"]["input_domain_fenced"],
            requested_input_events != 0
        );
        assert_eq!(effects.load(Ordering::Acquire), attempts);
        if requested_input_events == 0 {
            let fixture = rwi_core::fixture::Fixture::default();
            let (c, target) = tracked_context(&r, &p, &fixture).await;
            let lease = invoke(
                &r,
                &p,
                &c,
                "computer.claim",
                json!({"target": target, "ttl_ms": 60000}),
            )
            .await;
            let observation = invoke(
                &r,
                &p,
                &c,
                "computer.observe",
                json!({"target": target, "inline_image": false}),
            )
            .await;
            let next = invoke(&r, &p, &c, "computer.act_checked", json!({"target": target, "frame": observation.data["frame"], "lease": lease.data["lease"], "request_id": "independent-new-intent", "element": "Input", "intent": "text", "value": "new request"})).await;
            assert_eq!(next.data["dispatch"], "returned", "{:?}", next.data);
            assert_eq!(fixture.0.lock().unwrap().calls, 1);
            assert_eq!(effects.load(Ordering::Acquire), attempts);
            invoke(
                &r,
                &p,
                &c,
                "computer.release",
                json!({"lease": lease.data["lease"]}),
            )
            .await;
        }
    }
}
#[tokio::test]
async fn request_identity_does_not_reset_dispatch_count_at_context_or_agent_epoch() {
    check_dispatch_continuity(false).await;
}
#[tokio::test]
async fn explicit_spki_migration_preserves_legacy_journal_and_never_replays() {
    check_dispatch_continuity(true).await;
}
async fn check_dispatch_continuity(migrate: bool) {
    let temp = tempfile::tempdir().unwrap();
    let mut p = Principal {
        journal_identity: None,
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
    assert!(!wire(&r, &p, Some(&c), Call::CloseContext).await.is_error);
    let mut config = r.config.clone();
    drop(r);
    let old = p.clone();
    if migrate {
        p.journal_identity = Some(p.certificate.clone());
        p.certificate = PeerPin::parse(&format!("spki:{}", "c".repeat(64))).unwrap();
        config.principals[0] = p.clone();
    }
    let r = Runtime::new(config).unwrap();
    if migrate {
        assert_eq!(
            wire(&r, &old, None, Call::Describe).await.data["code"],
            "principal_denied"
        );
    }
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
            wire(
                &r,
                &p,
                None,
                Call::Operation {
                    selector: OperationSelector::Request {
                        device_id: "device".into(),
                        request_id: request.into()
                    }
                }
            )
            .await
            .data["replay_allowed"],
            false
        );
    }
    for reference in refs {
        assert_eq!(
            wire(
                &r,
                &p,
                None,
                Call::Cancel {
                    operation_ref: reference
                }
            )
            .await
            .data["code"],
            "execution_ref_expired"
        );
    }
    assert_eq!(
        fixture.0.lock().unwrap().calls,
        2,
        "restart and same-label work cannot replay either original action"
    );
}

// Regression probe: Live control/presence must not wait behind long native input.
// Uses only the offline synthetic backend, not an application, screen or input.
#[tokio::test]
#[allow(clippy::await_holding_lock)] // Deliberate offline regression: that lane must not be used.
async fn live_control_keeps_unsupported_result_while_native_lane_is_busy() {
    let temp = tempfile::tempdir().unwrap();
    let principal = Principal {
        journal_identity: None,
        certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
        observe: true,
        input: true,
    };
    let runtime = Runtime::new(Config {
        version: 1,
        device_id: "device".into(),
        bind: "127.0.0.1:0".parse().unwrap(),
        identity: rwi_link_trust::IdentityPaths::in_profile(temp.path()),
        principals: vec![principal.clone()],
        state_root: temp.path().join("state"),
        platform: crate::config::PlatformPolicy::Fixture,
    })
    .unwrap();
    let opened = wire(&runtime, &principal, None, Call::OpenContext).await;
    let token = opened.data["context"].as_str().unwrap();
    let args = json!({"target":opened.data["targets"][0]["target"],"action":"status"});
    let idle = invoke(&runtime, &principal, token, "computer.live", args.clone()).await;
    assert_eq!(idle.data["code"], "capability_unavailable");
    let context = runtime.context(principal.identity(), Some(token)).unwrap();
    let native_permit = context.gate.clone().try_acquire_owned().unwrap();
    // The worker cannot acquire this lock, even if it accidentally bypasses only the gate.
    let held_executor = context.executor.lock().unwrap();
    let busy = invoke(&runtime, &principal, token, "computer.live", args).await;
    drop(held_executor);
    drop(native_permit);
    println!(
        "idle={} busy={} no_input=true",
        idle.data["code"], busy.data["code"]
    );
    assert_eq!(
        busy.data["code"], idle.data["code"],
        "Live control is blocked by the input lane"
    );
}
