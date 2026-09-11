#![cfg(all(feature = "fixture", unix))]
//! Actual agent executable + real private mTLS TCP + actual relay final entry +
//! unchanged nineteen-tool MCP fixture worker. Not Windows/native/model evidence.
use base64::{Engine, engine::general_purpose::STANDARD};
use rcu_link_trust::IdentityPaths;
use rcu_native_link::{
    budget::{BudgetSpec, now_ms},
    client, entry, frame, invalid, net,
    private::Directory,
    sha,
    types::*,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

#[path = "full_link/codex_host.rs"]
mod codex_host;

const GOAL: &str = "bounded real private TLS fixture goal";
fn expect(test: bool, message: &str) -> io::Result<()> {
    if test { Ok(()) } else { Err(invalid(message)) }
}
fn raw_evidence(kind: &str, bytes: &[u8]) -> io::Result<()> {
    if let Some(root) = std::env::var_os("RCU_NATIVE_FLOW_EVIDENCE") {
        let dir = PathBuf::from(root).join("actual-wire");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(format!("{kind}-{}.ndjson", sha(bytes))), bytes)?;
    }
    Ok(())
}
fn data(v: Value) -> io::Result<Value> {
    expect(
        v.get("error").is_none() && v["result"]["isError"] != true,
        &v.to_string(),
    )?;
    Ok(v["result"]["structuredContent"].clone())
}
fn copy_image(dir: &Directory, name: &str, from: &Path) -> Image {
    let bytes = std::fs::read(from).unwrap();
    dir.create_new(name, &bytes, 128 * 1024 * 1024).unwrap();
    let path = dir.path().join(name);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    Image {
        path,
        sha256: sha(&bytes),
    }
}
struct Case {
    _temp: tempfile::TempDir,
    install: PathBuf,
    agent: Child,
    controller: ClientConfig,
    relay: Image,
    entry: PathBuf,
    sequence: u64,
}
impl Case {
    async fn new(label: &str, config: Value) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root =
            Directory::create(&temp.path().canonicalize().unwrap().join("private-run")).unwrap();
        let profiles = rcu_link_trust::provision(&root.path().join("profiles")).unwrap();
        let install = Directory::create(&root.path().join("installation")).unwrap();
        let bins = Directory::create(&install.path().join("bin")).unwrap();
        let agent_image = copy_image(
            &bins,
            "rcu-native-agent",
            Path::new(env!("CARGO_BIN_EXE_rcu-native-agent")),
        );
        let worker_path = std::env::var_os("RCU_NATIVE_TEST_WORKER")
            .expect("explicit prebuilt link-fixture worker path required; no hidden test build");
        let worker_image = copy_image(&bins, "rcu-mcp", Path::new(&worker_path));
        let relay = copy_image(
            &bins,
            "rcu-native-relay",
            Path::new(env!("CARGO_BIN_EXE_rcu-native-relay")),
        );
        let task = format!("flow-{label}");
        let now = now_ms();
        let approval = Approval {
            version: 1,
            task: task.clone(),
            controller_host: "owned-localhost-fixture".into(),
            guest_claim: None,
            native_binding: None,
            budget: BudgetSpec {
                issued_unix_ms: now,
                expires_unix_ms: now + 2_100_000,
                maximum_elapsed_ms: 2_100_000,
                clock_drift_tolerance_ms: 2000,
            },
            request_prefix: format!("{task}-"),
            files: vec![FileSpec {
                name: "allocated.txt".into(),
                initial_utf8: "initial untouched file".into(),
                expected_sha256: sha(GOAL.as_bytes()),
            }],
            expected_ui_value: GOAL.into(),
            stages: [
                Stage::Bootstrap,
                Stage::Preflight,
                Stage::Launch,
                Stage::Activate,
            ]
            .into_iter()
            .map(|stage| {
                (
                    stage,
                    vec![Step {
                        id: "actual-targets".into(),
                        tool: ControlTool::Targets,
                        arguments: json!({}),
                    }],
                )
            })
            .collect(),
            input_grant_template: Value::Null,
            lifecycle_grant_template: Value::Null,
            fixture_control: config,
        };
        let cfg = AgentConfig {
            version: 1,
            install_root: install.path().into(),
            state_root: install.path().join("state"),
            approval: install.path().join("approved-task.json"),
            bind: "127.0.0.1:0".parse().unwrap(),
            server_name: rcu_link_trust::SERVER_DNS_NAME.into(),
            trust: IdentityPaths::in_profile(&root.path().join("profiles/agent")),
            controller_peer_sha256: profiles.controller.certificate_sha256.as_str().into(),
            model_peer_sha256: profiles.model.certificate_sha256.as_str().into(),
            agent_image: agent_image.clone(),
            worker_image,
            backend: BackendMode::Fixture,
        };
        install
            .create_new("agent.json", &serde_json::to_vec(&cfg).unwrap(), 65536)
            .unwrap();
        install
            .create_new(
                "approved-task.json",
                &serde_json::to_vec(&approval).unwrap(),
                256 * 1024,
            )
            .unwrap();
        let mut agent = Command::new(&agent_image.path)
            .args([
                "--config",
                cfg.install_root.join("agent.json").to_str().unwrap(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut line = String::new();
        let got = tokio::time::timeout(
            Duration::from_secs(10),
            BufReader::new(agent.stdout.take().unwrap()).read_line(&mut line),
        )
        .await
        .unwrap()
        .unwrap();
        if got == 0 {
            let mut e = String::new();
            agent
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut e)
                .await
                .unwrap();
            panic!("agent startup: {e}");
        }
        let ready: Value = serde_json::from_str(&line).unwrap();
        let endpoint = ready["listening"].as_str().unwrap().parse().unwrap();
        let controller = ClientConfig {
            model_output: Default::default(),
            version: 1,
            root: root.path().into(),
            endpoint,
            server_name: cfg.server_name.clone(),
            trust: IdentityPaths::in_profile(&root.path().join("profiles/controller")),
            agent_peer_sha256: profiles.agent.certificate_sha256.as_str().into(),
            task,
            approval_sha256: approval.digest(),
            role: Role::Controller,
            relay_image: relay.clone(),
            entry_sha256: None,
        };
        let mut model = controller.clone();
        model.role = Role::Model;
        model.trust = IdentityPaths::in_profile(&root.path().join("profiles/model"));
        let entry_path = root.path().join(".mcp.json");
        rcu_native_link::plan::write_client_entry(model).unwrap();
        Self {
            _temp: temp,
            install: install.path().into(),
            agent,
            controller,
            relay,
            entry: entry_path,
            sequence: 0,
        }
    }
    async fn control(&mut self, operation: ControlOp) -> io::Result<Value> {
        self.sequence += 1;
        client::control(
            &self.controller,
            &ControlRequest {
                version: 1,
                request_id: format!("control-{}", self.sequence),
                operation,
            },
        )
        .await
    }
    async fn stages(&mut self) -> io::Result<()> {
        let first = self.control(ControlOp::Prepare).await?;
        expect(first["ok"] == true, &first.to_string())?;
        let mut previous = first["result"]["stage_receipt"]
            .as_str()
            .ok_or_else(|| invalid("prepare receipt"))?
            .to_owned();
        for stage in [
            Stage::Bootstrap,
            Stage::Preflight,
            Stage::Launch,
            Stage::Activate,
        ] {
            let result = self
                .control(ControlOp::RunStage {
                    stage,
                    previous_receipt: previous,
                })
                .await?;
            expect(result["ok"] == true, &result.to_string())?;
            previous = result["result"]["stage_receipt"]
                .as_str()
                .ok_or_else(|| invalid("stage receipt"))?
                .to_owned();
        }
        Ok(())
    }
    fn relay(&self) -> io::Result<Rpc> {
        let mut child = Command::new(&self.relay.path)
            .args(["--entry", self.entry.to_str().unwrap()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        Ok(Rpc {
            input: child.stdin.take(),
            output: BufReader::new(child.stdout.take().unwrap()),
            child,
            next: 10,
            transcript: Vec::new(),
        })
    }
    fn file(&self) -> io::Result<Vec<u8>> {
        std::fs::read(
            self.install
                .join("state/runs")
                .join(&self.controller.task)
                .join("allocated.txt"),
        )
    }
    async fn stop(&mut self) -> io::Result<()> {
        if self.agent.try_wait()?.is_none() {
            let cancel = self.control(ControlOp::Cancel).await;
            if let Ok(v) = cancel {
                let rows = v["result"]["cleanup"]["owned_workers"]
                    .as_array()
                    .ok_or_else(|| invalid("owned cleanup evidence missing"))?;
                expect(
                    rows.iter().all(|r| r["exit_observed"] == true),
                    "owned worker exit unconfirmed",
                )?;
                self.agent.start_kill()?; // only the directly retained test agent
            }
        }
        tokio::time::timeout(Duration::from_secs(15), self.agent.wait()).await??;
        Ok(())
    }
    async fn restart_same_installation(&mut self) -> io::Result<()> {
        self.stop().await?;
        self.agent = Command::new(self.install.join("bin/rcu-native-agent"))
            .args([
                "--config",
                self.install.join("agent.json").to_str().unwrap(),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let mut line = String::new();
        tokio::time::timeout(
            Duration::from_secs(10),
            BufReader::new(self.agent.stdout.take().unwrap()).read_line(&mut line),
        )
        .await??;
        let ready: Value = serde_json::from_str(&line)?;
        self.controller.endpoint = ready["listening"]
            .as_str()
            .ok_or_else(|| invalid("restarted listener missing"))?
            .parse()
            .map_err(io::Error::other)?;
        let path = self.entry.parent().unwrap().join("client.json");
        let mut model: ClientConfig = serde_json::from_slice(&std::fs::read(&path)?)?;
        model.endpoint = self.controller.endpoint;
        model.task = self.controller.task.clone();
        model.approval_sha256 = self.controller.approval_sha256.clone();
        std::fs::write(path, serde_json::to_vec(&model)?)?;
        Ok(())
    }
    fn evidence(&self, name: &str, outcome: &io::Result<()>, transcript: &[Value]) {
        if let Some(path) = std::env::var_os("RCU_NATIVE_FLOW_EVIDENCE") {
            let path = PathBuf::from(path);
            std::fs::create_dir_all(&path).unwrap();
            let mut journal = Vec::new();
            let saved = path.join(format!("{name}-state"));
            std::fs::create_dir_all(&saved).unwrap();
            for sub in ["journal", "responses"] {
                let source = self.install.join("state").join(sub);
                let dest = saved.join(sub);
                std::fs::create_dir_all(&dest).unwrap();
                if let Ok(entries) = std::fs::read_dir(source) {
                    for item in entries {
                        let item = item.unwrap();
                        if item.file_type().unwrap().is_file() {
                            std::fs::copy(item.path(), dest.join(item.file_name())).unwrap();
                        }
                    }
                }
            }
            for file in ["fixture-model.json", "fixture-original.png"] {
                let source = self.install.join("state/native-state").join(file);
                if source.is_file() {
                    std::fs::copy(source, saved.join(file)).unwrap();
                }
            }
            if let Ok(bytes) = self.file() {
                std::fs::write(saved.join("allocated.txt"), bytes).unwrap();
            }

            if let Ok(entries) = std::fs::read_dir(self.install.join("state/journal")) {
                let mut entries = entries.map(|v| v.unwrap().path()).collect::<Vec<_>>();
                entries.sort();
                for p in entries {
                    let b = std::fs::read(p).unwrap();
                    journal.push(json!({"sha256":sha(&b),"event":serde_json::from_slice::<Value>(&b).unwrap()}));
                }
            }
            let report = json!({"schema":"rcu.actual_owned_agent_relay_mcp_loopback.v1","case":name,"passed":outcome.is_ok(),"error":outcome.as_ref().err().map(ToString::to_string),"native_runtime_executed":false,"actual_Codex_model_run":false,"private_keys_included":false,"transport":"real_tcp_tls13_private_pairing","fixture_effect_file_sha256":self.file().ok().map(|b|sha(&b)),"transcript":transcript,"journal":journal});
            std::fs::write(
                path.join(format!("{name}.json")),
                serde_json::to_vec_pretty(&report).unwrap(),
            )
            .unwrap();
        }
    }
}
struct Rpc {
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    child: Child,
    next: u64,
    transcript: Vec<Value>,
}
impl Rpc {
    async fn send(&mut self, v: Value) -> io::Result<()> {
        let mut bytes = serde_json::to_vec(&v)?;
        bytes.push(b'\n');
        raw_evidence("request", &bytes)?;
        self.transcript
            .push(json!({"direction":"request","sha256":sha(&bytes),"bytes":bytes.len(),"body":v}));
        net::write_frame(self.input.as_mut().unwrap(), &bytes, frame::INBOUND).await
    }
    async fn receive(&mut self) -> io::Result<Value> {
        let raw = tokio::time::timeout(
            Duration::from_secs(15),
            frame::read_frame(&mut self.output, frame::OUTBOUND),
        )
        .await??
        .ok_or_else(|| invalid("relay_eof"))?;
        let value: Value = serde_json::from_slice(&raw)?;
        raw_evidence("response", &raw)?;
        self.transcript.push(json!({"direction":"response","sha256":sha(&raw),"bytes":raw.len(),"body":if raw.len()<65536{value.clone()}else{json!({"large_response_id":value["id"]})}}));
        Ok(value)
    }
    async fn call(&mut self, name: &str, args: Value) -> io::Result<Value> {
        self.next += 1;
        let id = self.next;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})).await?;
        let reply = self.receive().await?;
        expect(reply["id"] == id, "RPC response id mismatch")?;
        Ok(reply)
    }
    async fn setup(&mut self) -> io::Result<(Value, Value)> {
        self.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"actual-loopback-test","version":"1"}}})).await?;
        let init = self.receive().await?;
        expect(
            init["result"]["serverInfo"]["version"] == "0.4.0-m3a.7",
            "fixed worker version",
        )?;
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await?;
        self.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}))
            .await?;
        let listed = self.receive().await?;
        let mut tools = listed["result"]["tools"]
            .as_array()
            .ok_or_else(|| invalid("missing actual tools"))?
            .clone();
        tools.sort_by_key(|t| t["name"].as_str().unwrap().to_owned());
        expect(
            tools.len() == 19
                && sha(&serde_json::to_vec(&tools)?) == rcu_native_link::mcp::schema_sha(),
            "actual full schema mismatch",
        )?;
        data(self.call("computer.targets", json!({})).await?)?;
        let observed = data(
            self.call("computer.observe", json!({"target":"synthetic-only"}))
                .await?,
        )?;
        let lease = data(
            self.call(
                "computer.claim",
                json!({"target":"synthetic-only","ttl_ms":60000}),
            )
            .await?,
        )?;
        Ok((observed, lease))
    }
    async fn assert_diagnostic_only(&mut self) -> io::Result<()> {
        self.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"held-session-test","version":"1"}}})).await?;
        let initialized = self.receive().await?;
        expect(
            initialized["result"]["instructions"]
                .as_str()
                .is_some_and(|s| s.starts_with("DIAGNOSTIC ONLY")),
            "held session must identify diagnostic-only MCP",
        )?;
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await?;
        let reply = self.call("computer.targets", json!({})).await?;
        let diagnostic = &reply["result"]["structuredContent"];
        expect(
            reply["result"]["isError"] == true
                && diagnostic["code"] == "native_session_unavailable"
                && diagnostic["operation_dispatched"] == false
                && diagnostic["goal_verified"] == false,
            "held entry cannot admit operations or claim a goal",
        )?;
        expect(self.close().await?, "diagnostic EOF should close cleanly")?;
        expect(
            !self.transcript.last().unwrap()["stderr"]
                .as_str()
                .unwrap()
                .contains("model_end_ack_verified"),
            "diagnostic exit must never claim ModelEnd",
        )
    }
    async fn close(&mut self) -> io::Result<bool> {
        if let Some(mut input) = self.input.take() {
            input.shutdown().await?;
            drop(input);
        }
        let status = tokio::time::timeout(Duration::from_secs(35), self.child.wait()).await??;
        let mut stderr = String::new();
        if let Some(mut s) = self.child.stderr.take() {
            s.read_to_string(&mut stderr).await?;
        }
        self.transcript
            .push(json!({"relay_exit":status.code(),"stderr":stderr}));
        Ok(status.success())
    }
}

