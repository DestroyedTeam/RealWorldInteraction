#![cfg(unix)]
//! P: real MCP child + authenticated RPC, with explicit synthetic device peers.
//! No native OS input or platform success is inferred from these transport tests.
use rcu_core::device::{Call, DeviceResult, OperationRef, PROTOCOL_VERSION, Reply, Request};
use rcu_link_trust::{IdentityPaths, PeerPin};
use rcu_native_link::device::{AdapterProfile, PairedDevice};
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
impl Host {
    async fn start(profile: &std::path::Path) -> Self {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_rcu-mcp"))
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
    let provisioned = rcu_link_trust::provision(&path.join("trust")).unwrap();
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
    let server = tokio::spawn(rcu_native_link::device::serve(
        listener,
        tls,
        move |_peer: PeerPin, r: Request| {
            let records = records.clone();
            let seen = seen.clone();
            let cancel = cancel.clone();
            let name = name.clone();
            async move {
                seen.lock().unwrap().push(r.clone());
                let result = match &r.call {
                    Call::Describe => DeviceResult::value(
                        json!({"availability":"available","service":"healthy","context_valid":true}),
                    ),
                    Call::OpenContext => DeviceResult::value(
                        json!({"context":format!("context-{name}"),"targets":[{"target":format!("target-{name}")}]}),
                    ),
                    Call::Prepare { arguments, .. } => {
                        let operation_ref = OperationRef {
                            ref_version: 1,
                            device_id: name.clone(),
                            agent_instance: "test-instance".into(),
                            context_generation: format!("context-{name}"),
                            execution_nonce: uuid::Uuid::new_v4().to_string(),
                        };
                        records
                            .lock()
                            .unwrap()
                            .insert(operation_ref.execution_nonce.clone(), arguments.clone());
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
    use rcu_agent::config::{Config, PlatformPolicy, Principal};
    let path = root.join("real-runtime");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .unwrap();
    let provisioned = rcu_link_trust::provision(&path.join("trust")).unwrap();
    let identity_paths = IdentityPaths::in_profile(&path.join("trust/agent"));
    let tls = identity_paths
        .load()
        .unwrap()
        .server_config(std::slice::from_ref(&provisioned.model.certificate_sha256))
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = listener.local_addr().unwrap();
    let runtime = rcu_agent::runtime::Runtime::new(Config {
        version: 1,
        device_id: "runtime-device".into(),
        bind: endpoint,
        identity: identity_paths,
        state_root: path.join("state"),
        platform: PlatformPolicy::Fixture,
        principals: vec![Principal {
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
    let server = tokio::spawn(rcu_native_link::device::serve(
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
