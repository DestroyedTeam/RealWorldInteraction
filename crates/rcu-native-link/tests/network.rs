#![cfg(all(feature = "fixture", unix))]
//! Owned loopback transport/entry tests. These peers are explicitly NOT the
//! nineteen-tool MCP worker or a native goal oracle. Root tests those separately.
use rcu_link_trust::{IdentityPaths, PeerPin, ProvisionManifest};
use rcu_native_link::{
    entry, frame, net, sha,
    types::{AgentConfig, Attach, BackendMode, ClientConfig, Image, ModelEnd, Role},
};
use serde_json::{Value, json};
use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

struct Case {
    _temp: tempfile::TempDir,
    root: PathBuf,
    relay: Image,
    profiles: ProvisionManifest,
}
impl Case {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap().join("private-run");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let profiles = rcu_link_trust::provision(&root.join("profiles")).unwrap();
        let install = root.join("install");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&install)
            .unwrap();
        let path = install.join(entry::RELAY_BASENAME);
        std::fs::hard_link(env!("CARGO_BIN_EXE_rcu-native-relay"), &path).unwrap();
        let relay = Image {
            sha256: sha(&std::fs::read(&path).unwrap()),
            path,
        };
        Self {
            _temp: temp,
            root,
            relay,
            profiles,
        }
    }
    fn client(&self, endpoint: SocketAddr, role: Role) -> ClientConfig {
        ClientConfig {
            model_output: Default::default(),
            version: 1,
            root: self.root.clone(),
            endpoint,
            server_name: rcu_link_trust::SERVER_DNS_NAME.into(),
            trust: IdentityPaths::in_profile(&self.root.join("profiles").join(
                if role == Role::Model {
                    "model"
                } else {
                    "controller"
                },
            )),
            agent_peer_sha256: self.profiles.agent.certificate_sha256.as_str().into(),
            task: "owned-loopback-transport-only".into(),
            approval_sha256: "a".repeat(64),
            role,
            relay_image: self.relay.clone(),
            entry_sha256: None,
        }
    }
    fn agent(&self, bind: SocketAddr) -> AgentConfig {
        AgentConfig {
            version: 1,
            install_root: self.root.join("install"),
            state_root: self.root.join("not-started-state"),
            approval: self.root.join("not-executed-approval.json"),
            bind,
            server_name: rcu_link_trust::SERVER_DNS_NAME.into(),
            trust: IdentityPaths::in_profile(&self.root.join("profiles/agent")),
            controller_peer_sha256: self.profiles.controller.certificate_sha256.as_str().into(),
            model_peer_sha256: self.profiles.model.certificate_sha256.as_str().into(),
            agent_image: self.relay.clone(),
            worker_image: self.relay.clone(),
            backend: BackendMode::Fixture,
        }
    }
    fn write_private(&self, path: &Path, bytes: &[u8]) {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        file.write_all(bytes).unwrap();
        file.sync_all().unwrap();
    }
    fn entry(&self, config: &mut ClientConfig) -> PathBuf {
        let path = self.root.join(".mcp.json");
        let value = json!({"mcpServers":{entry::SERVER_KEY:{"command":self.relay.path,"args":["--entry",path]}}});
        let bytes = serde_json::to_vec_pretty(&value).unwrap();
        config.entry_sha256 = Some(sha(&bytes));
        self.write_private(&path, &bytes);
        self.write_private(
            &self.root.join("client.json"),
            &serde_json::to_vec_pretty(config).unwrap(),
        );
        path
    }
}
fn record(name: &str, value: Value) {
    if let Some(directory) = std::env::var_os("RCU_NATIVE_NETWORK_EVIDENCE") {
        std::fs::create_dir_all(&directory).unwrap();
        let document = json!({"schema":"rcu.owned_loopback_transport.v1", "scope":"transport_entry_only_not_nineteen_tools_not_native", "private_keys_included":false,"result":value});
        std::fs::write(
            PathBuf::from(directory).join(format!("{name}.json")),
            serde_json::to_vec_pretty(&document).unwrap(),
        )
        .unwrap();
    }
}
async fn listener(ip: IpAddr) -> tokio::net::TcpListener {
    net::bind_fixture_loopback(SocketAddr::new(ip, 0))
        .await
        .unwrap()
}
fn raw(value: Value) -> Vec<u8> {
    let mut result = serde_json::to_vec(&value).unwrap();
    result.push(b'\n');
    result
}
fn big_result(id: u64, count: usize) -> Vec<u8> {
    let mut bytes =
        format!("{{ \"jsonrpc\":\"2.0\", \"id\":{id}, \"result\":{{\"text\":\"中文\",\"pad\":\"")
            .into_bytes();
    bytes.resize(bytes.len() + count, b'x');
    bytes.extend_from_slice(b"\"}}\r\n");
    bytes
}

