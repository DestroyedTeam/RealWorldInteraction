//! P: the actual production entry starts with zero devices, no task or window.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};
#[test]
fn production_catalog_is_not_a_diagnostic_fallback_or_business_session() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcu-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut rpc = |v: Value, reply: bool| -> Value {
        writeln!(input, "{v}").unwrap();
        input.flush().unwrap();
        if !reply {
            return Value::Null;
        }
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };
    let init = rpc(
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"gateway-test","version":"1"}}}),
        true,
    );
    assert!(
        !init["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("DIAGNOSTIC ONLY")
    );
    rpc(
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        false,
    );
    let list = rpc(
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        true,
    );
    let names: std::collections::BTreeSet<_> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        rcu_core::device::catalog::definitions()
            .iter()
            .map(|d| d.name)
            .collect()
    );
    assert!(names.contains("computer.act_checked"));
    assert!(names.contains("computer.recover_input"));
    let targets = rpc(
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"computer.targets","arguments":{}}}),
        true,
    );
    assert_eq!(targets["result"]["structuredContent"]["service"], "healthy");
    assert_eq!(targets["result"]["structuredContent"]["targets"], json!([]));
    assert_eq!(
        targets["result"]["structuredContent"]["workflow_required"],
        false
    );
    drop(input);
    assert!(child.wait().unwrap().success());
}
