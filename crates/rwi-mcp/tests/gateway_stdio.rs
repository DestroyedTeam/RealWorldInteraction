//! P: the actual production entry starts with zero devices, no task or window.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};
#[test]
fn production_catalog_is_not_a_diagnostic_fallback_or_business_session() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rwi-mcp"))
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
    let mut expected: std::collections::BTreeSet<_> = rwi_core::device::catalog::definitions()
        .iter()
        .map(|d| d.name)
        .collect();
    expected.insert("computer.result");
    expected.insert("computer.js");
    expected.insert("computer.js_reset");
    expected.extend([
        "computer.snapshot",
        "computer.click",
        "computer.hover",
        "computer.reveal",
        "computer.drag",
        "computer.scroll",
        "computer.controls",
        "computer.type_text",
        "computer.press_key",
        "computer.place_window",
    ]);
    assert_eq!(names, expected);
    assert!(names.contains("computer.act_checked"));
    assert!(names.contains("computer.recover_input"));
    let move_tool = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "computer.move_window")
        .unwrap();
    assert!(
        move_tool["description"]
            .as_str()
            .unwrap()
            .contains("ColorSync UUID")
    );
    assert!(move_tool["inputSchema"].to_string().contains("\"uuid\""));
    for (i, name) in [
        "computer.snapshot",
        "computer.click",
        "computer.hover",
        "computer.reveal",
        "computer.drag",
        "computer.scroll",
        "computer.controls",
        "computer.type_text",
        "computer.press_key",
        "computer.place_window",
    ]
    .iter()
    .enumerate()
    {
        let tool = list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == *name)
            .unwrap();
        assert!(tool["title"].is_string());
        let invalid = rpc(
            json!({"jsonrpc":"2.0","id":20+i,"method":"tools/call","params":{"name":name,"arguments":{"unexpected":"field"}}}),
            true,
        );
        assert_eq!(
            invalid["result"]["structuredContent"]["code"],
            "invalid_arguments"
        );
    }
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
    let reference = targets["result"]["structuredContent"]["presentation"]["result_ref"].clone();
    let retained = rpc(
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"computer.result","arguments":{"result_ref":reference,"pointer":"/targets"}}}),
        true,
    );
    assert_eq!(retained["result"]["structuredContent"]["value"], json!([]));
    assert_eq!(
        retained["result"]["structuredContent"]["native_dispatched"],
        false
    );
    let invalid_source = rpc(
        json!({"jsonrpc":"2.0","id":40,"method":"tools/call","params":{"name":"computer.controls","arguments":{"observation_ref":reference,"frame":"not-an-observation","query":"Undo"}}}),
        true,
    );
    assert_eq!(
        invalid_source["result"]["structuredContent"]["code"],
        "observation_frame_mismatch"
    );
    // A nested convenience name is a routing error, not a native input failure.
    // Keep the real production process device-free; neither call may dispatch.
    let rejected = rpc(
        json!({"jsonrpc":"2.0","id":50,"method":"tools/call","params":{"name":"computer.js","arguments":{"code":"await rwi.call('computer.drag', {value:'ARGUMENT_NOT_FOR_DIAGNOSTICS'});","timeout_ms":2000}}}),
        true,
    );
    let long_name = rpc(
        json!({"jsonrpc":"2.0","id":51,"method":"tools/call","params":{"name":"computer.js","arguments":{"code":"await rwi.call('x'.repeat(4096), {});","timeout_ms":2000}}}),
        true,
    );
    let recursion = rpc(
        json!({"jsonrpc":"2.0","id":52,"method":"tools/call","params":{"name":"computer.js","arguments":{"code":"await rwi.call('computer.js', {});","timeout_ms":2000}}}),
        true,
    );
    drop(input);
    assert!(child.wait().unwrap().success());
    for (tool, response) in [("computer.drag", &rejected), ("computer.js", &recursion)] {
        assert_eq!(response["result"]["isError"], true);
        let data = &response["result"]["structuredContent"];
        let error = data["error"].as_str().unwrap();
        assert!(error.starts_with("js_unknown_capability"));
        assert!(
            error.contains(tool),
            "rejection must name the requested tool"
        );
        assert!(error.contains("native catalog"));
        assert!(!error.contains("ARGUMENT_NOT_FOR_DIAGNOSTICS"));
        assert_eq!(data["calls"], json!([]));
        assert_eq!(data["execution_summary"]["mutation_attempts"], 0);
        assert_eq!(data["execution_summary"]["replay_allowed"], false);
    }
    let long_error = long_name["result"]["structuredContent"]["error"]
        .as_str()
        .unwrap();
    assert!(
        long_error.len() < 384,
        "untrusted tool names must stay bounded"
    );
    let js = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "computer.js")
        .unwrap();
    assert!(
        js["description"]
            .as_str()
            .unwrap()
            .contains("not MCP convenience tools")
    );
}

