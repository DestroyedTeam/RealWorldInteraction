#![cfg(unix)]
//! Real MCP child + pinned loopback TLS, synthetic pixels/peer only. No GUI claim.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rwi_core::device::{
    Call, DeviceImage, DeviceResult, OperationRef, PROTOCOL_VERSION, Reply, Request,
};
use rwi_link_trust::IdentityPaths;
use rwi_native_link::device::{AdapterProfile, PairedDevice};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
    sync::Notify,
};
const SAMPLE: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGMQMYr6DwACngGgfe0KRgAAAABJRU5ErkJggg==";
const TARGET: &str = "synthetic-live-image-target";
const IMAGE: &str = "computer.live_panel_image";
#[derive(Default)]
struct State {
    calls: Mutex<Vec<Request>>,
    targets_fixture: Mutex<Option<Value>>,
    native_reply: Mutex<Option<Value>>,
    prepared: Mutex<BTreeMap<String, Value>>,
    binding: Mutex<String>,
    visible: AtomicBool,
    hold: AtomicBool,
    wrong_instance: AtomicBool,
    entered: Notify,
    release: Notify,
}
struct Peer {
    _temp: tempfile::TempDir,
    profile: std::path::PathBuf,
    state: Arc<State>,
    stop: tokio::sync::watch::Sender<bool>,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
}
impl Peer {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap().join("private");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let trust = rwi_link_trust::provision(&root.join("trust")).unwrap();
        let tls = IdentityPaths::in_profile(&root.join("trust/agent"))
            .load()
            .unwrap()
            .server_config(std::slice::from_ref(&trust.model.certificate_sha256))
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap();
        let (state, seen) = {
            let s = Arc::new(State::default());
            (s.clone(), s)
        };
        let (stop, rx) = tokio::sync::watch::channel(false);
        let server = tokio::spawn(rwi_native_link::device::serve(
            listener,
            tls,
            move |_, r: Request| {
                let s = seen.clone();
                async move {
                    s.calls.lock().unwrap().push(r.clone());
                    let mut instance = "image-fixture-instance";
                    let result = match &r.call {
                        Call::Describe => DeviceResult::value(
                            s.targets_fixture.lock().unwrap().clone().unwrap_or_else(||
                                json!({"availability":"available","service":"healthy","context_valid":true,"targets":[{"target":TARGET}],"executor_features":{"cached_live_image_v1":{"implemented":true}}})
                            ),
                        ),
                        Call::OpenContext => DeviceResult::value(
                            json!({"context":"image-fixture-context","targets":s.targets_fixture.lock().unwrap().as_ref().and_then(|v|v.get("targets")).cloned().unwrap_or_else(||json!([{"target":TARGET}]))}),
                        ),
                        Call::CloseContext => DeviceResult::value(json!({"closed":true})),
                        Call::Prepare { tool, arguments } if tool == "computer.live" => {
                            let op = OperationRef {
                                ref_version: 1,
                                device_id: "image-device".into(),
                                agent_instance: instance.into(),
                                context_generation: "image-fixture-context".into(),
                                execution_nonce: uuid::Uuid::new_v4().to_string(),
                            };
                            s.prepared
                                .lock()
                                .unwrap()
                                .insert(op.execution_nonce.clone(), arguments.clone());
                            DeviceResult::value(json!({"accepted":true,"operation_ref":op}))
                        }
                        Call::Execute { operation_ref } => {
                            let args = s
                                .prepared
                                .lock()
                                .unwrap()
                                .remove(&operation_ref.execution_nonce)
                                .unwrap();
                            let mut data = if args["action"] == "start" {
                                if let Some(binding) = args["options"]["host_view_id"].as_str() {
                                    *s.binding.lock().unwrap() = binding.into();
                                    s.visible.store(false, Ordering::Release);
                                    json!({"status":"running","expires_in_ms":60000,"host_view_binding":binding})
                                } else {
                                    s.native_reply.lock().unwrap().clone().expect("independent test explicitly supplies its data-only Native reply")
                                }
                            } else if args["action"] == "stop" {
                                s.visible.store(false, Ordering::Release);
                                json!({"status":"stopped"})
                            } else {
                                let shown = args["view"]["visible"].as_bool().unwrap_or(false);
                                s.visible.store(shown, Ordering::Release);
                                json!({"accepted":true,"presentation_allowed":shown,"sequence":args["view"]["sequence"]})
                            };
                            data["synthetic"] = json!(true);
                            data["operation_ref"] = json!(operation_ref);
                            DeviceResult::value(data)
                        }
                        Call::Cached { tool, arguments } if tool == "computer.live_image" => {
                            assert_eq!(r.context.as_deref(), Some("image-fixture-context"));
                            assert_eq!(arguments["target"], TARGET);
                            assert_eq!(arguments["query"]["view_id"], *s.binding.lock().unwrap());
                            assert!(s.visible.load(Ordering::Acquire));
                            if s.hold.load(Ordering::Acquire) {
                                s.entered.notify_one();
                                s.release.notified().await;
                            }
                            if s.wrong_instance.load(Ordering::Acquire) {
                                instance = "replaced-fixture-instance";
                            }
                            let cursor = json!({"generation":1,"sequence":1});
                            if arguments["query"]["after"] == cursor {
                                DeviceResult::value(json!({"state":"unchanged","cursor":cursor}))
                            } else {
                                let png = STANDARD.decode(SAMPLE).unwrap();
                                DeviceResult {
                                    is_error: false,
                                    data: json!({"state":"ready","cursor":cursor,"image_size":[1,1],"captured_unix_ms":10,"png_bytes":png.len(),"png_sha256":rwi_core::digest(&png),"target":"MUST_NOT_LEAK_TARGET","frame":"MUST_NOT_LEAK_FRAME","future":"MUST_NOT_LEAK_FUTURE"}),
                                    images: vec![DeviceImage {
                                        mime: "image/png".into(),
                                        base64: SAMPLE.into(),
                                    }],
                                }
                            }
                        }
                        _ => DeviceResult::error(
                            "synthetic_unexpected_call",
                            "This peer does not implement capture or input.",
                        ),
                    };
                    Reply {
                        version: PROTOCOL_VERSION,
                        id: r.id,
                        device: r.device,
                        instance: instance.into(),
                        result,
                    }
                }
            },
            rx,
        ));
        let profile = root.join("profile.json");
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&profile)
            .unwrap();
        out.write_all(
            &serde_json::to_vec(&AdapterProfile {
                devices: vec![PairedDevice {
                    device_id: "image-device".into(),
                    endpoint,
                    identity: IdentityPaths::in_profile(&root.join("trust/model")),
                    agent_pin: trust.agent.certificate_sha256,
                }],
            })
            .unwrap(),
        )
        .unwrap();
        Self {
            _temp: temp,
            profile,
            state,
            stop,
            server,
        }
    }
    fn image_calls(&self) -> usize {
        self.state
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| matches!(&r.call,Call::Cached{tool,..}if tool=="computer.live_image"))
            .count()
    }
    async fn stop(self) {
        assert!(
            self.state
                .calls
                .lock()
                .unwrap()
                .iter()
                .all(|r| !matches!(&r.call,Call::Prepare{tool,..}if tool!="computer.live"))
        );
        self.stop.send_replace(true);
        self.server.await.unwrap().unwrap();
    }
}
struct Host {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    buffer: BTreeMap<u32, Value>,
}
impl Host {
    async fn start(profile: &Path, apps: bool) -> Self {
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
        let mut h = Self {
            child,
            input,
            output,
            buffer: BTreeMap::new(),
        };
        let caps = if apps {
            json!({"extensions":{"io.modelcontextprotocol/ui":{"mimeTypes":["text/html;profile=mcp-app"]}}})
        } else {
            json!({})
        };
        h.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":caps,"clientInfo":{"name":"synthetic-live-image-transport","version":"1"}}})).await;
        assert!(h.receive(1).await.get("result").is_some());
        h.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        h
    }
    async fn send(&mut self, v: Value) {
        self.input
            .as_mut()
            .unwrap()
            .write_all(format!("{v}\n").as_bytes())
            .await
            .unwrap();
    }
    async fn receive(&mut self, id: u32) -> Value {
        if let Some(v) = self.buffer.remove(&id) {
            return v;
        }
        loop {
            let mut line = String::new();
            assert!(
                tokio::time::timeout(Duration::from_secs(8), self.output.read_line(&mut line))
                    .await
                    .unwrap()
                    .unwrap()
                    > 0
            );
            let v: Value = serde_json::from_str(&line).unwrap();
            if let Some(n) = v["id"].as_u64() {
                if n == u64::from(id) {
                    return v;
                }
                self.buffer.insert(n.try_into().unwrap(), v);
            }
        }
    }
    async fn call(&mut self, id: u32, name: &str, args: Value) -> Value {
        self.send(tool(id, name, args)).await;
        let v = self.receive(id).await;
        assert!(v.get("error").is_none(), "{v}");
        v["result"].clone()
    }
    async fn open(&mut self) -> Value {
        let targets = self
            .call(2, "computer.targets", json!({"device_id":"image-device"}))
            .await;
        assert_eq!(targets["structuredContent"]["targets"][0]["target"], TARGET);
        let start = self
            .call(
                3,
                "computer.show_live",
                json!({"target":TARGET,"options":{"display":{"id":1},"duration_ms":60000}}),
            )
            .await;
        assert_ne!(start["isError"], true, "{start}");
        let cap = start["_meta"]["rwi/live_panel"].clone();
        assert!(cap["view_key"].is_string());
        let result = self
            .call(4, "computer.live_panel", presence(&cap, 1, true, false))
            .await;
        assert_eq!(result["structuredContent"]["accepted"], true, "{result}");
        cap
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
fn image(cap: &Value, sequence: u64) -> Value {
    json!({"view_key":cap["view_key"],"page":cap["page"],"sequence":sequence})
}
fn presence(cap: &Value, sequence: u64, visible: bool, close: bool) -> Value {
    json!({"view_key":cap["view_key"],"page":cap["page"],"sequence":sequence,"visible":visible,"close":close})
}

#[tokio::test]
async fn independent_native_live_real_mcp_no_apps_no_presence_and_original_conflict() {
    let peer = Peer::new().await;
    let mut host = Host::start(&peer.profile, false).await;
    let native = json!({"session_id":"live:synthetic","status":"running","presentation":"RWI_Agent_native_nonactivating_panel","ownership":"this_MCP_device_context","host_view_binding":null,"host_view_lease_ms":null,"presenter_visible":false,"frames_captured":0,"expires_in_ms":60000,"input_sent":false});
    *peer.state.native_reply.lock().unwrap() = Some(native.clone());
    host.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    let catalog = host.receive(2).await;
    let listed = catalog["result"]["tools"].as_array().unwrap();
    assert!(
        listed
            .iter()
            .any(|t| t["name"] == "computer.show_native_live" && t.get("_meta").is_none())
    );
    assert!(!listed.iter().any(|t| t["name"] == "computer.show_live"));
    host.call(3, "computer.targets", json!({"device_id":"image-device"}))
        .await;
    let args = json!({"target":TARGET,"options":{"display":{"id":4},"duration_ms":60000}});
    let start = host
        .call(4, "computer.show_native_live", args.clone())
        .await;
    assert_eq!(start["isError"], false, "{start}");
    assert!(start.get("_meta").is_none());
    for (key, value) in native.as_object().unwrap() {
        assert_eq!(&start["structuredContent"][key], value);
    }
    assert!(start["structuredContent"].get("viewer").is_none());
    let mut bad = args.clone();
    bad["options"]["host_view_id"] = Value::Null;
    assert_eq!(
        host.call(5, "computer.show_native_live", bad).await["isError"],
        true
    );
    let mut conflict = native;
    conflict["host_view_binding"] = json!("old-component");
    conflict["start"] = json!("already_running; stop before reconfiguring");
    *peer.state.native_reply.lock().unwrap() = Some(conflict);
    let refused = host.call(6, "computer.show_native_live", args).await;
    assert_eq!(refused["isError"], true);
    assert_eq!(
        refused["structuredContent"]["code"],
        "live_presentation_restart_required"
    );
    assert_eq!(
        refused["structuredContent"]["host_view_binding"],
        "old-component"
    );
    let stop = host
        .call(7, "computer.live", json!({"target":TARGET,"action":"stop"}))
        .await;
    assert_eq!(stop["structuredContent"]["status"], "stopped");
    host.stop().await;
    {
        let calls = peer.state.calls.lock().unwrap();
        let prepared: Vec<_> = calls
            .iter()
            .filter_map(|r| match &r.call {
                Call::Prepare { tool, arguments } => Some((tool, arguments)),
                _ => None,
            })
            .collect();
        assert_eq!(
            prepared.len(),
            3,
            "two distinct explicit starts and one explicit stop; invalid call did not dispatch"
        );
        for (tool, args) in prepared {
            assert_eq!(tool, "computer.live");
            assert!(args.get("view").is_none());
            assert!(args["options"].get("host_view_id").is_none());
            assert!(args["options"].get("host_image_only").is_none());
        }
        assert!(
            !calls
                .iter()
                .any(|r| matches!(r.call, Call::Cached { .. } | Call::Cancel { .. }))
        );
    }
    peer.stop().await;
}
#[path = "live_image_transport/targets_summary.rs"]
mod targets_summary;
#[tokio::test]
async fn f33_live_image_real_mcp_negotiates_app_only_tool_and_rejects_forged_capability_before_rpc()
{
    let p = Peer::new().await;
    let mut h = Host::start(&p.profile, true).await;
    h.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    let tools = h.receive(2).await;
    let tool = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["name"] == IMAGE)
        .unwrap();
    assert_eq!(tool["_meta"]["ui"]["visibility"], json!(["app"]));
    assert!(tool["inputSchema"]["properties"].get("target").is_none());
    let r = h
        .call(
            3,
            IMAGE,
            json!({"view_key":"a".repeat(32),"page":"b".repeat(32),"sequence":1}),
        )
        .await;
    assert_eq!(r["isError"], true);
    assert!(r.get("_meta").is_none());
    assert_eq!(p.image_calls(), 0);
    h.stop().await;
    p.stop().await;
}
#[tokio::test]
async fn f33_live_image_real_mcp_non_apps_host_cannot_read_or_start_component() {
    let p = Peer::new().await;
    let mut h = Host::start(&p.profile, false).await;
    h.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
        .await;
    let tools = h.receive(2).await;
    assert!(
        tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["name"] != IMAGE)
    );
    let r = h
        .call(
            3,
            IMAGE,
            json!({"view_key":"a".repeat(32),"page":"b".repeat(32),"sequence":1}),
        )
        .await;
    assert_eq!(r["structuredContent"]["code"], "live_ui_not_negotiated");
    assert!(r.get("_meta").is_none());
    assert!(p.state.calls.lock().unwrap().is_empty());
    h.stop().await;
    p.stop().await;
}
#[tokio::test]
async fn f33_live_image_real_mcp_exact_pixels_only_in_meta_then_unchanged_and_close_revoke() {
    let p = Peer::new().await;
    let mut h = Host::start(&p.profile, true).await;
    let cap = h.open().await;
    let start = Instant::now();
    let r = h.call(5, IMAGE, image(&cap, 1)).await;
    assert_ne!(r["isError"], true, "{r}");
    let model = json!({"structuredContent":r["structuredContent"],"content":r["content"]});
    assert!(!model.to_string().contains(SAMPLE));
    assert!(!r.to_string().contains("MUST_NOT_LEAK"));
    assert_eq!(r["_meta"]["rwi/live_image"]["base64"], SAMPLE);
    assert_eq!(p.image_calls(), 1);
    println!(
        "F33 synthetic loopback image elapsed_ms={} model_json_bytes={} png_bytes=70; not a native or Host-render benchmark",
        start.elapsed().as_millis(),
        model.to_string().len()
    );
    tokio::time::sleep(Duration::from_millis(260)).await;
    let mut next = image(&cap, 2);
    next["after"] = json!({"generation":1,"sequence":1});
    let r = h.call(6, IMAGE, next).await;
    assert_eq!(r["_meta"]["rwi/live_image"]["state"], "unchanged");
    assert!(r["_meta"]["rwi/live_image"].get("base64").is_none());
    assert_eq!(p.image_calls(), 2);
    let close = h
        .call(7, "computer.live_panel", presence(&cap, 2, false, true))
        .await;
    assert_ne!(close["isError"], true);
    let r = h.call(8, IMAGE, image(&cap, 3)).await;
    assert_eq!(r["isError"], true);
    assert!(r.get("_meta").is_none());
    assert_eq!(p.image_calls(), 2);
    h.stop().await;
    p.stop().await;
}
#[tokio::test]
async fn f33_live_image_real_mcp_hide_and_resume_progress_while_pixels_wait_and_late_frame_is_dropped()
 {
    let p = Peer::new().await;
    let mut h = Host::start(&p.profile, true).await;
    let cap = h.open().await;
    p.state.hold.store(true, Ordering::Release);
    h.send(tool(10, IMAGE, image(&cap, 1))).await;
    tokio::time::timeout(Duration::from_secs(3), p.state.entered.notified())
        .await
        .unwrap();
    h.send(tool(
        11,
        "computer.live_panel",
        presence(&cap, 2, false, false),
    ))
    .await;
    let hidden = h.receive(11).await;
    assert_eq!(hidden["result"]["structuredContent"]["accepted"], true);
    h.send(tool(
        12,
        "computer.live_panel",
        presence(&cap, 3, true, false),
    ))
    .await;
    let shown = h.receive(12).await;
    assert_eq!(shown["result"]["structuredContent"]["accepted"], true);
    p.state.release.notify_one();
    let r = h.receive(10).await;
    assert_eq!(
        r["result"]["structuredContent"]["code"], "live_image_response_revoked",
        "{r}"
    );
    assert!(r["result"].get("_meta").is_none());
    assert_eq!(p.image_calls(), 1);
    h.stop().await;
    p.stop().await;
}
#[tokio::test]
async fn f33_live_image_real_mcp_changed_agent_instance_cannot_return_old_pixels() {
    let p = Peer::new().await;
    let mut h = Host::start(&p.profile, true).await;
    let cap = h.open().await;
    p.state.wrong_instance.store(true, Ordering::Release);
    let r = h.call(5, IMAGE, image(&cap, 1)).await;
    // Pinned connection rejects the changed instance before Gateway sees pixels.
    assert_eq!(
        r["structuredContent"]["code"],
        "live_image_transport_unavailable"
    );
    assert!(r.get("_meta").is_none());
    assert_eq!(p.image_calls(), 1);
    h.stop().await;
    p.stop().await;
}
