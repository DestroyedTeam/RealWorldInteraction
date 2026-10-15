//! A browser read never opens a Native context or acquires a desktop lease.
use super::*;
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use rwi_link_trust::IdentityPaths;
    use rwi_native_link::device::PairedDevice;
    use std::os::unix::fs::DirBuilderExt;
    #[tokio::test]
    async fn f15_browser_content_snapshot_gateway_no_native_context_and_standard_image() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().canonicalize().unwrap().join("private");
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
        let calls = Arc::new(Mutex::new(Vec::new()));
        let seen = calls.clone();
        let (stop, rx) = tokio::sync::watch::channel(false);
        let server = tokio::spawn(rwi_native_link::device::serve(
            listener,
            tls,
            move |_, request: Request| {
                let calls = seen.clone();
                async move {
                    calls.lock().unwrap().push(request.clone());
                    let result = match request.call {
                        Call::Describe => DeviceResult::value(
                            json!({"availability":"unavailable","native_context":null,"capabilities":[{"id":"browser.snapshot","implemented":true,"authorized":true,"available":true}]}),
                        ),
                        Call::BrowserSnapshot { arguments } => {
                            let mut r = DeviceResult::value(
                                json!({"session_ref":arguments.session_ref,"source":"offline_transport_fake","input_authority":false}),
                            );
                            r.images.push(device::DeviceImage {
                                mime: "image/png".into(),
                                base64: "offline_fake".into(),
                            });
                            r
                        }
                        _ => DeviceResult::error(
                            "unexpected_native_call",
                            "No Native context, input, lifecycle create or lease.",
                        ),
                    };
                    device::Reply {
                        version: device::PROTOCOL_VERSION,
                        id: request.id,
                        device: request.device,
                        instance: "offline-agent".into(),
                        result,
                    }
                }
            },
            rx,
        ));
        let g = Gateway::new(Ok(AdapterProfile {
            devices: vec![PairedDevice {
                device_id: "owned".into(),
                endpoint,
                identity: IdentityPaths::in_profile(&root.join("trust/model")),
                agent_pin: pairing.agent.certificate_sha256,
            }],
        }));
        let r = g
            .invoke(
                "browser.snapshot",
                json!({"device_id":"owned","session_ref":"browser:one"}),
                None,
            )
            .await;
        assert!(!r.is_error, "{r:?}");
        assert_eq!(r.images.len(), 1);
        assert_eq!(r.data["input_authority"], false);
        let m = serde_json::to_value(mcp_result(r)).unwrap();
        assert_eq!(m["content"][1]["type"], "image");
        assert_eq!(m["content"][1]["mimeType"], "image/png");
        {
            let c = calls.lock().unwrap();
            assert_eq!(c.len(), 2);
            assert!(matches!(c[0].call, Call::Describe));
            assert!(matches!(c[1].call, Call::BrowserSnapshot { .. }));
            assert!(c.iter().all(|c| c.context.is_none()));
            assert!(g.routes["owned"].context.try_lock().unwrap().is_none());
        }
        stop.send_replace(true);
        server.await.unwrap().unwrap();
    }
}
impl Gateway {
    pub(super) async fn invoke_browser_snapshot(
        &self,
        id: &str,
        args: Value,
        context: Option<RequestContext<RoleServer>>,
    ) -> DeviceResult {
        let arguments: device::browser::SnapshotArgs =
            serde_json::from_value(args).expect("validated");
        let mut shutdown = self.shutdown.subscribe();
        let cancelled = async {
            if let Some(c) = context {
                c.ct.cancelled().await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        tokio::pin!(cancelled);
        if *shutdown.borrow() {
            return DeviceResult::error(
                "host_disconnected",
                "No browser observation was forwarded.",
            );
        }
        let describe = tokio::select! {biased;
            _=shutdown.changed()=>return DeviceResult::error("host_disconnected","No browser observation was forwarded."),
            _=&mut cancelled=>return DeviceResult::error("operation_cancelled_before_dispatch","No browser observation was forwarded."),
            r=self.request(id,None,Call::Describe)=>r,
        };
        let description = match describe {
            Ok(r) => r,
            Err(e) => return e,
        };
        let available = description.result.data["capabilities"]
            .as_array()
            .and_then(|a| a.iter().find(|c| c["id"] == "browser.snapshot"))
            .is_some_and(|c| {
                c["implemented"] == true && c["available"] == true && c["authorized"] == true
            });
        if !available {
            return DeviceResult::error(
                "browser_snapshot_capability_unavailable",
                "This device does not advertise an authorized browser content read. No Native/old-protocol fallback was sent.",
            );
        }
        let result = tokio::select! {biased;
            _=shutdown.changed()=>Err(DeviceResult::error("browser_snapshot_unknown","Host disconnected; no input or automatic replay.")),
            _=&mut cancelled=>Err(DeviceResult::error("browser_snapshot_unknown","Read cancelled; no input or automatic replay.")),
            r=self.request(id,None,Call::BrowserSnapshot{arguments})=>r.map(|r|r.result),
        };
        let mut r = result.unwrap_or_else(|e| e);
        r.data["device_id"] = json!(id);
        r.data["automatic_replay"] = json!(false);
        r
    }
}
