//! Component/authenticated-RPC test: Gateway + Core ownership + synthetic held scope.
//! Does not traverse a real MCP stdio subprocess or production Runtime.dispatch.
//! This is not a native OS hang or owned-process-isolation acceptance test.
use super::*;
use rcu_core::{
    device::OperationRef,
    operation::{Operation, Registry},
};
use rcu_link_trust::IdentityPaths;
use rcu_native_link::device::PairedDevice;
use std::{
    os::unix::fs::DirBuilderExt,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[tokio::test]
async fn running_native_scope_remains_queryable_after_cancel_timeout_and_disconnect() {
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
    let endpoint = listener.local_addr().unwrap();
    let reference = OperationRef {
        ref_version: 1,
        device_id: "device".into(),
        agent_instance: "synthetic-instance".into(),
        context_generation: "synthetic-context".into(),
        execution_nonce: uuid::Uuid::new_v4().to_string(),
    };
    let live: Arc<Mutex<Option<Operation>>> = Default::default();
    let scope_started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let drop_cancel = Arc::new(AtomicBool::new(true));
    let executions = Arc::new(AtomicUsize::new(0));
    let (_stop, rx) = tokio::sync::watch::channel(false);
    let server = {
        let reference = reference.clone();
        let live = live.clone();
        let started = scope_started.clone();
        let release = release.clone();
        let drop_cancel = drop_cancel.clone();
        let executions = executions.clone();
        tokio::spawn(rcu_native_link::device::serve(
            listener,
            tls,
            move |_, r: Request| {
                let reference = reference.clone();
                let live = live.clone();
                let started = started.clone();
                let release = release.clone();
                let drop_cancel = drop_cancel.clone();
                let executions = executions.clone();
                async move {
                    let result = match r.call {
                        Call::Prepare { .. } => {
                            DeviceResult::value(json!({"accepted":true,"operation_ref":reference}))
                        }
                        Call::Execute { operation_ref } if operation_ref == reference => {
                            assert_eq!(
                                executions.fetch_add(1, Ordering::SeqCst),
                                0,
                                "Execute must not replay"
                            );
                            let op = Registry::default()
                                .begin(Some(reference.execution_nonce.clone()))
                                .unwrap();
                            let scope = rcu_core::operation::run(&op, || {
                                rcu_core::operation::native_call("synthetic_held_provider").unwrap()
                            });
                            *live.lock().unwrap() = Some(op.clone());
                            started.notify_one();
                            release.notified().await;
                            drop(scope);
                            op.finish();
                            DeviceResult::value(op.status())
                        }
                        Call::Cancel { operation_ref } if operation_ref == reference => {
                            if drop_cancel.load(Ordering::Acquire) {
                                std::future::pending::<()>().await;
                            }
                            let op = live.lock().unwrap().clone().unwrap();
                            op.cancel();
                            DeviceResult::value(op.status())
                        }
                        Call::Operation {
                            selector: device::OperationSelector::Execution { operation_ref },
                        } if operation_ref == reference => {
                            DeviceResult::value(live.lock().unwrap().as_ref().unwrap().status())
                        }
                        _ => DeviceResult::error(
                            "synthetic_request_rejected",
                            "Test peer only accepts its exact reference.",
                        ),
                    };
                    device::Reply {
                        version: device::PROTOCOL_VERSION,
                        id: r.id,
                        device: r.device,
                        instance: reference.agent_instance,
                        result,
                    }
                }
            },
            rx,
        ))
    };
    let profile = AdapterProfile {
        devices: vec![PairedDevice {
            device_id: "device".into(),
            endpoint,
            identity: IdentityPaths::in_profile(&root.join("trust/model")),
            agent_pin: pairing.agent.certificate_sha256,
        }],
    };
    let g = Gateway::new(Ok(profile.clone()));
    g.remember("device", &json!({"target":"synthetic-target"}));
    *g.routes["device"].context.lock().await = Some(Connection {
        token: reference.context_generation.clone(),
        instance: reference.agent_instance.clone(),
        targets: json!([]),
    });
    let invocation = {
        let g = g.clone();
        tokio::spawn(async move {
            g.invoke(
                "computer.observe",
                json!({"target":"synthetic-target","inline_image":false}),
                None,
            )
            .await
        })
    };
    scope_started.notified().await;
    assert!(
        live.lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .status()
            .native_call_pending
    );
    g.shutdown.send_replace(true); // Host EOF after Execute actually entered the held scope.
    let timeout = invocation.await.unwrap();
    assert_eq!(timeout.data["code"], "cancellation_delivery_unconfirmed");
    assert_eq!(timeout.data["operation_ref"], json!(reference));
    assert_eq!(timeout.data["cancellation_delivery_confirmed"], false);
    // A fresh Host can use the only returned locator while original native work is pending.
    let control = Gateway::new(Ok(profile.clone()));
    let query =
        json!({"selector":{"kind":"execution","operation_ref":timeout.data["operation_ref"]}});
    let pending = control
        .invoke("computer.operation", query.clone(), None)
        .await;
    assert_eq!(pending.data["native_call_pending"], true);
    assert_eq!(pending.data["terminal"], false);
    assert_eq!(pending.data["resources_released"], false);
    // Fail only this controller's route: the device and native owner stay alive.
    let dead_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut disconnected_profile = profile;
    disconnected_profile.devices[0].endpoint = dead_listener.local_addr().unwrap();
    drop(dead_listener);
    let disconnected = cancellation::CancelDispatch::new(
        Gateway::new(Ok(disconnected_profile)),
        reference.clone(),
    )
    .request()
    .await;
    assert_eq!(disconnected.data["code"], "device_connection_failed");
    assert_eq!(disconnected.data["operation_ref"], json!(reference));
    assert_eq!(
        control
            .invoke("computer.operation", query.clone(), None)
            .await
            .data["native_call_pending"],
        true
    );
    drop_cancel.store(false, Ordering::Release);
    let cancelled = control
        .invoke(
            "computer.cancel",
            json!({"operation_ref":disconnected.data["operation_ref"]}),
            None,
        )
        .await;
    assert_eq!(cancelled.data["cancel_requested"], true);
    assert_eq!(cancelled.data["native_call_pending"], true);
    release.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !live.lock().unwrap().as_ref().unwrap().status().terminal {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        control.invoke("computer.operation", query, None).await.data["resources_released"],
        true
    );
    assert_eq!(executions.load(Ordering::SeqCst), 1);
    server.abort();
    let _ = server.await;
}
