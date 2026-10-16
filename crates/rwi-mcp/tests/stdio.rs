#![cfg(feature = "legacy-local")]
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
        Self::start_with(&[])
    }
    fn start_with(args: &[&str]) -> Self {
        let state = tempfile::tempdir().unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_rwi-mcp"))
            .args(args)
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
    assert_eq!(
        init["result"]["serverInfo"]["name"],
        "real-world-interaction"
    );
    assert!(init["result"]["instructions"].as_str().unwrap().len() <= 512);
    c.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}));
    let tools = c.receive();
    let tools = tools["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 19);
    assert_eq!(
        init["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    let lifecycle_receipt = tools
        .iter()
        .find(|t| t["name"] == "computer.lifecycle_receipt")
        .unwrap();
    assert_eq!(lifecycle_receipt["annotations"]["readOnlyHint"], true);
    let preflight = tools
        .iter()
        .find(|t| t["name"] == "computer.lifecycle_preflight")
        .unwrap();
    assert_eq!(preflight["annotations"]["readOnlyHint"], true);
    assert_eq!(preflight["annotations"]["destructiveHint"], false);
    assert_eq!(preflight["inputSchema"]["additionalProperties"], false);
    let preflight_required = preflight["inputSchema"]["required"].as_array().unwrap();
    assert_eq!(preflight_required.len(), 2);
    assert!(preflight_required.contains(&json!("target")));
    assert!(preflight_required.contains(&json!("authorization_id")));
    let preflight_properties = preflight["inputSchema"]["properties"].as_object().unwrap();
    assert_eq!(preflight_properties.len(), 3);
    for field in ["target", "authorization_id", "operation_id"] {
        assert!(preflight_properties.contains_key(field));
    }
    for field in ["lease", "arguments", "executable", "allow_input"] {
        assert!(!preflight_properties.contains_key(field));
    }
    for name in ["computer.launch_application", "computer.activate_window"] {
        let tool = tools.iter().find(|t| t["name"] == name).unwrap();
        assert_eq!(tool["annotations"]["readOnlyHint"], false);
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
        assert!(
            tool["inputSchema"]["properties"]
                .get("executable")
                .is_none()
        );
        assert!(tool["inputSchema"]["properties"].get("arguments").is_none());
    }
    assert!(
        tools
            .iter()
            .all(|t| t["name"].as_str().unwrap().starts_with("computer."))
    );
    let act = tools.iter().find(|t| t["name"] == "computer.act").unwrap();
    assert_eq!(act["annotations"]["readOnlyHint"], false);
    assert_eq!(act["inputSchema"]["additionalProperties"], false);
    let checked = tools
        .iter()
        .find(|t| t["name"] == "computer.act_checked")
        .unwrap();
    assert_eq!(checked["annotations"]["readOnlyHint"], false);
    assert_eq!(checked["inputSchema"]["additionalProperties"], false);
    let required = checked["inputSchema"]["required"].as_array().unwrap();
    assert_eq!(required.len(), 6);
    for key in [
        "target",
        "frame",
        "element",
        "lease",
        "request_id",
        "intent",
    ] {
        assert!(required.contains(&json!(key)));
    }
    let properties = checked["inputSchema"]["properties"].as_object().unwrap();
    assert_eq!(properties.len(), 8);
    assert_eq!(properties["element"]["type"], "string");
    for key in ["locator", "reference", "name", "x", "y"] {
        assert!(!properties.contains_key(key));
    }
    assert!(
        init["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("Prefer act_checked")
    );
    let bind = tools
        .iter()
        .find(|t| t["name"] == "computer.bind_window")
        .unwrap();
    assert_eq!(bind["annotations"]["readOnlyHint"], true);
    for name in [
        "computer.act",
        "computer.act_checked",
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
fn query_registered_response(c: &mut Client, id: u32, response: &Value) -> Value {
    let data = &response["result"]["structuredContent"];
    let operation_id = data["operation_id"]
        .as_str()
        .expect("registered operation ID");
    assert!(!operation_id.is_empty());
    assert_eq!(data["operation"]["operation_id"], operation_id);
    let queried = c.call(
        id,
        "computer.operation",
        json!({"operation_id":operation_id}),
    );
    assert_ne!(queried["result"]["isError"], true);
    let snapshot = queried["result"]["structuredContent"].clone();
    assert_eq!(data["operation"], snapshot);
    assert_eq!(snapshot["terminal"], true);
    assert_eq!(snapshot["resources_released"], true);
    assert_eq!(snapshot["native_call_pending"], false);
    assert_eq!(snapshot["native_call_depth"], 0);
    let last = response["result"]["content"]
        .as_array()
        .unwrap()
        .last()
        .unwrap();
    let content_metadata: Value = serde_json::from_str(last["text"].as_str().unwrap()).unwrap();
    assert_eq!(content_metadata["operation_id"], operation_id);
    assert_eq!(content_metadata["operation"], snapshot);
    snapshot
}
#[test]
fn sdk_success_and_errors_return_queryable_explicit_and_automatic_operation_ids() {
    let mut c = Client::start();
    c.init();
    let explicit = c.call(
        2,
        "computer.observe",
        json!({"target":"unregistered","operation_id":"explicit-error"}),
    );
    assert_eq!(explicit["result"]["isError"], true);
    assert_eq!(
        explicit["result"]["structuredContent"]["error"],
        "unknown_target"
    );
    assert!(explicit["result"]["structuredContent"]["next_step"].is_string());
    let explicit_snapshot = query_registered_response(&mut c, 3, &explicit);
    assert_eq!(explicit_snapshot["operation_id"], "explicit-error");

    let automatic = c.call(4, "computer.observe", json!({"target":"unregistered"}));
    assert_eq!(automatic["result"]["isError"], true);
    assert_eq!(
        automatic["result"]["structuredContent"]["error"],
        "unknown_target"
    );
    let automatic_snapshot = query_registered_response(&mut c, 5, &automatic);
    assert_ne!(
        automatic_snapshot["operation_id"],
        explicit_snapshot["operation_id"]
    );

    let success = c.call(6, "computer.targets", json!({}));
    assert_ne!(success["result"]["isError"], true);
    assert_eq!(success["result"]["structuredContent"]["targets"], json!([]));
    let success_snapshot = query_registered_response(&mut c, 7, &success);
    assert_ne!(
        success_snapshot["operation_id"],
        automatic_snapshot["operation_id"]
    );

    // Registry rejection must not label this rejected request as the old operation.
    let duplicate = c.call(
        8,
        "computer.observe",
        json!({"target":"unregistered","operation_id":"explicit-error"}),
    );
    assert_eq!(
        duplicate["result"]["structuredContent"]["error"],
        "operation_id_reused"
    );
    assert!(
        duplicate["result"]["structuredContent"]
            .get("operation_id")
            .is_none()
    );
}
#[test]
fn sdk_lifecycle_preflight_is_readonly_and_returns_operation_on_engine_error() {
    let mut c = Client::start();
    c.init();
    let preflight = c.call(
        2,
        "computer.lifecycle_preflight",
        json!({"target":"unregistered","authorization_id":"launch"}),
    );
    assert_eq!(preflight["result"]["isError"], true);
    assert_eq!(
        preflight["result"]["structuredContent"]["error"],
        "unknown_target"
    );
    query_registered_response(&mut c, 3, &preflight);
    for (id, field) in ["lease", "arguments", "activate", "allow_input"]
        .into_iter()
        .enumerate()
    {
        let mut args = json!({"target":"unregistered","authorization_id":"launch"});
        args[field] = true.into();
        let rejected = c.call(4 + id as u32, "computer.lifecycle_preflight", args);
        assert!(rejected["error"].is_object() || rejected["result"]["isError"] == true);
    }
    let targets = c.call(8, "computer.targets", json!({}));
    assert_eq!(targets["result"]["structuredContent"]["targets"], json!([]));
}
#[test]
fn invalid_cli_cannot_implicitly_enable_input() {
    let state = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rwi-mcp"))
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
        let out = Command::new(env!("CARGO_BIN_EXE_rwi-mcp"))
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

#[test]
fn lifecycle_sdk_rejects_ungranted_command_overrides_and_has_no_implicit_target() {
    let mut c = Client::start();
    c.init();
    let denied=c.call(2,"computer.launch_application",json!({"target":"unknown","lease":"no","authorization_id":"launch","request_id":"one","arguments":["override"]}));
    assert!(denied["error"].is_object() || denied["result"]["isError"] == true);
    let unknown = c.call(
        3,
        "computer.launch_application",
        json!({"target":"unknown","lease":"no","authorization_id":"launch","request_id":"one"}),
    );
    assert_eq!(
        unknown["result"]["structuredContent"]["error"],
        "unknown_target"
    );
    let unknown=c.call(4,"computer.activate_window",json!({"target":"unknown","lease":"no","authorization_id":"activate","request_id":"one","expected_foreground":null}));
    assert_eq!(
        unknown["result"]["structuredContent"]["error"],
        "unknown_target"
    );
}
#[test]
fn lifecycle_cli_does_not_enable_windows_or_input() {
    let state = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_rwi-mcp"))
        .arg("--state-dir")
        .arg(state.path())
        .args(["--windows-lifecycle-grant", "/never-read-unallocated.json"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("Lifecycle grants require --windows-desktop")
    );
}

#[test]
fn sdk_checked_errors_keep_queryable_operation_and_reject_locator_shapes() {
    let mut c = Client::start();
    c.init();
    let args = json!({"target":"unregistered","frame":"old-frame","element":"exact-id","lease":"missing","request_id":"checked-error","intent":"click"});
    let error = c.call(2, "computer.act_checked", args.clone());
    assert_eq!(error["result"]["isError"], true);
    assert_eq!(
        error["result"]["structuredContent"]["error"],
        "unknown_target"
    );
    assert!(error["result"]["structuredContent"]["next_step"].is_string());
    query_registered_response(&mut c, 3, &error);
    for (index, field) in ["locator", "reference", "name", "x", "y"]
        .iter()
        .enumerate()
    {
        let mut invalid = args.clone();
        invalid[*field] = json!({"kind":"pixel","x":1,"y":2});
        let rejected = c.call(4 + index as u32, "computer.act_checked", invalid);
        assert!(rejected["error"].is_object() || rejected["result"]["isError"] == true);
        assert!(
            rejected["result"]["structuredContent"]
                .get("operation_id")
                .is_none()
        );
    }
    let mut invalid = args;
    invalid["element"] = json!({"kind":"semantic","name":"Input"});
    let rejected = c.call(10, "computer.act_checked", invalid);
    assert!(rejected["error"].is_object() || rejected["result"]["isError"] == true);
    let targets = c.call(11, "computer.targets", json!({}));
    assert_eq!(targets["result"]["structuredContent"]["targets"], json!([]));
}

#[cfg(not(feature = "synthetic-fixture"))]
#[test]
fn production_binary_cannot_enable_synthetic_fixture() {
    let state = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rwi-mcp"))
        .args(["--synthetic-only", "--state-dir"])
        .arg(state.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid_cli"));
}

#[cfg(feature = "synthetic-fixture")]
#[test]
fn synthetic_build_rejects_every_native_route_and_nonexclusive_fixture_cli() {
    for args in [
        vec!["--windows-desktop"],
        vec!["--macos-window", "1:1"],
        vec![
            "--windows-desktop",
            "--windows-lifecycle-grant",
            "/must-not-open.json",
        ],
        vec![
            "--windows-desktop",
            "--allow-input",
            "--windows-input-grant",
            "/must-not-open.json",
        ],
    ] {
        let state = tempfile::tempdir().unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_rwi-mcp"))
            .arg("--state-dir")
            .arg(state.path())
            .args(args)
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("synthetic_native_forbidden"));
        assert!(!state.path().join("receipts").exists());
    }
    for extras in [
        vec!["--windows-desktop"],
        vec!["--allow-input"],
        vec!["--pipe-server", "no-native"],
        vec!["--synthetic-only"],
    ] {
        let state = tempfile::tempdir().unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_rwi-mcp"))
            .args(["--synthetic-only", "--state-dir"])
            .arg(state.path())
            .args(extras)
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("synthetic_only_cli"));
        assert!(!state.path().join("receipts").exists());
    }
}

#[cfg(feature = "synthetic-fixture")]
#[test]
fn actual_sdk_checked_fixture_survives_model_delay_without_relaxing_legacy_act() {
    let mut c = Client::start_with(&["--synthetic-only"]);
    c.init();
    let targets = c.call(2, "computer.targets", json!({}));
    let target = &targets["result"]["structuredContent"];
    assert_eq!(target["implementation"], "synthetic-only-test-build");
    assert_eq!(target["targets"].as_array().unwrap().len(), 1);
    assert_eq!(target["targets"][0]["target"], "synthetic-only");
    assert_eq!(target["targets"][0]["capabilities"]["native"], false);
    let observed = c.call(
        3,
        "computer.observe",
        json!({"target":"synthetic-only","inline_image":false}),
    );
    let before = &observed["result"]["structuredContent"];
    assert_eq!(before["scene"]["assurance"], "synthetic-fixture");
    let element = before["scene"]["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "Toggle test option")
        .unwrap()["id"]
        .clone();
    let frame = before["frame"].clone();
    let resolved = c.call(
        4,
        "computer.resolve",
        json!({"target":"synthetic-only","frame":frame,"locator":{"kind":"element","id":element}}),
    );
    let reference = resolved["result"]["structuredContent"]["reference"].clone();
    let claim = c.call(
        5,
        "computer.claim",
        json!({"target":"synthetic-only","ttl_ms":60000}),
    );
    let lease = claim["result"]["structuredContent"]["lease"].clone();
    // Deliberately age the SAME server frame past the unchanged legacy 10s TTL.
    // Synthetic memory only: this test cannot register a native adapter.
    std::thread::sleep(Duration::from_millis(10_100));
    let legacy = c.call(6, "computer.act", json!({"target":"synthetic-only","reference":reference,"lease":lease,"request_id":"legacy-old-frame","intent":"click"}));
    assert_eq!(
        legacy["result"]["structuredContent"]["error"],
        "stale_reference"
    );
    let args = json!({"target":"synthetic-only","frame":frame,"element":element,"lease":lease,"request_id":"checked-once","intent":"click"});
    let checked = c.call(7, "computer.act_checked", args.clone());
    assert_ne!(checked["result"]["isError"], true, "{checked}");
    let receipt = &checked["result"]["structuredContent"];
    assert_eq!(receipt["dispatch"], "returned");
    assert_eq!(receipt["goal_verified"], false);
    assert_eq!(receipt["scope"]["element"], element);
    query_registered_response(&mut c, 8, &checked);
    let duplicate = c.call(9, "computer.act_checked", args);
    assert_ne!(duplicate["result"]["isError"], true, "{duplicate}");
    assert_eq!(
        duplicate["result"]["structuredContent"]["deduplicated"],
        true
    );
    let after = c.call(
        10,
        "computer.observe",
        json!({"target":"synthetic-only","inline_image":false}),
    );
    let after = &after["result"]["structuredContent"];
    let input = after["scene"]["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "Input")
        .unwrap()["id"]
        .clone();
    let verified = c.call(11, "computer.verify", json!({"target":"synthetic-only","frame":after["frame"],"element":input,"predicate":"element_value","expected":"test-option:on","timeout_ms":100,"stable_samples":1}));
    assert_eq!(
        verified["result"]["structuredContent"]["goal_verified"], true,
        "{verified}"
    );
    let released = c.call(12, "computer.release", json!({"lease":lease}));
    assert_eq!(released["result"]["structuredContent"]["released"], true);
}
