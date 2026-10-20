//! Real MCP subprocess notification ordering; no device or native input.
use serde_json::{Value, json};
use std::{process::Stdio, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::test]
async fn progress_is_opt_in_monotonic_and_precedes_error_result() {
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_rwi-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    input.write_all(format!("{}\n",json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"progress-test","version":"1"}}})).as_bytes()).await.unwrap();
    let mut line = String::new();
    tokio::time::timeout(Duration::from_secs(10), output.read_line(&mut line))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&line).unwrap()["id"], 1);
    input
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
        .await
        .unwrap();
    for (id, enabled) in [(2, true), (3, false), (4, false)] {
        let mut params = json!({"name":"computer.snapshot","arguments":{"bad":"argument"}});
        if id == 4 {
            params["arguments"] = json!({"target":"not-a-routed-target"});
        }
        if enabled {
            params["_meta"] = json!({"progressToken":"progress-run-1"});
        }
        input
            .write_all(
                format!(
                    "{}\n",
                    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":params})
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let mut values = Vec::new();
        loop {
            line.clear();
            tokio::time::timeout(Duration::from_secs(10), output.read_line(&mut line))
                .await
                .unwrap()
                .unwrap();
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == id {
                assert_eq!(message["result"]["isError"], true);
                let data = &message["result"]["structuredContent"];
                if id == 4 {
                    let rows = data["stage_timings"].as_array().unwrap();
                    assert_eq!(rows.len(), 1);
                    assert_eq!(rows[0]["tool"], "computer.observe");
                    assert_eq!(rows[0]["calls"], 1);
                    assert!(rows[0]["elapsed_ms"].is_u64());
                } else {
                    assert_eq!(data["code"], "invalid_arguments");
                    assert_eq!(data["stage_timings"], json!([]));
                }
                assert!(message["result"]["structuredContent"]["elapsed_ms"].is_u64());
                break;
            }
            assert_eq!(message["method"], "notifications/progress");
            assert_eq!(message["params"]["progressToken"], "progress-run-1");
            assert!(message["params"].get("total").is_none());
            values.push(message["params"]["progress"].as_f64().unwrap());
        }
        assert_eq!(values, if enabled { vec![1., 2.] } else { vec![] });
    }
    drop(input);
    assert!(
        tokio::time::timeout(Duration::from_secs(10), child.wait())
            .await
            .unwrap()
            .unwrap()
            .success()
    );
}
