#![cfg(unix)]
//! P: real MCP child + authenticated RPC, with explicit synthetic device peers.
//! No native OS input or platform success is inferred from these transport tests.
use rwi_core::device::{Call, DeviceResult, OperationRef, PROTOCOL_VERSION, Reply, Request};
use rwi_link_trust::{IdentityPaths, PeerPin};
use rwi_native_link::device::{AdapterProfile, PairedDevice};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
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

struct Host {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}

#[tokio::test]
async fn foreground_focus_real_mcp_routes_once_and_rejects_incomplete_args() {
    // Real MCP child and authenticated transport, synthetic peer only.
    // This cannot attest native foreground or grant native input authority.
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::set_permissions(&root, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let device = device(&root, "focus-device", false).await;
    let mut host = Host::start(&profile(&root, &[&device])).await;
    host.tool(2, "computer.targets", json!({})).await;
    let arguments = json!({
        "target": "target-focus-device", "lease": "fixture-lease",
        "reference": "fixture-window-reference", "request_id": "focus-transport-once",
        "expected_foreground": {"pid":7,"window":"cgwindow:77",
            "process_start":"fixture-process-start","native_epoch":"fixture-native-epoch"}
    });
    let result = host
        .tool(3, "computer.focus_window", arguments.clone())
        .await;
    assert_eq!(result["synthetic"], true, "{result}");
    let operation: OperationRef = serde_json::from_value(result["operation_ref"].clone()).unwrap();
    {
        let calls = device.calls.lock().unwrap();
        let prepared = calls
            .iter()
            .filter(|r| {
                matches!(&r.call,
            Call::Prepare { tool, arguments: sent }
                if tool == "computer.focus_window" && sent == &arguments)
            })
            .count();
        let executed = calls
            .iter()
            .filter(|r| {
                matches!(&r.call,
            Call::Execute { operation_ref } if operation_ref == &operation)
            })
            .count();
        assert_eq!(prepared, 1);
        assert_eq!(executed, 1);
    }
    let before = device.calls.lock().unwrap().len();
    let mut incomplete = arguments;
    incomplete.as_object_mut().unwrap().remove("lease");
    let rejected = host.tool(4, "computer.focus_window", incomplete).await;
    assert_eq!(rejected["code"], "invalid_arguments");
    assert_eq!(device.calls.lock().unwrap().len(), before);
    host.stop().await;
}

async fn http(
    address: &str,
    method: &str,
    path: &str,
    origin: &str,
    host: &str,
    body: &str,
) -> String {
    use tokio::io::AsyncReadExt;
    let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
    stream.write_all(format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nOrigin: {origin}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
    let mut out = String::new();
    tokio::time::timeout(Duration::from_secs(3), stream.read_to_string(&mut out))
        .await
        .unwrap()
        .unwrap();
    out
}

#[tokio::test]
async fn host_view_bridge_scopes_origin_page_sequence_and_mcp_eof() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::set_permissions(&root, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let d = device(&root, "live-device", false).await;
    let profile = profile(&root, &[&d]);
    let mut host = Host::start(&profile).await;
    let targets = host.tool(2, "computer.targets", json!({})).await;
    assert!(
        !targets["targets"].as_array().unwrap().is_empty(),
        "{targets}"
    );
    let live = host
        .tool(
            3,
            "computer.live",
            json!({"target":"target-live-device","action":"start","options":{"display":{"id":1}}}),
        )
        .await;
    let url = live["viewer"]["url"]
        .as_str()
        .unwrap_or_else(|| panic!("{live}"));
    let (address, path) = url
        .strip_prefix("http://")
        .unwrap()
        .split_once('/')
        .unwrap();
    let path = format!("/{path}");
    let origin = format!("http://{address}");
    let denied = http(address, "GET", &path, &origin, "attacker.invalid", "").await;
    assert!(denied.starts_with("HTTP/1.1 403"));
    let page = http(address, "GET", &path, &origin, address, "").await;
    assert!(page.starts_with("HTTP/1.1 200"));
    assert!(page.contains("frame-ancestors 'none'"));
    let nonce = page
        .split("const page='")
        .nth(1)
        .unwrap()
        .split('\'')
        .next()
        .unwrap();
    let endpoint = format!("{path}presence");
    let body = json!({"page":nonce,"sequence":2,"visible":true}).to_string();
    let before = d.calls.lock().unwrap().len();
    let denied = http(
        address,
        "POST",
        &endpoint,
        "https://attacker.invalid",
        address,
        &body,
    )
    .await;
    assert!(denied.starts_with("HTTP/1.1 403"));
    assert_eq!(d.calls.lock().unwrap().len(), before);
    let shown = http(address, "POST", &endpoint, &origin, address, &body).await;
    assert!(shown.starts_with("HTTP/1.1 200"));
    assert!(shown.contains("\"presentation_allowed\":true"));
    let before = d.calls.lock().unwrap().len();
    let stale = http(address, "POST", &endpoint, &origin, address, &body).await;
    assert!(stale.starts_with("HTTP/1.1 409"));
    assert!(stale.contains("stale_event"));
    assert_eq!(d.calls.lock().unwrap().len(), before);
    let forged=host.tool(4,"computer.live",json!({"target":"target-live-device","action":"status","view":{"view_id":"a".repeat(32),"sequence":1,"visible":true}})).await;
    assert_eq!(forged["code"], "host_view_reserved");
    assert_eq!(d.calls.lock().unwrap().len(), before);
    host.tool(
        5,
        "computer.live",
        json!({"target":"target-live-device","action":"stop"}),
    )
    .await;
    let ended = http(
        address,
        "POST",
        &endpoint,
        &origin,
        address,
        &json!({"page":nonce,"sequence":3,"visible":true}).to_string(),
    )
    .await;
    assert!(ended.starts_with("HTTP/1.1 410"));
    host.stop().await;
    assert!(tokio::net::TcpStream::connect(address).await.is_err());
}

#[tokio::test]
async fn semantic_only_snapshot_keeps_controls_without_fabricating_an_image() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::set_permissions(&root, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let d = device(&root, "semantic-device", false).await;
    let profile = profile(&root, &[&d]);
    let mut host = Host::start(&profile).await;
    host.tool(2, "computer.targets", json!({})).await;
    let s = host
        .tool(
            3,
            "computer.snapshot",
            json!({"target":"target-semantic-device"}),
        )
        .await;
    assert_eq!(s["frame"], "semantic-frame", "{s}");
    assert_eq!(s["visual_available"], false);
    assert!(s["view_id"].is_null());
    assert!(s["mapping"].is_null());
    assert_eq!(s["elements"][0]["id"], "ok");
    assert!(
        !d.calls
            .lock()
            .unwrap()
            .iter()
            .any(|r| matches!(&r.call,Call::Prepare{tool,..} if tool=="computer.image"))
    );
    host.stop().await;
}
#[tokio::test]
async fn snapshot_default_priority_and_explicit_pages_use_real_mcp_process() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::set_permissions(&root, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let d = device(&root, "priority-device", false).await;
    let profile = profile(&root, &[&d]);
    let mut host = Host::start(&profile).await;
    host.tool(
        2,
        "computer.targets",
        json!({"device_id":"priority-device"}),
    )
    .await;
    let snapshot = host
        .tool(
            3,
            "computer.snapshot",
            json!({"target":"target-priority-device","limit":1}),
        )
        .await;
    assert_eq!(snapshot["elements"][0]["id"], "close");
    assert_eq!(snapshot["elements"][0]["point"], json!([1841, 43]));
    assert_eq!(snapshot["elements"][0]["actions"], json!(["AXPress"]));
    assert_eq!(snapshot["available_controls"], 65);
    assert_eq!(snapshot["omitted_controls"], 64);
    let wide = host
        .tool(
            20,
            "computer.snapshot",
            json!({"target":"target-priority-device","limit":64}),
        )
        .await;
    assert_eq!(wide["elements"].as_array().unwrap().len(), 1);
    assert_eq!(wide["available_controls"], 65);
    assert_eq!(wide["omitted_controls"], 64);
    assert!(
        wide["control_projection"]
            .as_str()
            .unwrap()
            .contains("computer.controls")
    );
    assert_eq!(snapshot["coverage"]["complete"], false);
    assert_eq!(snapshot["visual_available"], false);
    assert!(snapshot["view_id"].is_null());
    let reference = snapshot["observation_ref"].clone();
    let first = host
        .tool(
            4,
            "computer.controls",
            json!({"observation_ref":reference,"frame":"priority-frame","limit":1}),
        )
        .await;
    assert_eq!(first["elements"][0]["id"], "row-0");
    assert_eq!(first["next_offset"], 1);
    let close=host.tool(5,"computer.controls",json!({"observation_ref":reference,"frame":"priority-frame","query":"AXButton","limit":1})).await;
    assert_eq!(close["elements"][0]["id"], "close");
    assert_eq!(close["elements"][0]["point"], json!([1841, 43]));
    assert_eq!(close["matches"], 1);
    let mut original_rows = Vec::new();
    let mut offset = 0;
    let mut id = 6;
    loop {
        let page=host.tool(id,"computer.result",json!({"result_ref":reference,"pointer":"/scene/elements","limit":16,"offset":offset})).await;
        assert_eq!(page["total"], 65, "{page}");
        assert_eq!(page["native_dispatched"], false);
        let rows = page["value"]
            .as_array()
            .unwrap_or_else(|| panic!("invalid retained page: {page}"));
        assert!(!rows.is_empty());
        original_rows.extend(rows.iter().cloned());
        if let Some(next) = page["next_offset"].as_u64() {
            assert!(next > offset);
            offset = next;
            id += 1;
        } else {
            break;
        }
    }
    assert_eq!(original_rows.len(), 65);
    assert_eq!(original_rows[0]["id"], "row-0");
    assert_eq!(original_rows[64]["id"], "close");
    assert!(d.calls.lock().unwrap().iter().all(|r|!matches!(&r.call,Call::Prepare{tool,..} if tool=="computer.image"||tool=="computer.act"||tool=="computer.claim")));
    host.stop().await;
}
impl Host {
    async fn start(profile: &std::path::Path) -> Self {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_rwi-mcp"))
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
        host.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"rpc-test","version":"1"}}})).await;
        assert!(host.read().await.get("result").is_some());
        host.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        host
    }
    async fn send(&mut self, v: Value) {
        self.input
            .as_mut()
            .unwrap()
            .write_all(format!("{v}\n").as_bytes())
            .await
            .unwrap();
    }
    async fn read(&mut self) -> Value {
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(10), self.output.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        serde_json::from_str(&line).unwrap()
    }
    async fn tool(&mut self, id: u32, name: &str, args: Value) -> Value {
        self.send(tool(id, name, args)).await;
        let v = self.read().await;
        assert_eq!(v["id"], id);
        v["result"]["structuredContent"].clone()
    }
    async fn stop(&mut self) {
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
fn tool(id: u32, name: &str, args: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})
}
struct Device {
    paired: PairedDevice,
    calls: Arc<Mutex<Vec<Request>>>,
    cancelled: Arc<Mutex<BTreeSet<String>>>,
    accepted: Arc<Mutex<BTreeMap<String, Value>>>,
    _stop: tokio::sync::watch::Sender<bool>,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
}
impl Drop for Device {
    fn drop(&mut self) {
        self.server.abort();
    }
}
async fn device(root: &std::path::Path, id: &str, slow_close: bool) -> Device {
    let path = root.join(id);
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .unwrap();
    let provisioned = rwi_link_trust::provision(&path.join("trust")).unwrap();
    let identity = IdentityPaths::in_profile(&path.join("trust/agent"))
        .load()
        .unwrap();
    let tls = identity
        .server_config(std::slice::from_ref(&provisioned.model.certificate_sha256))
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let paired = PairedDevice {
        device_id: id.into(),
        endpoint: listener.local_addr().unwrap(),
        identity: IdentityPaths::in_profile(&path.join("trust/model")),
        agent_pin: provisioned.agent.certificate_sha256,
    };
    let calls = Arc::new(Mutex::new(Vec::new()));
    let cancelled = Arc::new(Mutex::new(BTreeSet::new()));
    let seen = calls.clone();
    let accepted: Arc<Mutex<BTreeMap<String, Value>>> = Default::default();
    let records = accepted.clone();
    let cancel = cancelled.clone();
    let (stop, rx) = tokio::sync::watch::channel(false);
    let name = id.to_owned();
    let window_targets = Arc::new(Mutex::new(vec![json!({"target":format!("target-{id}")})]));
    let server = tokio::spawn(rwi_native_link::device::serve(
        listener,
        tls,
        move |_peer: PeerPin, r: Request| {
            let records = records.clone();
            let seen = seen.clone();
            let cancel = cancel.clone();
            let name = name.clone();
            let window_targets = window_targets.clone();
            async move {
                seen.lock().unwrap().push(r.clone());
                let result = match &r.call {
                    Call::Describe => {
                        let mut metadata = json!({"availability":"available","service":"healthy","context_valid":true});
                        if name == "window-device" {
                            metadata["targets"] = json!(*window_targets.lock().unwrap());
                        }
                        DeviceResult::value(metadata)
                    }
                    Call::OpenContext => DeviceResult::value(
                        json!({"context":format!("context-{name}"),"targets":[{"target":format!("target-{name}")}]}),
                    ),
                    Call::Prepare { tool, arguments } => {
                        let operation_ref = OperationRef {
                            ref_version: 1,
                            device_id: name.clone(),
                            agent_instance: "test-instance".into(),
                            context_generation: format!("context-{name}"),
                            execution_nonce: uuid::Uuid::new_v4().to_string(),
                        };
                        records.lock().unwrap().insert(
                            operation_ref.execution_nonce.clone(),
                            if name == "window-device"
                                || name == "priority-device"
                                || name.starts_with("degraded-")
                            {
                                json!({"tool":tool,"arguments":arguments})
                            } else {
                                arguments.clone()
                            },
                        );
                        if arguments["operation_id"] == "hold-prepare" {
                            std::future::pending::<()>().await;
                        }
                        DeviceResult::value(json!({"accepted":true,"operation_ref":operation_ref}))
                    }
                    Call::Cancel { operation_ref } => {
                        cancel
                            .lock()
                            .unwrap()
                            .insert(operation_ref.execution_nonce.clone());
                        DeviceResult::value(
                            json!({"cancel_requested":true,"terminal":false,"resources_released":false}),
                        )
                    }
                    Call::Execute { operation_ref } => {
                        let arguments =
                            records.lock().unwrap()[&operation_ref.execution_nonce].clone();
                        if name.starts_with("degraded-") {
                            let tool = arguments["tool"].as_str().unwrap();
                            let mut result = if tool == "computer.observe" {
                                DeviceResult::error(
                                    "ax_attribute_unavailable",
                                    "fixture AXWindows cannotComplete; no native GUI",
                                )
                            } else if tool == "computer.capture_visual" && name != "degraded-device"
                            {
                                let mut v = DeviceResult::value(
                                    json!({"frame_created":false,"input_authority_granted":false,"native_dispatch_entered":false,"operation":{"terminal":true,"native_call_pending":false,"resources_released":true,"resource_release_uncertain":false},"capture":{"window":{"window":"cgwindow:fixture","pid":7}},"synthetic":true}),
                                );
                                v.images.push(rwi_core::device::DeviceImage {mime:"image/png".into(),base64:"iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Wl6UwAAAABJRU5ErkJggg==".into()});
                                if name == "degraded-poison" {
                                    v.is_error = true;
                                    v.data["code"] = json!("fixture_capture_wrong_binding");
                                }
                                v
                            } else {
                                DeviceResult::error(
                                    "fixture_capture_unavailable",
                                    "fixture stops after visual request; no image or input inferred",
                                )
                            };
                            if tool == "computer.observe" {
                                result.data["diagnostic"] = json!({"api":"AXUIElementCopyAttributeValue","attribute":"AXWindows","ax_error":-25204,"operation_kind":"read","replay_allowed":false});
                                result.data["native_dispatch_entered"] = json!(false);
                                result.data["operation"] = json!({"terminal":true,"native_call_pending":false,"resources_released":true,"resource_release_uncertain":false});
                            }
                            return Reply {
                                version: PROTOCOL_VERSION,
                                id: r.id,
                                device: r.device,
                                instance: "test-instance".into(),
                                result,
                            };
                        }
                        if name == "priority-device" {
                            let tool = arguments["tool"].as_str().unwrap();
                            let data = if tool == "computer.observe" {
                                let mut elements:Vec<Value>=(0..64).map(|i|json!({"id":format!("row-{i}"),"name":"","role":"AXRow","actions":["AXShowDefaultUI","AXShowAlternateUI"],"point":[1941.5,1596.0+i as f64],"visible":false,"occluded":true})).collect();
                                elements.push(json!({"id":"close","name":"","role":"AXButton","actions":["AXPress"],"point":[1841,43],"enabled":true}));
                                json!({"frame":"priority-frame","artifact_id":null,"scene":{"image_size":[0,0],"blocked":false,"mutation_ready":false,"elements":elements},"adapter_status":{"observation_consistency":{"ax_coverage_after":{"complete":false,"absence_proven":false,"reasons":["node_budget"]}}},"synthetic":true,"operation_ref":operation_ref})
                            } else {
                                json!({"code":"synthetic_unsupported"})
                            };
                            return Reply {
                                version: PROTOCOL_VERSION,
                                id: r.id,
                                device: r.device,
                                instance: "test-instance".into(),
                                result: DeviceResult::value(data),
                            };
                        }
                        if name == "window-device" {
                            let tool = arguments["tool"].as_str().unwrap();
                            let args = &arguments["arguments"];
                            let data = match tool {
                                "computer.attach_window" => {
                                    let target = format!(
                                        "target-{name}:window:{}",
                                        operation_ref.execution_nonce
                                    );
                                    window_targets.lock().unwrap().push(json!({"target":target,"parent_target":args["target"],"window":args["selection"]}));
                                    json!({"target":target,"parent_target":args["target"],"attached":true,"source_binding_changed":false})
                                }
                                "computer.detach_window" => {
                                    window_targets
                                        .lock()
                                        .unwrap()
                                        .retain(|row| row["target"] != args["target"]);
                                    json!({"target":args["target"],"detached":true})
                                }
                                "computer.live" => {
                                    json!({"host_view_binding":args["options"]["host_view_id"],"expires_in_ms":60000,"presentation_allowed":args["view"]["visible"]})
                                }
                                "computer.observe" => {
                                    json!({"frame":format!("frame-{}",args["target"].as_str().unwrap()),"artifact_id":null,"scene":{"image_size":[0,0],"blocked":false,"mutation_ready":true,"binding":{"scope":{"window":"cgwindow:22","pid":7},"mutation_context":{"window_title":"Synthetic second window"}},"elements":[{"id":"button","name":"OK","role":"AXButton","actions":["AXPress"]}]},"adapter_status":{}})
                                }
                                _ => json!({"code":"synthetic_unsupported"}),
                            };
                            return Reply {
                                version: PROTOCOL_VERSION,
                                id: r.id,
                                device: r.device,
                                instance: "test-instance".into(),
                                result: DeviceResult::value(data),
                            };
                        }
                        if name == "live-device" || name == "semantic-device" {
                            let data = if name == "semantic-device" {
                                json!({"frame":"semantic-frame","artifact_id":null,"scene":{"image_size":[0,0],"blocked":false,"mutation_ready":true,"elements":[{"id":"ok","name":"OK","role":"AXButton","actions":["AXPress"]}]},"adapter_status":{"input_proposal_max_age_ms":120000},"synthetic":true,"operation_ref":operation_ref})
                            } else if arguments.get("view").is_some() {
                                json!({"accepted":true,"presentation_allowed":arguments["view"]["visible"],"sequence":arguments["view"]["sequence"],"synthetic":true,"operation_ref":operation_ref})
                            } else {
                                json!({"host_view_binding":arguments["options"]["host_view_id"],"status":"running","presentation_suspended":true,"expires_in_ms":60000,"synthetic":true,"operation_ref":operation_ref})
                            };
                            return Reply {
                                version: PROTOCOL_VERSION,
                                id: r.id,
                                device: r.device,
                                instance: "test-instance".into(),
                                result: DeviceResult::value(data),
                            };
                        }
                        if name == "repl-device" {
                            if arguments.get("ttl_ms").is_some() {
                                return Reply {
                                    version: PROTOCOL_VERSION,
                                    id: r.id,
                                    device: r.device,
                                    instance: "test-instance".into(),
                                    result: DeviceResult::value(
                                        json!({"lease":"repl-fixture-lease","operation_ref":operation_ref}),
                                    ),
                                };
                            }
                            if arguments
                                .as_object()
                                .is_some_and(|o| o.len() == 1 && o.contains_key("lease"))
                            {
                                return Reply {
                                    version: PROTOCOL_VERSION,
                                    id: r.id,
                                    device: r.device,
                                    instance: "test-instance".into(),
                                    result: DeviceResult::value(
                                        json!({"released":true,"operation_ref":operation_ref}),
                                    ),
                                };
                            }
                            if arguments["operation_id"] == "sdk-observe"
                                || arguments["operation_id"] == "sdk-blocked-observe"
                                || arguments["request_id"] == "sdk-action"
                            {
                                return Reply {
                                    version: PROTOCOL_VERSION,
                                    id: r.id,
                                    device: r.device,
                                    instance: "test-instance".into(),
                                    result: DeviceResult::value(
                                        json!({"frame":"fixture-frame","scene":{"mutation_ready":arguments["operation_id"] != "sdk-blocked-observe","blocked":arguments["operation_id"] == "sdk-blocked-observe","elements":[{"id":"fixture-element"}]},"system_accepted":arguments["request_id"] == "sdk-action","goal_verified":false,"operation_ref":operation_ref}),
                                    ),
                                };
                            }
                        }
                        let label = arguments["operation_id"].as_str().unwrap_or("none");
                        if label.starts_with("slow") {
                            loop {
                                if cancel
                                    .lock()
                                    .unwrap()
                                    .contains(&operation_ref.execution_nonce)
                                {
                                    break;
                                }
                                tokio::time::sleep(Duration::from_millis(5)).await;
                            }
                        }
                        DeviceResult::value(
                            json!({"operation_ref":operation_ref,"correlation_id":label,"device":name,"synthetic":true}),
                        )
                    }
                    Call::Operation { selector } => {
                        DeviceResult::value(json!({"selector":selector,"device":name}))
                    }
                    Call::CloseContext => {
                        if slow_close {
                            std::future::pending::<()>().await;
                        }
                        DeviceResult::value(json!({"closed":true}))
                    }
                    _ => DeviceResult::error("synthetic_unsupported", "Transport test only"),
                };
                Reply {
                    version: PROTOCOL_VERSION,
                    id: r.id,
                    device: r.device,
                    instance: "test-instance".into(),
                    result,
                }
            }
        },
        rx,
    ));
    Device {
        paired,
        calls,
        cancelled,
        accepted,
        _stop: stop,
        server,
    }
}
fn profile(root: &std::path::Path, devices: &[&Device]) -> std::path::PathBuf {
    let p = root.join("adapter.json");
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&p)
        .unwrap();
    f.write_all(
        &serde_json::to_vec(&AdapterProfile {
            devices: devices.iter().map(|d| d.paired.clone()).collect(),
        })
        .unwrap(),
    )
    .unwrap();
    p
}

