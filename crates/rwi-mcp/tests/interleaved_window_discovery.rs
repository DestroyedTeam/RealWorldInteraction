#![cfg(unix)]
//! Actual MCP process and authenticated local RPC; window inventory is synthetic.
//! This verifies lossless discovery/attachment routing, not the macOS Catalog,
//! native identity freshness, GUI delivery, or input authority.

use rwi_core::device::{Call, DeviceResult, OperationRef, PROTOCOL_VERSION, Reply, Request};
use rwi_link_trust::{IdentityPaths, PeerPin};
use rwi_native_link::device::{AdapterProfile, PairedDevice};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
};

const DEVICE: &str = "interleaved-device";
const ROOT_TARGET: &str = "interleaved-desktop";
const CONTEXT: &str = "interleaved-context";
const INSTANCE: &str = "interleaved-synthetic-instance";

fn selection(pid: u32) -> Value {
    let (window, epoch) = match pid {
        101 => ("cgwindow:701", "a7".repeat(32)),
        202 => ("cgwindow:702", "b9".repeat(32)),
        _ => panic!("unallocated synthetic PID"),
    };
    json!({"window":window,"pid":pid,"native_epoch":epoch,"relation":"initial"})
}

#[derive(Default)]
struct SyntheticInventory {
    prepared: BTreeMap<String, (String, Value)>,
    attached: BTreeMap<String, Value>,
    calls: Vec<Request>,
}

impl SyntheticInventory {
    fn reply(&mut self, request: Request) -> Reply {
        self.calls.push(request.clone());
        let result = match &request.call {
            Call::Describe => {
                let mut targets = vec![json!({"target":ROOT_TARGET})];
                targets.extend(self.attached.keys().map(|target| json!({"target":target})));
                DeviceResult::value(json!({"service":"healthy","availability":"available",
                    "context_valid":true,"targets":targets,"synthetic":true}))
            }
            Call::OpenContext => DeviceResult::value(json!({"context":CONTEXT,
                "targets":[{"target":ROOT_TARGET}],"synthetic":true})),
            Call::Prepare { tool, arguments } => {
                let operation_ref = OperationRef {
                    ref_version: 1,
                    device_id: DEVICE.into(),
                    agent_instance: INSTANCE.into(),
                    context_generation: CONTEXT.into(),
                    execution_nonce: uuid::Uuid::new_v4().to_string(),
                };
                self.prepared.insert(
                    operation_ref.execution_nonce.clone(),
                    (tool.clone(), arguments.clone()),
                );
                DeviceResult::value(json!({"accepted":true,"operation_ref":operation_ref}))
            }
            Call::Execute { operation_ref } => {
                let (tool, arguments) = self.prepared[&operation_ref.execution_nonce].clone();
                match tool.as_str() {
                    "computer.discover" => {
                        let pid = arguments["pid"].as_u64().unwrap() as u32;
                        let chosen = selection(pid);
                        DeviceResult::value(json!({"windows":[chosen],"complete":true,
                            "next_cursor":null,"synthetic":true,"input_sent":false}))
                    }
                    "computer.attach_window" => {
                        let chosen = arguments["selection"].clone();
                        let pid = chosen["pid"].as_u64().unwrap() as u32;
                        if chosen != selection(pid) {
                            DeviceResult::error("selection_stale", "Synthetic epoch mismatch")
                        } else {
                            let target = format!("{ROOT_TARGET}:window:{pid}");
                            self.attached.insert(target.clone(), chosen.clone());
                            DeviceResult::value(json!({"target":target,"parent_target":ROOT_TARGET,
                                "selection":chosen,"attached":true,"source_binding_changed":false,
                                "input_sent":false,"activated":false,"synthetic":true}))
                        }
                    }
                    "computer.observe" => {
                        let target = arguments["target"].as_str().unwrap();
                        match self.attached.get(target) {
                            Some(chosen) => DeviceResult::value(json!({"target":target,
                                "selection":chosen,"synthetic":true,"input_sent":false})),
                            None if target == ROOT_TARGET => DeviceResult::value(json!({
                                "target":ROOT_TARGET,"selection":null,"synthetic":true,
                                "input_sent":false})),
                            None => {
                                DeviceResult::error("unknown_target", "Synthetic retired target")
                            }
                        }
                    }
                    "computer.detach_window" => {
                        self.attached.remove(arguments["target"].as_str().unwrap());
                        DeviceResult::value(json!({"target":arguments["target"],
                            "detached":true,"input_sent":false,"synthetic":true}))
                    }
                    _ => DeviceResult::error("synthetic_unsupported", "No GUI or input fixture"),
                }
            }
            Call::CloseContext => DeviceResult::value(json!({"closed":true})),
            _ => DeviceResult::error("synthetic_unsupported", "No GUI or input fixture"),
        };
        Reply {
            version: PROTOCOL_VERSION,
            id: request.id,
            device: request.device,
            instance: INSTANCE.into(),
            result,
        }
    }
}

