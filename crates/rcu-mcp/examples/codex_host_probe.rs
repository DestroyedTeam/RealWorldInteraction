//! Opt-in operator-driven live Codex Host test. No fixture, shell input backend,
//! model inference, global install, permission grant or automatic input replay.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rcu_native_link::wire as frame;
use serde_json::{Value, json};
use std::{io, path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

fn invalid(message: &str) -> io::Error {
    io::Error::other(message.to_owned())
}
struct Directory {
    reader: rcu_link_trust::PrivateDirectory,
    path: PathBuf,
}
impl Directory {
    fn create(path: &std::path::Path) -> io::Result<Self> {
        Ok(Self {
            reader: rcu_link_trust::PrivateDirectory::create(path)?,
            path: path.into(),
        })
    }
    fn child(&self, name: &str) -> io::Result<PathBuf> {
        if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
            return Err(invalid("invalid evidence basename"));
        }
        Ok(self.path.join(name))
    }
    fn create_new(&self, name: &str, raw: &[u8], cap: usize) -> io::Result<()> {
        self.child(name)?;
        if raw.len() > cap {
            return Err(invalid("evidence byte limit"));
        }
        self.reader.write_new(name, raw)
    }
}

struct Host {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    evidence: Directory,
    sequence: u64,
}
impl Host {
    async fn request(&mut self, method: &str, params: Value) -> io::Result<Value> {
        self.sequence += 1;
        let id = self.sequence;
        let request = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
        self.evidence.create_new(
            &format!("request-{id:04}.json"),
            &serde_json::to_vec(&request)?,
            frame::INBOUND,
        )?;
        let mut raw = serde_json::to_vec(&request)?;
        raw.push(b'\n');
        let input = self
            .input
            .as_mut()
            .ok_or_else(|| invalid("host input closed"))?;
        input.write_all(&raw).await?;
        input.flush().await?;
        let mut notification = 0;
        loop {
            let raw = tokio::time::timeout(
                Duration::from_secs(90),
                frame::read_frame(&mut self.output, frame::OUTBOUND),
            )
            .await??
            .ok_or_else(|| invalid("actual Codex host EOF"))?;
            let value: Value = serde_json::from_slice(&raw)?;
            if value["id"] == id {
                self.evidence.create_new(
                    &format!("response-{id:04}.json"),
                    &raw,
                    frame::OUTBOUND,
                )?;
                if value.get("error").is_some() {
                    return Err(invalid(&value.to_string()));
                }
                return Ok(value["result"].clone());
            }
            if value.get("id").is_some() {
                return Err(invalid(
                    "unexpected host request; explicit operator handling required",
                ));
            }
            notification += 1;
            if notification > 256 {
                return Err(invalid("host notification budget"));
            }
            self.evidence.create_new(
                &format!("notification-{id:04}-{notification:03}.json"),
                &raw,
                frame::OUTBOUND,
            )?;
        }
    }

    fn display(&self, result: &Value) -> io::Result<()> {
        let mut images = Vec::new();
        for (index, block) in result["content"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            if block["type"] == "image" {
                let bytes = STANDARD
                    .decode(
                        block["data"]
                            .as_str()
                            .ok_or_else(|| invalid("missing image data"))?,
                    )
                    .map_err(io::Error::other)?;
                let name = format!("response-{:04}-image-{index}.png", self.sequence);
                self.evidence.create_new(&name, &bytes, frame::OUTBOUND)?;
                images.push(self.evidence.child(&name)?);
            }
        }
        println!(
            "{}",
            json!({"response_id":self.sequence,"isError":result["isError"],"structuredContent":result["structuredContent"],"images":images,"full_response":self.evidence.child(&format!("response-{:04}.json",self.sequence))?})
        );
        Ok(())
    }
}

