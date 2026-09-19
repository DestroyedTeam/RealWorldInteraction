//! Opt-in real Codex host integration. No model run, native target or user config.
use super::*;

async fn request(rpc: &mut Rpc, method: &str, params: Value) -> io::Result<Value> {
    rpc.next += 1;
    let id = rpc.next;
    rpc.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
        .await?;
    loop {
        let value = rpc.receive().await?;
        if value["id"] == id {
            expect(value.get("error").is_none(), &value.to_string())?;
            return Ok(value["result"].clone());
        }
        expect(
            value.get("id").is_none(),
            "unexpected host request requires explicit handling",
        )?;
    }
}

#[tokio::test]
#[ignore = "explicit local CODEX executable and prebuilt fixture worker required; not an offline default test"]
async fn actual_codex_plugin_skill_and_mcp_call_over_rust_tls() {
    let codex = PathBuf::from(std::env::var_os("RCU_CODEX_BINARY").expect("explicit Codex binary"));
    assert!(codex.is_absolute());
    let mut case = Case::new("codex-host", json!({})).await;
    let mut transcript = Vec::new();
    let outcome: io::Result<()> = async {
        let root = case.entry.parent().unwrap().to_owned();
        let client_path = root.join("client.json");
        let mut config: ClientConfig = serde_json::from_slice(&std::fs::read(&client_path)?)?;
        config.model_output = rwi_native_link::presentation::OutputMode::CompactV1;
        std::fs::write(client_path, serde_json::to_vec(&config)?)?;
        let market = Directory::create(&root.join("test-market"))?;
        let plugins = Directory::create(&market.path().join("plugins"))?;
        let output = plugins.path().join(rwi_native_link::plugin::NAME);
        let export = rwi_native_link::plugin::export(rwi_native_link::plugin::Export { entry:case.entry.clone(),output:output.clone() })?;
        transcript.push(json!({"plugin_export":export}));
        // Verify create-new protection; no second export may overwrite the first.
        expect(rwi_native_link::plugin::export(rwi_native_link::plugin::Export {entry:case.entry.clone(),output:output.clone()}).is_err(), "existing plugin must not be overwritten")?;
        let agents = Directory::create(&market.path().join(".agents"))?;
        let manifests = Directory::create(&agents.path().join("plugins"))?;
        manifests.create_new("marketplace.json", &serde_json::to_vec(&json!({"name":"rcu-isolated-validation","interface":{"displayName":"RWI isolated validation"},"plugins":[{"name":rwi_native_link::plugin::NAME,"source":{"source":"local","path":"./plugins/real-world-interaction"},"policy":{"installation":"AVAILABLE","authentication":"ON_USE"},"category":"Productivity"}]}))?,65536)?;
        let home = Directory::create(&root.join("isolated-home"))?;
        let codex_home = Directory::create(&home.path().join(".codex"))?;
        for args in [vec!["plugin","marketplace","add",market.path().to_str().unwrap()],vec!["plugin","add","real-world-interaction@rcu-isolated-validation"]] {
            let result = tokio::time::timeout(Duration::from_secs(30),Command::new(&codex).args(args).env("HOME",home.path()).env("CODEX_HOME",codex_home.path()).current_dir(home.path()).kill_on_drop(true).output()).await??;
            transcript.push(json!({"isolated_cli":{"exit":result.status.code(),"stdout":String::from_utf8_lossy(&result.stdout),"stderr":String::from_utf8_lossy(&result.stderr)}}));
            expect(result.status.success(), "isolated native plugin CLI installation failed")?;
        }
        case.stages().await?;
        let mut child = Command::new(&codex).args(["app-server","--listen","stdio://"])
            .env("HOME",home.path()).env("CODEX_HOME",codex_home.path()).current_dir(home.path())
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true).spawn()?;
        let mut rpc = Rpc {input:child.stdin.take(),output:BufReader::new(child.stdout.take().unwrap()),child,next:10,transcript:vec![]};
        let body: io::Result<()> = async {
            request(&mut rpc,"initialize",json!({"clientInfo":{"name":"rcu_native_host_test","version":"0.1.0"},"capabilities":{"experimentalApi":true}})).await?;
            rpc.send(json!({"jsonrpc":"2.0","method":"initialized"})).await?;
            let skills = request(&mut rpc,"skills/list",json!({"cwds":[home.path()],"forceReload":true})).await?;
            expect(skills.to_string().contains("real-world-interaction"),"installed skill not discovered by Codex")?;
            let thread = request(&mut rpc,"thread/start",json!({"cwd":home.path(),"ephemeral":true,"approvalPolicy":"never","sandbox":"read-only"})).await?;
            let thread_id = thread["thread"]["id"].as_str().ok_or_else(|| invalid("ephemeral test thread id missing"))?;
            let result = request(&mut rpc,"mcpServer/tool/call",json!({"threadId":thread_id,"server":"real_world_interaction","tool":"computer.targets","arguments":{}})).await?;
            expect(result.to_string().contains("synthetic-only"),"real Codex tool call did not reach the explicit Rust fixture worker")?;
            expect(result.to_string().contains("rcu.model-output.compact.v1"),"native relay compact projection not active")?;
            let observation = request(&mut rpc,"mcpServer/tool/call",json!({"threadId":thread_id,"server":"real_world_interaction","tool":"computer.observe","arguments":{"target":"synthetic-only","inline_image":false}})).await?;
            expect(observation["isError"] != true,"Codex observe returned an MCP error")?;
            expect(observation["structuredContent"]["frame"].is_string(),"Codex observe did not expose the actual frame")?;
            expect(observation["structuredContent"]["observation_view"].is_object(),"compact native observation missing")?;
            transcript.push(json!({"actual_codex_observe":observation,"fixture_only":true}));
            transcript.push(json!({"native_codex_host":true,"plugin_install":true,"skill_discovered":true,"actual_mcp_tool_call":true,"actual_tls":true,"model_inference":false,"windows_native":false,"tool_result":result}));
            Ok(())
        }.await;
        transcript.extend(rpc.transcript.clone());
        if let Some(evidence) = std::env::var_os("RCU_NATIVE_FLOW_EVIDENCE") {
            let evidence = PathBuf::from(evidence);
            std::fs::create_dir_all(evidence.join("model-output"))?;
            for saved in std::fs::read_dir(root.join("model-output-evidence"))? {
                let saved = saved?;
                std::fs::copy(saved.path(), evidence.join("model-output").join(saved.file_name()))?;
            }
            for relative in [".codex-plugin/plugin.json", ".mcp.json", "skills/real-world-interaction/SKILL.md", "README.md"] {
                let destination = evidence.join("exported-plugin").join(relative);
                std::fs::create_dir_all(destination.parent().unwrap())?;
                std::fs::copy(output.join(relative), destination)?;
            }
        }
        drop(rpc.input.take());
        if tokio::time::timeout(Duration::from_secs(8),rpc.child.wait()).await.is_err() { rpc.child.kill().await?; }
        body
    }.await;
    case.evidence("codex-native-host", &outcome, &transcript);
    let cleanup = case.stop().await;
    outcome.unwrap();
    cleanup.unwrap();
}
