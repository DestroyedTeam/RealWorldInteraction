//! EOF is a sticky admission latch, not merely a notification to current calls.
pub(super) struct Stop(pub tokio::sync::watch::Sender<bool>);
impl Drop for Stop {
    fn drop(&mut self) {
        // send() loses the value when there is no subscriber. Buffered calls
        // may subscribe only after EOF, so retain the closed state unconditionally.
        self.0.send_replace(true);
    }
}

#[cfg(test)]
#[test]
fn closed_state_survives_without_receivers() {
    let (sender, receiver) = tokio::sync::watch::channel(false);
    drop(receiver);
    drop(Stop(sender.clone()));
    assert!(*sender.subscribe().borrow());
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::gateway::{Connection, Gateway};
    use rcu_core::device::{Call, DeviceResult, Reply};
    use rcu_link_trust::IdentityPaths;
    use rcu_native_link::device::{AdapterProfile, PairedDevice};
    use serde_json::json;
    use std::{
        os::unix::fs::DirBuilderExt,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    #[tokio::test]
    async fn eof_without_subscriber_rejects_buffered_and_future_invokes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap().join("private");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let pairing = rcu_link_trust::provision(&root.join("trust")).unwrap();
        let tls = IdentityPaths::in_profile(&root.join("trust/agent"))
            .load()
            .unwrap()
            .server_config(std::slice::from_ref(&pairing.model.certificate_sha256))
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let profile = AdapterProfile {
            devices: vec![PairedDevice {
                device_id: "device".into(),
                endpoint: listener.local_addr().unwrap(),
                identity: IdentityPaths::in_profile(&root.join("trust/model")),
                agent_pin: pairing.agent.certificate_sha256,
            }],
        };
        let invokes = Arc::new(AtomicUsize::new(0));
        let seen = invokes.clone();
        let (stop, rx) = tokio::sync::watch::channel(false);
        let server = tokio::spawn(rcu_native_link::device::serve(
            listener,
            tls,
            move |_peer, request| {
                if matches!(request.call, Call::Prepare { .. } | Call::Execute { .. }) {
                    seen.fetch_add(1, Ordering::SeqCst);
                }
                async move {
                    Reply {
                        version: rcu_core::device::PROTOCOL_VERSION,
                        id: request.id,
                        device: request.device,
                        instance: "synthetic-peer".into(),
                        result: DeviceResult::value(json!({"synthetic_rpc":true})),
                    }
                }
            },
            rx,
        ));
        let gateway = Gateway::new(Ok(profile));
        gateway.remember("device", &json!({"target":"target"}));
        *gateway.routes["device"].context.lock().await = Some(Connection {
            token: "context".into(),
            instance: "synthetic-peer".into(),
            targets: json!([]),
        });
        assert_eq!(gateway.shutdown.receiver_count(), 0);
        // Construct a queued future but deliberately do not poll/subscribe yet.
        let action = json!({"target":"target","frame":"frame","element":"element","lease":"lease","request_id":"never-forwarded","intent":"click"});
        let queued = gateway.invoke("computer.act_checked", action.clone(), None);
        drop(Stop(gateway.shutdown.clone()));
        assert_eq!(gateway.shutdown.receiver_count(), 0);
        assert!(*gateway.shutdown.borrow());
        assert_eq!(queued.await.data["code"], "host_disconnected");
        // Even a syntactically valid action must stop at admission, before
        // resolving its device/lease or forwarding anything to a native agent.
        assert_eq!(
            gateway
                .invoke("computer.act_checked", action, None)
                .await
                .data["code"],
            "host_disconnected"
        );
        assert_eq!(gateway.shutdown.receiver_count(), 0);
        drop(Stop(gateway.shutdown.clone()));
        assert!(*gateway.shutdown.subscribe().borrow());
        assert_eq!(
            invokes.load(Ordering::SeqCst),
            0,
            "neither the buffered nor later Invoke may reach the authenticated device peer"
        );
        let _ = stop.send(true);
        server.await.unwrap().unwrap();
    }
}