#[tokio::test]
async fn actual_ipv4_ipv6_loopback_tls_preserves_small_and_large_frames() {
    let case = Case::new();
    let mut proofs = Vec::new();
    for ip in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
    ] {
        let socket = listener(ip).await;
        let address = socket.local_addr().unwrap();
        let server_config = net::server_tls(&case.agent(address)).unwrap();
        let client_config = case.client(address, Role::Controller);
        let large = big_result(2, 9 * 1024 * 1024);
        let expected_sha = sha(&large);
        let server = tokio::spawn(async move {
            let (tcp, peer) = tokio::time::timeout(Duration::from_secs(8), socket.accept())
                .await
                .unwrap()
                .unwrap();
            assert!(peer.ip().is_loopback());
            let mut tls = BufReader::new(net::accept(tcp, server_config).await.unwrap());
            let first = net::read_frame(&mut tls, frame::INBOUND)
                .await
                .unwrap()
                .unwrap();
            let first_value: Value = serde_json::from_slice(&first).unwrap();
            assert_eq!(first_value["id"], 1);
            net::write_frame(
                &mut tls,
                &raw(json!({"jsonrpc":"2.0","id":1,"result":{"transport_echo":true}})),
                frame::OUTBOUND,
            )
            .await
            .unwrap();
            let second = net::read_frame(&mut tls, frame::INBOUND)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(second.len(), frame::INBOUND);
            net::write_frame(&mut tls, &large, frame::OUTBOUND)
                .await
                .unwrap();
            (sha(&first), sha(&second), large.len())
        });
        let mut tls = BufReader::new(net::connect(&client_config).await.unwrap());
        let first = b"{ \"jsonrpc\": \"2.0\", \"id\": 1, \"method\":\"transport.echo\",\"params\":{\"text\":\"small\"}}\r\n";
        net::write_frame(&mut tls, first, frame::INBOUND)
            .await
            .unwrap();
        let one = net::read_frame(&mut tls, frame::OUTBOUND)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&one).unwrap()["id"], 1);
        let mut second =
            b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"transport.echo\",\"params\":{\"pad\":\""
                .to_vec();
        second.resize(frame::INBOUND - 4, b'y');
        second.extend_from_slice(b"\"}}\n");
        assert_eq!(second.len(), frame::INBOUND);
        net::write_frame(&mut tls, &second, frame::INBOUND)
            .await
            .unwrap();
        let returned = net::read_frame(&mut tls, frame::OUTBOUND)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sha(&returned), expected_sha);
        let (first_hash, second_hash, large_bytes) =
            tokio::time::timeout(Duration::from_secs(10), server)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(first_hash, sha(first));
        assert_eq!(second_hash, sha(&second));
        proofs.push(json!({"ip_family":if ip.is_ipv4(){"ipv4"}else{"ipv6"},"tls":"1.3","connections":1,"small_request_sha":first_hash,"large_request_sha":second_hash,"large_reply_sha":expected_sha,"large_reply_bytes":large_bytes,"byte_mismatches":0}));
    }
    record(
        "small-large-ipv4-ipv6",
        json!({"cases":proofs,"agent_pin":case.profiles.agent.certificate_sha256}),
    );
}

