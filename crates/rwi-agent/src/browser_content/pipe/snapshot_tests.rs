//! Protocol-shaped fixtures use Unix pipes only; no browser, input or Native evidence.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use rwi_core::device::browser::SnapshotArgs;
use std::io::Write;
use std::sync::{Arc, Mutex};
fn png_data() -> String {
    let mut b = Vec::new();
    {
        let mut e = png::Encoder::new(&mut b, 2, 1);
        e.set_color(png::ColorType::Rgb);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .unwrap()
            .write_image_data(&[20, 40, 60, 80, 100, 120])
            .unwrap();
    }
    STANDARD.encode(b)
}
fn response(method: &str, root_reads: &mut usize, changed: bool) -> Value {
    match method {
        "Target.getTargetInfo" => {
            json!({"targetInfo":{"targetId":"owned","browserContextId":"private","type":"page"}})
        }
        "Target.attachToTarget" => json!({"sessionId":"own-session"}),
        "Page.getFrameTree" => {
            json!({"frameTree":{"frame":{"id":"main","loaderId":"loader","url":"about:blank"}}})
        }
        "DOM.getDocument" => {
            *root_reads += 1;
            json!({"root":{"nodeType":9,"backendNodeId":if changed&&*root_reads>1{90}else{1}}})
        }
        "Accessibility.getFullAXTree" => {
            json!({"nodes":[{"ignored":false,"backendDOMNodeId":2,"role":{"value":"button"},"name":{"value":"验收 🙂"},"properties":[{"name":"disabled","value":{"value":false}}]}]})
        }
        "DOMSnapshot.captureSnapshot" => {
            json!({"strings":["main","iframe"],"documents":[{"frameId":0,"nodes":{"backendNodeId":[1,2]},"layout":{"nodeIndex":[1],"bounds":[[24,48,120,40]]}},{"frameId":1,"nodes":{"backendNodeId":[2]},"layout":{"nodeIndex":[0],"bounds":[[999,999,9,9]]}}]})
        }
        "Page.getLayoutMetrics" => {
            json!({"visualViewport":{"pageX":0,"pageY":0,"clientWidth":1920,"clientHeight":1200,"scale":1,"zoom":1},"cssVisualViewport":{"pageX":0,"pageY":0,"clientWidth":960,"clientHeight":600,"scale":1,"zoom":1}})
        }
        "Page.captureScreenshot" => json!({"data":png_data()}),
        "Target.detachFromTarget" => json!({}),
        _ => panic!("unexpected observation command {method}"),
    }
}
fn fake(
    changed: bool,
    wrong_session: bool,
) -> (Pipe, Arc<Mutex<Vec<Value>>>, std::thread::JoinHandle<()>) {
    fake_with_viewport(changed, wrong_session, false)
}
fn fake_with_viewport(
    changed: bool,
    wrong_session: bool,
    viewport_changed: bool,
) -> (Pipe, Arc<Mutex<Vec<Value>>>, std::thread::JoinHandle<()>) {
    let (tx, mut commands) = UnixStream::pair().unwrap();
    let (mut replies, rx) = UnixStream::pair().unwrap();
    tx.set_nonblocking(true).unwrap();
    rx.set_nonblocking(true).unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let worker = std::thread::spawn(move || {
        let mut roots = 0;
        let mut metric_reads = 0;
        loop {
            let mut raw = Vec::new();
            let mut b = [0u8; 1];
            loop {
                match commands.read(&mut b) {
                    Ok(0) | Err(_) => return,
                    Ok(_) => {
                        if b[0] == 0 {
                            break;
                        }
                        raw.push(b[0]);
                    }
                }
            }
            let c: Value = serde_json::from_slice(&raw).unwrap();
            let method = c["method"].as_str().unwrap();
            seen.lock().unwrap().push(c.clone());
            let mut result = response(method, &mut roots, changed);
            if method == "Page.getLayoutMetrics" {
                metric_reads += 1;
                if viewport_changed && metric_reads > 1 {
                    result["cssVisualViewport"]["clientWidth"] = json!(959);
                }
            }
            let mut v = json!({"id":c["id"],"result":result});
            if let Some(s) = c.get("sessionId") {
                v["sessionId"] = if wrong_session {
                    json!("foreign")
                } else {
                    s.clone()
                };
            }
            let mut raw = serde_json::to_vec(&v).unwrap();
            raw.push(0);
            if replies.write_all(&raw).is_err() {
                return;
            }
        }
    });
    (
        Pipe {
            tx,
            rx,
            framer: Framer::new(),
            id: 0,
            poisoned: false,
        },
        calls,
        worker,
    )
}
fn args() -> SnapshotArgs {
    SnapshotArgs {
        device_id: "mac-test".into(),
        session_ref: "browser:owned".into(),
        max_nodes: 128,
    }
}
#[test]
fn f15_browser_content_css_units_use_layout_metrics_not_png_or_os_dpi() {
    for factor in [1., 2., 2.5] {
        let metrics = json!({"visualViewport":{"clientWidth":640.*factor,"clientHeight":480.*factor,"scale":1.75,"zoom":1.25},"cssVisualViewport":{"clientWidth":640.,"clientHeight":480.,"scale":1.75,"zoom":1.25}});
        let scale = snapshot::layout_scale(&metrics).unwrap();
        assert_eq!(scale, factor);
        let raw = std::collections::BTreeMap::from([(
            2,
            json!([32. * factor, 64. * factor, 60. * factor, 20. * factor]),
        )]);
        assert_eq!(
            snapshot::css_rectangles(&raw, scale).unwrap()[&2],
            json!([32., 64., 60., 20.])
        );
    }
}
#[test]
fn f15_browser_content_css_missing_or_inconsistent_metrics_are_not_guessed() {
    for metrics in [
        json!({"cssVisualViewport":{"clientWidth":960,"clientHeight":600}}),
        json!({"visualViewport":{"clientWidth":0,"clientHeight":1200},"cssVisualViewport":{"clientWidth":960,"clientHeight":600}}),
        json!({"visualViewport":{"clientWidth":1920,"clientHeight":1800},"cssVisualViewport":{"clientWidth":960,"clientHeight":600}}),
    ] {
        assert!(snapshot::layout_scale(&metrics).is_err());
    }
    assert!(snapshot::css_rectangles(&Default::default(), f64::NAN).is_err());
}
#[test]
fn f15_browser_content_css_packet_preserves_physical_bounds_and_rejects_viewport_change() {
    let (mut p, _, w) = fake(false, false);
    let r = snapshot::collect(&mut p, "owned", "private", &args()).unwrap();
    drop(p);
    w.join().unwrap();
    assert_eq!(r.data["coordinate_mapping"]["physical_per_css_pixel"], 2.);
    assert_eq!(
        r.data["nodes"][0]["bounds_document_physical"],
        json!([24, 48, 120, 40])
    );
    assert_eq!(
        r.data["nodes"][0]["bounds_css_document"],
        json!([12., 24., 60., 20.])
    );
    assert_eq!(r.data["consistency"]["viewport_pre_post_equal"], true);
    assert_eq!(r.images[0].base64, png_data());
    let (mut p, calls, w) = fake_with_viewport(false, false, true);
    let e = snapshot::collect(&mut p, "owned", "private", &args()).unwrap_err();
    drop(p);
    w.join().unwrap();
    assert!(
        e.to_string()
            .contains("viewport_changed_observation_discarded")
    );
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c["method"] == "Target.detachFromTarget")
            .count(),
        1
    );
}
#[test]
fn f15_browser_content_snapshot_flat_session_original_png_and_indexed_layout() {
    let (mut p, calls, w) = fake(false, false);
    let r = snapshot::collect(&mut p, "owned", "private", &args()).unwrap();
    drop(p);
    w.join().unwrap();
    assert_eq!(r.images.len(), 1);
    assert_eq!(r.images[0].base64, png_data());
    assert_eq!(r.data["nodes"][0]["name"], "验收 🙂");
    assert_eq!(r.data["nodes"][0]["states"]["disabled"], false);
    assert_eq!(
        r.data["nodes"][0]["bounds_css_document"],
        json!([12., 24., 60., 20.])
    );
    assert_eq!(r.data["image"]["size"], json!([2, 1]));
    assert_eq!(r.data["debugger_detached"], true);
    assert_eq!(r.data["consistency"]["atomic"], false);
    assert_eq!(r.data["coverage"]["complete"], false);
    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 13);
    for c in calls.iter() {
        let m = c["method"].as_str().unwrap();
        assert!(
            !m.starts_with("Input.") && !m.starts_with("Runtime.") && m != "Target.activateTarget"
        );
        if !m.starts_with("Target.") {
            assert_eq!(c["sessionId"], "own-session");
        } else {
            assert!(c.get("sessionId").is_none());
        }
    }
    let screen = calls
        .iter()
        .find(|c| c["method"] == "Page.captureScreenshot")
        .unwrap();
    assert_eq!(
        screen["params"],
        json!({"format":"png","fromSurface":true,"captureBeyondViewport":false})
    );
}
#[test]
fn f15_browser_content_snapshot_document_change_discards_image_and_detaches_once() {
    let (mut p, calls, w) = fake(true, false);
    let e = snapshot::collect(&mut p, "owned", "private", &args()).unwrap_err();
    drop(p);
    w.join().unwrap();
    assert!(e.to_string().contains("document_changed"));
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c["method"] == "Target.detachFromTarget")
            .count(),
        1
    );
}
#[test]
fn f15_browser_content_snapshot_foreign_reply_poisons_without_second_send() {
    let (mut p, calls, w) = fake(false, true);
    assert!(snapshot::collect(&mut p, "owned", "private", &args()).is_err());
    let before = calls.lock().unwrap().len();
    assert!(p.poisoned);
    assert!(
        p.rpc(
            "Target.getTargetInfo",
            json!({}),
            Instant::now() + Duration::from_millis(10)
        )
        .is_err()
    );
    drop(p);
    w.join().unwrap();
    assert_eq!(calls.lock().unwrap().len(), before);
    assert_eq!(before, 3);
}
#[test]
fn f15_browser_content_snapshot_png_and_layout_invalid_are_not_silently_normalized() {
    for data in ["not base64".to_owned(), STANDARD.encode(b"not png")] {
        assert!(snapshot::original_png(&json!({"data":data})).is_err());
    }
    assert!(
        snapshot::rectangles(
            &json!({"strings":["main"],"documents":[{"frameId":99}]}),
            "main"
        )
        .is_err()
    );
    let rows = snapshot::nodes(
        &json!({"nodes":[{"ignored":false,"name":{"value":"🙂".repeat(400)}},{"ignored":false}]}),
        &Default::default(),
        1,
    )
    .unwrap();
    assert_eq!(rows["coverage"]["node_limit_reached"], true);
    assert_eq!(rows["nodes"][0]["truncated_fields"], json!(["name"]));
    assert_eq!(rows["nodes"][0]["name"].as_str().unwrap().len(), 1024);
}