#[tokio::test]
async fn actual_private_link_full_eight_stage_goal_and_independent_file_oracle() {
    let mut case = Case::new("positive", json!({"png_size":[1800,1700]})).await;
    let mut transcript = Vec::new();
    let outcome:io::Result<()>=async {
        case.stages().await?;
        let mut rpc=case.relay()?;
        let body:io::Result<()>=async {
            let (o,l)=rpc.setup().await?;
            let denied=case.control(ControlOp::RunStage{stage:Stage::Bootstrap,previous_receipt:"wrong".into()}).await?;
            expect(denied["ok"]==false,"concurrent stage must be locally denied")?;
            let state=case.control(ControlOp::Status).await?;
            expect(state["result"]["status"]["state"]["hold"]==false,"bad second connection must not HOLD active model")?;
            tokio::time::timeout(Duration::from_secs(20),async {
                for _ in 0..270 {let socket=tokio::net::TcpStream::connect(case.controller.endpoint).await?;drop(socket);}
                Ok::<_,io::Error>(())
            }).await??;
            data(rpc.call("computer.targets",json!({})).await?)?;
            let after_bad_tcp=tokio::time::timeout(Duration::from_secs(12),async {
                loop {match case.control(ControlOp::Status).await {
                    Ok(v)=>break Ok::<_,io::Error>(v),
                    Err(_)=>tokio::time::sleep(Duration::from_millis(200)).await,
                }}
            }).await??;
            expect(after_bad_tcp["result"]["status"]["state"]["hold"]==false,"unauthenticated TCP attempts cannot exhaust a global budget and cancel another owner")?;
            rpc.transcript.push(json!({"unauthenticated_owned_loopback_connects":270,"active_model_not_held":true}));

            let original=std::fs::read(case.install.join("state/native-state/fixture-original.png"))?;
            expect(original.len()>8*1024*1024,"actual original PNG larger than eight MiB")?;
            let first=data(rpc.call("computer.artifact",json!({"artifact_id":o["artifact_id"],"offset":0,"length":8*1024*1024,"as_image":false})).await?)?;
            let mut reconstructed=STANDARD.decode(first["base64"].as_str().ok_or_else(||invalid("first chunk missing"))?).map_err(io::Error::other)?;
            expect(reconstructed.len()==8*1024*1024,"exact first chunk length")?;
            let last=data(rpc.call("computer.artifact",json!({"artifact_id":o["artifact_id"],"offset":reconstructed.len(),"length":1024*1024,"as_image":false})).await?)?;
            reconstructed.extend(STANDARD.decode(last["base64"].as_str().ok_or_else(||invalid("last chunk missing"))?).map_err(io::Error::other)?);
            expect(reconstructed==original && first["sha256"]==sha(&original),"actual MCP/TCP/relay chunks must reconstruct identical PNG")?;
            expect(rcu_imaging::decode_original(&reconstructed).map_err(io::Error::other)?.size==[1800,1700],"actual PNG dimensions")?;
            rpc.transcript.push(json!({"large_actual_png_bytes":original.len(),"large_actual_png_sha256":sha(&original),"actual_chunks_reconstructed_identically":true}));let request=format!("{}-input",case.controller.task);
            let receipt=data(rpc.call("computer.act_checked",json!({"target":"synthetic-only","frame":o["frame"],"element":"Input","lease":l["lease"],"request_id":request,"intent":"text","value":GOAL,"operation_id":"model-input"})).await?)?;
            expect(receipt["goal_verified"]==false && receipt["dispatch"]=="returned","input acknowledgement not a goal")?;
            let fresh=data(rpc.call("computer.observe",json!({"target":"synthetic-only"})).await?)?;
            let verified=data(rpc.call("computer.verify",json!({"target":"synthetic-only","frame":fresh["frame"],"element":"Input","request_id":request,"predicate":"element_value","expected":GOAL,"timeout_ms":1000,"stable_samples":2})).await?)?;
            expect(verified["goal_verified"]==true,"actual independent fixture value oracle")?;
            data(rpc.call("computer.release",json!({"lease":l["lease"]})).await?)?;
            Ok(())
        }.await;
        let closed=rpc.close().await;transcript=rpc.transcript;
        body?;expect(closed?,"relay did not receive committed model END acknowledgement")?;
        expect(case.file()?==GOAL.as_bytes(),"independent real file effect mismatch")?;
        let status=case.control(ControlOp::Status).await?;
        let state=&status["result"]["status"]["state"];
        let previous=state["receipts"]["model"]["digest"].as_str().ok_or_else(||invalid(&status.to_string()))?.to_owned();
        let oracle=case.control(ControlOp::Oracle{previous_receipt:previous}).await?;expect(oracle["ok"]==true,&oracle.to_string())?;
        let previous=oracle["result"]["stage_receipt"].as_str().unwrap().to_owned();
        let clean=case.control(ControlOp::Cleanup{previous_receipt:previous}).await?;expect(clean["ok"]==true,&clean.to_string())?;
        tokio::time::timeout(Duration::from_secs(10),case.agent.wait()).await??;
        Ok(())
    }.await;
    let cleanup = case.stop().await;
    case.evidence("positive", &outcome, &transcript);
    outcome.unwrap();
    cleanup.unwrap();
}