struct Peer {
    state: Arc<Mutex<SyntheticInventory>>,
    profile: PathBuf,
    _stop: tokio::sync::watch::Sender<bool>,
    server: tokio::task::JoinHandle<std::io::Result<()>>,
}

impl Peer {
    async fn start(root: &Path) -> Self {
        let provisioned = rwi_link_trust::provision(&root.join("trust")).unwrap();
        let identity = IdentityPaths::in_profile(&root.join("trust/agent"))
            .load()
            .unwrap();
        let tls = identity
            .server_config(std::slice::from_ref(&provisioned.model.certificate_sha256))
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let paired = PairedDevice {
            device_id: DEVICE.into(),
            endpoint: listener.local_addr().unwrap(),
            identity: IdentityPaths::in_profile(&root.join("trust/model")),
            agent_pin: provisioned.agent.certificate_sha256,
        };
        let profile = root.join("adapter.json");
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&profile)
            .unwrap()
            .write_all(
                &serde_json::to_vec(&AdapterProfile {
                    devices: vec![paired],
                })
                .unwrap(),
            )
            .unwrap();
        let state = Arc::new(Mutex::new(SyntheticInventory::default()));
        let records = state.clone();
        let (stop, rx) = tokio::sync::watch::channel(false);
        let server = tokio::spawn(rwi_native_link::device::serve(
            listener,
            tls,
            move |_peer: PeerPin, request: Request| {
                let result = records.lock().unwrap().reply(request);
                async move { result }
            },
            rx,
        ));
        Self {
            state,
            profile,
            _stop: stop,
            server,
        }
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.server.abort();
    }
}

struct Host {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
}

impl Host {
    async fn start(profile: &Path) -> Self {
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
        host.send(
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
            "protocolVersion":"2025-11-25","capabilities":{},
            "clientInfo":{"name":"interleaved-window-contract-test","version":"1"}}}),
        )
        .await;
        assert!(host.read().await.get("result").is_some());
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

    async fn read(&mut self) -> Value {
        let mut line = String::new();
        let size = tokio::time::timeout(Duration::from_secs(10), self.output.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        assert_ne!(size, 0, "MCP child terminated without a response");
        serde_json::from_str(&line).unwrap()
    }

    async fn tool(&mut self, id: u32, name: &str, arguments: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call",
            "params":{"name":name,"arguments":arguments}}))
            .await;
        let response = self.read().await;
        assert_eq!(response["id"], id);
        response["result"]["structuredContent"].clone()
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

