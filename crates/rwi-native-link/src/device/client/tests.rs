//! Owned loopback mTLS, not native input or real-app acceptance.
use super::*;
use rwi_core::device::{DeviceResult, OperationRef};
use tokio::io::AsyncBufReadExt;
mod first_bind;
mod generations;
mod reconnect;

struct Case {
    _dir: tempfile::TempDir,
    device: PairedDevice,
    server_tls: Arc<rustls::ServerConfig>,
    client_tls: Arc<rustls::ClientConfig>,
}
impl Case {
    async fn new() -> (Self, TcpListener) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap().join("trust");
        let manifest = rwi_link_trust::provision(&root).unwrap();
        let identity = IdentityPaths::in_profile(&root.join("model"));
        let server_tls = IdentityPaths::in_profile(&root.join("agent"))
            .load()
            .unwrap()
            .server_config(&[manifest.model.certificate_sha256])
            .unwrap();
        let client_tls = identity
            .load()
            .unwrap()
            .client_config(&manifest.agent.certificate_sha256)
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let device = PairedDevice {
            device_id: "device".into(),
            endpoint: listener.local_addr().unwrap(),
            identity,
            agent_pin: manifest.agent.certificate_sha256,
        };
        (
            Self {
                _dir: dir,
                device,
                server_tls,
                client_tls,
            },
            listener,
        )
    }
    fn client(&self) -> Client {
        Client::new(self.device.clone(), self.device.identity.load().unwrap()).unwrap()
    }
    fn serve<H, F>(&self, listener: TcpListener, handler: H) -> Server
    where
        H: Fn(PeerPin, Request) -> F + Send + Sync + 'static,
        F: std::future::Future<Output = Reply> + Send + 'static,
    {
        let (stop, rx) = tokio::sync::watch::channel(false);
        Server {
            _stop: stop,
            task: tokio::spawn(serve(listener, self.server_tls.clone(), handler, rx)),
        }
    }
}
struct Server {
    _stop: tokio::sync::watch::Sender<bool>,
    task: tokio::task::JoinHandle<io::Result<()>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn request(id: &str, call: Call) -> Request {
    Request {
        version: PROTOCOL_VERSION,
        id: id.into(),
        device: "device".into(),
        context: None,
        call,
    }
}
fn reply(r: Request) -> Reply {
    Reply {
        version: PROTOCOL_VERSION,
        id: r.id,
        device: r.device,
        instance: "agent".into(),
        result: DeviceResult::value(serde_json::json!({"ok":true})),
    }
}
fn operation() -> OperationRef {
    OperationRef {
        ref_version: 1,
        device_id: "device".into(),
        agent_instance: "agent".into(),
        context_generation: "context".into(),
        execution_nonce: "execution".into(),
    }
}
async fn raw_request(
    stream: &mut BufReader<tokio_rustls::server::TlsStream<TcpStream>>,
) -> Request {
    let mut line = String::new();
    stream.read_line(&mut line).await.unwrap();
    serde_json::from_str(&line).unwrap()
}

#[tokio::test]
async fn sequential_clones_reuse_one_authenticated_stream_per_lane() {
    let (case, listener) = Case::new().await;
    let seen = Arc::new(AtomicU64::new(0));
    let calls = seen.clone();
    let _server = case.serve(listener, move |_, r| {
        calls.fetch_add(1, Ordering::Relaxed);
        async { reply(r) }
    });
    let client = case.client();
    for n in 0..12 {
        client
            .clone()
            .call(&request(&format!("work-{n}"), Call::OpenContext))
            .await
            .unwrap();
        client
            .clone()
            .call(&request(&format!("control-{n}"), Call::Describe))
            .await
            .unwrap();
    }
    let s = client.transport_stats();
    assert_eq!(s.authenticated_connections, 2);
    assert_eq!(s.keepalive_connections, 2);
    assert_eq!(s.reused_checkouts, 22);
    assert_eq!(s.replies_validated, 24);
    assert_eq!(seen.load(Ordering::Relaxed), 24);
}

#[tokio::test]
async fn saturated_work_lane_does_not_block_cancel() {
    let (case, listener) = Case::new().await;
    let (entered, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let release = Arc::new(Semaphore::new(0));
    let held = release.clone();
    let _server = case.serve(listener, move |_, r| {
        let entered = entered.clone();
        let held = held.clone();
        async move {
            if matches!(r.call, Call::Execute { .. }) {
                entered.send(()).unwrap();
                held.acquire().await.unwrap().forget();
            }
            reply(r)
        }
    });
    let client = case.client();
    let mut work = tokio::task::JoinSet::new();
    for n in 0..4 {
        let client = client.clone();
        work.spawn(async move {
            client
                .call(&request(
                    &format!("execute-{n}"),
                    Call::Execute {
                        operation_ref: operation(),
                    },
                ))
                .await
        });
    }
    for _ in 0..4 {
        tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .unwrap()
            .unwrap();
    }
    let cancel = tokio::time::timeout(
        Duration::from_secs(2),
        client.call(&request(
            "cancel",
            Call::Cancel {
                operation_ref: operation(),
            },
        )),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(cancel.id, "cancel");
    assert_eq!(client.transport_stats().authenticated_connections, 5);
    release.add_permits(4);
    while let Some(done) = work.join_next().await {
        done.unwrap().unwrap();
    }
}

#[tokio::test]
async fn dropping_inflight_call_discards_stream_without_replay() {
    let (case, listener) = Case::new().await;
    let (entered, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let release = Arc::new(Semaphore::new(0));
    let held = release.clone();
    let _server = case.serve(listener, move |_, r| {
        let entered = entered.clone();
        let held = held.clone();
        async move {
            entered.send(r.id.clone()).unwrap();
            if r.id == "uncertain" {
                held.acquire().await.unwrap().forget();
            }
            reply(r)
        }
    });
    let client = case.client();
    let original = client.clone();
    let task = tokio::spawn(async move {
        original
            .call(&request("uncertain", Call::OpenContext))
            .await
    });
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(3), rx.recv())
            .await
            .unwrap()
            .unwrap(),
        "uncertain"
    );
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    client
        .call(&request("new-action", Call::OpenContext))
        .await
        .unwrap();
    assert_eq!(rx.recv().await.unwrap(), "new-action");
    release.add_permits(1);
    assert!(rx.try_recv().is_err());
    let s = client.transport_stats();
    assert_eq!(s.authenticated_connections, 2);
    assert_eq!(s.reused_checkouts, 0);
    assert_eq!(s.request_writes_started, 2);
    assert_eq!(s.replies_validated, 1);
}

#[tokio::test]
async fn idle_client_expiry_opens_fresh_before_sending() {
    let (case, listener) = Case::new().await;
    let _server = case.serve(listener, |_, r| async { reply(r) });
    let client = case.client();
    client
        .call(&request("first", Call::Describe))
        .await
        .unwrap();
    client.shared.control.idle.lock().unwrap()[0].idle_since = Instant::now() - CLIENT_IDLE_TIMEOUT;
    client
        .call(&request("second", Call::Describe))
        .await
        .unwrap();
    assert_eq!(client.transport_stats().authenticated_connections, 2);
    assert_eq!(client.transport_stats().reused_checkouts, 0);
}

#[tokio::test]
async fn instance_change_discards_connection() {
    let (case, listener) = Case::new().await;
    let _server = case.serve(listener, |_, r| async {
        let changed = r.id != "first";
        let mut r = reply(r);
        if changed {
            r.instance = "different-agent".into();
        }
        r
    });
    let client = case.client();
    client
        .call(&request("first", Call::Describe))
        .await
        .unwrap();
    assert_eq!(
        client
            .call(&request("second", Call::Describe))
            .await
            .unwrap_err()
            .to_string(),
        "device_reply_binding"
    );
    client
        .call(&request("third", Call::Describe))
        .await
        .unwrap();
    assert_eq!(client.transport_stats().authenticated_connections, 2);
    assert_eq!(client.transport_stats().request_writes_started, 3);
}

#[tokio::test]
async fn new_client_uses_one_shot_when_server_has_no_alpn() {
    let (case, listener) = Case::new().await;
    let tls = case.server_tls.clone();
    let task = tokio::spawn(async move {
        for _ in 0..3 {
            let (tcp, _) = listener.accept().await.unwrap();
            let stream = TlsAcceptor::from(tls.clone()).accept(tcp).await.unwrap();
            assert_eq!(stream.get_ref().1.alpn_protocol(), None);
            let mut stream = BufReader::new(stream);
            let r = raw_request(&mut stream).await;
            write(&mut stream, &reply(r), RESPONSE_CAP).await.unwrap();
            stream.shutdown().await.unwrap();
        }
    });
    let client = case.client();
    for n in 0..3 {
        client
            .call(&request(&format!("old-{n}"), Call::Describe))
            .await
            .unwrap();
    }
    task.await.unwrap();
    assert_eq!(client.transport_stats().authenticated_connections, 3);
    assert_eq!(client.transport_stats().keepalive_connections, 0);
    assert_eq!(client.transport_stats().reused_checkouts, 0);
}

#[tokio::test]
async fn old_client_gets_one_reply_then_clean_eof_from_new_server() {
    let (case, listener) = Case::new().await;
    let _server = case.serve(listener, |_, r| async { reply(r) });
    let tcp = TcpStream::connect(case.device.endpoint).await.unwrap();
    let stream = TlsConnector::from(case.client_tls.clone())
        .connect(
            rwi_link_trust::server_name(rwi_link_trust::SERVER_DNS_NAME).unwrap(),
            tcp,
        )
        .await
        .unwrap();
    assert_eq!(stream.get_ref().1.alpn_protocol(), None);
    let mut stream = BufReader::new(stream);
    write(
        &mut stream,
        &request("old-client", Call::Describe),
        REQUEST_CAP,
    )
    .await
    .unwrap();
    assert!(
        crate::wire::read_frame(&mut stream, RESPONSE_CAP)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        tokio::time::timeout(
            Duration::from_secs(2),
            crate::wire::read_frame(&mut stream, RESPONSE_CAP)
        )
        .await
        .unwrap()
        .unwrap()
        .is_none()
    );
}

#[tokio::test]
async fn lost_reply_after_negotiation_is_not_retried_or_reused() {
    let (case, listener) = Case::new().await;
    let mut tls = (*case.server_tls).clone();
    tls.alpn_protocols = vec![KEEPALIVE_ALPN.to_vec()];
    let task = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut stream =
            BufReader::new(TlsAcceptor::from(Arc::new(tls)).accept(tcp).await.unwrap());
        assert_eq!(raw_request(&mut stream).await.id, "uncertain");
        stream.shutdown().await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
    });
    let client = case.client();
    assert!(
        client
            .call(&request("uncertain", Call::Describe))
            .await
            .is_err()
    );
    assert!(client.shared.control.idle.lock().unwrap().is_empty());
    assert_eq!(client.transport_stats().request_writes_started, 1);
    assert_eq!(client.transport_stats().replies_validated, 0);
    task.await.unwrap();
}