#[test]
fn live_ui_resource_negotiation_is_optional_and_does_not_grant_a_window() {
    let mime = "text/html;profile=mcp-app";
    let uri = "ui://real-world-interaction/live/v1.html";
    for (capabilities, supported) in [
        (json!({}), false),
        (
            json!({"experimental":{"io.modelcontextprotocol/ui":{"mimeTypes":[mime]}}}),
            false,
        ),
        (
            json!({"extensions":{"io.modelcontextprotocol/ui":{"mimeTypes":["image/png"]}}}),
            false,
        ),
        (
            json!({"extensions":{"io.modelcontextprotocol/ui":{"mimeTypes":[mime]}}}),
            true,
        ),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rwi-mcp"))
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
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":capabilities,"clientInfo":{"name":"rwi-ui-protocol-test","version":"1"}}}),
            true,
        );
        rpc(
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            false,
        );
        let tools = rpc(
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
            true,
        );
        let resources = rpc(
            json!({"jsonrpc":"2.0","id":3,"method":"resources/list","params":{}}),
            true,
        );
        let resource = rpc(
            json!({"jsonrpc":"2.0","id":4,"method":"resources/read","params":{"uri":uri}}),
            true,
        );
        let invalid = rpc(
            json!({"jsonrpc":"2.0","id":5,"method":"resources/read","params":{"uri":"file:///etc/passwd"}}),
            true,
        );
        let owned_only = rpc(
            json!({"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"computer.live_panel","arguments":{"view_key":"a".repeat(32),"page":"b".repeat(32),"sequence":1,"visible":true}}}),
            true,
        );
        let no_target_arg = rpc(
            json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"computer.live_panel","arguments":{"view_key":"a".repeat(32),"page":"b".repeat(32),"sequence":2,"visible":false,"target":"other-device:window"}}}),
            true,
        );
        drop(input);
        assert!(child.wait().unwrap().success());
        assert!(init["result"]["capabilities"]["resources"].is_object());
        let tools = tools["result"]["tools"].as_array().unwrap();
        let live = tools.iter().find(|t| t["name"] == "computer.live").unwrap();
        assert!(
            live["_meta"]["ui"].get("resourceUri").is_none(),
            "status/control must not recreate a view"
        );
        let render = tools.iter().find(|t| t["name"] == "computer.show_live");
        assert_eq!(render.is_some(), supported);
        if let Some(render) = render {
            assert_eq!(render["_meta"]["ui"]["resourceUri"], uri);
        }

        let presence = tools.iter().find(|t| t["name"] == "computer.live_panel");
        assert_eq!(presence.is_some(), supported);
        if let Some(presence) = presence {
            assert_eq!(presence["_meta"]["ui"]["visibility"], json!(["app"]));
            assert!(
                presence["inputSchema"]["properties"]
                    .get("target")
                    .is_none()
            );
        }
        assert_eq!(resources["result"]["resources"][0]["uri"], uri);
        let content = &resource["result"]["contents"][0];
        assert_eq!(content["mimeType"], mime);
        assert_eq!(content["_meta"]["ui"]["csp"]["connectDomains"], json!([]));
        assert!(
            content["text"]
                .as_str()
                .unwrap()
                .contains("ui/resource-teardown")
        );
        assert!(invalid["error"].is_object());
        assert_eq!(
            owned_only["result"]["structuredContent"]["code"],
            if supported {
                "live_ui_event_rejected"
            } else {
                "live_ui_not_negotiated"
            }
        );
        assert_eq!(
            no_target_arg["result"]["structuredContent"]["code"],
            if supported {
                "invalid_live_ui_event"
            } else {
                "live_ui_not_negotiated"
            }
        );
    }
}