#[tokio::test]
async fn actual_unknown_effect_and_no_effect_ack_never_become_goal_or_replay() {
    for (label, config, effect) in [
        ("lost-effect", json!({"disconnect_after_effect":true}), true),
        ("ack-no-effect", json!({"ack_without_effect":true}), false),
    ] {
        let mut case = Case::new(label, config).await;
        let mut transcript = Vec::new();
        let outcome:io::Result<()>=async {
            case.stages().await?;
            let mut rpc=case.relay()?;
            let body:io::Result<()>=async {
                let(o,l)=rpc.setup().await?;let request=format!("{}-one",case.controller.task);
                let result=data(rpc.call("computer.act_checked",json!({"target":"synthetic-only","frame":o["frame"],"element":"Input","lease":l["lease"],"request_id":request,"intent":"text","value":GOAL,"operation_id":"single-op"})).await?)?;
                expect(result["goal_verified"]==false,"effect acknowledgement never goal")?;
                if effect { expect(result["dispatch"]=="uncertain","lost after effect must stay uncertain")?; }
                else {
                    expect(result["dispatch"]=="returned","actual no-effect acknowledgement expected")?;
                    let fresh=data(rpc.call("computer.observe",json!({"target":"synthetic-only"})).await?)?;
                    let oracle=data(rpc.call("computer.verify",json!({"target":"synthetic-only","frame":fresh["frame"],"element":"Input","request_id":request,"predicate":"element_value","expected":GOAL,"timeout_ms":300,"stable_samples":2})).await?)?;
                    expect(oracle["goal_verified"]==false,"no-effect oracle must fail")?;
                    data(rpc.call("computer.release",json!({"lease":l["lease"]})).await?)?;
                }
                Ok(())
            }.await;
            let closed=rpc.close().await;transcript=rpc.transcript;body?;
            expect(!closed?,"incomplete/unknown goal must not produce model END success")?;
            let before=std::fs::read(case.install.join("state/native-state/fixture-model.json"))?;
            let model:Value=serde_json::from_slice(&before)?;expect(model["calls"]==1,"exactly one actual fixture dispatch")?;
            expect((case.file()?==GOAL.as_bytes())==effect,"file effect must be independent of acknowledgement")?;
            let status=case.control(ControlOp::Reconcile).await?;
            expect(status["result"]["status"]["state"]["hold"]==true,"unknown/incomplete model remains durable HOLD")?;
            expect(status["result"]["hold_cleared"]==false,"readonly reconciliation never clears HOLD")?;
            expect(status["result"]["file_readback"]["all_match"]==effect,"read-only file reconciliation independently confirms actual bytes without clearing HOLD")?;
            let mut retried=case.relay()?;
            retried.assert_diagnostic_only().await?;
            expect(std::fs::read(case.install.join("state/native-state/fixture-model.json"))?==before,"reconnection cannot replay or reset actual effect")?;
            transcript.extend(retried.transcript);
            Ok(())
        }.await;
        let cleanup = case.stop().await;
        case.evidence(label, &outcome, &transcript);
        outcome.unwrap();
        cleanup.unwrap();
    }
}