#[tokio::test]
async fn real_loopback_wrong_server_pin_client_pin_and_name_are_tls_failures() {
    let case = Case::new();
    let mut results = Vec::new();
    for fault in ["server_pin", "client_pin", "server_dns"] {
        let socket = listener(IpAddr::V4(Ipv4Addr::LOCALHOST)).await;
        let address = socket.local_addr().unwrap();
        let mut agent = case.agent(address);
        let mut client = case.client(address, Role::Controller);
        if fault == "server_pin" {
            client.agent_peer_sha256 = case.profiles.model.certificate_sha256.as_str().into();
        }
        if fault == "client_pin" {
            agent.controller_peer_sha256 = "b".repeat(64);
        }
        let tls_config = net::server_tls(&agent).unwrap();
        let server = tokio::spawn(async move {
            let (tcp, _) = tokio::time::timeout(Duration::from_secs(8), socket.accept())
                .await
                .unwrap()
                .unwrap();
            match net::accept(tcp, tls_config).await {
                Err(_) => true,
                Ok(mut stream) => stream.read_u8().await.is_err(),
            }
        });
        let client_rejected = if fault == "server_dns" {
            let identity = client.trust.load().unwrap();
            let tls = identity
                .client_config(&PeerPin::parse(&client.agent_peer_sha256).unwrap())
                .unwrap();
            let tcp = tokio::net::TcpStream::connect(address).await.unwrap();
            tokio::time::timeout(
                net::IO_TIMEOUT,
                tokio_rustls::TlsConnector::from(tls).connect(
                    rustls::pki_types::ServerName::try_from("wrong-agent.local").unwrap(),
                    tcp,
                ),
            )
            .await
            .unwrap()
            .is_err()
        } else {
            match net::connect(&client).await {
                Err(_) => true,
                Ok(mut stream) => {
                    let _ = stream.write_all(&[42]).await;
                    stream.read_u8().await.is_err()
                }
            }
        };
        assert!(client_rejected, "{fault}");
        assert!(
            tokio::time::timeout(Duration::from_secs(7), server)
                .await
                .unwrap()
                .unwrap()
        );
        results.push(
            json!({"fault":fault,"tcp_connected":true,"application_exchange_accepted":false}),
        );
    }
    assert!(
        net::bind_fixture_loopback("0.0.0.0:0".parse().unwrap())
            .await
            .is_err()
    );
    assert!(
        net::bind_fixture_loopback("127.0.0.1:1234".parse().unwrap())
            .await
            .is_err()
    );
    let mut invalid_name = case.client("127.0.0.1:1".parse().unwrap(), Role::Controller);
    invalid_name.server_name = "wrong-agent.local".into();
    assert!(net::connect(&invalid_name).await.is_err());
    record(
        "tls-negatives",
        json!({"cases":results,"configured_bad_name_rejected_before_connect":true,"nonloopback_or_fixed_fixture_bind_refused":true}),
    );
}