#[tokio::test]
async fn actual_mcp_interleaved_discoveries_preserve_both_exact_selections_and_root() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("private");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    assert_eq!(
        std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let peer = Peer::start(&root).await;
    let mut host = Host::start(&peer.profile).await;
    let targets = host
        .tool(2, "computer.targets", json!({"device_id":DEVICE}))
        .await;
    assert_eq!(targets["targets"], json!([{"target":ROOT_TARGET}]));

    // Reproduce caller order: a second PID-filtered discover intervenes before
    // the first returned exact selection is attached. Do not rediscover A.
    let a = host
        .tool(
            3,
            "computer.discover",
            json!({"target":ROOT_TARGET,"pid":101}),
        )
        .await;
    let b = host
        .tool(
            4,
            "computer.discover",
            json!({"target":ROOT_TARGET,"pid":202}),
        )
        .await;
    assert_eq!(a["windows"][0], selection(101));
    assert_eq!(b["windows"][0], selection(202));
    let attach_a_args = json!({"target":ROOT_TARGET,"selection":a["windows"][0]});
    let attach_b_args = json!({"target":ROOT_TARGET,"selection":b["windows"][0]});
    let attached_a = host
        .tool(5, "computer.attach_window", attach_a_args.clone())
        .await;
    let attached_b = host
        .tool(6, "computer.attach_window", attach_b_args.clone())
        .await;
    assert_eq!(attached_a["selection"], selection(101));
    assert_eq!(attached_b["selection"], selection(202));
    let target_a = attached_a["target"].as_str().unwrap();
    let target_b = attached_b["target"].as_str().unwrap();
    assert_ne!(target_a, target_b);
    for result in [&attached_a, &attached_b] {
        assert_eq!(result["parent_target"], ROOT_TARGET);
        assert_eq!(result["source_binding_changed"], false);
        assert_eq!(result["input_sent"], false);
        assert_eq!(result["activated"], false);
        assert_eq!(result["synthetic"], true);
    }

    for (id, target, chosen) in [
        (7, target_a, selection(101)),
        (8, target_b, selection(202)),
        (9, ROOT_TARGET, Value::Null),
    ] {
        let observation = host
            .tool(id, "computer.observe", json!({"target":target}))
            .await;
        assert_eq!(observation["target"], target);
        assert_eq!(observation["selection"], chosen);
    }

    let mut forged = attach_a_args.clone();
    forged["selection"]["native_epoch"] = json!("0".repeat(64));
    let rejected = host
        .tool(10, "computer.attach_window", forged.clone())
        .await;
    assert_eq!(rejected["code"], "selection_stale");
    let count_before_invalid = peer.state.lock().unwrap().calls.len();
    forged["selection"]
        .as_object_mut()
        .unwrap()
        .remove("native_epoch");
    let invalid = host.tool(11, "computer.attach_window", forged).await;
    assert_eq!(invalid["code"], "invalid_arguments");
    assert_eq!(peer.state.lock().unwrap().calls.len(), count_before_invalid);

    assert_eq!(
        host.tool(12, "computer.detach_window", json!({"target":target_a}))
            .await["detached"],
        true
    );
    let retired = host
        .tool(13, "computer.observe", json!({"target":target_a}))
        .await;
    assert_eq!(retired["code"], "unknown_target");
    let still_b = host
        .tool(14, "computer.observe", json!({"target":target_b}))
        .await;
    assert_eq!(still_b["selection"], selection(202));
    host.stop().await;

    let state = peer.state.lock().unwrap();
    let prepared: Vec<_> = state
        .calls
        .iter()
        .filter_map(|request| match &request.call {
            Call::Prepare { tool, arguments } => Some((tool.as_str(), arguments)),
            _ => None,
        })
        .collect();
    assert_eq!(
        prepared
            .iter()
            .filter(|(tool, _)| *tool == "computer.discover")
            .count(),
        2
    );
    let attachments: Vec<_> = prepared
        .iter()
        .filter(|(tool, _)| *tool == "computer.attach_window")
        .collect();
    assert_eq!(
        attachments.len(),
        3,
        "two successes and one exact rejected epoch, no retry"
    );
    assert_eq!(*attachments[0].1, attach_a_args);
    assert_eq!(*attachments[1].1, attach_b_args);
    assert_eq!(
        state
            .calls
            .iter()
            .filter(|r| matches!(r.call, Call::OpenContext))
            .count(),
        1
    );
    assert_eq!(
        state
            .calls
            .iter()
            .filter(|r| matches!(r.call, Call::Execute { .. }))
            .count(),
        prepared.len()
    );
    assert!(prepared.iter().all(|(tool, _)| matches!(
        *tool,
        "computer.discover"
            | "computer.attach_window"
            | "computer.observe"
            | "computer.detach_window"
    )));
    assert!(
        state
            .calls
            .iter()
            .filter(|request| matches!(request.call, Call::Prepare { .. }))
            .all(|request| request.context.as_deref() == Some(CONTEXT))
    );
    let mut execution_counts = BTreeMap::new();
    for request in &state.calls {
        if let Call::Execute { operation_ref } = &request.call {
            assert!(request.context.is_none());
            assert_eq!(operation_ref.device_id, DEVICE);
            assert_eq!(operation_ref.agent_instance, INSTANCE);
            assert_eq!(operation_ref.context_generation, CONTEXT);
            *execution_counts
                .entry(&operation_ref.execution_nonce)
                .or_insert(0) += 1;
        }
    }
    assert_eq!(execution_counts.len(), prepared.len());
    assert!(execution_counts.values().all(|count| *count == 1));
    assert_eq!(state.attached.len(), 1);
    assert_eq!(state.attached[target_b], selection(202));
}