async fn direct_rpc<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
    peer: &mut BufReader<T>,
    value: Value,
) -> io::Result<Value> {
    direct_rpc_ready(peer, value, None).await
}
async fn direct_rpc_ready<T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin>(
    peer: &mut BufReader<T>,
    value: Value,
    expected_ready: Option<&Attach>,
) -> io::Result<Value> {
    let mut raw = serde_json::to_vec(&value)?;
    raw.push(b'\n');
    raw_evidence("request", &raw)?;
    net::write_frame(peer, &raw, frame::INBOUND).await?;
    if value.get("id").is_none() {
        return Ok(Value::Null);
    }
    if let Some(attach) = expected_ready {
        let raw = tokio::time::timeout(
            Duration::from_secs(15),
            frame::read_frame(peer, frame::ATTACH_CAP),
        )
        .await??
        .ok_or_else(|| invalid("direct model ready EOF"))?;
        let ready: net::ModelReady = serde_json::from_slice(&raw)?;
        expect(
            ready.transport_version == rcu_native_link::TRANSPORT_VERSION
                && ready.model_ready
                && ready.task == attach.task
                && ready.nonce == attach.nonce
                && ready.usable_ms == 1_200_000,
            "direct full usable ready binding",
        )?;
        raw_evidence("transport-ready-not-mcp", &raw)?;
    }
    let raw = tokio::time::timeout(
        Duration::from_secs(15),
        frame::read_frame(peer, frame::OUTBOUND),
    )
    .await??
    .ok_or_else(|| invalid("direct worker EOF"))?;
    raw_evidence("response", &raw)?;
    let reply: Value = serde_json::from_slice(&raw)?;
    expect(reply["id"] == value["id"], "direct real RPC id")?;
    Ok(reply)
}
#[tokio::test]
async fn actual_physical_tcp_loss_after_file_effect_and_agent_restart_stay_held() {
    let mut case = Case::new("physical-loss", json!({})).await;
    let mut transcript = Vec::new();
    let outcome:io::Result<()>=async {
        case.stages().await?;
        let config=entry::load(&case.entry)?;
        let mut peer=BufReader::new(net::connect(&config).await?);
        let attach=Attach{version:rcu_native_link::TRANSPORT_VERSION,task:config.task.clone(),approval_sha256:config.approval_sha256.clone(),role:Role::Model,nonce:uuid::Uuid::new_v4().to_string()};
        net::write_json(&mut peer,&serde_json::to_value(&attach)?,frame::ATTACH_CAP).await?;
        let ack=tokio::time::timeout(net::IO_TIMEOUT,frame::read_frame(&mut peer,frame::ATTACH_CAP)).await??.ok_or_else(||invalid("model attach EOF"))?;
        expect(serde_json::from_slice::<Value>(&ack)?["attached"]==true,"actual model attached")?;
        direct_rpc(&mut peer,json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"physical-loss-test","version":"1"}}})).await?;
        direct_rpc(&mut peer,json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await?;
        direct_rpc_ready(&mut peer,json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),Some(&attach)).await?;
        let o=data(direct_rpc(&mut peer,json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"computer.observe","arguments":{"target":"synthetic-only"}}})).await?)?;
        let l=data(direct_rpc(&mut peer,json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"computer.claim","arguments":{"target":"synthetic-only","ttl_ms":60000}}})).await?)?;
        // Send exactly once, deliberately do NOT read the action reply or END.
        let value=json!({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"computer.act_checked","arguments":{"target":"synthetic-only","frame":o["frame"],"element":"Input","lease":l["lease"],"request_id":format!("{}-single",config.task),"operation_id":"physical-once","intent":"text","value":GOAL}}});
        let mut raw=serde_json::to_vec(&value)?;raw.push(b'\n');raw_evidence("request",&raw)?;net::write_frame(&mut peer,&raw,frame::INBOUND).await?;
        tokio::time::timeout(Duration::from_secs(8),async{
            while case.file()?!=GOAL.as_bytes(){tokio::time::sleep(Duration::from_millis(20)).await;}
            Ok::<_,io::Error>(())
        }).await??;
        drop(peer); // Real owned TCP endpoint closes, not a synthetic marker.
        let held=tokio::time::timeout(Duration::from_secs(12),async{
            loop {let v=case.control(ControlOp::Reconcile).await?;
                if v["result"]["status"]["state"]["hold"]==true {break Ok::<_,io::Error>(v);}
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }).await??;
        expect(held["result"]["file_readback"]["all_match"]==true,"effect oracle true still cannot clear transport HOLD")?;
        expect(held["result"]["status"]["state"]["receipts"]["model"].is_null(),"lost model cannot have successful end receipt")?;
        let before=std::fs::read(case.install.join("state/native-state/fixture-model.json"))?;
        case.restart_same_installation().await?;
        let restarted=case.control(ControlOp::Reconcile).await?;
        expect(restarted["result"]["status"]["state"]["hold"]==true,"actual agent restart preserves HOLD")?;
        let mut rpc=case.relay()?;rpc.assert_diagnostic_only().await?;transcript=rpc.transcript;
        expect(before==std::fs::read(case.install.join("state/native-state/fixture-model.json"))?,"actual restarted agent did not replay physical effect")?;
        transcript.push(json!({"physical_tcp_dropped_after_actual_file_effect":true,"action_reply_intentionally_unread":true,"agent_restarted_same_installation":true,"model_end_receipt":null,"replay_count":0}));
        Ok(())
    }.await;
    let cleanup = case.stop().await;
    case.evidence("physical-loss-restart", &outcome, &transcript);
    outcome.unwrap();
    cleanup.unwrap();
}