#[test]
fn exact_final_entry_and_client_failures_precede_network() {
    use std::os::unix::fs::symlink;
    let case = Case::new();
    let mut config = case.client("127.0.0.1:12345".parse().unwrap(), Role::Model);
    let path = case.entry(&mut config);
    entry::load(&path).unwrap_or_else(|error| panic!("valid entry failed: {error}"));
    assert!(
        entry::load_current(&path).is_err(),
        "test image is not the entry relay"
    );
    let entry_bytes = std::fs::read(&path).unwrap();
    let client_bytes = std::fs::read(case.root.join("client.json")).unwrap();
    let mut refusals = Vec::new();
    for fault in [
        "old_command",
        "extra_args",
        "extra_server",
        "unknown_entry_field",
        "old_image_sha",
        "old_entry_sha",
        "controller_role",
        "wrong_root",
        "unknown_client_field",
    ] {
        let mut e: Value = serde_json::from_slice(&entry_bytes).unwrap();
        let mut c: Value = serde_json::from_slice(&client_bytes).unwrap();
        match fault {
            "old_command" => {
                e["mcpServers"][entry::SERVER_KEY]["command"] =
                    json!(case.root.join("old-0.6-relay"))
            }
            "extra_args" => e["mcpServers"][entry::SERVER_KEY]["args"]
                .as_array_mut()
                .unwrap()
                .push(json!("--unsafe")),
            "extra_server" => e["mcpServers"]["old_fixture"] = json!({"command":"old","args":[]}),
            "unknown_entry_field" => e["env"] = json!({"IGNORED":"no"}),
            "old_image_sha" => c["relay_image"]["sha256"] = json!("b".repeat(64)),
            "old_entry_sha" => c["entry_sha256"] = json!("c".repeat(64)),
            "controller_role" => c["role"] = json!("controller"),
            "wrong_root" => c["root"] = json!(case.root.join("other-root")),
            "unknown_client_field" => c["skip_verifier"] = json!(true),
            _ => unreachable!(),
        }
        let e = serde_json::to_vec(&e).unwrap();
        if fault != "old_entry_sha" {
            c["entry_sha256"] = json!(sha(&e));
        }
        case.write_private(&path, &e);
        case.write_private(
            &case.root.join("client.json"),
            &serde_json::to_vec(&c).unwrap(),
        );
        assert!(entry::load(&path).is_err(), "{fault}");
        refusals.push(fault);
    }
    case.write_private(&path, &entry_bytes);
    case.write_private(&case.root.join("client.json"), &client_bytes);
    std::fs::rename(&path, case.root.join("entry.saved")).unwrap();
    symlink(case.root.join("entry.saved"), &path).unwrap();
    assert!(entry::load(&path).is_err());
    record(
        "entry-negatives",
        json!({"valid_final_entry":true,"wrong_current_executable_refused":true,"refusals":refusals,"entry_symlink_refused":true,"network_connections":0}),
    );
}

async fn attach<RW: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
    stream: &mut BufReader<RW>,
) -> Attach {
    let bytes = net::read_frame(stream, frame::ATTACH_CAP)
        .await
        .unwrap()
        .unwrap();
    let attach: Attach = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(attach.role, Role::Model);
    assert!(uuid::Uuid::parse_str(&attach.nonce).is_ok());
    net::write_json(stream, &json!({"version":rcu_native_link::TRANSPORT_VERSION,"attached":true,"task":attach.task,"nonce":attach.nonce,"role":"model"}), frame::ATTACH_CAP).await.unwrap();
    // Explicit transport-only peer readiness, not a real MCP schema proof.
    net::write_json(
        stream,
        &net::ModelReady {
            transport_version: rcu_native_link::TRANSPORT_VERSION,
            model_ready: true,
            task: attach.task.clone(),
            nonce: attach.nonce.clone(),
            usable_ms: 1_200_000,
        },
        frame::ATTACH_CAP,
    )
    .await
    .unwrap();
    attach
}
async fn finish<RW: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
    stream: &mut BufReader<RW>,
    attach: &Attach,
) {
    let bytes = net::read_frame(stream, frame::ATTACH_CAP)
        .await
        .unwrap()
        .unwrap();
    let end: ModelEnd = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(end.transport_version, rcu_native_link::TRANSPORT_VERSION);
    assert!(end.model_end);
    assert_eq!(end.task, attach.task);
    assert_eq!(end.nonce, attach.nonce);
    net::write_json(stream, &json!({"transport_version":rcu_native_link::TRANSPORT_VERSION,"model_end_ack":true,"task":attach.task,"stage_receipt":"d".repeat(64),"complete":true}), frame::ATTACH_CAP).await.unwrap();
}