#[tokio::test]
async fn selected_targets_probe_only_the_requested_paired_endpoint() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("private");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let a = device(&root, "device-a", false).await;
    let b = device(&root, "device-b", false).await;
    let mut host = Host::start(&profile(&root, &[&a, &b])).await;

    let selected = host
        .tool(2, "computer.targets", json!({"device_id":"device-a"}))
        .await;
    assert_eq!(selected["targets"].as_array().unwrap().len(), 1);
    assert_eq!(selected["targets"][0]["target"], "target-device-a");
    assert_eq!(selected["devices"].as_array().unwrap().len(), 1);
    assert_eq!(selected["devices"][0]["device_id"], "device-a");
    assert!(b.calls.lock().unwrap().is_empty());

    let unknown = host
        .tool(3, "computer.targets", json!({"device_id":"device-missing"}))
        .await;
    assert_eq!(unknown["code"], "device_unknown");
    let malformed = host
        .tool(4, "computer.targets", json!({"device_id":"!"}))
        .await;
    assert_eq!(malformed["code"], "invalid_arguments");
    assert!(b.calls.lock().unwrap().is_empty());

    let all = host.tool(5, "computer.targets", json!({})).await;
    assert_eq!(all["targets"].as_array().unwrap().len(), 2);
    assert!(!b.calls.lock().unwrap().is_empty());
    host.stop().await;
}
async fn wait_until(mut f: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !f() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn js_native_call_stop_preserves_exact_reference_without_next_dispatch() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("private");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let d = device(&root, "repl-device", false).await;
    let mut host = Host::start(&profile(&root, &[&d])).await;
    host.tool(2, "computer.targets", json!({})).await;
    host.send(tool(3,"computer.js",json!({"code":"await rcu.call('computer.observe', {target:'target-repl-device', operation_id:'slow-js', inline_image:false}); await rcu.call('computer.observe', {target:'target-repl-device', operation_id:'must-not-dispatch', inline_image:false});"}))).await;
    wait_until(|| executed_label(&d, "slow-js")).await;
    let exact = nonce(&d, "slow-js");
    host.send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":3,"reason":"Host Stop nested native operation"}})).await;
    let result = host.read().await;
    let receipt = &result["result"]["structuredContent"];
    assert_eq!(receipt["state_retained"], false, "{result}");
    assert_eq!(receipt["calls"].as_array().unwrap().len(), 1);
    assert_eq!(
        receipt["calls"][0]["operation_ref"]["execution_nonce"],
        exact
    );
    assert_eq!(receipt["calls"][0]["cancellation_delivery_confirmed"], true);
    assert!(d.cancelled.lock().unwrap().contains(&exact));
    assert!(!executed_label(&d, "must-not-dispatch"));
    assert_eq!(d.calls.lock().unwrap().iter().filter(|r| matches!(&r.call, Call::Execute{operation_ref} if operation_ref.execution_nonce == exact)).count(),1);
    let status = host.tool(4,"computer.operation",json!({"selector":{"kind":"execution","operation_ref":receipt["calls"][0]["operation_ref"]}})).await;
    assert_eq!(status["device"], "repl-device");
    host.stop().await;
}

