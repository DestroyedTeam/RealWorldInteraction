use super::*;
const SAMPLE: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGMQMYr6DwACngGgfe0KRgAAAABJRU5ErkJggg==";
fn ready() -> DeviceResult {
    let png = STANDARD.decode(SAMPLE).unwrap();
    DeviceResult {
        is_error: false,
        data: json!({"state":"ready","cursor":{"generation":1,"sequence":7},"image_size":[1,1],"png_bytes":png.len(),"png_sha256":rwi_core::digest(&png),"captured_unix_ms":10,"target":"NEVER_VISIBLE_TARGET","frame":"NEVER_VISIBLE_FRAME","source_rect":{"x":5},"lease":"NEVER_VISIBLE_LEASE","untrusted":"NEVER_VISIBLE_FUTURE"}),
        images: vec![device::DeviceImage {
            mime: "image/png".into(),
            base64: SAMPLE.into(),
        }],
    }
}
fn serialized(r: CallToolResult) -> Value {
    serde_json::to_value(r).unwrap()
}
#[test]
fn f32_live_image_only_meta_contains_exact_pixels_and_bounded_descriptive_fields() {
    let r = serialized(component_reply(ready(), 3));
    assert_eq!(r["structuredContent"]["state"], "ready");
    assert_eq!(r["_meta"]["rwi/live_image"]["base64"], SAMPLE);
    assert_eq!(r["_meta"]["rwi/live_image"]["request_sequence"], 3);
    assert!(
        r["content"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["type"] != "image")
    );
    assert!(!r["structuredContent"].to_string().contains(SAMPLE));
    assert!(!r["content"].to_string().contains(SAMPLE));
    assert!(!r.to_string().contains("NEVER_VISIBLE"));
    assert_eq!(
        STANDARD
            .decode(r["_meta"]["rwi/live_image"]["base64"].as_str().unwrap())
            .unwrap(),
        STANDARD.decode(SAMPLE).unwrap()
    );
}
#[test]
fn f32_live_image_unchanged_has_no_pixel_payload() {
    let mut d = ready();
    d.data["state"] = json!("unchanged");
    d.images.clear();
    let r = serialized(component_reply(d, 1));
    assert_eq!(r["_meta"]["rwi/live_image"]["state"], "unchanged");
    assert!(r["_meta"]["rwi/live_image"].get("base64").is_none());
}
#[test]
fn f32_live_image_malformed_bounds_mime_hash_shape_and_cursor_refuse_without_pixels() {
    for mode in 0..11 {
        let mut d = ready();
        match mode {
            0 => d.images[0].mime = "text/html".into(),
            1 => d.images[0].base64 = "?".into(),
            2 => d.data["image_size"] = json!([2, 1]),
            3 => d.data["image_size"] = json!([0, 1]),
            4 => d.data["png_sha256"] = json!("a".repeat(64)),
            5 => d.data["png_bytes"] = json!(1),
            6 => d.data["cursor"]["generation"] = json!(0),
            7 => d.images.clear(),
            8 => d.images.push(d.images[0].clone()),
            9 => d.data["state"] = json!("unchanged"),
            10 => d.images[0].base64 = "A".repeat(MAX_LIVE_IMAGE_BYTES.div_ceil(3) * 4 + 1),
            _ => unreachable!(),
        }
        let r = serialized(component_reply(d, 1));
        assert_eq!(r["isError"], true);
        assert!(r.get("_meta").is_none());
        assert!(!r.to_string().contains(SAMPLE));
    }
}
#[test]
fn f32_live_image_agent_errors_do_not_leak_backend_messages_or_images() {
    let mut d = ready();
    d.is_error = true;
    d.data["code"] = json!("live_image_hidden");
    d.data["message"] = json!("NEVER_VISIBLE_CREDENTIAL");
    let r = serialized(component_reply(d, 1));
    assert_eq!(r["isError"], true);
    assert_eq!(r["structuredContent"]["code"], "live_image_hidden");
    assert!(!r.to_string().contains("NEVER_VISIBLE"));
    assert!(r.get("_meta").is_none());
}
#[test]
fn f32_live_image_descriptor_is_app_only_and_cannot_select_targets_or_publish_presence() {
    let t = serde_json::to_value(tool()).unwrap();
    assert_eq!(t["_meta"]["ui"]["visibility"], json!(["app"]));
    assert_eq!(t["annotations"]["readOnlyHint"], true);
    for name in [
        "target",
        "capture",
        "input",
        "visible",
        "close",
        "view_id",
        "device_id",
    ] {
        assert!(t["inputSchema"]["properties"].get(name).is_none());
    }
    assert!(
        !device::catalog::definitions()
            .iter()
            .any(|d| d.name == METHOD)
    );
}
#[test]
fn f32_live_image_strict_safe_input_fields_only() {
    let base = json!({"view_key":"a".repeat(32),"page":"b".repeat(32),"sequence":1});
    assert!(
        serde_json::from_value::<ImageArgs>(base.clone())
            .unwrap()
            .valid()
    );
    for name in [
        "target",
        "capture",
        "input",
        "visible",
        "close",
        "view_id",
        "device_id",
    ] {
        let mut v = base.clone();
        v[name] = json!(true);
        assert!(serde_json::from_value::<ImageArgs>(v).is_err());
    }
    for seq in [0_u64, 9_007_199_254_740_992] {
        let mut v = base.clone();
        v["sequence"] = json!(seq);
        assert!(!serde_json::from_value::<ImageArgs>(v).unwrap().valid());
    }
    let mut v = base;
    v["after"] = json!({"generation":1,"sequence":0});
    assert!(!serde_json::from_value::<ImageArgs>(v).unwrap().valid());
}
