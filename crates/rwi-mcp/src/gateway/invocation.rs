//! Prepare/Execute is private transport mechanics, not a model-facing workflow.
use super::*;
pub(super) fn browser_control_request(
    name: &str,
    arguments: Option<&serde_json::Map<String, Value>>,
) -> bool {
    name == "browser.session"
        && matches!(
            arguments
                .and_then(|a| a.get("action"))
                .and_then(Value::as_str),
            Some("query" | "close")
        )
}
impl Gateway {
    pub(super) async fn invoke(
        &self,
        tool: &str,
        args: Value,
        context: Option<RequestContext<RoleServer>>,
    ) -> DeviceResult {
        if tool == "computer.live" {
            return self.host_live(args, context).await;
        }
        let detached = (tool == "computer.detach_window").then(|| args["target"].clone());
        let result = self.invoke_device(tool, args, context).await;
        if let Some(target) = detached
            && !result.is_error
            && result.data["detached"] == true
            && result.data["target"] == target
            && let Some(target) = target.as_str()
        {
            // The native session ended with its backend. Retire only this
            // target's Host presence page; other window viewers remain valid.
            self.presence.remove_target(target);
        }
        result
    }
    /// Internal typed adapters may attach Host-view presence. Public tools and
    /// the JS bridge must go through invoke(), which rejects forged presence.
    pub(super) async fn invoke_device(
        &self,
        tool: &str,
        mut args: Value,
        context: Option<RequestContext<RoleServer>>,
    ) -> DeviceResult {
        let mut shutdown = self.shutdown.subscribe();
        if *shutdown.borrow() {
            return DeviceResult::error(
                "host_disconnected",
                "No operation is forwarded after Host EOF.",
            );
        }
        if context.as_ref().is_some_and(|c| c.ct.is_cancelled()) {
            return DeviceResult::error(
                "operation_cancelled_before_dispatch",
                "Host cancelled before acceptance.",
            );
        }
        numeric_inputs(&mut args);
        if let Err(e) = device::catalog::validate(tool, &args) {
            return DeviceResult::error("invalid_arguments", &e.to_string());
        }
        if tool == "computer.targets" {
            return self.targets(args["device_id"].as_str()).await;
        }
        let id = match self.route_id(tool, &args) {
            Ok(id) => id,
            Err(e) => return e,
        };
        let Some(route) = self.routes.get(&id) else {
            return DeviceResult::error("device_unknown", "Device is not paired.");
        };
        if tool == "browser.session" {
            return self.invoke_browser_session(&id, args, context).await;
        }
        if tool == "browser.snapshot" {
            return self.invoke_browser_snapshot(&id, args, context).await;
        }
        if tool == "browser.act" {
            return self.invoke_browser_input(&id, args, context).await;
        }
        let host_cancelled = async {
            if let Some(context) = context {
                context.ct.cancelled().await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        tokio::pin!(host_cancelled);
        // Exact operation references authenticate control requests independently
        // of the volatile window context. A slow OpenContext must never hold
        // status/cancel behind the work lane's context-initialization lock.
        let context_independent = matches!(tool, "computer.operation" | "computer.cancel");
        let token = if context_independent {
            None
        } else {
            tokio::select! {
                biased;
                _=shutdown.changed()=>return DeviceResult::error("host_disconnected","No request was forwarded while waiting for context."),
                _=&mut host_cancelled=>return DeviceResult::error("operation_cancelled_before_dispatch","No request was forwarded while waiting for context."),
                connection=route.context.lock()=>connection.as_ref().map(|c| c.token.clone()),
            }
        };
        let executable = device::catalog::executable(tool);
        let call = match tool {
            "computer.operation" => Call::Operation {
                selector: serde_json::from_value::<device::api::OperationQueryArgs>(args)
                    .expect("validated")
                    .selector,
            },
            "computer.cancel" => Call::Cancel {
                operation_ref: serde_json::from_value::<device::api::OperationArgs>(args)
                    .expect("validated")
                    .operation_ref,
            },
            _ if executable => Call::Prepare {
                tool: tool.into(),
                arguments: args,
            },
            _ => Call::Cached {
                tool: tool.into(),
                arguments: args,
            },
        };
        // A dropped/failed Prepare may leave an accepted record, but it cannot
        // acquire native/input resources or Execute itself. Never retry it here.
        let response = tokio::select! {
            biased;
            _=shutdown.changed()=>return DeviceResult::error("host_disconnected","No Execute was sent by this call; acceptance may expire on the device."),
            _=&mut host_cancelled=>return DeviceResult::error("operation_cancelled_before_dispatch","No Execute was sent by this call."),
            r=self.request(&id,token.clone(),call)=>r,
        };
        let reply = match response {
            Ok(r) => r,
            Err(e) => return e,
        };
        if !context_independent && reply.result.data["code"] == "context_unknown" {
            // Do not wait behind another discovery or clear a newer context
            // because an old request failed. Discovery owns the next refresh.
            if let Ok(mut current) = route.context.try_lock()
                && current.as_ref().map(|c| &c.token) == token.as_ref()
            {
                *current = None;
            }
        }
        if reply.result.is_error || !executable {
            // Durable history must not populate volatile handle routing.
            if tool != "computer.operation" && tool != "computer.cancel" {
                self.remember(&id, &reply.result.data);
            }
            return reply.result;
        }
        let reference = match serde_json::from_value::<device::OperationRef>(
            reply.result.data["operation_ref"].clone(),
        ) {
            Ok(r)
                if r.valid_shape()
                    && r.device_id == id
                    && r.agent_instance == reply.instance
                    && Some(&r.context_generation) == token.as_ref() =>
            {
                r
            }
            _ => {
                return DeviceResult::error(
                    "acceptance_reference_invalid",
                    "No Execute was sent. Device did not supply a correctly bound execution reference.",
                );
            }
        };
        self.dispatch_accepted(reference, shutdown, host_cancelled)
            .await
    }
    async fn invoke_browser_session(
        &self,
        id: &str,
        args: Value,
        context: Option<RequestContext<RoleServer>>,
    ) -> DeviceResult {
        let arguments: device::browser::SessionArgs =
            serde_json::from_value(args).expect("validated");
        let request_id = arguments.request_id().to_owned();
        let mut shutdown = self.shutdown.subscribe();
        if *shutdown.borrow() {
            return DeviceResult::error(
                "host_disconnected",
                "No browser request forwarded after Host EOF.",
            );
        }
        let cancelled = async {
            if let Some(c) = context {
                c.ct.cancelled().await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        tokio::pin!(cancelled);
        let described = tokio::select! {
            biased;
            _=shutdown.changed()=>return DeviceResult::error("host_disconnected", "No browser request forwarded."),
            _=&mut cancelled=>return DeviceResult::error("operation_cancelled_before_dispatch", "No browser request forwarded."),
            r=self.request(id, None, Call::Describe)=>r,
        };
        let described = match described {
            Ok(r) => r,
            Err(e) => return e,
        };
        let cap = described.result.data["capabilities"]
            .as_array()
            .and_then(|a| a.iter().find(|c| c["id"] == "browser.session"));
        if !cap.is_some_and(|c| {
            c["implemented"] == true && c["available"] == true && c["authorized"] == true
        }) {
            return DeviceResult::error(
                "browser_capability_unavailable",
                "This paired Agent does not advertise an authorized browser-content lifecycle. No native fallback or old protocol call was sent.",
            );
        }
        if *shutdown.borrow() {
            return DeviceResult::error(
                "host_disconnected",
                "No browser mutation forwarded after Host EOF.",
            );
        }
        let result = tokio::select! {
            biased;
            _=shutdown.changed()=>Err(DeviceResult::error("browser_result_unknown", "Host disconnected; query the original browser.session request_id. Do not replay.")),
            _=&mut cancelled=>Err(DeviceResult::error("browser_result_unknown", "Caller cancelled; device lifecycle may finish. Query the original browser.session request_id. Do not replay.")),
            r=self.request(id, None, Call::BrowserSession { arguments })=>r.map(|r| r.result),
        };
        let mut result = result.unwrap_or_else(|e| e);
        result.data["device_id"] = json!(id);
        result.data["request_id"] = json!(request_id);
        result.data["automatic_replay"] = json!(false);
        result
    }
    async fn dispatch_accepted(
        &self,
        reference: device::OperationRef,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
        host_cancelled: impl std::future::Future<Output = ()> + Send,
    ) -> DeviceResult {
        let id = reference.device_id.clone();
        tokio::pin!(host_cancelled);
        let mut cancellation = cancellation::CancelDispatch::new(self.clone(), reference.clone());
        // If Stop raced acceptance, cancellation wins before the Execute future
        // is polled. Device Execute/Cancel then serialize on the same record.
        if *shutdown.borrow() {
            return cancellation.request().await;
        }
        let response = tokio::select! {
            biased;
            _=shutdown.changed()=>return cancellation.request().await,
            _=&mut host_cancelled=>return cancellation.request().await,
            r=self.request(&id,None,Call::Execute{operation_ref:reference.clone()})=>r,
        };
        let reply = match response {
            Ok(r) => r,
            Err(mut e) => {
                e.data["operation_ref"] = json!(reference);
                return e;
            }
        };
        cancellation.disarm();
        let mut result = reply.result;
        if !result.data.is_object() {
            result.data = json!({"value":result.data});
        }
        // Even an agent rejection after acceptance (e.g. context closed before
        // Execute) must retain the caller's exact accepted execution locator.
        result.data["operation_ref"] = json!(reference);
        self.remember(&id, &result.data);
        result
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use rwi_agent::{
        config::{Config, PlatformPolicy, Principal},
        runtime::Runtime,
    };
    use rwi_link_trust::IdentityPaths;
    use rwi_native_link::device::PairedDevice;
    use std::os::unix::fs::DirBuilderExt;
    #[tokio::test]
    async fn accepted_reference_then_ready_stop_never_polls_execute() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap().join("private");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let pairing = rwi_link_trust::provision(&root.join("trust")).unwrap();
        let identity = IdentityPaths::in_profile(&root.join("trust/agent"));
        let tls = identity
            .load()
            .unwrap()
            .server_config(std::slice::from_ref(&pairing.model.certificate_sha256))
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap();
        let runtime = Runtime::new(Config {
            version: 1,
            device_id: "device".into(),
            bind: endpoint,
            identity,
            principals: vec![Principal {
                journal_identity: None,
                certificate: pairing.model.certificate_sha256,
                observe: true,
                input: true,
            }],
            state_root: root.join("state"),
            platform: PlatformPolicy::Fixture,
        })
        .unwrap();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let seen = calls.clone();
        let (stop, rx) = tokio::sync::watch::channel(false);
        let server = tokio::spawn(rwi_native_link::device::serve(
            listener,
            tls,
            move |peer, request: Request| {
                seen.lock().unwrap().push(request.call.clone());
                runtime.clone().handle(peer, request)
            },
            rx,
        ));
        let g = Gateway::new(Ok(AdapterProfile {
            devices: vec![PairedDevice {
                device_id: "device".into(),
                endpoint,
                identity: IdentityPaths::in_profile(&root.join("trust/model")),
                agent_pin: pairing.agent.certificate_sha256,
            }],
        }));
        let opened = g
            .request("device", None, Call::OpenContext)
            .await
            .unwrap()
            .result;
        let token = opened.data["context"].as_str().unwrap();
        for eof in [false, true] {
            let accepted=g.request("device",Some(token.into()),Call::Prepare{tool:"computer.observe".into(),arguments:json!({"target":opened.data["targets"][0]["target"],"inline_image":false})}).await.unwrap().result;
            assert_eq!(accepted.data["native_dispatch_started"], false);
            let reference: device::OperationRef =
                serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
            let (shutdown, rx) = tokio::sync::watch::channel(false);
            if eof {
                shutdown.send_replace(true);
            }
            let result = if eof {
                g.dispatch_accepted(reference.clone(), rx, std::future::pending())
                    .await
            } else {
                g.dispatch_accepted(reference.clone(), rx, std::future::ready(()))
                    .await
            };
            assert_eq!(result.data["code"], "operation_cancel_requested");
            let status = g
                .request(
                    "device",
                    None,
                    Call::Operation {
                        selector: device::OperationSelector::Execution {
                            operation_ref: reference,
                        },
                    },
                )
                .await
                .unwrap()
                .result;
            assert_eq!(status.data["phase"], "cancelled_before_execute");
            assert_eq!(status.data["dispatch_started"], false);
        }
        {
            let calls = calls.lock().unwrap();
            assert_eq!(
                calls
                    .iter()
                    .filter(|c| matches!(c, Call::Cancel { .. }))
                    .count(),
                2
            );
            assert!(!calls.iter().any(|c| matches!(c, Call::Execute { .. })));
        }
        let accepted = g.request("device", Some(token.into()), Call::Prepare {
            tool: "computer.observe".into(),
            arguments: json!({"target":opened.data["targets"][0]["target"],"inline_image":false}),
        }).await.unwrap().result;
        let reference: device::OperationRef =
            serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
        assert!(
            !g.request("device", Some(token.into()), Call::CloseContext)
                .await
                .unwrap()
                .result
                .is_error
        );
        let rejected = g
            .dispatch_accepted(
                reference.clone(),
                g.shutdown.subscribe(),
                std::future::pending(),
            )
            .await;
        assert_eq!(rejected.data["code"], "execution_ref_expired");
        assert_eq!(rejected.data["operation_ref"], json!(reference));
        stop.send_replace(true);
        server.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn cancellation_timeout_and_disconnect_preserve_accepted_read_reference() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap().join("private");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let pairing = rwi_link_trust::provision(&root.join("trust")).unwrap();
        let identity = IdentityPaths::in_profile(&root.join("trust/agent"));
        let tls = identity
            .load()
            .unwrap()
            .server_config(std::slice::from_ref(&pairing.model.certificate_sha256))
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap();
        let runtime = Runtime::new(Config {
            version: 1,
            device_id: "device".into(),
            bind: endpoint,
            identity,
            principals: vec![Principal {
                journal_identity: None,
                certificate: pairing.model.certificate_sha256,
                observe: true,
                input: true,
            }],
            state_root: root.join("state"),
            platform: PlatformPolicy::Fixture,
        })
        .unwrap();
        let (_stop, rx) = tokio::sync::watch::channel(false);
        let server = tokio::spawn(rwi_native_link::device::serve(
            listener,
            tls,
            move |peer, request: Request| {
                let runtime = runtime.clone();
                async move {
                    if matches!(request.call, Call::Cancel { .. }) {
                        std::future::pending::<()>().await;
                    }
                    runtime.handle(peer, request).await
                }
            },
            rx,
        ));
        let g = Gateway::new(Ok(AdapterProfile {
            devices: vec![PairedDevice {
                device_id: "device".into(),
                endpoint,
                identity: IdentityPaths::in_profile(&root.join("trust/model")),
                agent_pin: pairing.agent.certificate_sha256,
            }],
        }));
        let opened = g
            .request("device", None, Call::OpenContext)
            .await
            .unwrap()
            .result;
        let accepted=g.request("device",Some(opened.data["context"].as_str().unwrap().into()),Call::Prepare{tool:"computer.observe".into(),arguments:json!({"target":opened.data["targets"][0]["target"],"inline_image":false})}).await.unwrap().result;
        let reference: device::OperationRef =
            serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
        let mut cancellation = cancellation::CancelDispatch::new(g.clone(), reference.clone());
        let timeout = cancellation.request().await;
        assert_eq!(timeout.data["code"], "cancellation_delivery_unconfirmed");
        assert_eq!(timeout.data["operation_ref"], json!(reference));
        assert_eq!(timeout.data["cancellation_delivery_confirmed"], false);
        assert!(timeout.data["cancellation_accepted"].is_null());
        // The returned locator still addresses the exact service record; no
        // durable request ID exists and no execution is guessed/replayed.
        let status=g.invoke("computer.operation",json!({"selector":{"kind":"execution","operation_ref":timeout.data["operation_ref"]}}),None).await;
        assert_eq!(status.data["phase"], "accepted");
        server.abort();
        let _ = server.await;
        let disconnected = cancellation::CancelDispatch::new(g, reference.clone())
            .request()
            .await;
        assert_eq!(disconnected.data["code"], "device_connection_failed");
        assert_eq!(disconnected.data["operation_ref"], json!(reference));
        assert_eq!(disconnected.data["cancellation_delivery_confirmed"], false);
        assert_eq!(disconnected.data["original_action_replayed"], false);
    }
}

#[cfg(all(test, unix))]
mod browser_session_tests {
    use super::*;
    use rwi_link_trust::IdentityPaths;
    use rwi_native_link::device::PairedDevice;
    use std::os::unix::fs::DirBuilderExt;
    #[tokio::test]
    async fn f14_browser_mcp_forwards_without_native_context_or_native_availability() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap().join("private");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let pairing = rwi_link_trust::provision(&root.join("trust")).unwrap();
        let identity = IdentityPaths::in_profile(&root.join("trust/agent"));
        let tls = identity
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
                            json!({"availability":"unavailable","native_context":null,"capabilities":[{"id":"browser.session","implemented":true,"available":true,"authorized":true}]}),
                        ),
                        Call::BrowserSession { arguments } => DeviceResult::value(
                            json!({"session_ref":"browser:instance:owned","request_id":arguments.request_id(),"source":"offline_transport_fake"}),
                        ),
                        _ => DeviceResult::error(
                            "unexpected_native_call",
                            "No Native context should be opened.",
                        ),
                    };
                    device::Reply {
                        version: device::PROTOCOL_VERSION,
                        id: request.id,
                        device: request.device,
                        instance: "test-instance".into(),
                        result,
                    }
                }
            },
            rx,
        ));
        let gateway = Gateway::new(Ok(AdapterProfile {
            devices: vec![PairedDevice {
                device_id: "device".into(),
                endpoint,
                identity: IdentityPaths::in_profile(&root.join("trust/model")),
                agent_pin: pairing.agent.certificate_sha256,
            }],
        }));
        let result=gateway.invoke("browser.session",json!({"action":"create","device_id":"device","request_id":"f14-lifecycle-one","url":"about:blank"}),None).await;
        assert!(!result.is_error, "{result:?}");
        assert_eq!(result.data["request_id"], "f14-lifecycle-one");
        {
            let c = calls.lock().unwrap();
            assert_eq!(c.len(), 2);
            assert!(c.iter().all(|r| r.context.is_none()));
            assert!(matches!(c[0].call, Call::Describe));
            assert!(matches!(c[1].call, Call::BrowserSession { .. }));
            assert!(
                gateway.routes["device"]
                    .context
                    .try_lock()
                    .unwrap()
                    .is_none()
            );
        }
        stop.send_replace(true);
        server.await.unwrap().unwrap();
    }
}