#[tokio::test]
async fn js_sdk_batch_and_parent_lease_cleanup_use_real_authenticated_fixture_rpc() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("private");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let d = device(&root, "repl-device", false).await;
    let mut host = Host::start(&profile(&root, &[&d])).await;
    host.tool(2, "computer.targets", json!({})).await;
    let result = host.tool(3,"computer.js",json!({"code":"state.win = rcu.window('target-repl-device'); await state.win.observe({operation_id:'sdk-observe'}); await state.win.withInput(async b => { await b.act({element:'fixture-element',intent:'click',requestId:'sdk-action'}); const observation = await b.observe({operation_id:'sdk-observe'}); emit(observation.data.frame); });"})).await;
    assert!(result["error"].is_null(), "{result}");
    assert_eq!(result["state_retained"], true, "{result}");
    assert_eq!(result["outputs"], json!(["fixture-frame"]));
    assert_eq!(
        result["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["tool"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "computer.observe",
            "computer.claim",
            "computer.act_checked",
            "computer.observe",
            "computer.release"
        ]
    );
    assert_eq!(result["calls"][2]["request_id"], "sdk-action");
    assert_eq!(result["calls"][2]["goal_verified"], false);
    assert_eq!(result["cleanup"], json!([]));
    let failed = host.tool(4,"computer.js",json!({"code":"await rcu.call('computer.claim', {target:'target-repl-device',ttl_ms:30000}); throw new Error('after-claim');"})).await;
    assert_eq!(failed["state_retained"], false);
    assert_eq!(failed["cleanup"][0]["lease"], "repl-fixture-lease");
    assert_eq!(failed["cleanup"][0]["result"]["released"], true, "{failed}");
    host.stop().await;
}

