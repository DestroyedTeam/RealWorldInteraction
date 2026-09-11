#![cfg(unix)]
//! P: real MCP child + authenticated RPC, with explicit synthetic device peers.
//! No native OS input or platform success is inferred from these transport tests.
use rcu_core::device::{Call, DeviceResult, Reply, Request};
use rcu_link_trust::{IdentityPaths, PeerPin};
use rcu_native_link::device::{AdapterProfile, PairedDevice};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
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
    let cancel = cancelled.clone();
    let (stop, rx) = tokio::sync::watch::channel(false);
    let name = id.to_owned();
    let server = tokio::spawn(rcu_native_link::device::serve(
        listener,
        tls,
        move |_peer: PeerPin, r: Request| {
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
                    Call::Cancel { operation_id } => {
                        cancel.lock().unwrap().insert(operation_id.clone());
                        DeviceResult::value(
                            json!({"cancel_requested":true,"terminal":false,"resources_released":false}),
                        )
                    }
                    Call::Invoke { arguments, .. } => {
                        let op = arguments["operation_id"].as_str().unwrap_or("none");
                        if op.starts_with("slow") {
                            loop {
                                if cancel.lock().unwrap().contains(op) {
                                    break;
                                }
                                tokio::time::sleep(Duration::from_millis(5)).await;
                            }
                        }
                        DeviceResult::value(
                            json!({"operation_id":op,"device":name,"synthetic":true}),
                        )
                    }
                    Call::Operation { operation_id } => {
                        DeviceResult::value(json!({"operation_id":operation_id,"device":name}))
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
                    version: 1,
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
    for (id, device) in [(3, "a"), (4, "b")] {
        host.tool(id,"computer.observe",json!({"target":format!("target-device-{device}"),"operation_id":"same-id","inline_image":false})).await;
    }
    assert_eq!(
        host.tool(5, "computer.operation", json!({"operation_id":"same-id"}))
            .await["code"],
        "ambiguous_resource"
    );
    assert_eq!(
        host.tool(
            6,
            "computer.operation",
            json!({"device_id":"device-a","operation_id":"same-id"})
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
    wait_until(||a.calls.lock().unwrap().iter().any(|r|matches!(&r.call,Call::Invoke{arguments,..} if arguments["operation_id"]=="slow-cancel"))).await;
    host.send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":7,"reason":"Host Stop"}})).await;
    wait_until(|| a.cancelled.lock().unwrap().contains("slow-cancel")).await;
    assert!(!b.cancelled.lock().unwrap().contains("slow-cancel"));
    assert!(
        a.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|r| matches!(&r.call,Call::Cancel{operation_id} if operation_id=="slow-cancel"))
            .all(|r| r.context.as_deref() == Some("context-device-a"))
    );
    let response = host.read().await;
    assert_eq!(response["id"], 7);
    host.send(tool(
        8,
        "computer.observe",
        json!({"target":"target-device-b","operation_id":"slow-eof","inline_image":false}),
    ))
    .await;
    wait_until(||b.calls.lock().unwrap().iter().any(|r|matches!(&r.call,Call::Invoke{arguments,..} if arguments["operation_id"]=="slow-eof"))).await;
    let began = std::time::Instant::now();
    host.stop().await;
    assert!(
        began.elapsed() < Duration::from_secs(6),
        "EOF cleanup must be one bounded parallel budget, not per device"
    );
    assert!(
        b.cancelled.lock().unwrap().contains("slow-eof"),
        "dropping the Host waiter must request native cancellation"
    );
    for d in [&a, &b] {
        let calls = d.calls.lock().unwrap();
        for id in ["same-id", "slow-cancel", "slow-eof"] {
            assert!(calls.iter().filter(|r|matches!(&r.call,Call::Invoke{arguments,..} if arguments["operation_id"]==id)).count()<=1,"original operation replayed");
        }
    }
}