fn spawn_from_entry(path: &Path) -> tokio::process::Child {
    let value: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let server = &value["mcpServers"][entry::SERVER_KEY];
    let args = server["args"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>();
    tokio::process::Command::new(server["command"].as_str().unwrap())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap()
}
async fn wait_owned(child: &mut tokio::process::Child) -> std::process::ExitStatus {
    match tokio::time::timeout(Duration::from_secs(15), child.wait()).await {
        Ok(result) => result.unwrap(),
        Err(_) => {
            let kill = child.start_kill();
            let reaped = tokio::time::timeout(Duration::from_secs(2), child.wait())
                .await
                .is_ok();
            panic!(
                "owned relay deadline: kill_accepted={}, reaped={reaped}",
                kill.is_ok()
            );
        }
    }
}
async fn collect<R: tokio::io::AsyncRead + Unpin>(reader: R) -> Vec<u8> {
    let mut bytes = Vec::new();
    reader
        .take(frame::OUTBOUND as u64 + 1)
        .read_to_end(&mut bytes)
        .await
        .unwrap();
    assert!(bytes.len() <= frame::OUTBOUND);
    bytes
}

#[tokio::test]
async fn actual_final_entry_relay_preserves_mcp_bytes_and_hides_attach_end_control() {
    let case = Case::new();
    let socket = listener(IpAddr::V4(Ipv4Addr::LOCALHOST)).await;
    let address = socket.local_addr().unwrap();
    let tls_config = net::server_tls(&case.agent(address)).unwrap();
    let mut client = case.client(address, Role::Model);
    let path = case.entry(&mut client);
    let first_request =
        raw(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"only_test_peer":true}}));
    let second_request = raw(json!({"jsonrpc":"2.0","id":2,"method":"transport.large"}));
    let first_sha = sha(&first_request);
    let second_sha = sha(&second_request);
    let small = raw(json!({"jsonrpc":"2.0","id":1,"result":{"transport_only":true}}));
    let large = big_result(2, 9 * 1024 * 1024);
    let expected = [small.as_slice(), large.as_slice()].concat();
    let expected_sha = sha(&expected);
    let server = tokio::spawn(async move {
        let (tcp, _) = tokio::time::timeout(Duration::from_secs(8), socket.accept())
            .await
            .unwrap()
            .unwrap();
        let mut stream = BufReader::new(net::accept(tcp, tls_config).await.unwrap());
        let attached = attach(&mut stream).await;
        let first = net::read_frame(&mut stream, frame::INBOUND)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sha(&first), first_sha);
        net::write_frame(&mut stream, &small, frame::OUTBOUND)
            .await
            .unwrap();
        let second = net::read_frame(&mut stream, frame::INBOUND)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sha(&second), second_sha);
        tokio::time::sleep(Duration::from_millis(50)).await;
        net::write_frame(&mut stream, &large, frame::OUTBOUND)
            .await
            .unwrap();
        finish(&mut stream, &attached).await;
    });
    let mut child = spawn_from_entry(&path);
    let stdout = tokio::spawn(collect(child.stdout.take().unwrap()));
    let stderr = tokio::spawn(collect(child.stderr.take().unwrap()));
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(&first_request).await.unwrap();
    stdin.write_all(&second_request).await.unwrap();
    stdin.shutdown().await.unwrap();
    drop(stdin);
    assert!(wait_owned(&mut child).await.success());
    let output = stdout.await.unwrap();
    let stderr = stderr.await.unwrap();
    assert_eq!(output, expected);
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("model_end_ack_verified")
    );
    tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap();
    record(
        "actual-final-entry-relay",
        json!({"requests":2,"responses":2,"stdout_sha256":expected_sha,"stdout_bytes":output.len(),"attach_or_end_control_in_stdout":false,"entry_sha256":client.entry_sha256,"relay_image_sha256":client.relay_image.sha256,"spawned_from_actual_entry":true,"worker_implementation":"explicit_transport_test_peer_not_native_or_19tools"}),
    );
}

