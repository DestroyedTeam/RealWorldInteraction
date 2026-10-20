use super::*;
use crate::gateway::{mcp_result_with_text_projection, portable_numbers};

fn fixture() -> DeviceResult {
    let elements: Vec<_> = (0..12).map(|i| json!({"id":format!("{i:064x}"),"name":format!("Close {i}"),"point":[17,29],"actions":["AXPress"],"future":null})).collect();
    DeviceResult {
        is_error: false,
        data: json!({"target":"exact-device:instance:context:window","frame":"exact-frame","view_id":"exact-view","observation_ref":"exact-observation","details_ref":"exact-details",
            "window":{"pid":7,"window":"cgwindow:77","title":"Fixture","future":null},
            "input_domain_fenced":false,"blocked":false,"visual_only":false,"visual_available":true,"mutation_ready":true,"input_reference_available":true,
            "native_complete":"unknown","elements":elements,"available_controls":12,"omitted_controls":0,
            "mapping":{"original_size":[800,600],"view_size":[800,600],"original_to_desktop":[1,0,0,1,0,0],"future":null},
            "coverage":{"complete":false,"absence_proven":false,"future":null},
            "consistency":{"pixel_semantic_atomic":false,"future":null},"future_root":{"large":9007199254740993_u64,"unknown":null}}),
        images: vec![rwi_core::device::DeviceImage {
            mime: "image/png".into(),
            base64: "exact-opaque-image-bytes".into(),
        }],
    }
}
fn reply(r: DeviceResult, requested: bool) -> Value {
    let text = project(requested, &r);
    serde_json::to_value(mcp_result_with_text_projection(r, text)).unwrap()
}
fn text(v: &Value) -> Value {
    serde_json::from_str(v["content"][0]["text"].as_str().unwrap()).unwrap()
}