#[tokio::test]
async fn js_sdk_rejects_observed_blocked_state_before_any_claim_or_action_rpc() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("private");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let d = device(&root, "repl-device", false).await;
    let mut host = Host::start(&profile(&root, &[&d])).await;
    host.tool(2, "computer.targets", json!({})).await;
    let result=host.tool(3,"computer.js",json!({"code":"state.win=rcu.window('target-repl-device');await state.win.observe({operation_id:'sdk-blocked-observe'});await state.win.withInput(b=>b.act({element:'fixture-element',intent:'click',requestId:'must-not-send'}));"})).await;
    assert!(
        result["error"]
            .as_str()
            .unwrap()
            .contains("client_observation_not_ready"),
        "{result}"
    );
    assert_eq!(result["calls"].as_array().unwrap().len(), 1);
    assert_eq!(result["calls"][0]["tool"], "computer.observe");
    assert_eq!(result["cleanup"], json!([]));
    assert_eq!(result["state_retained"], false);
    host.stop().await;
}
#[tokio::test]
async fn multi_device_collision_cancellation_and_bounded_eof() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("private");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let a = device(&root, "device-a", true).await;
    let b = device(&root, "device-b", true).await;
    let mut host = Host::start(&profile(&root, &[&a, &b])).await;
    let targets = host.tool(2, "computer.targets", json!({})).await;
    assert_eq!(targets["targets"].as_array().unwrap().len(), 2, "{targets}");
    let mut references = Vec::new();
    for (id, device) in [(3, "a"), (4, "b")] {
        let result=host.tool(id,"computer.observe",json!({"target":format!("target-device-{device}"),"operation_id":"same-id","inline_image":false})).await;
        references.push(result["operation_ref"].clone());
    }
    assert_ne!(references[0], references[1]);
    assert_eq!(
        host.tool(5, "computer.operation", json!({"operation_id":"same-id"}))
            .await["code"],
        "invalid_arguments"
    );
    assert_eq!(
        host.tool(
            6,
            "computer.operation",
            json!({"selector":{"kind":"execution","operation_ref":references[0]}})
        )
        .await["device"],
        "device-a"
    );
    host.send(tool(
        7,
        "computer.observe",
        json!({"target":"target-device-a","operation_id":"slow-cancel","inline_image":false}),
    ))
    .await;
    wait_until(|| executed_label(&a, "slow-cancel")).await;
    let cancel_nonce = nonce(&a, "slow-cancel");
    host.send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":7,"reason":"Host Stop"}})).await;
    wait_until(|| a.cancelled.lock().unwrap().contains(&cancel_nonce)).await;
    assert!(!b.cancelled.lock().unwrap().contains(&cancel_nonce));
    assert!(
        a.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| matches!(&r.call,Call::Cancel{operation_ref} if operation_ref.execution_nonce==cancel_nonce))
            .all(|r| r.context.is_none())
    );
    let response = host.read().await;
    assert_eq!(response["id"], 7);
    host.send(tool(
        8,
        "computer.observe",
        json!({"target":"target-device-b","operation_id":"slow-eof","inline_image":false}),
    ))
    .await;
    wait_until(|| executed_label(&b, "slow-eof")).await;
    let eof_nonce = nonce(&b, "slow-eof");
    let began = std::time::Instant::now();
    host.stop().await;
    assert!(
        began.elapsed() < Duration::from_secs(6),
        "EOF cleanup must be one bounded parallel budget, not per device"
    );
    assert!(
        b.cancelled.lock().unwrap().contains(&eof_nonce),
        "dropping the Host waiter must request native cancellation"
    );
    for d in [&a, &b] {
        let calls = d.calls.lock().unwrap();
        for id in ["same-id", "slow-cancel", "slow-eof"] {
            assert!(calls.iter().filter(|r|matches!(&r.call,Call::Prepare{arguments,..} if arguments["operation_id"]==id)).count()<=1,"original operation replayed");
        }
    }
}

