#![cfg(unix)]
//! Real MCP stdio/pinned TLS with a data-only peer. No OS backend or GUI input.
use rwi_core::device::{
    Call, DeviceImage, DeviceResult, OperationRef, PROTOCOL_VERSION, Reply, Request,
};
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

const TARGET: &str = "fixture-snapshot-target";
const CONTEXT: &str = "fixture-snapshot-context";
const INSTANCE: &str = "fixture-snapshot-instance";
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScLbtAAAAABJRU5ErkJggg==";

fn observation() -> DeviceResult {
    let controls: Vec<Value> = (0..12)
        .map(|i| json!({"id":format!("{i:064x}"),"role":"AXButton","name":format!("Close synthetic button {i}"),"point":[0.5,0.5],"actions":["AXPress"],"enabled":true,"visible":true,"future_control":null}))
        .collect();
    let coverage = json!({"complete":false,"absence_proven":false,"input_authority":false,
        "reasons":["synthetic metadata, not native coverage"],"unknown":null,
        "details":(0..16).map(|i|json!({"index":i,"state":"synthetic unknown, not a native claim"})).collect::<Vec<_>>()});
    DeviceResult::value(json!({
        "frame":"fixture-frame","artifact_id":"fixture-artifact","input_domain_fenced":false,
        "scene":{"binding":{"scope":{"pid":7,"window":"cgwindow:77"},"mutation_context":{"window_title":"Synthetic snapshot"}},
            "mutation_ready":true,"blocked":false,"assurance":"synthetic-fixture","image_size":[1,1],
            "pixel_to_desktop":[1,0,0,1,0,0],"elements":controls},
        "adapter_status":{"input_proposal_max_age_ms":120000,"diagnostic_padding":"🙂".repeat(4096),
            "observation_consistency":{"scope":"synthetic-data-only","pixel_semantic_atomic":false,
                "ax_coverage_after":coverage,"future_unknown":{"large":9007199254740993_u64,"value":null}}}
    }))
}
fn image_result() -> DeviceResult {
    DeviceResult {
        is_error: false,
        data: json!({"view_id":"fixture-view","input_reference_available":true,"mapping":{
            "original_size":[1,1],"view_size":[1,1],"view_to_original":[1,0,0,1,0,0],
            "original_to_desktop":[1,0,0,1,0,0],"future_mapping":null}}),
        images: vec![DeviceImage {
            mime: "image/png".into(),
            base64: PNG.into(),
        }],
    }
}
struct Host {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}
impl Host {
    async fn start(profile: &std::path::Path) -> Self {
        let binary = std::env::var_os("RWI_F56_PROBE_SLOT")
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
        host.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"snapshot-summary-fixture","version":"F56"}}})).await;
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
fn text(reply: &Value) -> Value {
    serde_json::from_str(reply["content"][0]["text"].as_str().unwrap()).unwrap()
}
fn check_snapshot(reply: &Value) {
    assert_eq!(reply["isError"], false, "{reply}");
    let d = &reply["structuredContent"];
    assert_eq!(d["elements"].as_array().unwrap().len(), 12);
    assert_eq!(d["elements"][0]["id"], format!("{:064x}", 0));
    assert_eq!(d["elements"][0]["point"], json!([0.5, 0.5]));
    assert_eq!(
        d["mapping"]["original_to_desktop"],
        json!([1, 0, 0, 1, 0, 0])
    );
    assert_eq!(
        d["consistency"]["future_unknown"]["large"],
        "9007199254740993"
    );
    assert_eq!(reply["content"][1]["type"], "image");
    assert_eq!(reply["content"][1]["data"], PNG);
    assert_eq!(reply["content"][1]["mimeType"], "image/png");
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
    let accepted = Arc::new(Mutex::new(BTreeMap::<String, String>::new()));
    let (stop, rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(rwi_native_link::device::serve(
        listener,
        tls,
        move |_, request: Request| {
            let seen = seen.clone();
            let accepted = accepted.clone();
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
                    Call::Prepare { tool, arguments: _ } => {
                        assert!(matches!(
                            tool.as_str(),
                            "computer.observe" | "computer.image"
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
                            .insert(reference.execution_nonce.clone(), tool);
                        DeviceResult::value(json!({"operation_ref":reference}))
                    }
                    Call::Execute { operation_ref } => {
                        let tool = accepted
                            .lock()
                            .unwrap()
                            .get(&operation_ref.execution_nonce)
                            .unwrap()
                            .clone();
                        match tool.as_str() {
                            "computer.observe" => observation(),
                            "computer.image" => image_result(),
                            _ => unreachable!(),
                        }
                    }
                    Call::Cached { tool, arguments: _ } => {
                        assert_eq!(tool, "computer.image");
                        image_result()
                    }
                    Call::CloseContext => DeviceResult::value(json!({"closed":true})),
                    _ => DeviceResult::error(
                        "unexpected_fixture_call",
                        "Data-only snapshot peer; no Native input implementation.",
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
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&profile)
        .unwrap();
    use std::io::Write;
    f.write_all(
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
    host.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    let schema = host.read(2).await;
    let snapshot = schema["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "computer.snapshot")
        .unwrap();
    let advertised = snapshot["inputSchema"]["properties"]
        .get("text_projection")
        .is_some();
    assert_eq!(
        host.tool(3, "computer.targets", json!({"device_id":"device"}))
            .await["isError"],
        false
    );
    let full = host
        .tool(
            4,
            "computer.snapshot",
            json!({"target":TARGET,"max_edge":64}),
        )
        .await;
    check_snapshot(&full);
    assert_eq!(text(&full), full["structuredContent"]);
    let mut native_details_readable = false;
    if let Some(reference) = full["structuredContent"].get("native_details") {
        assert_eq!(
            reference["result_ref"],
            full["structuredContent"]["observation_ref"]
        );
        assert_eq!(reference["pointer"], "/adapter_status");
        let read = host.tool(8, "computer.result", reference.clone()).await;
        assert_eq!(read["isError"], false);
        assert_eq!(read["structuredContent"]["source_tool"], "computer.observe");
        assert_eq!(read["structuredContent"]["native_dispatched"], false);
        assert_eq!(read["structuredContent"]["value_included"], false);
        assert!(
            read["structuredContent"]["children"]
                .as_array()
                .unwrap()
                .contains(&json!("observation_consistency"))
        );
        let mut child = reference.clone();
        child["pointer"] = json!("/adapter_status/observation_consistency");
        let read = host.tool(9, "computer.result", child).await;
        assert_eq!(read["isError"], false);
        assert_eq!(read["structuredContent"]["native_dispatched"], false);
        assert_eq!(
            read["structuredContent"]["value"]["scope"],
            "synthetic-data-only"
        );
        native_details_readable = true;
    }
    let compact = host
        .tool(
            5,
            "computer.snapshot",
            json!({"target":TARGET,"max_edge":64,"text_projection":"structured_summary_v1"}),
        )
        .await;
    let projected = !compact["isError"].as_bool().unwrap();
    if projected {
        check_snapshot(&compact);
        assert_eq!(
            text(&compact)["native_details"],
            compact["structuredContent"]["native_details"]
        );
        assert_eq!(
            text(&compact)["_text_projection"]["version"],
            "snapshot_summary_v1"
        );
        assert!(text(&compact).get("elements").is_none());
        assert_eq!(
            compact["structuredContent"]["elements"],
            full["structuredContent"]["elements"]
        );
    }
    let bad = host
        .tool(
            6,
            "computer.snapshot",
            json!({"target":TARGET,"text_projection":"wrong-mode"}),
        )
        .await;
    assert_eq!(bad["isError"], true);
    let unknown = host
        .tool(
            7,
            "computer.snapshot",
            json!({"target":TARGET,"extra_field":true}),
        )
        .await;
    assert_eq!(unknown["isError"], true);
    let unknown_specific = text(&unknown)["message"]
        .as_str()
        .unwrap()
        .contains("extra_field");
    host.stop().await;
    stop.send_replace(true);
    server.await.unwrap().unwrap();
    let calls = requests.lock().unwrap();
    let observe_count = calls
        .iter()
        .filter(|r| matches!(&r.call,Call::Prepare{tool,..}if tool=="computer.observe"))
        .count();
    assert_eq!(observe_count, if projected { 2 } else { 1 });
    let full_bytes = full["content"][0]["text"].as_str().unwrap().len();
    let compact_bytes = compact["content"][0]["text"].as_str().unwrap().len();
    json!({"native_details_readable":native_details_readable,"schema_has_mode":advertised,"optin_summary":projected,"optin_isError":compact["isError"],"default_text_equals_structured":true,"exact_controls_points_and_PNG_retained":true,"unknown_argument_specific":unknown_specific,"default_text_bytes":full_bytes,"optin_text_bytes":compact_bytes,"observe_count":observe_count,"MCP_child_exit0_checked":true,"synthetic_only_no_OS_input":true})
}

#[tokio::test]
async fn native_details_ref_actual_mcp_protocol_without_extra_observe() {
    let observed = run().await;
    assert_eq!(observed["native_details_readable"], true);
    assert_eq!(observed["observe_count"], 2);
}

#[tokio::test]
async fn f56_probe_protocol_observation() {
    println!("F56_OBSERVED={}", run().await);
}

#[tokio::test]
async fn f56_snapshot_summary_actual_mcp_protocol_and_argument_errors() {
    let observed = run().await;
    assert_eq!(observed["schema_has_mode"], true);
    assert_eq!(observed["optin_summary"], true);
    assert_eq!(observed["optin_isError"], false);
    assert_eq!(observed["unknown_argument_specific"], true);
    assert!(
        observed["optin_text_bytes"].as_u64().unwrap()
            < observed["default_text_bytes"].as_u64().unwrap()
    );
}
