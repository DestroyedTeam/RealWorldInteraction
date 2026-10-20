//! Real MCP child and isolated synthetic TLS peer. No production profile or OS I/O.
#![cfg(unix)]
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
const TARGET: &str = "selection-fixture-root";
const CONTEXT: &str = "selection-fixture-context";
const INSTANCE: &str = "selection-fixture-instance";
struct Host {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}
impl Host {
    async fn start(profile: &std::path::Path) -> Self {
        let binary = std::env::var_os("RWI_F77_PROBE_SLOT")
            .map(|p| {
                let slot: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
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
        host.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"selection-preparation-fixture","version":"F77"}}})).await;
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
            let n = tokio::time::timeout(Duration::from_secs(15), self.output.read_line(&mut line))
                .await
                .unwrap()
                .unwrap();
            assert_ne!(n, 0, "unexpected MCP EOF");
            let v: Value = serde_json::from_str(&line).unwrap();
            if v.get("id").is_some() {
                assert_eq!(v["id"], id);
                return v;
            }
        }
    }
    async fn tool(&mut self, id: u32, name: &str, args: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).await;
        let v = self.read(id).await;
        assert!(v.get("error").is_none(), "{v}");
        v["result"].clone()
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
fn args(pid: u32) -> Value {
    json!({"target":TARGET,"selection":{"window":format!("window:{pid}"),"pid":pid,"native_epoch":format!("epoch-{pid}"),"relation":"initial"},"correlation_id":"fixed-correlation"})
}
fn execution(tool: &str, args: &Value, counts: &mut BTreeMap<u32, usize>) -> DeviceResult {
    if tool == "computer.discover" {
        let pid = args["pid"].as_u64().unwrap() as u32;
        return DeviceResult::value(
            json!({"activated":false,"input_sent":false,"enumeration_complete":true,
            "filter":{"pid":pid,"title_contains":null},"catalog_id":format!("catalog-{pid}"),"matching_windows":1,"next_cursor":null,
            "windows":[{"window":format!("window:{pid}"),"pid":pid,"native_epoch":if pid==8 {"changed".to_owned()} else {format!("epoch-{pid}")}}]}),
        );
    }
    assert_eq!(tool, "computer.attach_window");
    let pid = args["selection"]["pid"].as_u64().unwrap() as u32;
    let count = counts.entry(pid).or_default();
    *count += 1;
    if pid == 10 || *count == 2 {
        DeviceResult::value(
            json!({"target":format!("owned-child-{pid}"),"parent_target":TARGET,"binding_changed":false,"input_authority_granted":false,"future_unknown":null}),
        )
    } else {
        DeviceResult::error(
            "catalog_expired",
            "Original synthetic pre-dispatch rejection.",
        )
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
    let seen = Arc::new(Mutex::new(Vec::<Request>::new()));
    let requests = seen.clone();
    let accepted = Arc::new(Mutex::new(BTreeMap::<String, (String, Value)>::new()));
    let counts = Arc::new(Mutex::new(BTreeMap::<u32, usize>::new()));
    let (stop, rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(rwi_native_link::device::serve(
        listener,
        tls,
        move |_, request: Request| {
            let seen = seen.clone();
            let accepted = accepted.clone();
            let counts = counts.clone();
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
                        assert!(matches!(
                            tool.as_str(),
                            "computer.attach_window" | "computer.discover"
                        ));
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
                            .insert(reference.execution_nonce.clone(), (tool, arguments));
                        DeviceResult::value(json!({"operation_ref":reference}))
                    }
                    Call::Execute { operation_ref } => {
                        let (tool, args) = accepted
                            .lock()
                            .unwrap()
                            .remove(&operation_ref.execution_nonce)
                            .unwrap();
                        let mut result = execution(&tool, &args, &mut counts.lock().unwrap());
                        result.data["native_dispatch_entered"] = json!(false);
                        result.data["input_domain_fenced"] = json!(false);
                        let clean =
                            !(tool == "computer.attach_window" && args["selection"]["pid"] == 9);
                        result.data["operation"] = json!({"terminal":true,"resources_released":clean,"native_call_pending":!clean,"resource_release_uncertain":!clean,"cancel_requested":false});
                        result.data["operation_ref"] = json!(operation_ref);
                        result
                    }
                    Call::CloseContext => DeviceResult::value(json!({"closed":true})),
                    other => panic!("No input or other native call permitted: {other:?}"),
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
    assert_eq!(
        host.tool(2, "computer.targets", json!({"device_id":"device"}))
            .await["isError"],
        false
    );
    let direct = host.tool(3, "computer.attach_window", args(7)).await;
    let refreshed = direct["isError"] == false;
    let mut detail_verified = false;
    if refreshed {
        assert_eq!(direct["structuredContent"]["target"], "owned-child-7");
        assert_eq!(direct["structuredContent"]["future_unknown"], Value::Null);
        let reference = &direct["structuredContent"]["selection_preparation"]["details_ref"];
        let details = host
            .tool(
                4,
                "computer.result",
                json!({"result_ref":reference,"pointer":"/selection_preparation/initial_error"}),
            )
            .await;
        assert_eq!(
            details["structuredContent"]["value"]["code"],
            "catalog_expired"
        );
        assert!(
            direct["structuredContent"]["selection_preparation"]
                .get("discovery_pages")
                .is_none()
        );
        detail_verified = true;
    } else {
        assert_eq!(direct["structuredContent"]["code"], "catalog_expired");
    }
    let js = host
        .tool(
            5,
            "computer.js",
            json!({"code":format!("emit(await rwi.call('computer.attach_window', {}));",args(17))}),
        )
        .await;
    let js_refreshed = js["isError"] != true;
    assert_eq!(
        js_refreshed, refreshed,
        "Canonical direct/JS route disagrees: {js}"
    );
    if js_refreshed {
        assert_eq!(
            js["structuredContent"]["outputs"][0]["data"]["target"],
            "owned-child-17"
        );
    }
    let changed = host.tool(6, "computer.attach_window", args(8)).await;
    assert_eq!(changed["isError"], true);
    let uncertain = host.tool(7, "computer.attach_window", args(9)).await;
    assert_eq!(uncertain["isError"], true);
    assert!(
        uncertain["structuredContent"]
            .get("selection_preparation")
            .is_none()
    );
    let normal = host.tool(8, "computer.attach_window", args(10)).await;
    assert_eq!(normal["isError"], false);
    assert!(
        normal["structuredContent"]
            .get("selection_preparation")
            .is_none()
    );
    host.stop().await;
    stop.send_replace(true);
    server.await.unwrap().unwrap();
    let calls = requests.lock().unwrap();
    let attach = calls
        .iter()
        .filter(|r| matches!(&r.call,Call::Prepare {tool,..} if tool=="computer.attach_window"))
        .count();
    let discover = calls
        .iter()
        .filter(|r| matches!(&r.call,Call::Prepare {tool,..} if tool=="computer.discover"))
        .count();
    assert_eq!(attach, if refreshed { 7 } else { 5 });
    assert_eq!(discover, if refreshed { 3 } else { 0 });
    json!({"direct_refresh":refreshed,"js_refresh":js_refreshed,"original_error_retained_and_readable":detail_verified,"epoch_change_refused":true,"uncertain_attachment_not_retried":true,"normal_attachment_unchanged":true,"attach_prepare_count":attach,"discover_prepare_count":discover,"MCP_child_exit0_checked":true,"synthetic_only_no_OS_input":true})
}
#[tokio::test]
async fn f77_probe_protocol_observation() {
    println!("F77_OBSERVED={}", run().await);
}
#[tokio::test]
async fn f77_selection_refresh_actual_mcp_direct_js_and_retained_error() {
    let result = run().await;
    assert_eq!(result["direct_refresh"], true);
    assert_eq!(result["js_refresh"], true);
    assert_eq!(result["original_error_retained_and_readable"], true);
}