#[tokio::test]
async fn rejected_oversize_request_does_not_connect() {
    let (case, _listener) = Case::new().await;
    let client = case.client();
    let r = request(
        "large",
        Call::Prepare {
            tool: "computer.act".into(),
            arguments: serde_json::json!({"value":"x".repeat(REQUEST_CAP)}),
        },
    );
    assert_eq!(
        client.call(&r).await.unwrap_err().to_string(),
        "device_frame_limit"
    );
    assert_eq!(client.transport_stats().authenticated_connections, 0);
}

#[tokio::test]
async fn negotiated_connection_closed_after_success_never_replays_next_call() {
    let (case, listener) = Case::new().await;
    let mut tls = (*case.server_tls).clone();
    tls.alpn_protocols = vec![KEEPALIVE_ALPN.to_vec()];
    let task = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut stream =
            BufReader::new(TlsAcceptor::from(Arc::new(tls)).accept(tcp).await.unwrap());
        let first = raw_request(&mut stream).await;
        write(&mut stream, &reply(first), RESPONSE_CAP)
            .await
            .unwrap();
        // The peer goes away despite negotiation. CloseContext, unlike the
        // narrowly read-only Describe exception, still must never be retried.
        stream.shutdown().await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
    });
    let client = case.client();
    client
        .call(&request("first", Call::Describe))
        .await
        .unwrap();
    assert!(
        client
            .call(&request("second", Call::CloseContext))
            .await
            .is_err()
    );
    assert_eq!(client.transport_stats().authenticated_connections, 1);
    assert_eq!(client.transport_stats().reused_checkouts, 1);
    assert!(client.shared.control.idle.lock().unwrap().is_empty());
    task.await.unwrap();
}