#[test]
fn f56_snapshot_summary_request_is_optional_exact_and_adapter_only() {
    let mut none = None;
    assert!(!request("computer.snapshot", &mut none).unwrap());
    assert!(none.is_none());
    let mut args = Some(
        json!({"target":"exact","max_edge":2048,"text_projection":MODE})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert!(request("computer.snapshot", &mut args).unwrap());
    assert_eq!(
        args.unwrap(),
        json!({"target":"exact","max_edge":2048})
            .as_object()
            .unwrap()
            .clone()
    );
}
#[test]
fn f56_snapshot_summary_does_not_consume_another_tools_field() {
    for name in [
        "computer.targets",
        "computer.image",
        "computer.js",
        "computer.act",
    ] {
        let mut args = Some(
            json!({"text_projection":MODE,"unknown":null})
                .as_object()
                .unwrap()
                .clone(),
        );
        let before = args.clone();
        assert!(!request(name, &mut args).unwrap());
        assert_eq!(args, before);
    }
}
#[test]
fn f56_snapshot_summary_invalid_modes_fail_closed() {
    for v in [
        json!(null),
        json!(true),
        json!(1),
        json!({}),
        json!([]),
        json!("exact_duplicates_v1"),
        json!("wrong"),
    ] {
        let mut args = Some(
            json!({"target":"exact","text_projection":v})
                .as_object()
                .unwrap()
                .clone(),
        );
        let error = request("computer.snapshot", &mut args).unwrap_err();
        assert!(error.is_error);
        assert_eq!(error.data["code"], "invalid_arguments");
    }
}
#[test]
fn f56_snapshot_summary_schema_optional_isolated_and_no_shared_mutation() {
    let schema = std::sync::Arc::new(
        json!({"type":"object","properties":{"target":{"type":"string"}},"required":["target"]})
            .as_object()
            .unwrap()
            .clone(),
    );
    let mut tools = vec![
        rmcp::model::Tool::new("computer.snapshot", "fixture", schema.clone()),
        rmcp::model::Tool::new("computer.targets", "fixture", schema.clone()),
    ];
    advertise(&mut tools);
    assert_eq!(
        tools[0].input_schema["properties"]["text_projection"]["enum"],
        json!([MODE])
    );
    assert_eq!(tools[0].input_schema["required"], json!(["target"]));
    assert!(schema["properties"].get("text_projection").is_none());
    assert_eq!(&*tools[1].input_schema, &*schema);
    advertise(&mut []);
}
#[test]
fn f56_snapshot_summary_default_full_text_and_images_are_unchanged() {
    let raw = fixture();
    let mut expected = raw.data.clone();
    portable_numbers(&mut expected);
    let v = reply(raw, false);
    assert_eq!(v["structuredContent"], expected);
    assert_eq!(text(&v), expected);
    assert_eq!(v["content"][1]["data"], "exact-opaque-image-bytes");
}
#[test]
fn f56_snapshot_summary_preserves_complete_structured_native_refs_points_and_images() {
    let raw = fixture();
    let before = raw.clone();
    let projection = project(true, &raw).unwrap();
    assert_eq!(raw.data, before.data);
    assert_eq!(raw.images[0].base64, before.images[0].base64);
    let mut expected = before.data;
    portable_numbers(&mut expected);
    let v = reply(raw, true);
    let t = text(&v);
    assert_eq!(v["structuredContent"], expected);
    assert_eq!(v["content"][1]["data"], "exact-opaque-image-bytes");
    assert_eq!(v["content"][1]["mimeType"], "image/png");
    assert_eq!(projection[MARKER]["version"], "snapshot_summary_v1");
    assert_eq!(t["native_complete"], "unknown");
    assert_eq!(t["frame"], "exact-frame");
    assert_eq!(t["window"], expected["window"]);
    for key in SECTIONS {
        assert!(t.get(key).is_none());
        assert_eq!(t[MARKER]["structured_paths"][key], format!("/{key}"));
    }
    assert_eq!(
        v["structuredContent"]["elements"][0]["point"],
        json!([17, 29])
    );
    assert_eq!(
        v["structuredContent"]["elements"][0]["id"],
        format!("{:064x}", 0)
    );
}
#[test]
fn f56_snapshot_summary_unknown_root_and_nested_values_are_not_destroyed() {
    let raw = fixture();
    let v = reply(raw, true);
    let t = text(&v);
    assert_eq!(t["future_root"], v["structuredContent"]["future_root"]);
    assert_eq!(t["future_root"]["large"], "9007199254740993");
    for key in ["mapping", "coverage", "consistency"] {
        assert!(v["structuredContent"][key].get("future").unwrap().is_null());
    }
}
#[test]
fn f56_snapshot_summary_errors_uncertainty_and_reserved_fields_remain_full() {
    let mut raw = fixture();
    raw.is_error = true;
    assert!(project(true, &raw).is_none());
    for key in [
        MARKER,
        "code",
        "message",
        "diagnostic",
        "dispatch",
        "dispatch_error",
        "receipt",
        "operation",
    ] {
        let mut raw = fixture();
        raw.data[key] = Value::Null;
        assert!(project(true, &raw).is_none(), "{key}");
    }
}
#[test]
fn f56_snapshot_summary_readiness_fences_and_degraded_snapshots_remain_full() {
    for (key, value) in [
        ("input_domain_fenced", json!(true)),
        ("blocked", json!(true)),
        ("visual_only", json!(true)),
        ("visual_available", json!(false)),
        ("mutation_ready", json!(false)),
        ("input_reference_available", json!(false)),
    ] {
        let mut raw = fixture();
        raw.data[key] = value;
        assert!(project(true, &raw).is_none(), "{key}");
        raw.data.as_object_mut().unwrap().remove(key);
        assert!(project(true, &raw).is_none());
    }
}
#[test]
fn f56_snapshot_summary_malformed_shapes_retain_full_text() {
    for (ptr, value) in [
        ("/target", json!("")),
        ("/frame", Value::Null),
        ("/view_id", Value::Null),
        ("/window/pid", json!(0)),
        ("/window/window", Value::Null),
        ("/elements", json!(false)),
        ("/elements/0/id", Value::Null),
        ("/available_controls", json!(0)),
        ("/mapping", Value::Null),
        ("/coverage", json!(false)),
        ("/consistency", json!([])),
    ] {
        let mut raw = fixture();
        *raw.data.pointer_mut(ptr).unwrap() = value;
        let v = reply(raw.clone(), true);
        assert!(project(true, &raw).is_none(), "{ptr}");
        assert_eq!(text(&v), v["structuredContent"]);
    }
}
#[test]
fn f56_snapshot_summary_unknown_or_missing_media_is_not_projected() {
    let mut raw = fixture();
    raw.images.clear();
    assert!(project(true, &raw).is_none());
    let mut raw = fixture();
    raw.images[0].mime = "image/future".into();
    assert!(project(true, &raw).is_none());
    let mut raw = fixture();
    raw.images[0].base64.clear();
    assert!(project(true, &raw).is_none());
    let mut raw = fixture();
    raw.images.push(raw.images[0].clone());
    assert!(project(true, &raw).is_none());
}
#[test]
fn f56_snapshot_summary_text_never_expands_a_tiny_response() {
    let mut raw = fixture();
    raw.data["elements"] = json!([]);
    raw.data["available_controls"] = json!(0);
    for key in ["mapping", "coverage", "consistency"] {
        raw.data[key] = json!({});
    }
    raw.data.as_object_mut().unwrap().remove("future_root");
    assert!(project(true, &raw).is_none());
}
#[test]
fn f56_snapshot_summary_recorded_packet_optional_exact_png_and_structured() {
    let Some(path) = std::env::var_os("RWI_F56_SNAPSHOT_RECORDS") else {
        return;
    };
    use base64::Engine;
    let packet: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let original = &packet["beforeSnapshot"];
    let mut raw = DeviceResult::value(original["structuredContent"].clone());
    for c in original["content"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["type"] == "image")
    {
        let bytes = std::fs::read(c["file"].as_str().unwrap()).unwrap();
        raw.images.push(rwi_core::device::DeviceImage {
            mime: c["mimeType"].as_str().unwrap().into(),
            base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        });
    }
    let expected = raw.clone();
    let v = reply(raw, true);
    assert_eq!(v["structuredContent"], expected.data);
    assert_eq!(v["content"][1]["data"], expected.images[0].base64);
    assert_eq!(text(&v)[MARKER]["version"], "snapshot_summary_v1");
    println!(
        "F56_RECORDED_PACKET full_text_bytes={} compact_text_bytes={} exact_PNG_bytes_retained=true",
        serde_json::to_vec(&expected.data).unwrap().len(),
        v["content"][0]["text"].as_str().unwrap().len()
    );
}
