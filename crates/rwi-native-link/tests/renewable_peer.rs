#![cfg(unix)]
//! Actual loopback device RPC; no GUI/input or system trust modifications.
use rwi_core::device::{Call, DeviceResult, PROTOCOL_VERSION, Reply, Request};
use rwi_link_trust::IdentityPaths;
use rwi_native_link::device::{Client, PairedDevice, serve_pinned};
use serde_json::json;

#[tokio::test]
async fn device_rpc_uses_configured_spki_principal_not_rotating_leaf_hash() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("renewable");
    let data = rwi_link_trust::provision_renewable(&root).unwrap();
    let server_identity = IdentityPaths::in_renewable_bundle(&root, "agent")
        .unwrap()
        .load()
        .unwrap();
    let client_paths = IdentityPaths::in_renewable_bundle(&root, "model").unwrap();
    let client_identity = client_paths.load().unwrap();
    let expected = data.peers[2].pin.clone();
    assert_ne!(expected, client_identity.fingerprint());
    let tls = server_identity
        .server_config(std::slice::from_ref(&expected))
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = Client::new(
        PairedDevice {
            device_id: "renewal-fixture".into(),
            endpoint: listener.local_addr().unwrap(),
            identity: client_paths,
            agent_pin: data.peers[0].pin.clone(),
        },
        client_identity,
    )
    .unwrap();
    let (stop, rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(serve_pinned(
        listener,
        tls,
        vec![expected.clone()],
        |peer, request| async move {
            Reply {
                version: PROTOCOL_VERSION,
                id: request.id,
                device: request.device,
                instance: "renewal-fixture-instance".into(),
                result: DeviceResult::value(json!({"principal":peer})),
            }
        },
        rx,
    ));
    for i in 0..2 {
        let request = Request {
            version: PROTOCOL_VERSION,
            id: format!("describe-{i}"),
            device: "renewal-fixture".into(),
            context: None,
            call: Call::Describe,
        };
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), client.call(&request))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.result.data["principal"], expected.as_str());
    }
    assert!(client.transport_stats().reused_checkouts > 0);
    stop.send(true).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}