#[tokio::test]
async fn concurrent_replies_are_exclusive_and_pool_stays_bounded() {
    let (s, idle) = parallel_replies(false).await;
    // A cold first-bind barrier can discard the other three initial work
    // connections. Their replacements are fresh handshakes, not request replay.
    assert!((4..=7).contains(&s.authenticated_connections));
    assert_eq!(s.replies_validated, 30);
    assert_eq!(s.request_writes_started, 30);
    assert_eq!(s.reused_checkouts, 30 - s.authenticated_connections);
    assert!(s.retired_returns_discarded <= 3);
    assert!((1..=4).contains(&idle));
    assert_eq!(s.describe_reconnect_attempts, 0);
}

#[tokio::test]
async fn warm_concurrent_replies_keep_exact_four_connection_reuse_counts() {
    let (s, idle) = parallel_replies(true).await;
    assert_eq!(s.authenticated_connections, 4);
    assert_eq!(s.replies_validated, 31);
    assert_eq!(s.request_writes_started, 31);
    assert_eq!(s.reused_checkouts, 27);
    assert_eq!(s.retired_returns_discarded, 0);
    assert_eq!(idle, 4);
}

async fn parallel_replies(warm: bool) -> (TransportStats, usize) {
    let (case, listener) = Case::new().await;
    let _server = case.serve(listener, |_, r| async {
        tokio::time::sleep(Duration::from_millis(if r.id.ends_with('0') {
            10
        } else {
            1
        }))
        .await;
        reply(r)
    });
    let client = case.client();
    if warm {
        client
            .call(&request("warm", Call::OpenContext))
            .await
            .unwrap();
    }
    let mut calls = tokio::task::JoinSet::new();
    for n in 0..30 {
        let client = client.clone();
        calls.spawn(async move {
            let id = format!("parallel-{n}");
            assert_eq!(
                client
                    .call(&request(&id, Call::OpenContext))
                    .await
                    .unwrap()
                    .id,
                id
            );
        });
    }
    while let Some(done) = calls.join_next().await {
        done.unwrap();
    }
    let idle = client.shared.work.idle.lock().unwrap().len();
    (client.transport_stats(), idle)
}

#[tokio::test]
async fn wrong_reply_id_is_discarded_without_native_retry() {
    let (case, listener) = Case::new().await;
    let _server = case.serve(listener, |_, r| async {
        let mut r = reply(r);
        r.id = "someone-elses-request".into();
        r
    });
    let client = case.client();
    assert_eq!(
        client
            .call(&request("mine", Call::Describe))
            .await
            .unwrap_err()
            .to_string(),
        "device_reply_binding"
    );
    assert!(client.shared.control.idle.lock().unwrap().is_empty());
    assert_eq!(client.transport_stats().request_writes_started, 1);
}
