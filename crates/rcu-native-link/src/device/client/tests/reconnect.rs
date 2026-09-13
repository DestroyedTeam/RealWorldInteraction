//! Owned mTLS peers exercise the narrowly bounded Describe exception only.
use super::*;
use rcu_core::device::OperationSelector;

fn keepalive(case: &Case) -> Arc<rustls::ServerConfig> {
    let mut tls = (*case.server_tls).clone();
    tls.alpn_protocols = vec![KEEPALIVE_ALPN.to_vec()];
    Arc::new(tls)
}
async fn accept(
    listener: &TcpListener,
    tls: Arc<rustls::ServerConfig>,
) -> BufReader<tokio_rustls::server::TlsStream<TcpStream>> {
    let (tcp, _) = listener.accept().await.unwrap();
    BufReader::new(TlsAcceptor::from(tls).accept(tcp).await.unwrap())
}
async fn no_more_connections(listener: &TcpListener) {
    assert!(
        tokio::time::timeout(Duration::from_millis(50), listener.accept())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn stale_describe_reauthenticates_once_and_retires_both_lanes() {
    let (case, listener) = Case::new().await;
    let tls = keepalive(&case);
    let task = tokio::spawn(async move {
        let mut old_work = accept(&listener, tls.clone()).await;
        let r = raw_request(&mut old_work).await;
        assert!(matches!(r.call, Call::OpenContext));
        write(&mut old_work, &reply(r), RESPONSE_CAP).await.unwrap();
        let mut control = accept(&listener, tls.clone()).await;
        let r = raw_request(&mut control).await;
        write(&mut control, &reply(r), RESPONSE_CAP).await.unwrap();
        let original = raw_request(&mut control).await;
        assert!(matches!(original.call, Call::Describe));
        assert_eq!(original.context.as_deref(), Some("existing-context"));
        control.shutdown().await.unwrap();
        let mut fresh = accept(&listener, tls.clone()).await;
        let retried = raw_request(&mut fresh).await;
        assert_eq!(
            serde_json::to_value(&original).unwrap(),
            serde_json::to_value(&retried).unwrap()
        );
        let mut r = reply(retried);
        r.instance = "restarted-agent".into();
        write(&mut fresh, &r, RESPONSE_CAP).await.unwrap();
        // A later explicit OpenContext must not consume the still-live old work
        // stream; it is not part of the automatic Describe retry.
        let mut new_work = accept(&listener, tls).await;
        let r = raw_request(&mut new_work).await;
        assert_eq!(r.id, "explicit-open-after-query");
        assert!(matches!(r.call, Call::OpenContext));
        let mut r = reply(r);
        r.instance = "restarted-agent".into();
        write(&mut new_work, &r, RESPONSE_CAP).await.unwrap();
    });
    let client = case.client();
    client
        .call(&request("warm-work", Call::OpenContext))
        .await
        .unwrap();
    client
        .call(&request("warm-control", Call::Describe))
        .await
        .unwrap();
    let mut describe = request("query-after-restart", Call::Describe);
    describe.context = Some("existing-context".into());
    let r = client.call(&describe).await.unwrap();
    assert_eq!(r.instance, "restarted-agent");
    assert!(client.shared.work.idle.lock().unwrap().is_empty());
    client
        .call(&request("explicit-open-after-query", Call::OpenContext))
        .await
        .unwrap();
    task.await.unwrap();
    let s = client.transport_stats();
    assert_eq!(s.authenticated_connections, 4);
    assert_eq!(s.keepalive_connections, 4);
    assert_eq!(s.request_writes_started, 5);
    assert_eq!(s.replies_validated, 4);
    assert_eq!(s.reused_checkouts, 1);
    assert_eq!(s.pool_invalidations, 1);
    assert_eq!(s.describe_reconnect_attempts, 1);
    assert_eq!(s.describe_reconnect_successes, 1);
    let d = s.last_describe_reconnect.unwrap();
    assert_eq!(d.original_failure.message, "device_response_lost");
    assert_eq!(d.outcome, "reply_validated");
    assert!(d.reconnect_failure.is_none());
    assert!(d.context_idle_bookkeeping_possible);
    assert!(!d.native_input_replayed);
}

#[tokio::test]
async fn a_second_disconnect_preserves_both_failures_and_never_retries_again() {
    let (case, listener) = Case::new().await;
    let tls = keepalive(&case);
    let task = tokio::spawn(async move {
        let mut stream = accept(&listener, tls.clone()).await;
        let r = raw_request(&mut stream).await;
        write(&mut stream, &reply(r), RESPONSE_CAP).await.unwrap();
        assert_eq!(raw_request(&mut stream).await.id, "query");
        stream.shutdown().await.unwrap();
        let mut second = accept(&listener, tls).await;
        assert_eq!(raw_request(&mut second).await.id, "query");
        second.shutdown().await.unwrap();
        no_more_connections(&listener).await;
    });
    let client = case.client();
    client.call(&request("warm", Call::Describe)).await.unwrap();
    let error = client
        .call(&request("query", Call::Describe))
        .await
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "device_response_lost; describe_reconnect_failed: device_response_lost; no further retry"
    );
    task.await.unwrap();
    let s = client.transport_stats();
    assert_eq!(s.authenticated_connections, 2);
    assert_eq!(s.request_writes_started, 3);
    assert_eq!(s.pool_invalidations, 2);
    assert_eq!(s.describe_reconnect_attempts, 1);
    assert_eq!(s.describe_reconnect_successes, 0);
    let d = s.last_describe_reconnect.unwrap();
    assert_eq!(d.outcome, "failed");
    assert_eq!(d.original_failure.message, "device_response_lost");
    assert_eq!(d.reconnect_failure.unwrap().message, "device_response_lost");
    assert!(!d.context_idle_bookkeeping_possible);
    assert!(client.shared.control.idle.lock().unwrap().is_empty());
}

#[tokio::test]
async fn every_non_describe_call_remains_single_send_after_reused_stream_loses_reply() {
    for call in [
        Call::OpenContext,
        Call::CloseContext,
        Call::Prepare {
            tool: "computer.act".into(),
            arguments: serde_json::json!({}),
        },
        Call::Execute {
            operation_ref: operation(),
        },
        Call::Cached {
            tool: "computer.lifecycle_receipt".into(),
            arguments: serde_json::json!({}),
        },
        Call::Operation {
            selector: OperationSelector::Execution {
                operation_ref: operation(),
            },
        },
        Call::Operation {
            selector: OperationSelector::Request {
                device_id: "device".into(),
                request_id: "request".into(),
            },
        },
        Call::Cancel {
            operation_ref: operation(),
        },
    ] {
        let (case, listener) = Case::new().await;
        let tls = keepalive(&case);
        let task = tokio::spawn(async move {
            let mut stream = accept(&listener, tls).await;
            let r = raw_request(&mut stream).await;
            write(&mut stream, &reply(r), RESPONSE_CAP).await.unwrap();
            assert_eq!(raw_request(&mut stream).await.id, "uncertain");
            stream.shutdown().await.unwrap();
            no_more_connections(&listener).await;
        });
        let client = case.client();
        client.call(&request("warm", call.clone())).await.unwrap();
        assert!(client.call(&request("uncertain", call)).await.is_err());
        task.await.unwrap();
        let s = client.transport_stats();
        assert_eq!(s.authenticated_connections, 1);
        assert_eq!(s.reused_checkouts, 1);
        assert_eq!(s.request_writes_started, 2);
        assert_eq!(s.replies_validated, 1);
        assert_eq!(s.describe_reconnect_attempts, 0);
        assert!(client.shared.control.idle.lock().unwrap().is_empty());
        assert!(client.shared.work.idle.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn reused_describe_protocol_binding_and_partial_frame_errors_never_retry() {
    for corruption in [
        "json", "array", "partial", "version", "device", "id", "instance",
    ] {
        let (case, listener) = Case::new().await;
        let tls = keepalive(&case);
        let task = tokio::spawn(async move {
            let mut stream = accept(&listener, tls).await;
            let r = raw_request(&mut stream).await;
            write(&mut stream, &reply(r), RESPONSE_CAP).await.unwrap();
            let mut r = reply(raw_request(&mut stream).await);
            let bytes = match corruption {
                "json" => b"{broken}\n".to_vec(),
                "array" => b"[]\n".to_vec(),
                "partial" => b"{\"version\":".to_vec(),
                _ => {
                    match corruption {
                        "version" => r.version += 1,
                        "device" => r.device = "wrong-device".into(),
                        "id" => r.id = "wrong-request".into(),
                        "instance" => r.instance = "changed-within-stream".into(),
                        _ => unreachable!(),
                    }
                    let mut bytes = serde_json::to_vec(&r).unwrap();
                    bytes.push(b'\n');
                    bytes
                }
            };
            stream.write_all(&bytes).await.unwrap();
            stream.flush().await.unwrap();
            stream.shutdown().await.unwrap();
            no_more_connections(&listener).await;
        });
        let client = case.client();
        client.call(&request("warm", Call::Describe)).await.unwrap();
        assert!(
            client
                .call(&request("rejected", Call::Describe))
                .await
                .is_err(),
            "{corruption}"
        );
        task.await.unwrap();
        let s = client.transport_stats();
        assert_eq!(s.authenticated_connections, 1, "{corruption}");
        assert_eq!(s.request_writes_started, 2, "{corruption}");
        assert_eq!(s.describe_reconnect_attempts, 0, "{corruption}");
        assert!(client.shared.control.idle.lock().unwrap().is_empty());
    }
}

#[tokio::test]
async fn reconnect_reauthenticates_and_rejects_a_different_certificate_without_another_attempt() {
    let (case, listener) = Case::new().await;
    let (other, _unused) = Case::new().await;
    let tls = keepalive(&case);
    let untrusted = keepalive(&other);
    let task = tokio::spawn(async move {
        let mut stream = accept(&listener, tls).await;
        let r = raw_request(&mut stream).await;
        write(&mut stream, &reply(r), RESPONSE_CAP).await.unwrap();
        raw_request(&mut stream).await;
        stream.shutdown().await.unwrap();
        let (tcp, _) = listener.accept().await.unwrap();
        assert!(TlsAcceptor::from(untrusted).accept(tcp).await.is_err());
        no_more_connections(&listener).await;
    });
    let client = case.client();
    client.call(&request("warm", Call::Describe)).await.unwrap();
    assert!(
        client
            .call(&request("query", Call::Describe))
            .await
            .is_err()
    );
    task.await.unwrap();
    let s = client.transport_stats();
    assert_eq!(s.authenticated_connections, 1);
    assert_eq!(s.request_writes_started, 2);
    assert_eq!(s.describe_reconnect_attempts, 1);
    assert_eq!(s.describe_reconnect_successes, 0);
    assert_eq!(s.last_describe_reconnect.unwrap().outcome, "failed");
    assert!(client.shared.control.idle.lock().unwrap().is_empty());
}

#[tokio::test]
async fn cancelling_the_one_reconnect_keeps_completion_unobserved_and_drops_its_stream() {
    let (case, listener) = Case::new().await;
    let tls = keepalive(&case);
    let (entered, rx) = tokio::sync::oneshot::channel();
    let (release, wait) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let mut stream = accept(&listener, tls.clone()).await;
        let r = raw_request(&mut stream).await;
        write(&mut stream, &reply(r), RESPONSE_CAP).await.unwrap();
        raw_request(&mut stream).await;
        stream.shutdown().await.unwrap();
        let mut fresh = accept(&listener, tls).await;
        assert_eq!(raw_request(&mut fresh).await.id, "cancelled");
        entered.send(()).unwrap();
        wait.await.unwrap();
        no_more_connections(&listener).await;
    });
    let client = case.client();
    client.call(&request("warm", Call::Describe)).await.unwrap();
    let copy = client.clone();
    let query = tokio::spawn(async move { copy.call(&request("cancelled", Call::Describe)).await });
    rx.await.unwrap();
    query.abort();
    assert!(query.await.unwrap_err().is_cancelled());
    let s = client.transport_stats();
    assert_eq!(s.describe_reconnect_attempts, 1);
    assert_eq!(s.describe_reconnect_successes, 0);
    assert_eq!(
        s.last_describe_reconnect.unwrap().outcome,
        "completion_unobserved"
    );
    assert!(client.shared.control.idle.lock().unwrap().is_empty());
    release.send(()).unwrap();
    task.await.unwrap();
}