#[tokio::test]
async fn end_control_waits_for_stdout_delivery_not_merely_reply_receipt() {
    let case = Case::new();
    let socket = listener(IpAddr::V4(Ipv4Addr::LOCALHOST)).await;
    let address = socket.local_addr().unwrap();
    let tls_config = net::server_tls(&case.agent(address)).unwrap();
    let client = case.client(address, Role::Model);
    let request = raw(json!({"jsonrpc":"2.0","id":1,"method":"initialize"}));
    let response = raw(json!({"jsonrpc":"2.0","id":1,"result":{"must_be_delivered":true}}));
    let expected = response.clone();
    let (checked_tx, checked_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (tcp, _) = tokio::time::timeout(Duration::from_secs(8), socket.accept())
            .await
            .unwrap()
            .unwrap();
        let mut stream = BufReader::new(net::accept(tcp, tls_config).await.unwrap());
        let attached = attach(&mut stream).await;
        net::read_frame(&mut stream, frame::INBOUND)
            .await
            .unwrap()
            .unwrap();
        net::write_frame(&mut stream, &response, frame::OUTBOUND)
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(150), stream.fill_buf())
                .await
                .is_err(),
            "ModelEnd arrived before stdout writer could finish"
        );
        checked_tx.send(()).unwrap();
        finish(&mut stream, &attached).await;
    });
    let (mut input, model_input) = tokio::io::duplex(1024);
    let (model_output, mut output) = tokio::io::duplex(1);
    let relay =
        tokio::spawn(async move { net::relay_model(&client, model_input, model_output).await });
    input.write_all(&request).await.unwrap();
    input.shutdown().await.unwrap();
    drop(input);
    checked_rx.await.unwrap();
    let mut returned = Vec::new();
    output.read_to_end(&mut returned).await.unwrap();
    assert_eq!(returned, expected);
    assert_eq!(relay.await.unwrap().unwrap(), "d".repeat(64));
    tokio::time::timeout(Duration::from_secs(10), server)
        .await
        .unwrap()
        .unwrap();
    record(
        "model-end-after-delivery",
        json!({"stdout_backpressured":true,"premature_model_end":false,"end_after_full_stdout":true}),
    );
}

#[tokio::test]
async fn disconnect_or_control_pollution_exits_actual_relay_with_stdin_still_open() {
    let case = Case::new();
    let mut records = Vec::new();
    for fault in [
        "disconnect",
        "control_pollution",
        "wrong_attach_nonce",
        "missing_end_ack",
        "incomplete_end_ack",
    ] {
        let socket = listener(IpAddr::V4(Ipv4Addr::LOCALHOST)).await;
        let address = socket.local_addr().unwrap();
        let tls_config = net::server_tls(&case.agent(address)).unwrap();
        let mut client = case.client(address, Role::Model);
        let path = case.entry(&mut client);
        let server = tokio::spawn(async move {
            let (tcp, _) = tokio::time::timeout(Duration::from_secs(8), socket.accept())
                .await
                .unwrap()
                .unwrap();
            let mut stream = BufReader::new(net::accept(tcp, tls_config).await.unwrap());
            if fault == "wrong_attach_nonce" {
                let bytes = net::read_frame(&mut stream, frame::ATTACH_CAP)
                    .await
                    .unwrap()
                    .unwrap();
                let received: Attach = serde_json::from_slice(&bytes).unwrap();
                net::write_json(&mut stream, &json!({"version":rcu_native_link::TRANSPORT_VERSION,"attached":true,"task":received.task,"nonce":uuid::Uuid::new_v4().to_string(),"role":"model"}), frame::ATTACH_CAP).await.unwrap();
                return;
            }
            let attached = attach(&mut stream).await;
            if fault == "control_pollution" {
                net::write_json(
                    &mut stream,
                    &json!({"transport_version":rcu_native_link::TRANSPORT_VERSION,"unexpected_control":true}),
                    frame::ATTACH_CAP,
                )
                .await
                .unwrap();
            } else if fault == "missing_end_ack" || fault == "incomplete_end_ack" {
                let end = net::read_frame(&mut stream, frame::ATTACH_CAP)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    serde_json::from_slice::<Value>(&end).unwrap()["model_end"],
                    true
                );
                if fault == "incomplete_end_ack" {
                    net::write_json(&mut stream, &json!({"transport_version":rcu_native_link::TRANSPORT_VERSION,"model_end_ack":true,"task":attached.task,"stage_receipt":"d".repeat(64),"complete":false}), frame::ATTACH_CAP).await.unwrap();
                }
            }
        });
        let mut child = spawn_from_entry(&path);
        let stdout = tokio::spawn(collect(child.stdout.take().unwrap()));
        let stderr = tokio::spawn(collect(child.stderr.take().unwrap()));
        if fault == "missing_end_ack" || fault == "incomplete_end_ack" {
            drop(child.stdin.take());
        }
        // Otherwise deliberately keep ChildStdin alive: the OS read thread
        // cannot receive EOF. The relay must still stop after peer loss.
        let began = std::time::Instant::now();
        assert!(!wait_owned(&mut child).await.success());
        assert!(stdout.await.unwrap().is_empty());
        assert!(
            String::from_utf8(stderr.await.unwrap())
                .unwrap()
                .contains("held_no_reconnect_or_replay")
        );
        tokio::time::timeout(Duration::from_secs(10), server)
            .await
            .unwrap()
            .unwrap();
        records.push(json!({"fault":fault,"stdout_bytes":0,"exit_success":false,"elapsed_ms":began.elapsed().as_millis(),"stdin_deliberately_open":fault!="missing_end_ack" && fault!="incomplete_end_ack","connections":1}));
    }
    record("loss-no-replay-control-isolation", json!({"cases":records}));
}