#[tokio::test]
async fn completed_old_task_does_not_auto_exit_new_approval_agent_before_prepare() {
    let mut case = Case::new("next-approval", json!({})).await;
    let mut transcript = Vec::new();
    let outcome:io::Result<()>=async {
        case.stages().await?;
        let mut rpc=case.relay()?;let request=format!("{}-one",case.controller.task);
        let body:io::Result<()>=async {
            let(o,l)=rpc.setup().await?;
            data(rpc.call("computer.act_checked",json!({"target":"synthetic-only","frame":o["frame"],"element":"Input","lease":l["lease"],"request_id":request,"intent":"text","value":GOAL,"operation_id":"next-task-prior-op"})).await?)?;
            let o=data(rpc.call("computer.observe",json!({"target":"synthetic-only"})).await?)?;
            let verified=data(rpc.call("computer.verify",json!({"target":"synthetic-only","frame":o["frame"],"element":"Input","request_id":request,"predicate":"element_value","expected":GOAL,"timeout_ms":1000,"stable_samples":2})).await?)?;
            expect(verified["goal_verified"]==true,"prior task actual goal")?;
            data(rpc.call("computer.release",json!({"lease":l["lease"]})).await?)?;Ok(())
        }.await;
        let closed=rpc.close().await;transcript=rpc.transcript;body?;expect(closed?,"prior END receipt")?;
        let status=case.control(ControlOp::Status).await?;
        let previous=status["result"]["status"]["state"]["receipts"]["model"]["digest"].as_str().ok_or_else(||invalid("prior Model receipt"))?.to_owned();
        let oracle=case.control(ControlOp::Oracle{previous_receipt:previous}).await?;expect(oracle["ok"]==true,&oracle.to_string())?;
        let previous=oracle["result"]["stage_receipt"].as_str().unwrap().to_owned();
        let done=case.control(ControlOp::Cleanup{previous_receipt:previous}).await?;expect(done["ok"]==true,&done.to_string())?;
        tokio::time::timeout(Duration::from_secs(10),case.agent.wait()).await??;
        let old_model=std::fs::read(case.install.join("state/native-state/fixture-model.json"))?;
        let journal_dir=case.install.join("state/journal");let mut old_events=BTreeMap::new();
        for item in std::fs::read_dir(&journal_dir)? {let p=item?.path();old_events.insert(p.clone(),sha(&std::fs::read(p)?));}
        // This test replaces only its private fixture Approval after real prior
        // Complete; it does not execute the Windows approve-next installer.
        let path=case.install.join("approved-task.json");let mut next:Approval=serde_json::from_slice(&std::fs::read(&path)?)?;
        next.task="flow-next-approval-second".into();next.request_prefix=format!("{}-",next.task);
        let now=now_ms();next.budget.issued_unix_ms=now;next.budget.expires_unix_ms=now+120000;
        std::fs::write(&path,serde_json::to_vec(&next)?)?;
        case.controller.task=next.task.clone();case.controller.approval_sha256=next.digest();
        case.restart_same_installation().await?;
        tokio::time::sleep(Duration::from_millis(600)).await;
        expect(case.agent.try_wait()?.is_none(),"old Complete record must not auto-exit a new approval agent")?;
        let prepared=case.control(ControlOp::Prepare).await?;expect(prepared["ok"]==true,&prepared.to_string())?;
        let status=case.control(ControlOp::Status).await?;
        expect(status["result"]["status"]["state"]["task"]==next.task && status["result"]["status"]["state"]["next"]=="bootstrap","new approved task reaches Prepare without resetting predecessor")?;
        for(path,hash) in old_events{expect(sha(&std::fs::read(path)?)==hash,"prior journal bytes must remain immutable")?;}
        expect(std::fs::read(case.install.join("state/native-state/fixture-model.json"))?==old_model,"new task Prepare must not reset old actual application model")?;
        transcript.push(json!({"prior_task_actual_eight_stages_complete":true,"new_private_fixture_approval_loaded":true,"new_task_prepare_complete":true,"previous_journal_and_application_model_unchanged":true,"windows_approve_next_executed":false}));
        Ok(())
    }.await;
    let cleanup = case.stop().await;
    case.evidence("completed-next-approval", &outcome, &transcript);
    outcome.unwrap();
    cleanup.unwrap();
}