fn nonce(d: &Device, label: &str) -> String {
    d.accepted
        .lock()
        .unwrap()
        .iter()
        .find(|(_, v)| v["operation_id"] == label)
        .unwrap()
        .0
        .clone()
}
fn executed_label(d: &Device, label: &str) -> bool {
    let records = d.accepted.lock().unwrap();
    let Some((nonce, _)) = records.iter().find(|(_, v)| v["operation_id"] == label) else {
        return false;
    };
    d.calls.lock().unwrap().iter().any(
        |r| matches!(&r.call,Call::Execute{operation_ref} if &operation_ref.execution_nonce==nonce),
    )
}

#[tokio::test]
async fn actual_host_eof_before_prepare_reply_never_sends_execute() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("private");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let device = device(&root, "device-a", false).await;
    let mut host = Host::start(&profile(&root, &[&device])).await;
    host.tool(2, "computer.targets", json!({})).await;
    host.send(tool(
        3,
        "computer.observe",
        json!({"target":"target-device-a","operation_id":"hold-prepare","inline_image":false}),
    ))
    .await;
    wait_until(|| !device.accepted.lock().unwrap().is_empty()).await;
    host.stop().await;
    assert!(
        !device
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|r| matches!(r.call, Call::Execute { .. }))
    );
    assert!(
        device.cancelled.lock().unwrap().is_empty(),
        "no invented reference before acceptance response"
    );
}

