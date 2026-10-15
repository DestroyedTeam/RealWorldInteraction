#![cfg(unix)]
//! Real MCP stdio and pinned TLS with a data-only synthetic device, never OS input.
use rwi_core::device::{Call, DeviceResult, OperationRef, PROTOCOL_VERSION, Reply, Request};
use rwi_link_trust::IdentityPaths;
use rwi_native_link::device::{AdapterProfile, PairedDevice};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
};

const TARGET: &str = "fixture-hotkey-target";
const CONTEXT: &str = "fixture-hotkey-context";
const INSTANCE: &str = "fixture-hotkey-instance";

fn failure_data() -> Value {
    let recorded = std::env::var_os("RWI_F49_PROBE_SLOT").map(|path| {
        let slot: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let raw: Value = serde_json::from_slice(
            &std::fs::read(slot["recorded_native_input"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        raw["result"]["structuredContent"].clone()
    });
    let mut data = recorded.unwrap_or_else(|| {
        json!({
            "native_dispatch_entered":false,"input_domain_fenced":false,
            "receipt":{"dispatch":"rejected_before_dispatch","dispatch_error":{
                "code":"foreground_changed","next_step":"Global keyboard receiver changed; no further down event."
            },"goal_verified":false,"replay":"forbidden"},
            "retained_unknown":{"button":"Close","point":[17,29]}
        })
    });
    assert_eq!(data["native_dispatch_entered"], false);
    assert_eq!(
        data["receipt"]["dispatch_error"]["code"],
        "foreground_changed"
    );
    // Recorded facts are evaluator data, never authorization for a real device.
    data.as_object_mut().unwrap().remove("operation_ref");
    data.as_object_mut().unwrap().remove("receipt_ref");
    data["receipt"]["scope"] = json!({"pid":7,"window":"cgwindow:77","element":null});
    data
}

struct Host {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}
impl Host {
    async fn start(profile: &std::path::Path) -> Self {
        let binary = std::env::var_os("RWI_F49_PROBE_SLOT")
            .map(|path| {
                let slot: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
                std::ffi::OsString::from(slot["MCP_binary"].as_str().unwrap())
            })
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_rwi-mcp").into());
        let mut child = tokio::process::Command::new(binary)
            .arg("--profile")
            .arg(profile)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut host = Self {
            child,
            input,
            output,
        };
        host.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"hotkey-receipt-fixture","version":"F49"}}})).await;
        assert!(host.read(1).await.get("result").is_some());
        host.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        host
    }
    async fn send(&mut self, value: Value) {
        self.input
            .as_mut()
            .unwrap()
            .write_all(format!("{value}\n").as_bytes())
            .await
            .unwrap();
    }
    async fn read(&mut self, id: u32) -> Value {
        loop {
            let mut line = String::new();
            let length =
                tokio::time::timeout(Duration::from_secs(15), self.output.read_line(&mut line))
                    .await
                    .unwrap()
                    .unwrap();
            assert_ne!(length, 0, "unexpected MCP EOF");
            let value: Value = serde_json::from_str(&line).unwrap();
            if value.get("id").is_some() {
                assert_eq!(value["id"], id);
                return value;
            }
        }
    }
    async fn tool(&mut self, id: u32, name: &str, args: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).await;
        let value = self.read(id).await;
        assert!(value.get("error").is_none(), "{value}");
        value["result"].clone()
    }
    async fn stop(mut self) {
        self.input.take();
        assert!(
            tokio::time::timeout(Duration::from_secs(7), self.child.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
    }
}

async fn run() -> Value {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("private");
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
    let requests = Arc::new(Mutex::new(Vec::<Request>::new()));
    let seen = requests.clone();
    let accepted = Arc::new(Mutex::new(BTreeMap::<String, Value>::new()));
    let rejected = failure_data();
    let original_receipt = rejected["receipt"].clone();
    let (stop, rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(rwi_native_link::device::serve(
        listener,
        tls,
        move |_, request: Request| {
            let seen = seen.clone();
            let accepted = accepted.clone();
            let rejected = rejected.clone();
            async move {
                seen.lock().unwrap().push(request.clone());
                let targets = json!([{"target":TARGET,"identity":{"host":"fixture-host","guest":null,"session":"fixture-session","seat":"fixture-seat","desktop":"fixture-desktop"},"capabilities":{"synthetic":true}}]);
                let result = match request.call {
                    Call::Describe => DeviceResult::value(
                        json!({"availability":"available","context_valid":request.context.as_deref()==Some(CONTEXT),"targets":targets}),
                    ),
                    Call::OpenContext => {
                        DeviceResult::value(json!({"context":CONTEXT,"targets":targets}))
                    }
                    Call::Prepare { tool, arguments } => {
                        assert_eq!(tool, "computer.hotkey");
                        let reference = OperationRef {
                            ref_version: 1,
                            device_id: request.device.clone(),
                            agent_instance: INSTANCE.into(),
                            context_generation: CONTEXT.into(),
                            execution_nonce: uuid::Uuid::new_v4().to_string(),
                        };
                        accepted
                            .lock()
                            .unwrap()
                            .insert(reference.execution_nonce.clone(), arguments);
                        DeviceResult::value(json!({"operation_ref":reference}))
                    }
                    Call::Execute { operation_ref } => {
                        let arguments = accepted
                            .lock()
                            .unwrap()
                            .get(&operation_ref.execution_nonce)
                            .unwrap()
                            .clone();
                        let mut data = rejected;
                        data["receipt"]["request_id"] = arguments["request_id"].clone();
                        if arguments["request_id"] == "f49-success" {
                            data["receipt"]["dispatch"] = json!("returned");
                            data["receipt"]["dispatch_error"] = Value::Null;
                        }
                        DeviceResult::value(data)
                    }
                    // A successful status query may contain historical failure data.
                    Call::Operation { .. } => {
                        let mut data = rejected;
                        data["historical_lookup"] = json!(true);
                        DeviceResult::value(data)
                    }
                    Call::CloseContext => DeviceResult::value(json!({"closed":true})),
                    _ => DeviceResult::error(
                        "unexpected_fixture_call",
                        "Synthetic metadata peer only; no Native input implementation.",
                    ),
                };
                Reply {
                    version: PROTOCOL_VERSION,
                    id: request.id,
                    device: request.device,
                    instance: INSTANCE.into(),
                    result,
                }
            }
        },
        rx,
    ));
    let profile = root.join("profile.json");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&profile)
        .unwrap();
    use std::io::Write;
    file.write_all(
        &serde_json::to_vec(&AdapterProfile {
            devices: vec![PairedDevice {
                device_id: "device".into(),
                endpoint,
                identity: IdentityPaths::in_profile(&root.join("trust/model")),
                agent_pin: pairing.agent.certificate_sha256,
            }],
        })
        .unwrap(),
    )
    .unwrap();
    let mut host = Host::start(&profile).await;
    let targets = host
        .tool(2, "computer.targets", json!({"device_id":"device"}))
        .await;
    assert_eq!(targets["isError"], false, "{targets}");
    let arguments = json!({"target":TARGET,"request_id":"f49-direct","lease":"fixture-lease","key":"cmd+tab","expected_foreground":{"pid":7,"window":"cgwindow:77","process_start":"fixture-process-start","native_epoch":"fixture-native-epoch"}});
    let direct = host.tool(3, "computer.hotkey", arguments.clone()).await;
    let mut expected_receipt = original_receipt;
    expected_receipt["request_id"] = json!("f49-direct");
    assert_eq!(direct["structuredContent"]["receipt"], expected_receipt);
    assert_eq!(
        direct["structuredContent"]["native_dispatch_entered"],
        false
    );
    let query = host.tool(4,"computer.operation",json!({"selector":{"kind":"execution","operation_ref":direct["structuredContent"]["operation_ref"]}})).await;
    assert_eq!(
        query["isError"], false,
        "history lookup is not a new failed input"
    );
    assert_eq!(query["structuredContent"]["historical_lookup"], true);
    let mut js_arguments = arguments.clone();
    js_arguments["request_id"] = json!("f49-js");
    let js = host
        .tool(
            5,
            "computer.js",
            json!({"code":format!("emit(await rwi.call('computer.hotkey', {js_arguments}));")}),
        )
        .await;
    let js_data = &js["structuredContent"];
    assert_eq!(js_data["calls"].as_array().map(Vec::len), Some(1), "{js}");
    assert_eq!(js_data["calls"][0]["tool"], "computer.hotkey");
    assert_eq!(js_data["calls"][0]["is_error"], direct["isError"]);
    assert_eq!(js_data["original_action_replayed"], false);
    let mut success_arguments = arguments;
    success_arguments["request_id"] = json!("f49-success");
    let success = host.tool(6, "computer.hotkey", success_arguments).await;
    assert_eq!(success["isError"], false);
    assert_eq!(
        success["structuredContent"]["receipt"]["dispatch_error"],
        Value::Null
    );
    host.stop().await;
    stop.send_replace(true);
    server.await.unwrap().unwrap();
    let calls = requests.lock().unwrap();
    assert_eq!(
        calls
            .iter()
            .filter(|r| matches!(r.call, Call::Prepare { .. }))
            .count(),
        3
    );
    assert_eq!(
        calls
            .iter()
            .filter(|r| matches!(r.call, Call::Execute { .. }))
            .count(),
        3
    );
    assert!(!calls.iter().any(|r| matches!(r.call, Call::Cancel { .. })));
    json!({"direct_MCP_isError":direct["isError"],"direct_root_code":direct["structuredContent"]["code"],"JS_call_is_error":js_data["calls"][0]["is_error"],"JS_call_root_code":js_data["calls"][0]["code"],"history_query_isError":query["isError"],"success_control_isError":success["isError"],"original_receipt_retained":true,"prepare_count":3,"execute_count":3,"native_OS_input_sent":false})
}

#[tokio::test]
async fn f49_probe_protocol_observation() {
    // This observation entry point intentionally reports either version's flag.
    // A frozen copy can compare original/changed/rolled-back/reapplied binaries.
    println!("F49_OBSERVED={}", run().await);
}

#[tokio::test]
async fn f49_hotkey_error_metadata_direct_and_js_not_success_or_query() {
    let result = run().await;
    assert_eq!(result["direct_MCP_isError"], true, "{result}");
    assert_eq!(result["direct_root_code"], "foreground_changed");
    assert_eq!(result["JS_call_is_error"], true);
    assert_eq!(result["JS_call_root_code"], "foreground_changed");
    assert_eq!(result["history_query_isError"], false);
    assert_eq!(result["success_control_isError"], false);
}
