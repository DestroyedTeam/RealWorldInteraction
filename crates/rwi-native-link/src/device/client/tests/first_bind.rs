//! Cold/rebuilt pools: the first validated instance wins even if an older peer
//! returns a first reply or handshake failure later. All peers are owned mTLS.
use super::*;

async fn seed_then_retire(client: &Client, rebuilt: bool) {
    if rebuilt {
        client.call(&request("seed", Call::Describe)).await.unwrap();
        let token = client.shared.checkout(&client.shared.control, true).1;
        client.shared.retire(&token);
    }
    assert!(client.shared.control.idle.lock().unwrap().is_empty());
    assert!(client.shared.work.idle.lock().unwrap().is_empty());
}

async fn late_first_reply(rebuilt: bool, corrupt: bool) {
    let (case, listener) = Case::new().await;
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let (notice, held) = (entered.clone(), release.clone());
    let _server = case.serve(listener, move |_, r| {
        let (notice, held) = (notice.clone(), held.clone());
        async move {
            let late = r.id == "late-first-A";
            if late {
                notice.notify_one();
                held.notified().await;
            }
            let mut r = reply(r);
            r.instance = if late { "agent-A" } else { "agent-B" }.into();
            if late && corrupt {
                r.id = "wrong-request".into();
            }
            r
        }
    });
    let client = case.client();
    seed_then_retire(&client, rebuilt).await;
    let unbound = client.shared.checkout(&client.shared.control, true).1;
    let copy = client.clone();
    let late =
        tokio::spawn(async move { copy.call(&request("late-first-A", Call::Describe)).await });
    entered.notified().await;
    let winner = client
        .call(&request("first-B", Call::Describe))
        .await
        .unwrap();
    assert_eq!(winner.instance, "agent-B");
    let bound = client.shared.checkout(&client.shared.control, true).1;
    assert!(!Arc::ptr_eq(&unbound, &bound));
    assert!(client.shared.send_permitted(&unbound));
    release.notify_one();
    let old = late.await.unwrap();
    if corrupt {
        assert_eq!(old.unwrap_err().to_string(), "device_reply_binding");
    } else {
        // Preserve the original request's response/effect evidence even though
        // this connection can no longer participate in the current pool.
        let old = old.unwrap();
        assert_eq!(old.id, "late-first-A");
        assert_eq!(old.instance, "agent-A");
    }
    assert!(client.shared.is_current(&bound));
    // Either the invalid reply or the explicit A/B instance conflict revokes
    // the provisional lineage, never the B pool's current token.
    assert!(!client.shared.send_permitted(&unbound));
    {
        let idle = client.shared.control.idle.lock().unwrap();
        assert_eq!(idle.len(), 1);
        assert_eq!(idle[0].instance.as_deref(), Some("agent-B"));
    }
    client
        .call(&request("reuse-B", Call::Describe))
        .await
        .unwrap();
    let s = client.transport_stats();
    assert_eq!(s.authenticated_connections, 2 + u64::from(rebuilt));
    assert_eq!(s.request_writes_started, 3 + u64::from(rebuilt));
    assert_eq!(s.pool_invalidations, u64::from(rebuilt));
    assert_eq!(s.retired_returns_discarded, u64::from(!corrupt));
    assert_eq!(s.reused_checkouts, 1);
    assert_eq!(s.describe_reconnect_attempts, 0);
}

#[tokio::test]
async fn cold_late_first_success_cannot_replace_the_first_bound_instance() {
    late_first_reply(false, false).await;
}
#[tokio::test]
async fn rebuilt_late_first_success_cannot_replace_the_first_bound_instance() {
    late_first_reply(true, false).await;
}
#[tokio::test]
async fn cold_late_first_error_cannot_retire_the_first_bound_instance() {
    late_first_reply(false, true).await;
}
#[tokio::test]
async fn rebuilt_late_first_error_cannot_retire_the_first_bound_instance() {
    late_first_reply(true, true).await;
}