async fn run() -> io::Result<()> {
    let mut args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let project_probe = args.last().is_some_and(|a| a == "--project-config");
    let project_tools = args.last().is_some_and(|a| a == "--project-tools");
    if project_probe || project_tools {
        args.pop();
    }
    if args.len() != 3 || args.iter().any(|p| !p.is_absolute()) {
        return Err(invalid(
            "expected ABS_CODEX_BINARY ABS_ISOLATED_HOME ABS_NEW_EVIDENCE_DIR",
        ));
    }
    let evidence = Directory::create(&args[2])?;
    let home = if project_probe || project_tools {
        PathBuf::from(std::env::var_os("HOME").ok_or_else(|| invalid("HOME missing"))?)
    } else {
        args[1].clone()
    };
    let mut child = Command::new(&args[0])
        .args(["app-server", "--listen", "stdio://"])
        .env("HOME", &home)
        .env("CODEX_HOME", home.join(".codex"))
        .current_dir(&args[1])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let mut host = Host {
        input: child.stdin.take(),
        output: BufReader::new(child.stdout.take().unwrap()),
        child,
        evidence,
        sequence: 0,
    };
    host.request("initialize",json!({"clientInfo":{"name":"rcu_live_validation","version":"0.1.0"},"capabilities":{"experimentalApi":true}})).await?;
    let input = host.input.as_mut().unwrap();
    input
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"initialized\"}\n")
        .await?;
    input.flush().await?;
    let skills = host
        .request("skills/list", json!({"cwds":[args[1]],"forceReload":true}))
        .await?;
    if project_probe {
        // Real user's config/Skill discovery only. No thread/start, MCP client
        // startup, model inference or exclusive native Model attachment occurs.
        let config = host
            .request("config/read", json!({"cwd":args[1],"includeLayers":true}))
            .await?;
        let server = &config["config"]["mcp_servers"]["real_computer_use_native_link"];
        println!(
            "{}",
            json!({"real_user_project_probe":true,"native_server_config":server,
            "skill_discovered":skills.to_string().contains("real-computer-use-native"),
            "config_response":host.evidence.child(&format!("response-{:04}.json",host.sequence))?,
            "native_model_connection_started":false})
        );
        drop(host.input.take());
        if tokio::time::timeout(Duration::from_secs(5), host.child.wait())
            .await
            .is_err()
        {
            host.child.kill().await?;
        }
        return Ok(());
    }
    if !skills.to_string().contains("real-computer-use-native") {
        return Err(invalid("real native Skill not discovered"));
    }
    let mut thread_params =
        json!({"cwd":args[1],"ephemeral":true,"approvalPolicy":"never","sandbox":"read-only"});
    if project_tools {
        let config = host
            .request("config/read", json!({"cwd":args[1],"includeLayers":false}))
            .await?;
        let mut servers = config["config"]["mcp_servers"]
            .as_object()
            .ok_or_else(|| invalid("effective MCP config missing"))?
            .clone();
        if servers
            .get("real_computer_use_native_link")
            .is_none_or(|s| s["enabled"] != true)
        {
            return Err(invalid("effective native entry not enabled"));
        }
        for (name, server) in &mut servers {
            // config/read exposes absent optional values as JSON null; passing
            // them as TOML overrides turns them into invalid empty strings.
            if let Some(fields) = server.as_object_mut() {
                fields.retain(|_, value| !value.is_null());
            }
            if name != "real_computer_use_native_link" {
                server["enabled"] = json!(false);
            }
        }
        // Per-test thread overrides only: never launch unrelated/legacy MCPs or
        // change their saved global settings while inspecting the native entry.
        thread_params["config"] = json!({"mcp_servers":servers});
        if let Some(plugins) = config["config"]["plugins"].as_object() {
            let disabled = plugins
                .keys()
                .map(|name| (name.clone(), json!({"enabled":false})))
                .collect::<serde_json::Map<_, _>>();
            thread_params["config"]["plugins"] = json!(disabled);
        }
    }
    let thread = host.request("thread/start", thread_params).await?;
    let thread = thread["thread"]["id"]
        .as_str()
        .ok_or_else(|| invalid("missing ephemeral host thread"))?
        .to_owned();
    println!(
        "{}",
        json!({"ready":true,"skill_discovered":true,"actual_codex_host":true,"model_inference":false,"thread":thread})
    );
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = lines.next_line().await? {
        let command: Value = serde_json::from_str(&line)?;
        if command["finish"] == true {
            break;
        }
        if command["inventory"] == true {
            let result = host
                .request(
                    "mcpServerStatus/list",
                    json!({"threadId":thread,"detail":"toolsAndAuthOnly","limit":100}),
                )
                .await?;
            host.display(&result)?;
            continue;
        }
        let tool = command["tool"]
            .as_str()
            .ok_or_else(|| invalid("computer tool required"))?;
        if !tool.starts_with("computer.") {
            return Err(invalid("only RCU computer tools in this test"));
        }
        let result = host.request("mcpServer/tool/call",json!({"threadId":thread,"server":"real_computer_use_native_link","tool":tool,"arguments":command["arguments"]})).await?;
        host.display(&result)?;
    }
    drop(host.input.take());
    let exit = match tokio::time::timeout(Duration::from_secs(10), host.child.wait()).await {
        Ok(status) => json!({"host_exit":status?.code(),"host_forced":false}),
        Err(_) => {
            host.child.kill().await?;
            json!({"host_forced":true,"agent_cleanup_requires_independent_verification":true})
        }
    };
    println!("{exit}");
    Ok(())
}

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let result = runtime.block_on(run());
    runtime.shutdown_timeout(Duration::from_millis(100));
    if let Err(error) = result {
        eprintln!("live Codex validation: {error}");
        std::process::exit(1);
    }
}
