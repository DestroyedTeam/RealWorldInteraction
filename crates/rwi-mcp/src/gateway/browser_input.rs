//! Explicit browser input forwards once, without a Native context or desktop lease.
use super::*;
impl Gateway {
    pub(super) async fn invoke_browser_input(
        &self,
        id: &str,
        args: Value,
        context: Option<RequestContext<RoleServer>>,
    ) -> DeviceResult {
        let arguments: device::browser::ActArgs = serde_json::from_value(args).expect("validated");
        let request_id = arguments.request_id.clone();
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
            return DeviceResult::error("host_disconnected", "No page input was forwarded.");
        }
        let describe = tokio::select! {biased;
            _=shutdown.changed()=>return DeviceResult::error("host_disconnected","No page input was forwarded."),
            _=&mut cancelled=>return DeviceResult::error("operation_cancelled_before_dispatch","No page input was forwarded."),
            r=self.request(id,None,Call::Describe)=>r,
        };
        let description = match describe {
            Ok(r) => r,
            Err(e) => return e,
        };
        if !description.result.data["capabilities"]
            .as_array()
            .and_then(|a| a.iter().find(|c| c["id"] == "browser.act"))
            .is_some_and(|c| {
                c["implemented"] == true && c["available"] == true && c["authorized"] == true
            })
        {
            return DeviceResult::error(
                "browser_input_capability_unavailable",
                "No authorized page input capability; no Native or old-protocol fallback.",
            );
        }
        // Cancellation after forwarding cannot retract device work. Preserve the original
        // durable request_id and report uncertainty, never resend or activate a window.
        let result = tokio::select! {biased;
            _=shutdown.changed()=>Err(DeviceResult::error("browser_result_unknown","Host disconnected; query ORIGINAL browser.session request_id. Never replay.")),
            _=&mut cancelled=>Err(DeviceResult::error("browser_result_unknown","Caller cancelled; query ORIGINAL browser.session request_id. Never replay.")),
            r=self.request(id,None,Call::BrowserAct{arguments})=>r.map(|r|r.result),
        };
        let mut r = result.unwrap_or_else(|e| e);
        r.data["device_id"] = json!(id);
        r.data["request_id"] = json!(request_id);
        r.data["automatic_replay"] = json!(false);
        r
    }
}
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use rwi_link_trust::IdentityPaths;
    use rwi_native_link::device::PairedDevice;
    use std::{
        os::unix::fs::DirBuilderExt,
        sync::atomic::{AtomicBool, Ordering},
    };
    #[tokio::test]
    async fn f16_browser_content_input_gateway_uses_no_native_context_or_fallback() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().canonicalize().unwrap().join("private");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let pair = rwi_link_trust::provision(&root.join("trust")).unwrap();
        let tls = IdentityPaths::in_profile(&root.join("trust/agent"))
            .load()
            .unwrap()
            .server_config(std::slice::from_ref(&pair.model.certificate_sha256))
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let seen = calls.clone();
        let available = Arc::new(AtomicBool::new(true));
        let cap = available.clone();
        let (stop, rx) = tokio::sync::watch::channel(false);
        let server = tokio::spawn(rwi_native_link::device::serve(
            listener,
            tls,
            move |_, q: Request| {
                let calls = seen.clone();
                let cap = cap.clone();
                async move {
                    calls.lock().unwrap().push(q.clone());
                    let result = match q.call {
                        Call::Describe => DeviceResult::value(
                            json!({"availability":"unavailable","native_context":null,"capabilities":[{"id":"browser.act","implemented":true,"authorized":cap.load(Ordering::SeqCst),"available":cap.load(Ordering::SeqCst)}]}),
                        ),
                        Call::BrowserAct { arguments } => DeviceResult::value(
                            json!({"request_id":arguments.request_id,"state":"offline_fake","goal_verified":false,"native_input":false}),
                        ),
                        _ => DeviceResult::error(
                            "unexpected_native_call",
                            "No Native context, lease, activation or fallback.",
                        ),
                    };
                    device::Reply {
                        version: device::PROTOCOL_VERSION,
                        id: q.id,
                        device: q.device,
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
                agent_pin: pair.agent.certificate_sha256,
            }],
        }));
        let a = json!({"device_id":"owned","session_ref":"browser:one","request_id":"once","observation_ref":"browser-observation:one","element_ref":"e1","action":"hover"});
        let r = g.invoke("browser.act", a.clone(), None).await;
        assert!(!r.is_error, "{r:?}");
        assert_eq!(r.data["goal_verified"], false);
        {
            let c = calls.lock().unwrap();
            assert_eq!(c.len(), 2);
            assert!(matches!(c[0].call, Call::Describe));
            assert!(matches!(c[1].call, Call::BrowserAct { .. }));
            assert!(c.iter().all(|q| q.context.is_none()));
        }
        available.store(false, Ordering::SeqCst);
        assert_eq!(
            g.invoke("browser.act", a, None).await.data["code"],
            "browser_input_capability_unavailable"
        );
        {
            let c = calls.lock().unwrap();
            assert_eq!(c.len(), 3);
            assert!(matches!(c[2].call, Call::Describe));
        }
        assert!(g.routes["owned"].context.try_lock().unwrap().is_none());
        assert!(!super::super::invocation::browser_control_request(
            "browser.act",
            None
        ));
        stop.send_replace(true);
        server.await.unwrap().unwrap();
    }
}