async fn late_handshake_failure(rebuilt: bool) {
    let (case, listener) = Case::new().await;
    let mut tls = (*case.server_tls).clone();
    tls.alpn_protocols = vec![KEEPALIVE_ALPN.to_vec()];
    let tls = Arc::new(tls);
    let (entered, started) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let seed = if rebuilt {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut seed =
                BufReader::new(TlsAcceptor::from(tls.clone()).accept(tcp).await.unwrap());
            let r = raw_request(&mut seed).await;
            assert_eq!(r.id, "seed");
            write(&mut seed, &reply(r), RESPONSE_CAP).await.unwrap();
            Some(seed)
        } else {
            None
        };
        // A is connected but its authentication is deliberately held. B then
        // completes mTLS and wins the previously unbound generation.
        let (old_tcp, _) = listener.accept().await.unwrap();
        entered.send(()).unwrap();
        let (tcp, _) = listener.accept().await.unwrap();
        let mut winner = BufReader::new(TlsAcceptor::from(tls).accept(tcp).await.unwrap());
        let mut r = reply(raw_request(&mut winner).await);
        r.instance = "agent-B".into();
        write(&mut winner, &r, RESPONSE_CAP).await.unwrap();
        released.await.unwrap();
        drop(old_tcp);
        let mut r = reply(raw_request(&mut winner).await);
        assert_eq!(r.id, "reuse-B");
        r.instance = "agent-B".into();
        write(&mut winner, &r, RESPONSE_CAP).await.unwrap();
        drop(seed);
    });
    let client = case.client();
    seed_then_retire(&client, rebuilt).await;
    let unbound = client.shared.checkout(&client.shared.control, true).1;
    let copy = client.clone();
    let late =
        tokio::spawn(async move { copy.call(&request("old-handshake", Call::Describe)).await });
    started.await.unwrap();
    client
        .call(&request("first-B", Call::Describe))
        .await
        .unwrap();
    let bound = client.shared.checkout(&client.shared.control, true).1;
    assert!(!Arc::ptr_eq(&unbound, &bound));
    assert!(client.shared.send_permitted(&unbound));
    release.send(()).unwrap();
    assert!(late.await.unwrap().is_err());
    assert!(client.shared.is_current(&bound));
    assert!(!client.shared.send_permitted(&unbound));
    assert_eq!(client.shared.control.idle.lock().unwrap().len(), 1);
    client
        .call(&request("reuse-B", Call::Describe))
        .await
        .unwrap();
    server.await.unwrap();
    let s = client.transport_stats();
    assert_eq!(s.authenticated_connections, 1 + u64::from(rebuilt));
    assert_eq!(s.request_writes_started, 2 + u64::from(rebuilt));
    assert_eq!(s.pool_invalidations, u64::from(rebuilt));
    assert_eq!(s.reused_checkouts, 1);
    assert_eq!(s.describe_reconnect_attempts, 0);
}

#[tokio::test]
async fn cold_late_handshake_failure_cannot_retire_the_first_bound_instance() {
    late_handshake_failure(false).await;
}
#[tokio::test]
async fn rebuilt_late_handshake_failure_cannot_retire_the_first_bound_instance() {
    late_handshake_failure(true).await;
}

