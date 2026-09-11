//! Real compiled server process, official SDK framing. No native targets or desktop calls.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
struct Client {
    child: Child,
    input: Option<ChildStdin>,
    output: mpsc::Receiver<Value>,
    _state: tempfile::TempDir,
}
impl Client {
    fn start() -> Self {
        let state = tempfile::tempdir().unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_rcu-mcp"))
            .arg("--state-dir")
            .arg(state.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let Ok(line) = line else { break };
                if let Ok(value) = serde_json::from_str(&line) {
                    let _ = tx.send(value);
                }
            }
        });
        Self {
            child,
            input: Some(input),
            output: rx,
            _state: state,
        }
    }
    fn send(&mut self, v: Value) {
        let input = self.input.as_mut().unwrap();
        writeln!(input, "{v}").unwrap();
        input.flush().unwrap();
    }
    fn receive(&self) -> Value {
        self.output
            .recv_timeout(Duration::from_secs(10))
            .expect("bounded MCP response")
    }
    fn init(&mut self) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"offline-rust-test","version":"1"}}}));
        let response = self.receive();
        self.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        response
    }
    fn call(&mut self, id: u32, name: &str, args: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}}));
        self.receive()
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.input.take();
        for _ in 0..50 {
            if self.child.try_wait().unwrap().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[test]
fn sdk_initialize_discovery_zero_targets_unknown_target_and_disconnect() {
    let mut c = Client::start();
    let init = c.init();
    assert_eq!(init["result"]["serverInfo"]["name"], "real-computer-use");
    assert!(init["result"]["instructions"].as_str().unwrap().len() <= 512);
    c.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}));
    let tools = c.receive();
    let tools = tools["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 14);
    assert!(
        tools
            .iter()
            .all(|t| t["name"].as_str().unwrap().starts_with("computer."))
    );
    let act = tools.iter().find(|t| t["name"] == "computer.act").unwrap();
    assert_eq!(act["annotations"]["readOnlyHint"], false);
    assert_eq!(act["inputSchema"]["additionalProperties"], false);
    let bind = tools
        .iter()
        .find(|t| t["name"] == "computer.bind_window")
        .unwrap();
    assert_eq!(bind["annotations"]["readOnlyHint"], true);
    for name in [
        "computer.act",
        "computer.claim",
        "computer.release",
        "computer.cancel",
    ] {
        assert_eq!(
            tools.iter().find(|t| t["name"] == name).unwrap()["annotations"]["readOnlyHint"],
            false
        );
    }
    let targets = c.call(3, "computer.targets", json!({}));
    assert_eq!(targets["result"]["structuredContent"]["targets"], json!([]));
    let unknown = c.call(4, "computer.observe", json!({"target":"unregistered"}));
    assert_eq!(unknown["result"]["isError"], true);
    assert_eq!(
        unknown["result"]["structuredContent"]["error"],
        "unknown_target"
    );
    let discover = c.call(
        5,
        "computer.discover",
        json!({"target":"unregistered","operation_id":"empty-discovery"}),
    );
    assert_eq!(
        discover["result"]["structuredContent"]["error"],
        "unknown_target"
    );
    let status = c.call(
        6,
        "computer.operation",
        json!({"operation_id":"empty-discovery"}),
    );
    assert_eq!(
        status["result"]["structuredContent"]["resources_released"],
        true
    );
    assert_eq!(
        status["result"]["structuredContent"]["strong_native_preemption"],
        false
    );
    c.input.take();
    for _ in 0..100 {
        if let Some(status) = c.child.try_wait().unwrap() {
            assert!(status.success());
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("server did not exit after transport EOF");
}
#[test]
fn actual_sdk_rejects_misspelled_action_parameters() {
    let mut c = Client::start();
    c.init();
    let result=c.call(2,"computer.act",json!({"target":"a","reference":"b","lease":"c","request_id":"d","intent":"click","args":[]}));
    assert!(result["error"].is_object() || result["result"]["isError"] == true);
}
#[test]
fn invalid_cli_cannot_implicitly_enable_input() {
    let state = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rcu-mcp"))
        .arg("--state-dir")
        .arg(state.path())
        .arg("--allow-input")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("requires an explicitly pinned native target")
    );
}
#[test]
fn windows_input_rejects_missing_or_unpaired_grant_before_native_access() {
    for args in [
        vec!["--windows-desktop", "--allow-input"],
        vec!["--windows-input-grant", "/not-read-without-enable.json"],
    ] {
        let state = tempfile::tempdir().unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_rcu-mcp"))
            .arg("--state-dir")
            .arg(state.path())
            .args(args)
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(
            !state
                .path()
                .join("receipts")
                .read_dir()
                .map(|mut r| r.next().is_some())
                .unwrap_or(false)
        );
        let errors = String::from_utf8_lossy(&out.stderr);
        assert!(
            errors.contains("input_grant_required")
                || errors.contains("requires --windows-desktop and --allow-input")
        );
    }
}
