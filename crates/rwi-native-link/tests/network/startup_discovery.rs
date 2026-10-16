use super::*;

#[tokio::test]
async fn actual_relay_keeps_diagnostic_catalog_after_pre_mcp_rejection_without_retry() {
    for rejection in 0..3 {
        check_rejected_start(rejection).await;
    }
}

async fn check_rejected_start(rejection: u8) {
    let case = Case::new();
    let listener = listener(IpAddr::V4(Ipv4Addr::LOCALHOST)).await;
    let endpoint = listener.local_addr().unwrap();
    let tls = net::server_tls(&case.agent(endpoint)).unwrap();
    let mut client = case.client(endpoint, Role::Model);
    let entry = case.entry(&mut client);
    let rejected = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let stream = net::accept(tcp, tls).await.unwrap();
        let mut stream = BufReader::new(stream);
        let attach = frame::read_frame(&mut stream, frame::ATTACH_CAP)
            .await
            .unwrap()
            .unwrap();
        let attach: Attach = serde_json::from_slice(&attach).unwrap();
        assert_eq!(attach.role, Role::Model);
        // Deliberately reject before any MCP bytes or stage admission.
        if rejection != 0 {
            let nonce = if rejection == 1 {
                attach.nonce
            } else {
                uuid::Uuid::new_v4().to_string()
            };
            net::write_json(
                &mut stream,
                &json!({"version":rwi_native_link::TRANSPORT_VERSION,
                "attached":false,"task":attach.task,"nonce":nonce,"role":"model",
                "reason":"illegal_stage_transition","hold_reason":"agent_restart_unfinished_task",
                "next_stage":"model"}),
                frame::ATTACH_CAP,
            )
            .await
            .unwrap();
        }
        drop(stream);
        assert!(
            tokio::time::timeout(Duration::from_millis(500), listener.accept())
                .await
                .is_err()
        );
    });
    let mut child = spawn_from_entry(&entry);
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let stderr = tokio::spawn(collect(child.stderr.take().unwrap()));
    input.write_all(&raw(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"diagnostic-test","version":"1"}}}))).await.unwrap();
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        frame::read_frame(&mut output, frame::OUTBOUND),
    )
    .await
    .unwrap()
    .unwrap()
    .unwrap();
    let response: Value = serde_json::from_slice(&response).unwrap();
    assert_eq!(
        response["result"]["serverInfo"]["name"],
        "real-world-interaction"
    );
    assert!(
        response["result"]["instructions"]
            .as_str()
            .unwrap()
            .starts_with("DIAGNOSTIC ONLY")
    );
    input
        .write_all(&raw(
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        ))
        .await
        .unwrap();
    for (id, method, params) in [
        (2, "tools/list", json!({})),
        (
            3,
            "tools/call",
            json!({"name":"computer.targets","arguments":{}}),
        ),
        (
            4,
            "tools/call",
            json!({"name":"computer.act_checked","arguments":{}}),
        ),
    ] {
        input
            .write_all(&raw(
                json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
            ))
            .await
            .unwrap();
        let response = tokio::time::timeout(
            Duration::from_secs(2),
            frame::read_frame(&mut output, frame::OUTBOUND),
        )
        .await
        .unwrap()
        .unwrap()
        .unwrap();
        let response: Value = serde_json::from_slice(&response).unwrap();
        assert_eq!(response["id"], id);
        if id == 2 {
            assert_eq!(response["result"]["tools"].as_array().unwrap().len(), 19);
        } else {
            assert_eq!(response["result"]["isError"], true);
            let d = &response["result"]["structuredContent"];
            assert_eq!(d["mode"], "diagnostic_only");
            assert_eq!(d["operation_dispatched"], false);
            assert_eq!(d["automatic_reconnect"], false);
            let reason = d["startup_error"].as_str().unwrap();
            match rejection {
                0 => assert!(
                    reason.contains("attach_ack_missing") || reason.contains("closed connection")
                ),
                1 => assert!(reason.contains("agent_restart_unfinished_task")),
                _ => assert_eq!(reason, "attach_rejection_binding_mismatch"),
            }
        }
    }
    drop(input);
    assert!(
        tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
    let stderr = String::from_utf8(stderr.await.unwrap()).unwrap();
    assert!(!stderr.contains("model_end_ack_verified"));
    rejected.await.unwrap();
}