#[tokio::test]
async fn outgoing_frame_cap_and_whole_flush_timeout_are_bounded() {
    let mut sink = tokio::io::sink();
    assert!(net::write_frame(&mut sink, b"{}\n", 2).await.is_err());
    assert!(net::write_frame(&mut sink, b"{}", 3).await.is_err());
    assert!(
        net::write_json(&mut sink, &json!({"pad":"x".repeat(1000)}), 100)
            .await
            .is_err()
    );
    struct NeverFlush;
    impl tokio::io::AsyncWrite for NeverFlush {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            bytes: &[u8],
        ) -> std::task::Poll<io::Result<usize>> {
            std::task::Poll::Ready(Ok(bytes.len()))
        }
        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<io::Result<()>> {
            std::task::Poll::Pending
        }
        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }
    let began = std::time::Instant::now();
    let error = net::write_frame(&mut NeverFlush, b"{}\n", 3)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("whole_write_flush_deadline"));
    assert!(began.elapsed() < Duration::from_secs(7));
    record(
        "write-flush-bound",
        json!({"write_completed_flush_stalled":true,"deadline_ms":net::IO_TIMEOUT.as_millis(),"elapsed_ms":began.elapsed().as_millis(),"result":"held"}),
    );
}

#[tokio::test]
async fn non_exact_ready_never_becomes_usable_mcp_or_stdout() {
    for usable_ms in [1_199_999, 1_200_001] {
        let case = Case::new();
        let socket = listener(IpAddr::V4(Ipv4Addr::LOCALHOST)).await;
        let address = socket.local_addr().unwrap();
        let tls = net::server_tls(&case.agent(address)).unwrap();
        let config = case.client(address, Role::Model);
        let server = tokio::spawn(async move {
            let (tcp, _) = socket.accept().await.unwrap();
            let mut stream = BufReader::new(net::accept(tcp, tls).await.unwrap());
            let raw = frame::read_frame(&mut stream, frame::ATTACH_CAP)
                .await
                .unwrap()
                .unwrap();
            let attach: Attach = serde_json::from_slice(&raw).unwrap();
            net::write_json(&mut stream,&json!({"version":rcu_native_link::TRANSPORT_VERSION,"attached":true,"task":attach.task,"nonce":attach.nonce,"role":"model"}),frame::ATTACH_CAP).await.unwrap();
            net::write_json(
                &mut stream,
                &net::ModelReady {
                    transport_version: rcu_native_link::TRANSPORT_VERSION,
                    model_ready: true,
                    task: attach.task,
                    nonce: attach.nonce,
                    usable_ms,
                },
                frame::ATTACH_CAP,
            )
            .await
            .unwrap();
        });
        let (stdout, mut capture) = tokio::io::duplex(1024);
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            net::relay_model(&config, tokio::io::empty(), stdout),
        )
        .await
        .unwrap();
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("actual_model_ready_binding_or_full_budget")
        );
        let mut raw = Vec::new();
        capture.read_to_end(&mut raw).await.unwrap();
        assert!(raw.is_empty(), "internal ready is never MCP output");
        server.await.unwrap();
    }
}
