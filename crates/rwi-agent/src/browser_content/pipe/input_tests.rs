//! A fake CDP peer checks event framing and no-replay, not real browser effects.
use super::input::{Receipt, collect, hit_in_subtree, point};
use super::*;
use crate::browser_content::input::Grounding;
use rwi_core::device::browser::{ActArgs, Action};
use std::{
    io::{BufRead, BufReader, Write},
    sync::{Arc, Mutex},
};
fn metrics() -> Value {
    json!({"cssVisualViewport":{"pageX":0,"pageY":200,"offsetX":0,"offsetY":0,"clientWidth":960,"clientHeight":633,"scale":1},"visualViewport":{"pageX":0,"pageY":400,"offsetX":0,"offsetY":0,"clientWidth":1920,"clientHeight":1266,"scale":1}})
}
fn dom() -> Value {
    json!({"strings":["frame"],"documents":[{"frameId":0,"nodes":{"backendNodeId":[1,7,8,9],"parentIndex":[-1,0,1,0],"nodeType":[9,1,1,1]},"layout":{"nodeIndex":[1],"bounds":[[60,480,200,80]]}}]})
}
fn grounding() -> Grounding {
    let m = metrics();
    Grounding {
        reference: "browser-observation:one".into(),
        document: json!({"frame_id":"frame","loader_id":"loader","url":"about:blank","backend_document_node":1}),
        viewport: m["cssVisualViewport"].clone(),
        legacy: m["visualViewport"].clone(),
        expires: Instant::now() + Duration::from_secs(30),
        elements: std::collections::BTreeMap::from([("e1".into(), 7)]),
    }
}
fn args(action: Action) -> ActArgs {
    ActArgs {
        device_id: "owned".into(),
        session_ref: "browser:one".into(),
        request_id: "once".into(),
        observation_ref: "browser-observation:one".into(),
        element_ref: "e1".into(),
        action,
    }
}
#[derive(Default)]
struct Mode {
    disabled: bool,
    overlay: bool,
    changed: bool,
    lost_at: usize,
    shift_after_move: bool,
    overlay_after_move: bool,
    changed_after_move: bool,
}
fn fake(mode: Mode) -> (Pipe, Arc<Mutex<Vec<Value>>>, std::thread::JoinHandle<()>) {
    let (tx, read) = UnixStream::pair().unwrap();
    let (write, rx) = UnixStream::pair().unwrap();
    tx.set_nonblocking(true).unwrap();
    rx.set_nonblocking(true).unwrap();
    read.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let h = std::thread::spawn(move || {
        let mut reader = BufReader::new(read);
        let mut write = write;
        let mut inputs = 0;
        let mut frames = 0;
        loop {
            let mut raw = Vec::new();
            let n = reader.read_until(0, &mut raw).unwrap_or(0);
            if n == 0 {
                break;
            }
            let q: Value = serde_json::from_slice(&raw[..n - 1]).unwrap();
            seen.lock().unwrap().push(q.clone());
            let method = q["method"].as_str().unwrap();
            let value = match method {
                "Target.getTargetInfo" => {
                    json!({"targetInfo":{"targetId":"target","browserContextId":"context","type":"page"}})
                }
                "Target.attachToTarget" => json!({"sessionId":"flat"}),
                "Page.getFrameTree" => {
                    frames += 1;
                    json!({"frameTree":{"frame":{"id":"frame","loaderId":if (mode.changed&&frames>1)||(mode.changed_after_move&&inputs>0){"changed"}else{"loader"},"url":"about:blank"}}})
                }
                "DOM.getDocument" => json!({"root":{"nodeType":9,"backendNodeId":1}}),
                "Page.getLayoutMetrics" => metrics(),
                "DOMSnapshot.captureSnapshot" => {
                    let mut d = dom();
                    if mode.shift_after_move && inputs > 0 {
                        d["documents"][0]["layout"]["bounds"][0] = json!([60, 420, 200, 80]);
                    }
                    d
                }
                "Accessibility.getPartialAXTree" => {
                    json!({"nodes":[{"backendDOMNodeId":7,"ignored":false,"properties":[{"name":"disabled","value":{"type":"boolean","value":mode.disabled}}]}]})
                }
                "DOM.getNodeForLocation" => {
                    json!({"backendNodeId":if mode.overlay||(mode.overlay_after_move&&inputs>0){9}else{8},"frameId":"frame"})
                }
                "Input.dispatchMouseEvent" => {
                    inputs += 1;
                    if inputs == mode.lost_at {
                        break;
                    }
                    json!({})
                }
                "Target.detachFromTarget" => json!({}),
                _ => panic!("unexpected CDP method {method}"),
            };
            let mut reply = json!({"id":q["id"],"result":value});
            if let Some(session) = q.get("sessionId") {
                reply["sessionId"] = session.clone()
            }
            let mut raw = serde_json::to_vec(&reply).unwrap();
            raw.push(0);
            if write.write_all(&raw).is_err() {
                break;
            }
            if method == "Target.detachFromTarget" {
                break;
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
        h,
    )
}
#[test]
fn f16_browser_content_pointer_recheck_follows_same_node_after_one_move_without_retry() {
    let (mut p, calls, h) = fake(Mode {
        shift_after_move: true,
        ..Mode::default()
    });
    let mut r = Receipt::default();
    let v = collect(
        &mut p,
        "target",
        "context",
        &args(Action::Click),
        &grounding(),
        7,
        &mut r,
    )
    .unwrap();
    assert_eq!(v["initial_move_point_css_viewport"], json!([80., 60.]));
    assert_eq!(v["point_css_viewport"], json!([80., 30.]));
    assert_eq!(v["pre_press_revalidated"], true);
    assert_eq!(r.attempted, 3);
    assert_eq!(r.acknowledged, 3);
    assert!(!r.uncertain());
    drop(p);
    h.join().unwrap();
    let c = calls.lock().unwrap();
    let input: Vec<_> = c
        .iter()
        .filter(|q| q["method"] == "Input.dispatchMouseEvent")
        .collect();
    assert_eq!(input.len(), 3);
    assert_eq!(input[0]["params"]["type"], "mouseMoved");
    assert_eq!(input[1]["params"]["type"], "mousePressed");
    assert_eq!(input[2]["params"]["type"], "mouseReleased");
    assert_eq!(input[0]["params"]["y"], 60.);
    assert_eq!(input[1]["params"]["y"], 30.);
    assert_eq!(input[2]["params"]["y"], 30.);
    assert_eq!(
        c.iter()
            .filter(|q| q["method"] == "DOMSnapshot.captureSnapshot")
            .count(),
        2
    );
}
#[test]
fn f16_browser_content_pointer_recheck_post_move_change_sends_no_press_and_does_not_fence_known_move()
 {
    for mode in [
        Mode {
            overlay_after_move: true,
            ..Mode::default()
        },
        Mode {
            changed_after_move: true,
            ..Mode::default()
        },
    ] {
        let (mut p, calls, h) = fake(mode);
        let mut r = Receipt::default();
        assert!(
            collect(
                &mut p,
                "target",
                "context",
                &args(Action::Click),
                &grounding(),
                7,
                &mut r
            )
            .is_err()
        );
        assert_eq!(r.attempted, 1);
        assert_eq!(r.acknowledged, 1);
        assert!(!r.uncertain());
        drop(p);
        h.join().unwrap();
        let c = calls.lock().unwrap();
        let events: Vec<_> = c
            .iter()
            .filter(|q| q["method"] == "Input.dispatchMouseEvent")
            .collect();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["params"]["type"], "mouseMoved");
    }
    assert!(
        Receipt {
            attempted: 2,
            acknowledged: 1,
            button_down_acknowledged: false
        }
        .uncertain()
    );
    assert!(
        Receipt {
            attempted: 2,
            acknowledged: 2,
            button_down_acknowledged: true
        }
        .uncertain()
    );
}
#[test]
fn f16_browser_content_input_geometry_scroll_and_clipping_do_not_use_png_or_os_scale() {
    let v = metrics()["cssVisualViewport"].clone();
    let p = point(&json!([30, 240, 100, 40]), &v).unwrap();
    assert_eq!(p.document, [80, 260]);
    assert_eq!(p.viewport, [80., 60.]);
    let p = point(&json!([-10, 190, 40, 40]), &v).unwrap();
    assert_eq!(p.document, [15, 215]);
    assert_eq!(p.viewport, [15., 15.]);
    assert!(point(&json!([1, 0, 20, 30]), &v).is_err());
    let mut pinch = v.clone();
    pinch["scale"] = json!(2);
    assert!(point(&json!([30, 240, 100, 40]), &pinch).is_err());
    assert!(point(&json!([0.7, 240, 0.1, 40]), &v).is_err());
    assert!(hit_in_subtree(&dom(), "frame", 7, 8));
    assert!(!hit_in_subtree(&dom(), "frame", 7, 9));
    assert!(!hit_in_subtree(&dom(), "other", 7, 8));
    assert!(!hit_in_subtree(&dom(), "frame", 1, 8));
}
#[test]
fn f16_browser_content_input_hover_and_click_send_only_exact_page_events_after_checks() {
    for (action, kinds) in [
        (Action::Hover, vec!["mouseMoved"]),
        (
            Action::Click,
            vec!["mouseMoved", "mousePressed", "mouseReleased"],
        ),
    ] {
        let (mut p, calls, h) = fake(Mode::default());
        let mut receipt = Receipt::default();
        let v = collect(
            &mut p,
            "target",
            "context",
            &args(action),
            &grounding(),
            7,
            &mut receipt,
        )
        .unwrap();
        assert_eq!(v["goal_verified"], false);
        assert_eq!(v["debugger_detached"], true);
        assert_eq!(receipt.acknowledged, kinds.len());
        drop(p);
        h.join().unwrap();
        let c = calls.lock().unwrap();
        let events: Vec<_> = c
            .iter()
            .filter(|q| q["method"] == "Input.dispatchMouseEvent")
            .collect();
        assert_eq!(
            events
                .iter()
                .map(|q| q["params"]["type"].as_str().unwrap())
                .collect::<Vec<_>>(),
            kinds
        );
        assert!(events.iter().all(|q| q["sessionId"] == "flat"
            && q["params"]["x"] == 80.
            && q["params"]["y"] == 60.
            && q["params"]["modifiers"] == 0));
        let hit = c
            .iter()
            .find(|q| q["method"] == "DOM.getNodeForLocation")
            .unwrap();
        assert_eq!(hit["params"]["y"], 260);
        assert_eq!(hit["params"]["ignorePointerEventsNone"], false);
        assert!(
            c.iter()
                .all(|q| !q["method"].as_str().unwrap().starts_with("Runtime.")
                    && q["method"] != "Page.bringToFront")
        );
    }
}
#[test]
fn f16_browser_content_input_disabled_occluded_or_changed_document_send_zero_input() {
    for mode in [
        Mode {
            disabled: true,
            ..Mode::default()
        },
        Mode {
            overlay: true,
            ..Mode::default()
        },
        Mode {
            changed: true,
            ..Mode::default()
        },
    ] {
        let (mut p, calls, h) = fake(mode);
        let mut receipt = Receipt::default();
        assert!(
            collect(
                &mut p,
                "target",
                "context",
                &args(Action::Click),
                &grounding(),
                7,
                &mut receipt
            )
            .is_err()
        );
        assert_eq!(receipt.attempted, 0);
        drop(p);
        h.join().unwrap();
        assert!(
            calls
                .lock()
                .unwrap()
                .iter()
                .all(|q| q["method"] != "Input.dispatchMouseEvent")
        );
    }
}
#[test]
fn f16_browser_content_input_lost_ack_never_resends_or_invents_a_release() {
    for lost_at in 1..=3 {
        let (mut p, calls, h) = fake(Mode {
            lost_at,
            ..Mode::default()
        });
        let mut receipt = Receipt::default();
        assert!(
            collect(
                &mut p,
                "target",
                "context",
                &args(Action::Click),
                &grounding(),
                7,
                &mut receipt
            )
            .is_err()
        );
        assert_eq!(receipt.attempted, lost_at);
        assert_eq!(receipt.acknowledged, lost_at - 1);
        assert!(p.poisoned);
        let id = p.id;
        assert!(
            p.rpc_on(
                Some("flat"),
                "Input.dispatchMouseEvent",
                json!({}),
                Instant::now() + Duration::from_millis(1)
            )
            .is_err()
        );
        assert_eq!(p.id, id);
        drop(p);
        h.join().unwrap();
        let c = calls.lock().unwrap();
        assert_eq!(
            c.iter()
                .filter(|q| q["method"] == "Input.dispatchMouseEvent")
                .count(),
            lost_at
        );
        assert_eq!(c.last().unwrap()["method"], "Input.dispatchMouseEvent");
    }
}