async fn late_handshake_success(rebuilt: bool, retired: bool) {
    let (case, listener) = Case::new().await;
    let mut tls = (*case.server_tls).clone();
    tls.alpn_protocols = vec![KEEPALIVE_ALPN.to_vec()];
    let tls = Arc::new(tls);
    let (entered, started) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let _seed = if rebuilt {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut seed =
                BufReader::new(TlsAcceptor::from(tls.clone()).accept(tcp).await.unwrap());
            let r = raw_request(&mut seed).await;
            assert_eq!(r.id, "seed");
            write(&mut seed, &reply(r), RESPONSE_CAP).await.unwrap();
            Some(seed)
        } else {
            None
        };
        let (old_tcp, _) = listener.accept().await.unwrap();
        entered.send(()).unwrap();
        let (tcp, _) = listener.accept().await.unwrap();
        let mut winner = BufReader::new(TlsAcceptor::from(tls.clone()).accept(tcp).await.unwrap());
        let r = raw_request(&mut winner).await;
        assert_eq!(r.id, "first-B");
        assert!(matches!(r.call, Call::OpenContext));
        write(&mut winner, &reply(r), RESPONSE_CAP).await.unwrap();
        released.await.unwrap();
        // A's handshake succeeds only AFTER B's first reply bound the pool.
        let mut old = BufReader::new(TlsAcceptor::from(tls).accept(old_tcp).await.unwrap());
        if retired {
            assert!(!matches!(
                crate::wire::read_frame(&mut old, REQUEST_CAP).await,
                Ok(Some(_))
            ));
        } else {
            let r = raw_request(&mut old).await;
            assert_eq!(r.id, "late-handshake-A");
            assert!(matches!(r.call, Call::OpenContext));
            write(&mut old, &reply(r), RESPONSE_CAP).await.unwrap();
            // The provisional A connection must not displace the B pool entry.
            let r = raw_request(&mut winner).await;
            assert_eq!(r.id, "reuse-B");
            write(&mut winner, &reply(r), RESPONSE_CAP).await.unwrap();
        }
    });
    let client = case.client();
    seed_then_retire(&client, rebuilt).await;
    let unbound = client.shared.checkout(&client.shared.work, true).1;
    let copy = client.clone();
    let late = tokio::spawn(async move {
        copy.call(&request("late-handshake-A", Call::OpenContext))
            .await
    });
    started.await.unwrap();
    client
        .call(&request("first-B", Call::OpenContext))
        .await
        .unwrap();
    let bound = client.shared.checkout(&client.shared.work, true).1;
    assert!(!Arc::ptr_eq(&unbound, &bound));
    assert!(!client.shared.is_current(&unbound));
    assert!(client.shared.send_permitted(&unbound));
    assert_eq!(
        client.transport_stats().request_writes_started,
        1 + u64::from(rebuilt)
    );
    if retired {
        client.shared.retire(&bound);
        assert!(!client.shared.send_permitted(&unbound));
    }
    release.send(()).unwrap();
    let result = late.await.unwrap();
    if retired {
        assert_eq!(
            result.unwrap_err().to_string(),
            "device_pool_retired_not_sent"
        );
        assert!(client.shared.work.idle.lock().unwrap().is_empty());
    } else {
        let r = result.unwrap();
        assert_eq!(r.id, "late-handshake-A");
        assert_eq!(r.instance, "agent");
        assert!(client.shared.is_current(&bound));
        assert!(client.shared.send_permitted(&unbound));
        assert_eq!(client.shared.work.idle.lock().unwrap().len(), 1);
        client
            .call(&request("reuse-B", Call::OpenContext))
            .await
            .unwrap();
    }
    server.await.unwrap();
    let s = client.transport_stats();
    assert_eq!(s.authenticated_connections, 2 + u64::from(rebuilt));
    assert_eq!(
        s.request_writes_started,
        (if retired { 1 } else { 3 }) + u64::from(rebuilt)
    );
    assert_eq!(
        s.pool_invalidations,
        u64::from(rebuilt) + u64::from(retired)
    );
    assert_eq!(s.retired_returns_discarded, u64::from(!retired));
    assert_eq!(s.reused_checkouts, u64::from(!retired));
    assert_eq!(s.describe_reconnect_attempts, 0);
}

#[tokio::test]
async fn cold_late_successful_handshake_sends_its_original_open_context_once() {
    late_handshake_success(false, false).await;
}
#[tokio::test]
async fn rebuilt_late_successful_handshake_sends_its_original_open_context_once() {
    late_handshake_success(true, false).await;
}
#[tokio::test]
async fn real_retirement_revokes_the_provisional_first_bind_send_permission() {
    for rebuilt in [false, true] {
        late_handshake_success(rebuilt, true).await;
    }
}
