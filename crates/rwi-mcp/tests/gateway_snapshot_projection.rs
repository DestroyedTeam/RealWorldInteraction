#![cfg(unix)]
//! Actual production MCP process with an archive-derived, authenticated fake device.
//! This is formatting/cache acceptance only: no native Agent, GUI, capture or input.
use rwi_core::device::{
    Call, DeviceImage, DeviceResult, OperationRef, PROTOCOL_VERSION, Reply, Request,
};
use rwi_link_trust::{IdentityPaths, PeerPin};
use rwi_native_link::device::{AdapterProfile, PairedDevice};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
};

const DEVICE: &str = "r94-format-fixture";
const INSTANCE: &str = "r94-format-fixture-instance";
const CONTEXT: &str = "r94-format-fixture-context";
const PNG: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Wl6UwAAAABJRU5ErkJggg==";

struct Host {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    id: u64,
}
impl Host {
    async fn start(profile: &std::path::Path) -> Self {
        let binary = std::env::var_os("RWI_R94_MCP_BINARY")
            .unwrap_or_else(|| env!("CARGO_BIN_EXE_rwi-mcp").into());
        let mut child = tokio::process::Command::new(binary)
            .arg("--profile")
            .arg(profile)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut host = Self {
            child,
            input,
            output,
            id: 1,
        };
        let init = host
            .rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize",
            "params":{"protocolVersion":"2025-11-25","capabilities":{},
                "clientInfo":{"name":"r94-format-only-fixture","version":"1"}}}))
            .await;
        assert!(init["reply"].get("result").is_some(), "{init}");
        host.write(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        host
    }
    async fn write(&mut self, value: Value) {
        let input = self.input.as_mut().unwrap();
        input
            .write_all(format!("{value}\n").as_bytes())
            .await
            .unwrap();
        input.flush().await.unwrap();
    }
    async fn rpc(&mut self, value: Value) -> Value {
        let id = value["id"].clone();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        self.write(value).await;
        loop {
            let mut line = String::new();
            let n = tokio::time::timeout_at(deadline, self.output.read_line(&mut line))
                .await
                .expect("bounded fixture MCP reply")
                .unwrap();
            assert!(n > 0, "MCP EOF before reply");
            let reply: Value = serde_json::from_str(&line).unwrap();
            if reply["id"] == id {
                return json!({"reply":reply,"wire_bytes":line.len()});
            }
            assert!(reply.get("method").is_some(), "unmatched response {reply}");
        }
    }
    async fn tool(&mut self, name: &str, arguments: Value) -> Value {
        self.id += 1;
        self.rpc(json!({"jsonrpc":"2.0","id":self.id,"method":"tools/call",
            "params":{"name":name,"arguments":arguments}}))
            .await
    }
    async fn stop(mut self) {
        self.input.take();
        assert!(
            tokio::time::timeout(Duration::from_secs(10), self.child.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
    }
}
fn data(response: &Value) -> &Value {
    &response["reply"]["result"]["structuredContent"]
}
fn success(response: &Value) {
    assert!(response["reply"]["error"].is_null(), "{response}");
    assert_ne!(response["reply"]["result"]["isError"], true, "{response}");
}
fn exact_synthetic_image(response: &Value) {
    let content = response["reply"]["result"]["content"].as_array().unwrap();
    let images: Vec<_> = content.iter().filter(|c| c["type"] == "image").collect();
    assert_eq!(images.len(), 1, "no missing/extra fixture image allowed");
    assert_eq!(images[0]["mimeType"], "image/png");
    assert_eq!(
        images[0]["data"], PNG,
        "same-length substitutions must fail"
    );
}
fn bytes(response: &Value) -> Value {
    let content = response["reply"]["result"]["content"].as_array().unwrap();
    let text_bytes: usize = content
        .iter()
        .filter_map(|c| c["text"].as_str())
        .map(str::len)
        .sum();
    let image_base64_bytes: Vec<usize> = content
        .iter()
        .filter_map(|c| c["data"].as_str())
        .map(str::len)
        .collect();
    json!({"content_text_utf8_bytes":text_bytes,"structured_content_encoded_bytes":serde_json::to_vec(data(response)).unwrap().len(),
        "jsonrpc_line_utf8_bytes":response["wire_bytes"],"image_base64_bytes":image_base64_bytes,
        "native_cpu_ms":null,"native_capture_ms":null,"scope":"actual MCP formatting only; synthetic image"})
}
async fn export(host: &mut Host, reference: &str) -> (Value, String) {
    let mut offset = 0;
    let mut text = String::new();
    let mut expected_sha: Option<String> = None;
    let mut metadata_sha: Option<String> = None;
    loop {
        let response = host
            .tool(
                "computer.result",
                json!({"result_ref":reference,"pointer":"","byte_offset":offset}),
            )
            .await;
        success(&response);
        let d = data(&response);
        assert_eq!(d["native_dispatched"], false);
        assert_eq!(d["byte_offset"], offset);
        let sha = d["selected_sha256"].as_str().unwrap();
        assert_eq!(expected_sha.get_or_insert_with(|| sha.into()).as_str(), sha);
        let sha = d["metadata_sha256"].as_str().unwrap();
        assert_eq!(metadata_sha.get_or_insert_with(|| sha.into()).as_str(), sha);
        text.push_str(d["text"].as_str().unwrap());
        match d["next_byte_offset"].as_u64() {
            Some(next) => {
                assert_eq!(next as usize, text.len());
                offset = next;
            }
            None => {
                assert_eq!(d["total_bytes"], text.len());
                break;
            }
        }
    }
    let sha = rwi_core::digest(text.as_bytes());
    assert_eq!(expected_sha.unwrap(), sha);
    assert_eq!(metadata_sha.unwrap(), sha);
    (serde_json::from_str(&text).unwrap(), sha)
}
fn native_shape(sample: &Value) -> Value {
    let s = &sample["projected_snapshot"];
    let mut consistency = s["consistency"].clone();
    consistency["future_fixture_unknown"] = Value::Null;
    consistency["future_fixture_false"] = json!(false);
    consistency["ax_coverage_after"]["future_fixture_limits"] =
        json!({"max_nodes":null,"known":false});
    json!({"frame":s["frame"],"artifact_id":"r94-synthetic-artifact","synthetic":true,
        "native_GUI_effect_verified":false,"fixture_contract":{"archive_derived_not_original_native":true,"sample":sample["label"],"unknown":null},
        "scene":{"image_size":s["mapping"]["original_size"],"pixel_to_desktop":s["mapping"]["original_to_desktop"],
            "binding":{"scope":{"window":s["window"]["window"],"pid":s["window"]["pid"]},
                "mutation_context":{"window_title":s["window"]["title"]}},
            "assurance":"synthetic-format-only","blocked":s["blocked"],"mutation_ready":s["mutation_ready"],"elements":s["elements"]},
        "input_domain_fenced":s["input_domain_fenced"],
        "adapter_status":{"input_proposal_max_age_ms":s["reference_max_age_ms"],"observation_consistency":consistency}})
}

#[tokio::test]
async fn r94_actual_mcp_format_cache_and_bytes_are_fixture_only() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/r94-snapshot-projection.json")).unwrap();
    assert_eq!(fixture["original_native_payload_reconstructed"], false);
    let samples = fixture["samples"].as_array().unwrap().clone();
    assert_eq!(samples.len(), 6);
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("private");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let provisioned = rwi_link_trust::provision(&root.join("trust")).unwrap();
    let tls = IdentityPaths::in_profile(&root.join("trust/agent"))
        .load()
        .unwrap()
        .server_config(std::slice::from_ref(&provisioned.model.certificate_sha256))
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let paired = PairedDevice {
        device_id: DEVICE.into(),
        endpoint: listener.local_addr().unwrap(),
        identity: IdentityPaths::in_profile(&root.join("trust/model")),
        agent_pin: provisioned.agent.certificate_sha256,
    };
    assert!(paired.endpoint.ip().is_loopback());
    let targets: Value = json!(
        (0..samples.len())
            .map(|i| json!({"target":format!("r94-fixture-target-{i}")}))
            .collect::<Vec<_>>()
    );
    let records: Arc<Mutex<BTreeMap<String, (String, usize)>>> = Default::default();
    let observed: Arc<Mutex<BTreeMap<String, Value>>> = Default::default();
    let calls: Arc<Mutex<Vec<String>>> = Default::default();
    let samples = Arc::new(samples);
    let (stop, rx) = tokio::sync::watch::channel(false);
    let records2 = records.clone();
    let observed2 = observed.clone();
    let calls2 = calls.clone();
    let samples2 = samples.clone();
    let server = tokio::spawn(rwi_native_link::device::serve(
        listener,
        tls,
        move |_peer: PeerPin, request: Request| {
            let records = records2.clone();
            let observed = observed2.clone();
            let calls = calls2.clone();
            let samples = samples2.clone();
            let targets = targets.clone();
            async move {
                let result = match &request.call {
                    Call::Describe => DeviceResult::value(
                        json!({"availability":"available","service":"fixture_only","context_valid":true,"targets":targets,"synthetic":true}),
                    ),
                    Call::OpenContext => DeviceResult::value(
                        json!({"context":CONTEXT,"targets":targets,"synthetic":true}),
                    ),
                    Call::Prepare { tool, arguments } => {
                        assert!(
                            matches!(tool.as_str(), "computer.observe" | "computer.image"),
                            "fixture rejects {tool}"
                        );
                        calls.lock().unwrap().push(tool.clone());
                        let target = arguments["target"].as_str().unwrap();
                        let index: usize = target
                            .strip_prefix("r94-fixture-target-")
                            .unwrap()
                            .parse()
                            .unwrap();
                        assert!(index < samples.len());
                        let op = OperationRef {
                            ref_version: 1,
                            device_id: DEVICE.into(),
                            agent_instance: INSTANCE.into(),
                            context_generation: CONTEXT.into(),
                            execution_nonce: uuid::Uuid::new_v4().to_string(),
                        };
                        records
                            .lock()
                            .unwrap()
                            .insert(op.execution_nonce.clone(), (tool.clone(), index));
                        DeviceResult::value(json!({"accepted":true,"operation_ref":op}))
                    }
                    Call::Execute { operation_ref } => {
                        let (tool, index) = records
                            .lock()
                            .unwrap()
                            .remove(&operation_ref.execution_nonce)
                            .unwrap();
                        let sample = &samples[index];
                        if tool == "computer.observe" {
                            let mut raw = native_shape(sample);
                            raw["operation_ref"] = json!(operation_ref);
                            observed
                                .lock()
                                .unwrap()
                                .insert(raw["frame"].as_str().unwrap().into(), raw.clone());
                            DeviceResult::value(raw)
                        } else {
                            let s = &sample["projected_snapshot"];
                            let mut result = DeviceResult::value(
                                json!({"view_id":s["view_id"],"mapping":s["mapping"],
                                "input_reference_available":s["input_reference_available"],"synthetic":true}),
                            );
                            result.images.push(DeviceImage {
                                mime: "image/png".into(),
                                base64: PNG.into(),
                            });
                            result
                        }
                    }
                    Call::CloseContext => DeviceResult::value(json!({"closed":true})),
                    _ => panic!("unallocated fixture call {:?}", request.call),
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
    let profile = root.join("adapter.json");
    let mut f = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&profile)
        .unwrap();
    f.write_all(
        &serde_json::to_vec(&AdapterProfile {
            devices: vec![paired],
        })
        .unwrap(),
    )
    .unwrap();
    drop(f);
    let mut host = Host::start(&profile).await;
    success(
        &host
            .tool("computer.targets", json!({"device_id":DEVICE}))
            .await,
    );
    let compact = std::env::var("RWI_R94_EXPECT_COMPACT").map_or(true, |v| v != "0");
    let mut measurements = Vec::new();
    for (index, sample) in samples.iter().enumerate() {
        let target = format!("r94-fixture-target-{index}");
        let mut pair = Vec::new();
        for include_timings in [false, true] {
            let response = host
                .tool(
                    "computer.snapshot",
                    json!({"target":target,"limit":128,"include_timings":include_timings}),
                )
                .await;
            success(&response);
            exact_synthetic_image(&response);
            let displayed = data(&response);
            let before = calls.lock().unwrap().len();
            let (full, full_sha) =
                export(&mut host, displayed["details_ref"].as_str().unwrap()).await;
            let (native, native_sha) =
                export(&mut host, displayed["observation_ref"].as_str().unwrap()).await;
            assert_eq!(
                before,
                calls.lock().unwrap().len(),
                "computer.result must not RPC"
            );
            assert_eq!(
                native,
                observed.lock().unwrap()[displayed["frame"].as_str().unwrap()]
            );
            assert_eq!(
                native["fixture_contract"]["archive_derived_not_original_native"],
                true
            );
            assert_eq!(full["consistency"]["ax_coverage_after"], full["coverage"]);
            let mut expected = full.clone();
            expected["details_ref"] = displayed["details_ref"].clone();
            if !include_timings {
                expected.as_object_mut().unwrap().remove("stage_timings");
                expected.as_object_mut().unwrap().remove("elapsed_ms");
            }
            // Equal coverage is a display-only reference even with inline timings.
            // The retained full reply above must still contain the original object.
            if compact {
                expected["consistency"]
                    .as_object_mut()
                    .unwrap()
                    .remove("ax_coverage_after");
                expected["consistency"]["ax_coverage_after_ref"] = json!("/coverage");
                assert_eq!(
                    displayed["consistency"]["ax_coverage_after_ref"],
                    "/coverage"
                );
                assert_eq!(
                    displayed.pointer("/coverage"),
                    full.pointer("/consistency/ax_coverage_after")
                );
            }
            if include_timings {
                assert_eq!(displayed["stage_timings"], full["stage_timings"]);
                assert_eq!(displayed["elapsed_ms"], full["elapsed_ms"]);
            }
            assert_eq!(
                displayed, &expected,
                "ONLY declared display transform permitted"
            );
            assert_eq!(
                displayed["coverage"]["future_fixture_limits"],
                json!({"max_nodes":null,"known":false})
            );
            assert!(displayed["consistency"]["future_fixture_unknown"].is_null());
            assert_eq!(displayed["consistency"]["future_fixture_false"], false);
            for key in [
                "frame",
                "view_id",
                "mapping",
                "window",
                "blocked",
                "mutation_ready",
                "input_domain_fenced",
                "input_reference_available",
                "reference_max_age_ms",
            ] {
                assert_eq!(
                    displayed[key], sample["projected_snapshot"][key],
                    "changed {key}"
                );
            }
            let shown = displayed["elements"].as_array().unwrap();
            for original in sample["projected_snapshot"]["elements"].as_array().unwrap() {
                assert_eq!(
                    shown
                        .iter()
                        .find(|row| row["id"] == original["id"])
                        .unwrap(),
                    original,
                    "IDs, coordinates, actions, truncation and unknown control fields must survive"
                );
            }
            pair.push(
                json!({"include_timings":include_timings,"bytes":bytes(&response),
                "full_metadata_sha256":full_sha,"fixture_native_metadata_sha256":native_sha}),
            );
        }
        assert_eq!(
            pair[0]["bytes"]["image_base64_bytes"],
            pair[1]["bytes"]["image_base64_bytes"]
        );
        measurements.push(json!({"sample":sample["label"],"responses":pair}));
    }
    assert_eq!(calls.lock().unwrap().len(), 24);
    assert!(
        calls
            .lock()
            .unwrap()
            .iter()
            .all(|tool| matches!(tool.as_str(), "computer.observe" | "computer.image"))
    );
    host.stop().await;
    drop(stop);
    tokio::time::timeout(Duration::from_secs(10), server)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    println!(
        "R94_ACTUAL_MCP_FORMAT_MEASUREMENTS={}",
        json!({"scope":"archive-derived authenticated fake device only",
        "native_GUI_effect_verified":false,"production_agent_contacted":false,"input_sent":false,"synthetic_images":true,
        "synthetic_image_exact_content_verified":true,
        "original_native_payload_reconstructed":false,"default_projection_expected":compact,"samples":measurements})
    );
}