/// Actual production Runtime + mTLS, explicitly using the synthetic backend.
async fn runtime_device(root: &std::path::Path) -> Device {
    use rwi_agent::config::{Config, PlatformPolicy, Principal};
    let path = root.join("real-runtime");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .unwrap();
    let provisioned = rwi_link_trust::provision(&path.join("trust")).unwrap();
    let identity_paths = IdentityPaths::in_profile(&path.join("trust/agent"));
    let tls = identity_paths
        .load()
        .unwrap()
        .server_config(std::slice::from_ref(&provisioned.model.certificate_sha256))
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap();
    let runtime = rwi_agent::runtime::Runtime::new(Config {
        version: 1,
        device_id: "runtime-device".into(),
        bind: endpoint,
        identity: identity_paths,
        state_root: path.join("state"),
        platform: PlatformPolicy::Fixture,
        principals: vec![Principal {
            journal_identity: None,
            certificate: provisioned.model.certificate_sha256,
            observe: true,
            input: true,
        }],
    })
    .unwrap();
    let paired = PairedDevice {
        device_id: "runtime-device".into(),
        endpoint,
        identity: IdentityPaths::in_profile(&path.join("trust/model")),
        agent_pin: provisioned.agent.certificate_sha256,
    };
    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let (stop, rx) = tokio::sync::watch::channel(false);
    let server = tokio::spawn(rwi_native_link::device::serve(
        listener,
        tls,
        move |peer, request: Request| {
            seen.lock().unwrap().push(request.clone());
            runtime.clone().handle(peer, request)
        },
        rx,
    ));
    Device {
        paired,
        calls,
        cancelled: Default::default(),
        accepted: Default::default(),
        _stop: stop,
        server,
    }
}

