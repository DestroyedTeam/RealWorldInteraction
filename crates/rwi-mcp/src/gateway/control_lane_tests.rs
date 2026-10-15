//! Authenticated loopback component tests only. No native provider or input.
//! A slow work-lane context open must not gate full-reference control requests.
use super::*;
use rwi_link_trust::IdentityPaths;
use rwi_native_link::device::PairedDevice;
use std::{
    os::unix::fs::DirBuilderExt,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

struct DescribeRace {
    stale_valid: bool,
    describes: AtomicUsize,
    opens: AtomicUsize,
    fresh_describes: AtomicUsize,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

struct Peer {
    _directory: tempfile::TempDir,
    gateway: Gateway,
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    calls: Arc<Mutex<Vec<Call>>>,
    server: tokio::task::JoinHandle<io::Result<()>>,
    stop: tokio::sync::watch::Sender<bool>,
    describe_race: Option<Arc<DescribeRace>>,
}

impl Peer {
    async fn new() -> Self {
        Self::new_inner(None, None).await
    }

    async fn new_describe_race(stale_valid: bool) -> Self {
        Self::new_inner(
            Some(Arc::new(DescribeRace {
                stale_valid,
                describes: AtomicUsize::new(0),
                opens: AtomicUsize::new(0),
                fresh_describes: AtomicUsize::new(0),
                entered: tokio::sync::Notify::new(),
                release: tokio::sync::Notify::new(),
            })),
            None,
        )
        .await
    }

    async fn new_fresh_reply(result: DeviceResult, instance: &str) -> Self {
        Self::new_inner(None, Some((result, instance.into()))).await
    }

    async fn new_inner(
        describe_race: Option<Arc<DescribeRace>>,
        fresh_reply: Option<(DeviceResult, String)>,
    ) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap().join("private");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let pairing = rwi_link_trust::provision(&root.join("trust")).unwrap();
        let tls = IdentityPaths::in_profile(&root.join("trust/agent"))
            .load()
            .unwrap()
            .server_config(std::slice::from_ref(&pairing.model.certificate_sha256))
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let calls = Arc::new(Mutex::new(Vec::new()));
        let (stop, shutdown) = tokio::sync::watch::channel(false);
        let server = {
            let entered = entered.clone();
            let release = release.clone();
            let calls = calls.clone();
            let describe_race = describe_race.clone();
            let fresh_reply = fresh_reply.clone();
            tokio::spawn(rwi_native_link::device::serve(
                listener,
                tls,
                move |_, request: Request| {
                    let entered = entered.clone();
                    let release = release.clone();
                    let calls = calls.clone();
                    let describe_race = describe_race.clone();
                    let fresh_reply = fresh_reply.clone();
                    async move {
                        calls.lock().unwrap().push(request.call.clone());
                        let fresh_context = request.context.as_deref().filter(|token| {
                            *token == "context" || token.starts_with("new-context-")
                        });
                        let reply_instance =
                            if matches!(&request.call, Call::Describe) && fresh_context.is_some() {
                                fresh_reply
                                    .as_ref()
                                    .map(|reply| reply.1.as_str())
                                    .unwrap_or("instance")
                            } else {
                                "instance"
                            }
                            .to_owned();
                        let result = match request.call {
                            Call::Describe if fresh_context.is_some() => {
                                let token = fresh_context.unwrap();
                                if let Some(race) = &describe_race {
                                    race.fresh_describes.fetch_add(1, Ordering::SeqCst);
                                }
                                fresh_reply
                                    .as_ref()
                                    .map(|reply| reply.0.clone())
                                    .unwrap_or_else(|| {
                                        DeviceResult::value(json!({
                                            "availability":"available", "context_valid":true,
                                            "targets":[{"target": token.strip_prefix("new-context-")
                                                .map(|count| format!("new-target-{count}"))
                                                .unwrap_or_else(|| "fresh-target".into())}],
                                            "sample_context":token,
                                            "native_context":{"epoch":token},
                                            "runtime_status":{"pid":77},
                                            "input_domain_fenced":true
                                        }))
                                    })
                            }
                            Call::Describe if describe_race.is_some() => {
                                let race = describe_race.as_ref().unwrap();
                                assert_eq!(request.context.as_deref(), Some("old-context"));
                                match race.describes.fetch_add(1, Ordering::SeqCst) {
                                    0 => {
                                        race.entered.notify_one();
                                        race.release.notified().await;
                                        DeviceResult::value(json!({
                                            "availability":"available",
                                            "context_valid":race.stale_valid,
                                            "targets":[{"target":"stale-target"}]
                                        }))
                                    }
                                    1 => DeviceResult::value(json!({
                                        "availability":"available", "context_valid":false,
                                        "targets":[]
                                    })),
                                    _ => panic!("The old epoch Describe must not retry or replay"),
                                }
                            }
                            Call::Describe => DeviceResult::value(json!({
                                "availability":"available", "context_valid":false,
                                "targets":[]
                            })),
                            Call::OpenContext if describe_race.is_some() => {
                                let race = describe_race.as_ref().unwrap();
                                let count = race.opens.fetch_add(1, Ordering::SeqCst) + 1;
                                DeviceResult::value(json!({
                                    "context":format!("new-context-{count}"),
                                    "targets":[{"target":format!("new-target-{count}")}]
                                }))
                            }
                            Call::OpenContext => {
                                entered.notify_one();
                                release.notified().await;
                                DeviceResult::value(json!({"context":"context", "targets":[]}))
                            }
                            Call::Operation {
                                selector: device::OperationSelector::Execution { operation_ref },
                            } if operation_ref.execution_nonce == "missing-context" => {
                                assert!(
                                    request.context.is_none(),
                                    "Control uses its full reference"
                                );
                                DeviceResult::error("context_unknown", "Synthetic old reference")
                            }
                            Call::Operation { .. } | Call::Cancel { .. } => {
                                assert!(
                                    request.context.is_none(),
                                    "Control uses its full reference"
                                );
                                DeviceResult::value(json!({"control_received":true}))
                            }
                            _ => DeviceResult::error(
                                "fixture_unexpected_call",
                                "No native/input calls are allowed.",
                            ),
                        };
                        device::Reply {
                            version: device::PROTOCOL_VERSION,
                            id: request.id,
                            device: request.device,
                            instance: reply_instance,
                            result,
                        }
                    }
                },
                shutdown,
            ))
        };
        let gateway = Gateway::new(Ok(AdapterProfile {
            devices: vec![PairedDevice {
                device_id: "device".into(),
                endpoint,
                identity: IdentityPaths::in_profile(&root.join("trust/model")),
                agent_pin: pairing.agent.certificate_sha256,
            }],
        }));
        Self {
            _directory: directory,
            gateway,
            entered,
            release,
            calls,
            server,
            stop,
            describe_race,
        }
    }

    async fn hold_open(&self) -> tokio::task::JoinHandle<DeviceResult> {
        let gateway = self.gateway.clone();
        let open = tokio::spawn(async move { gateway.inspect_device("device").await });
        tokio::time::timeout(Duration::from_secs(5), self.entered.notified())
            .await
            .expect("Authenticated OpenContext reached the fixture");
        assert!(self.gateway.routes["device"].context.try_lock().is_err());
        open
    }

    async fn finish(self, open: tokio::task::JoinHandle<DeviceResult>) {
        self.release.notify_one();
        let opened = tokio::time::timeout(Duration::from_secs(5), open)
            .await
            .unwrap()
            .unwrap();
        assert!(!opened.is_error);
        self.stop().await;
    }

    async fn stop(self) {
        assert!(
            !self
                .calls
                .lock()
                .unwrap()
                .iter()
                .any(|call| { matches!(call, Call::Prepare { .. } | Call::Execute { .. }) })
        );
        self.stop.send_replace(true);
        self.server.await.unwrap().unwrap();
    }
}

fn reference() -> Value {
    json!({"ref_version":1, "device_id":"device", "agent_instance":"instance",
        "context_generation":"context", "execution_nonce":"original-execution"})
}

async fn assert_late_describe_preserves_new_context(stale_valid: bool) {
    let peer = Peer::new_describe_race(stale_valid).await;
    let race = peer.describe_race.as_ref().unwrap().clone();
    *peer.gateway.routes["device"].context.lock().await = Some(Connection {
        token: "old-context".into(),
        instance: "instance".into(),
        targets: json!([{"target":"old-target"}]),
    });
    let gateway = peer.gateway.clone();
    let first = tokio::spawn(async move { gateway.inspect_device("device").await });
    tokio::time::timeout(Duration::from_secs(5), race.entered.notified())
        .await
        .expect("First Describe reached its authenticated barrier");
    let second = tokio::time::timeout(
        Duration::from_secs(5),
        peer.gateway.inspect_device("device"),
    )
    .await
    .unwrap();
    assert!(!second.is_error);
    assert_eq!(second.data["targets"], json!([{"target":"new-target-1"}]));
    race.release.notify_one();
    let late = tokio::time::timeout(Duration::from_secs(5), first)
        .await
        .unwrap()
        .unwrap();
    assert!(!late.is_error);
    let (token, targets) = {
        let context = peer.gateway.routes["device"].context.lock().await;
        let context = context.as_ref().unwrap();
        (context.token.clone(), context.targets.clone())
    };
    let opens = race.opens.load(Ordering::SeqCst);
    let describes = race.describes.load(Ordering::SeqCst);
    let fresh_describes = race.fresh_describes.load(Ordering::SeqCst);
    peer.stop().await;
    assert_eq!(describes, 2, "The original two descriptions never replay");
    assert_eq!(
        fresh_describes, 2,
        "Opener and late inspector each read the new token once"
    );
    for result in [&second, &late] {
        assert_eq!(result.data["context_valid"], true);
        assert_eq!(result.data["sample_context"], "new-context-1");
        assert_eq!(result.data["native_context"]["epoch"], "new-context-1");
        assert_eq!(result.data["runtime_status"]["pid"], 77);
        assert_eq!(result.data["input_domain_fenced"], true);
    }
    assert_eq!(
        opens, 1,
        "A late old Describe must not reopen the new context"
    );
    assert_eq!(token, "new-context-1");
    assert_eq!(targets, json!([{"target":"new-target-1"}]));
    assert_eq!(late.data["targets"], targets);
}

#[tokio::test]
async fn late_invalid_describe_preserves_new_context() {
    assert_late_describe_preserves_new_context(false).await;
}

#[tokio::test]
async fn late_valid_describe_preserves_new_context() {
    assert_late_describe_preserves_new_context(true).await;
}

#[tokio::test]
async fn old_control_context_error_does_not_clear_current_context() {
    let peer = Peer::new().await;
    *peer.gateway.routes["device"].context.lock().await = Some(Connection {
        token: "new-context".into(),
        instance: "instance".into(),
        targets: json!([]),
    });
    let mut old_reference = reference();
    old_reference["execution_nonce"] = json!("missing-context");
    let result = peer
        .gateway
        .invoke(
            "computer.operation",
            json!({"selector":{"kind":"execution", "operation_ref":old_reference}}),
            None,
        )
        .await;
    assert_eq!(result.data["code"], "context_unknown");
    assert_eq!(
        peer.gateway.routes["device"]
            .context
            .lock()
            .await
            .as_ref()
            .unwrap()
            .token,
        "new-context"
    );
    peer.stop().await;
}

#[tokio::test]
async fn status_and_cancel_reach_control_lane_while_open_context_is_pending() {
    let peer = Peer::new().await;
    let open = peer.hold_open().await;
    let result = tokio::time::timeout(Duration::from_secs(2), async {
        tokio::join!(
            peer.gateway.invoke(
                "computer.operation",
                json!({"selector":{"kind":"execution", "operation_ref":reference()}}),
                None,
            ),
            peer.gateway.invoke(
                "computer.cancel",
                json!({"operation_ref":reference()}),
                None
            ),
        )
    })
    .await;
    peer.finish(open).await;
    let (status, cancel) = result.expect("Full-reference controls must bypass OpenContext lock");
    assert_eq!(status.data["control_received"], true);
    assert_eq!(cancel.data["control_received"], true);
}

#[tokio::test]
async fn host_eof_interrupts_ordinary_call_waiting_for_context_lock() {
    let peer = Peer::new().await;
    let open = peer.hold_open().await;
    peer.gateway
        .remember("device", &json!({"target":"fixture-target"}));
    let result = {
        let invocation = peer.gateway.invoke(
            "computer.observe",
            json!({"target":"fixture-target", "inline_image":false}),
            None,
        );
        tokio::pin!(invocation);
        // Poll before EOF so this cannot pass merely because the call saw an
        // already-closed Host at entry. The fixture owns the context lock.
        std::future::poll_fn(|context| {
            assert!(std::future::Future::poll(invocation.as_mut(), context).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        peer.gateway.shutdown.send_replace(true);
        tokio::time::timeout(Duration::from_secs(2), &mut invocation).await
    };
    peer.finish(open).await;
    assert_eq!(
        result.expect("EOF must stop context-lock wait").data["code"],
        "host_disconnected"
    );
}

#[tokio::test]
async fn targets_fresh_context_first_open_returns_actual_describe_and_fence() {
    let peer = Peer::new().await;
    let open = peer.hold_open().await;
    peer.release.notify_one();
    let result = tokio::time::timeout(Duration::from_secs(5), open)
        .await
        .unwrap()
        .unwrap();
    assert!(!result.is_error);
    assert_eq!(result.data["context_valid"], true);
    assert_eq!(result.data["sample_context"], "context");
    assert_eq!(result.data["native_context"]["epoch"], "context");
    assert_eq!(result.data["runtime_status"]["pid"], 77);
    assert_eq!(result.data["input_domain_fenced"], true);
    assert_eq!(result.data["targets"], json!([{"target":"fresh-target"}]));
    let calls = peer.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 3);
    assert!(matches!(calls[0], Call::Describe));
    assert!(matches!(calls[1], Call::OpenContext));
    assert!(matches!(calls[2], Call::Describe));
    peer.stop().await;
}

#[tokio::test]
async fn targets_fresh_context_valid_fast_path_has_no_extra_rpc() {
    let peer = Peer::new().await;
    *peer.gateway.routes["device"].context.lock().await = Some(Connection {
        token: "context".into(),
        instance: "instance".into(),
        targets: json!([{"target":"cached-target"}]),
    });
    let result = peer.gateway.inspect_device("device").await;
    assert!(!result.is_error);
    assert_eq!(result.data["sample_context"], "context");
    assert_eq!(result.data["targets"], json!([{"target":"fresh-target"}]));
    let calls = peer.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert!(matches!(calls[0], Call::Describe));
    peer.stop().await;
}

#[tokio::test]
async fn targets_fresh_context_late_old_describe_cannot_mix_epochs() {
    assert_late_describe_preserves_new_context(false).await;
    assert_late_describe_preserves_new_context(true).await;
}

#[tokio::test]
async fn targets_fresh_context_failure_and_restart_do_not_offer_or_reopen() {
    let cases = [
        (
            DeviceResult::value(json!({"availability":"available", "context_valid":false,
            "targets":[{"target":"unvalidated"}]})),
            "instance",
        ),
        (
            DeviceResult::value(json!({"availability":"unavailable", "context_valid":true,
            "targets":[{"target":"unvalidated"}], "reason":"native_permission_changed"})),
            "instance",
        ),
        (
            DeviceResult::value(json!({"availability":"available", "context_valid":true,
            "targets":null})),
            "instance",
        ),
        (
            DeviceResult::error("permission_denied", "Actual fresh rejection"),
            "instance",
        ),
        (
            DeviceResult::value(json!({"availability":"available", "context_valid":true,
            "targets":[{"target":"unvalidated"}]})),
            "new-agent-instance",
        ),
    ];
    for (reply, instance) in cases {
        let peer = Peer::new_fresh_reply(reply.clone(), instance).await;
        let open = peer.hold_open().await;
        peer.release.notify_one();
        let result = tokio::time::timeout(Duration::from_secs(5), open)
            .await
            .unwrap()
            .unwrap();
        assert!(result.is_error || result.data["availability"] == "unavailable");
        assert!(result.data.get("targets").is_none());
        if reply.data["availability"] == "unavailable" {
            assert_eq!(result.data["reason"], "native_permission_changed");
            assert_eq!(result.data["context_valid"], true);
        }
        if reply.is_error {
            assert_eq!(result.data["code"], "permission_denied");
        }
        assert!(peer.gateway.routes["device"].context.lock().await.is_none());
        let calls = peer.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 3);
        assert_eq!(
            calls
                .iter()
                .filter(|call| matches!(call, Call::OpenContext))
                .count(),
            1
        );
        peer.stop().await;
    }
}
