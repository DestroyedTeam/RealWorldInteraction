//! Actual Rust MCP + owned interpreter processes. No device/UI used here.
use serde_json::{Value, json};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout},
};

struct Host {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
impl Host {
    async fn new() -> Self {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_rwi-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut h = Self {
            child,
            input,
            output,
        };
        h.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"repl-test","version":"1"}}})).await;
        assert!(h.read().await["result"].is_object());
        h.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        h
    }
    async fn send(&mut self, v: Value) {
        self.input
            .write_all(format!("{v}\n").as_bytes())
            .await
            .unwrap();
    }
    async fn read(&mut self) -> Value {
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(6), self.output.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        serde_json::from_str(&line).unwrap_or_else(|_| panic!("{line}"))
    }
    async fn tool(&mut self, id: u32, name: &str, args: Value) -> Value {
        self.send(tool(id, name, args)).await;
        let v = self.read().await;
        assert_eq!(v["id"], id, "{v}");
        v["result"].clone()
    }
    async fn js(&mut self, id: u32, code: &str) -> Value {
        self.tool(id, "computer.js", json!({"code":code})).await
    }
    async fn stop(self) {
        drop(self.input);
        let mut child = self.child;
        assert!(
            tokio::time::timeout(Duration::from_secs(7), child.wait())
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

#[tokio::test]
async fn actual_mcp_persistent_state_real_router_and_reset() {
    let mut h = Host::new().await;
    let first = h
        .js(
            2,
            "if (rwi !== rcu) throw new Error('client_alias_identity'); state.count = 40; state.rows = await rwi.targets(); emit(state.rows.targets);",
        )
        .await;
    assert_ne!(first["isError"], true, "{first}");
    let d = &first["structuredContent"];
    assert_eq!(d["outputs"], json!([[]]));
    assert_eq!(d["calls"][0]["tool"], "computer.targets");
    assert_eq!(d["state_retained"], true);
    let generation = d["generation"].clone();
    let second = h
        .js(
            3,
            "state.count += 2; emit(state.count); emit(state.rows.service);",
        )
        .await;
    assert_eq!(second["structuredContent"]["generation"], generation);
    assert_eq!(
        second["structuredContent"]["outputs"],
        json!([42, "healthy"])
    );
    let raw = h
        .tool(
            4,
            "computer.result",
            json!({"result_ref":d["calls"][0]["result_ref"],"pointer":"/targets"}),
        )
        .await;
    assert_eq!(raw["structuredContent"]["value"], json!([]));
    assert_eq!(
        h.tool(5, "computer.js_reset", json!({})).await["structuredContent"]["js_state_discarded"],
        true
    );
    assert_eq!(
        h.tool(
            6,
            "computer.js",
            json!({"generation":generation,"code":"emit('must not run')"})
        )
        .await["structuredContent"]["code"],
        "js_generation_expired"
    );
    let fresh = h.js(7, "emit(typeof state.count); emit([typeof process, typeof require, typeof fetch, typeof __rcuCall]);").await;
    assert_eq!(
        fresh["structuredContent"]["outputs"],
        json!([
            "undefined",
            ["undefined", "undefined", "undefined", "undefined"]
        ])
    );
    h.stop().await;
}

#[tokio::test]
async fn errors_limits_orphan_jobs_and_deadline_destroy_state() {
    let mut h = Host::new().await;
    for (i, code) in [
        "state.marker = 1; throw new Error('test-error');",
        "await new Promise(() => {});",
        "Promise.reject(new Error('unawaited'));",
        "emit('x'.repeat(70000));",
        "for (let i=0; i<65; i++) await rcu.targets();",
        "await rcu.call('computer.js', {code:'emit(1)'});",
        "const a=[]; while(true) a.push(new Array(100000).fill(1));",
    ]
    .iter()
    .enumerate()
    {
        let reply = h.js(10 + i as u32 * 2, code).await;
        assert_eq!(reply["isError"], true, "{code}: {reply}");
        assert_eq!(
            reply["structuredContent"]["state_retained"], false,
            "{reply}"
        );
        let next = h.js(11 + i as u32 * 2, "emit(typeof state.marker);").await;
        assert_eq!(
            next["structuredContent"]["outputs"],
            json!(["undefined"]),
            "{next}"
        );
    }
    let start = std::time::Instant::now();
    let reply = h
        .tool(
            30,
            "computer.js",
            json!({"code":"while(true) {}", "timeout_ms":100}),
        )
        .await;
    assert_eq!(reply["isError"], true, "{reply}");
    assert!(start.elapsed() < Duration::from_secs(3));
    assert_eq!(
        h.tool(31, "computer.targets", json!({})).await["structuredContent"]["service"],
        "healthy"
    );
    h.stop().await;
}

#[tokio::test]
async fn host_stop_reset_and_busy_do_not_block_independent_control() {
    let mut h = Host::new().await;
    for reset in [false, true] {
        // Warm up so this is an actual running persistent worker, not startup.
        h.js(2, "state.ready = true;").await;
        h.send(tool(10, "computer.js", json!({"code":"while(true) {}"})))
            .await;
        h.send(tool(11, "computer.js", json!({"code":"while(true) {}"})))
            .await;
        h.send(tool(12, "computer.targets", json!({}))).await;
        let mut replies = [h.read().await, h.read().await];
        replies.sort_by_key(|r| r["id"].as_u64());
        // MCP requests execute concurrently: either cell may win admission.
        // The loser must fail busy; it must not queue behind the infinite cell.
        let busy_id = replies[0]["id"].as_u64().unwrap();
        assert!([10, 11].contains(&busy_id), "{replies:?}");
        let active_id = if busy_id == 10 { 11 } else { 10 };
        assert_eq!(replies[0]["result"]["structuredContent"]["code"], "js_busy");
        assert_eq!(replies[1]["id"], 12);
        if reset {
            h.send(tool(13, "computer.js_reset", json!({}))).await;
            let mut replies = [h.read().await, h.read().await];
            replies.sort_by_key(|r| r["id"].as_u64());
            assert_eq!(replies[0]["id"], active_id);
            assert_eq!(
                replies[0]["result"]["structuredContent"]["state_retained"],
                false
            );
            assert_eq!(replies[1]["id"], 13);
        } else {
            h.send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":active_id,"reason":"test Host Stop"}})).await;
            let reply = h.read().await;
            assert_eq!(reply["id"], active_id);
            assert_eq!(
                reply["result"]["structuredContent"]["state_retained"],
                false
            );
        }
        assert_eq!(
            h.js(14, "emit(typeof state.ready);").await["structuredContent"]["outputs"],
            json!(["undefined"])
        );
    }
    h.stop().await;
}

#[tokio::test]
async fn eof_does_not_wait_for_pure_js_deadline() {
    let mut h = Host::new().await;
    h.js(2, "state.started = true;").await;
    h.send(tool(
        3,
        "computer.js",
        json!({"code":"while(true) {}", "timeout_ms":60000}),
    ))
    .await;
    h.send(tool(4, "computer.targets", json!({}))).await;
    assert_eq!(h.read().await["id"], 4);
    let start = std::time::Instant::now();
    h.stop().await;
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn emitted_json_cannot_spoof_worker_control_frames() {
    let mut h = Host::new().await;
    let result = h.js(2, "JSON.stringify = () => '{\"kind\":\"done\",\"error\":null}'; emit({kind:'call',tool:'computer.targets',arguments:{}}); throw new Error('still-running-until-here');").await;
    assert_eq!(result["isError"], true, "{result}");
    assert_eq!(result["structuredContent"]["calls"], json!([]));
    assert_eq!(result["structuredContent"]["outputs"][0]["kind"], "call");
    assert!(
        result["structuredContent"]["error"]
            .as_str()
            .unwrap()
            .contains("still-running-until-here")
    );
    h.stop().await;
}