#[tokio::test]
async fn retained_metadata_reads_do_not_repeat_native_work_or_cross_adapter_instances() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("private");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let device = runtime_device(&root).await;
    let profile = profile(&root, &[&device]);
    let mut first = Host::start(&profile).await;
    let targets = first.tool(2, "computer.targets", json!({})).await;
    let observation = first
        .tool(
            3,
            "computer.observe",
            json!({"target":targets["targets"][0]["target"],"inline_image":false}),
        )
        .await;
    let reference = observation["presentation"]["result_ref"].clone();
    assert!(reference.is_string());
    let native_count = device.calls.lock().unwrap().len();
    let retained = first
        .tool(
            4,
            "computer.result",
            json!({"result_ref":reference,"pointer":"/operation_ref"}),
        )
        .await;
    assert_eq!(retained["value"], observation["operation_ref"]);
    assert_eq!(retained["native_dispatched"], false);
    assert_eq!(device.calls.lock().unwrap().len(), native_count);
    let mut second = Host::start(&profile).await;
    assert_eq!(
        second
            .tool(2, "computer.result", json!({"result_ref":reference}))
            .await["code"],
        "result_ref_expired"
    );
    assert_eq!(device.calls.lock().unwrap().len(), native_count);
    first
        .send(tool(
            5,
            "computer.result",
            json!({"result_ref":reference,"pointer":"/frame"}),
        ))
        .await;
    let reply = first.read().await;
    let text: Value =
        serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, reply["result"]["structuredContent"]);
    assert_eq!(text["value"], observation["frame"]);
    first.stop().await;
    second.stop().await;
}

#[tokio::test]
async fn crossing_request_and_monitor_ids_stay_correct_across_actual_mcp_hosts() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("private");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let device = runtime_device(&root).await;
    let profile = profile(&root, &[&device]);
    let mut first = Host::start(&profile).await;
    let targets = first.tool(2, "computer.targets", json!({})).await;
    let target = &targets["targets"][0]["target"];
    assert!(target.is_string(), "{targets}");
    let lease = first
        .tool(3, "computer.claim", json!({"target":target,"ttl_ms":60000}))
        .await;
    for (id, request, monitor, value) in [
        (4, "req-a", "op-b", "action A"),
        (6, "op-b", "op-c", "action B"),
    ] {
        let observation = first
            .tool(
                id,
                "computer.observe",
                json!({"target":target,"inline_image":false}),
            )
            .await;
        let result=first.tool(id+1,"computer.act_checked",json!({"target":target,"frame":observation["frame"],"lease":lease["lease"],"element":"Input","request_id":request,"operation_id":monitor,"intent":"text","value":value})).await;
        assert_eq!(result["dispatch"], "returned", "{result}");
    }
    assert_eq!(
        first
            .tool(8, "computer.operation", json!({"operation_id":"op-b"}))
            .await["code"],
        "invalid_arguments"
    );
    let read = first
        .tool(
            9,
            "computer.observe",
            json!({"target":target,"correlation_id":"op-b","inline_image":false}),
        )
        .await;
    let reference = read["operation_ref"].clone();
    // Fresh Host has no volatile routing table, but an exact live reference still routes.
    let mut second = Host::start(&profile).await;
    let execution_query = json!({"selector":{"kind":"execution","operation_ref":reference}});
    assert_eq!(
        second
            .tool(2, "computer.operation", execution_query.clone())
            .await["terminal"],
        true
    );
    assert_eq!(
        first
            .tool(11, "computer.release", json!({"lease":lease["lease"]}))
            .await["released"],
        true
    );
    first.stop().await;
    assert_eq!(
        second.tool(3, "computer.operation", execution_query).await["code"],
        "execution_ref_expired"
    );
    let query = |id: &str| json!({"selector":{"kind":"request","device_id":"runtime-device","request_id":id}});
    let a = second.tool(4, "computer.operation", query("req-a")).await;
    let b = second.tool(5, "computer.operation", query("op-b")).await;
    assert_eq!(a["reservation"]["operation_id"], "req-a");
    assert_eq!(b["reservation"]["operation_id"], "op-b");
    assert_eq!(a["result"]["data"]["correlation_id"], "op-b");
    assert_eq!(b["result"]["data"]["correlation_id"], "op-c");
    assert_eq!(a["replay_allowed"], false);
    assert_eq!(b["replay_allowed"], false);
    second.stop().await;
    assert_eq!(
        device
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| matches!(&r.call,Call::Prepare{tool,..} if tool=="computer.act_checked"))
            .count(),
        2,
        "queries/reconnect must not replay either mutation"
    );
}

