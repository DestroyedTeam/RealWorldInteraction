//! Deterministic held replies test device-scoped retirement without OS/network faults.
use super::*;

async fn late_return(corrupt: bool) {
    let (case, listener) = Case::new().await;
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let (notice, held) = (entered.clone(), release.clone());
    let _server = case.serve(listener, move |_, r| {
        let (notice, held) = (notice.clone(), held.clone());
        async move {
            let late = r.id == "late";
            let invalid = r.id == "invalidate" || late && corrupt;
            let new = r.id.starts_with("new");
            if late {
                notice.notify_one();
                held.notified().await;
            }
            let mut r = reply(r);
            if invalid {
                r.id = "wrong-request".into();
            }
            if new {
                r.instance = "new-agent".into();
            }
            r
        }
    });
    let client = case.client();
    client
        .call(&request("warm-work", Call::OpenContext))
        .await
        .unwrap();
    let copy = client.clone();
    let late = tokio::spawn(async move { copy.call(&request("late", Call::OpenContext)).await });
    entered.notified().await;
    client
        .call(&request("other-work", Call::OpenContext))
        .await
        .unwrap();
    client
        .call(&request("warm-control", Call::Describe))
        .await
        .unwrap();
    assert_eq!(client.shared.work.idle.lock().unwrap().len(), 1);
    let retired = client.shared.checkout(&client.shared.control, true).1;
    assert!(
        client
            .call(&request("invalidate", Call::Describe))
            .await
            .is_err()
    );
    assert!(!client.shared.is_current(&retired));
    assert!(client.shared.work.idle.lock().unwrap().is_empty());
    assert!(client.shared.control.idle.lock().unwrap().is_empty());
    client
        .call(&request("new-query", Call::Describe))
        .await
        .unwrap();
    let current = client.shared.checkout(&client.shared.control, true).1;
    release.notify_one();
    let result = late.await.unwrap();
    assert_eq!(result.is_err(), corrupt);
    assert!(client.shared.is_current(&current));
    assert!(client.shared.work.idle.lock().unwrap().is_empty());
    assert_eq!(client.shared.control.idle.lock().unwrap().len(), 1);
    client
        .call(&request("new-query-again", Call::Describe))
        .await
        .unwrap();
    let s = client.transport_stats();
    assert_eq!(s.authenticated_connections, 4);
    assert_eq!(s.pool_invalidations, 1);
    assert_eq!(s.describe_reconnect_attempts, 0);
    assert_eq!(s.retired_returns_discarded, u64::from(!corrupt));
}

#[tokio::test]
async fn late_valid_reply_cannot_repopulate_either_lane_after_retirement() {
    late_return(false).await;
}

#[tokio::test]
async fn late_failure_cannot_retire_a_new_generation() {
    late_return(true).await;
}

#[tokio::test]
async fn new_authenticated_stream_instance_retires_old_idle_sibling() {
    let (case, listener) = Case::new().await;
    let _server = case.serve(listener, |_, r| async {
        let new = r.id != "old";
        let mut r = reply(r);
        if new {
            r.instance = "new-agent".into();
        }
        r
    });
    let client = case.client();
    let origin = client.shared.checkout(&client.shared.control, true).1;
    client.call(&request("old", Call::Describe)).await.unwrap();
    assert!(client.shared.send_permitted(&origin));
    client
        .call(&request("new-work", Call::OpenContext))
        .await
        .unwrap();
    assert!(!client.shared.send_permitted(&origin));
    assert!(client.shared.control.idle.lock().unwrap().is_empty());
    assert_eq!(client.shared.work.idle.lock().unwrap().len(), 1);
    client
        .call(&request("new-control", Call::Describe))
        .await
        .unwrap();
    let s = client.transport_stats();
    assert_eq!(s.authenticated_connections, 3);
    assert_eq!(s.reused_checkouts, 0);
    assert_eq!(s.pool_invalidations, 1);
    assert_eq!(s.describe_reconnect_attempts, 0);
}

#[tokio::test]
async fn retiring_before_the_first_write_is_not_sent_or_automatically_retried() {
    let (case, listener) = Case::new().await;
    let _server = case.serve(listener, |_, _| async {
        panic!("retired request reached peer")
    });
    let client = case.client();
    let token = client.shared.checkout(&client.shared.control, true).1;
    let mut connection = client.connect(token.clone()).await.unwrap();
    client.shared.retire(&token);
    let request = request("retired", Call::Describe);
    let bytes = serde_json::to_vec(&request).unwrap();
    let Err(error) = client.exchange(&mut connection, &request, &bytes).await else {
        panic!("retired stream sent")
    };
    assert_eq!(error.error.to_string(), "device_pool_retired_not_sent");
    assert!(!error.disconnected);
    assert_eq!(client.transport_stats().request_writes_started, 0);
    assert_eq!(client.transport_stats().describe_reconnect_attempts, 0);
}

#[tokio::test]
async fn retirement_is_local_to_the_affected_paired_device_client() {
    let (a, listener_a) = Case::new().await;
    let (b, listener_b) = Case::new().await;
    let _server_a = a.serve(listener_a, |_, r| async { reply(r) });
    let _server_b = b.serve(listener_b, |_, r| async { reply(r) });
    let (a, b) = (a.client(), b.client());
    a.call(&request("a", Call::Describe)).await.unwrap();
    b.call(&request("b", Call::Describe)).await.unwrap();
    let token = a.shared.checkout(&a.shared.control, true).1;
    a.shared.retire(&token);
    b.call(&request("b-again", Call::Describe)).await.unwrap();
    assert_eq!(a.transport_stats().pool_invalidations, 1);
    assert_eq!(b.transport_stats().pool_invalidations, 0);
    assert_eq!(b.transport_stats().authenticated_connections, 1);
    assert_eq!(b.transport_stats().reused_checkouts, 1);
}