#[tokio::test]
async fn independent_window_handles_route_through_actual_mcp_tls_and_refresh_catalog() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::set_permissions(&root, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let d = device(&root, "window-device", false).await;
    let other = device(&root, "other-device", false).await;
    let profile = profile(&root, &[&d, &other]);
    let mut host = Host::start(&profile).await;
    host.send(json!({"jsonrpc":"2.0","id":90,"method":"tools/list","params":{}}))
        .await;
    let listed = host.read().await;
    for name in ["computer.attach_window", "computer.detach_window"] {
        let definition = listed["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == name)
            .unwrap();
        assert!(definition["title"].is_string());
        assert!(definition["inputSchema"]["properties"]["target"].is_object());
    }
    host.tool(2, "computer.targets", json!({})).await;
    let attached=host.tool(3,"computer.attach_window",json!({"target":"target-window-device","selection":{"window":"cgwindow:22","pid":7,"native_epoch":"fixture-epoch","relation":"initial"}})).await;
    let target = attached["target"].as_str().unwrap();
    assert_ne!(target, "target-window-device");
    let snapshot = host
        .tool(4, "computer.snapshot", json!({"target":target}))
        .await;
    assert_eq!(snapshot["window"]["title"], "Synthetic second window");
    let refreshed = host
        .tool(5, "computer.targets", json!({"device_id":"window-device"}))
        .await;
    assert_eq!(refreshed["targets"].as_array().unwrap().len(), 2);
    let root_live=host.tool(40,"computer.live",json!({"target":"target-window-device","action":"start","options":{"display":{"id":1}}})).await;
    let child_live = host
        .tool(
            41,
            "computer.live",
            json!({"target":target,"action":"start","options":{"display":{"id":1}}}),
        )
        .await;
    let (address, child_path) = child_live["viewer"]["url"]
        .as_str()
        .unwrap()
        .strip_prefix("http://")
        .unwrap()
        .split_once('/')
        .unwrap();
    let (_, root_path) = root_live["viewer"]["url"]
        .as_str()
        .unwrap()
        .strip_prefix("http://")
        .unwrap()
        .split_once('/')
        .unwrap();
    let origin = format!("http://{address}");
    let before = d.calls.lock().unwrap().len();
    let other_before = other.calls.lock().unwrap().len();
    let foreign = host
        .tool(
            6,
            "computer.observe",
            json!({"target":target,"device_id":"other-device"}),
        )
        .await;
    // observe has no device override: schema or routing must reject without RPC.
    assert!(matches!(
        foreign["code"].as_str(),
        Some("invalid_arguments" | "foreign_resource")
    ));
    assert_eq!(d.calls.lock().unwrap().len(), before);
    assert_eq!(other.calls.lock().unwrap().len(), other_before);
    let detached = host
        .tool(7, "computer.detach_window", json!({"target":target}))
        .await;
    assert_eq!(detached["detached"], true);
    let before = d.calls.lock().unwrap().len();
    let ended = http(
        address,
        "GET",
        &format!("/{child_path}"),
        &origin,
        address,
        "",
    )
    .await;
    assert!(ended.starts_with("HTTP/1.1 410"));
    assert_eq!(d.calls.lock().unwrap().len(), before);
    let retained = http(
        address,
        "GET",
        &format!("/{root_path}"),
        &origin,
        address,
        "",
    )
    .await;
    assert!(retained.starts_with("HTTP/1.1 200"));

    let refreshed = host
        .tool(8, "computer.targets", json!({"device_id":"window-device"}))
        .await;
    assert_eq!(refreshed["targets"].as_array().unwrap().len(), 1);
    let calls = d.calls.lock().unwrap().clone();
    assert_eq!(
        calls
            .iter()
            .filter(|r| matches!(r.call, Call::OpenContext))
            .count(),
        1
    );
    assert_eq!(
        calls
            .iter()
            .filter(|r| matches!(&r.call,Call::Prepare{tool,..} if tool=="computer.attach_window"))
            .count(),
        1
    );
    assert!(!calls.iter().any(|r|matches!(&r.call,Call::Prepare{tool,..} if tool=="computer.bind_window"||tool=="computer.act"||tool=="computer.act_checked")));
    host.stop().await;
}

#[tokio::test]
async fn r20_degraded_ax_snapshot_attempts_only_exact_readonly_visual() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    std::fs::set_permissions(&root, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let d = device(&root, "degraded-device", false).await;
    let mut host = Host::start(&profile(&root, &[&d])).await;
    host.tool(2, "computer.targets", json!({})).await;
    let result = host
        .tool(
            3,
            "computer.snapshot",
            json!({"target":"target-degraded-device","max_edge":2048}),
        )
        .await;
    let prepared: Vec<_> = d
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter_map(|r| match &r.call {
            Call::Prepare { tool, arguments } => Some((tool.clone(), arguments.clone())),
            _ => None,
        })
        .collect();
    println!("degraded_readonly_calls={prepared:?}; response={result}");
    host.stop().await;
    assert_eq!(
        prepared.len(),
        2,
        "must retain AX diagnostic but attempt one exact-target visual read; no image success is inferred from this fixture"
    );
    assert_eq!(prepared[0].0, "computer.observe");
    assert_eq!(prepared[1].0, "computer.capture_visual");
    assert_eq!(prepared[1].1["target"], "target-degraded-device");
    assert_eq!(prepared[1].1["max_edge"], 2048);
    assert_eq!(result["goal_verified"], false);
}

#[tokio::test]
async fn r20_actual_snapshot_media_degradation_and_poison_rejection() {
    for (name, expected_images) in [("degraded-image", 1), ("degraded-poison", 0)] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        std::fs::set_permissions(&root, std::os::unix::fs::PermissionsExt::from_mode(0o700))
            .unwrap();
        let d = device(&root, name, false).await;
        let mut host = Host::start(&profile(&root, &[&d])).await;
        host.tool(2, "computer.targets", json!({})).await;
        host.send(tool(
            3,
            "computer.snapshot",
            json!({"target":format!("target-{name}"),"max_edge":2048}),
        ))
        .await;
        let reply = host.read().await;
        host.stop().await;
        let data = &reply["result"]["structuredContent"];
        assert_eq!(
            reply["result"]["isError"], true,
            "AX failure must remain explicit"
        );
        assert_eq!(data["code"], "ax_attribute_unavailable");
        assert_eq!(data["observation_error"]["diagnostic"]["ax_error"], -25204);
        assert!(
            data["frame"].is_null()
                && data["view_id"].is_null()
                && data["available_controls"].is_null()
        );
        for key in [
            "input_reference_available",
            "input_authority_granted",
            "mutation_ready",
            "native_complete",
            "goal_verified",
            "input_replayed",
        ] {
            assert_eq!(data[key], false, "lost safety {key}");
        }
        let count = reply["result"]["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| x["type"] == "image")
            .count();
        assert_eq!(count, expected_images);
        assert_eq!(data["visual_available"], expected_images == 1);
        let text: Value =
            serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(&text, data);
        let prepared: Vec<_> = d
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter_map(|r| match &r.call {
                Call::Prepare { tool, .. } => Some(tool.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            prepared,
            vec!["computer.observe", "computer.capture_visual"]
        );
        println!(
            "degraded_variant={name}; images={count}; AX_error=-25204; frame/view=null; authority=false; native_gui=false"
        );
    }
}
