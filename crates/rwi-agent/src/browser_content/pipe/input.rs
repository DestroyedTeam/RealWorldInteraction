//! Page events only: fresh document/layout/hit checks, never OS input or JavaScript.
use super::*;
use crate::browser_content::input::Grounding;
use rwi_core::device::{
    DeviceResult,
    browser::{ActArgs, Action},
};
use snapshot::{css_rectangles, document, frame, layout_scale, rectangles, target_matches};

pub(super) struct Point {
    pub document: [i32; 2],
    pub viewport: [f64; 2],
}
pub(super) fn point(rect: &Value, viewport: &Value) -> io::Result<Point> {
    let number = |v: &Value| {
        v.as_f64()
            .filter(|n| n.is_finite())
            .ok_or_else(|| invalid("browser_input_coordinate_invalid"))
    };
    let r = rect
        .as_array()
        .filter(|r| r.len() == 4)
        .ok_or_else(|| invalid("browser_input_rectangle_missing"))?;
    let [x, y, w, h] = [
        number(&r[0])?,
        number(&r[1])?,
        number(&r[2])?,
        number(&r[3])?,
    ];
    let px = number(&viewport["pageX"])?;
    let py = number(&viewport["pageY"])?;
    let width = number(&viewport["clientWidth"])?;
    let height = number(&viewport["clientHeight"])?;
    if number(&viewport["scale"])? != 1.
        || number(&viewport["offsetX"])? != 0.
        || number(&viewport["offsetY"])? != 0.
    {
        return Err(invalid("browser_input_visual_pinch_or_offset_unsupported"));
    }
    if w <= 0. || h <= 0. || width <= 0. || height <= 0. || px < 0. || py < 0. {
        return Err(invalid("browser_input_geometry_invalid"));
    }
    let left = x.max(px);
    let top = y.max(py);
    let right = (x + w).min(px + width);
    let bottom = (y + h).min(py + height);
    if ![left, top, right, bottom].iter().all(|n| n.is_finite()) || right <= left || bottom <= top {
        return Err(invalid("browser_element_outside_viewport_no_scroll"));
    }
    // DOM.getNodeForLocation uses document CSS integers; Input uses viewport CSS.
    // Chromium's DOM agent applies LayoutZoomFactor then DocumentToFrame. Never
    // use the original PNG pixels, OS display coordinates or DPR for this mapping.
    let dx = ((left + right) / 2.).floor();
    let dy = ((top + bottom) / 2.).floor();
    if dx < left
        || dx >= right
        || dy < top
        || dy >= bottom
        || dx > f64::from(i32::MAX)
        || dy > f64::from(i32::MAX)
        || dx < 0.
        || dy < 0.
    {
        return Err(invalid("browser_input_no_integer_hit_point"));
    }
    Ok(Point {
        document: [dx as i32, dy as i32],
        viewport: [dx - px, dy - py],
    })
}
pub(super) fn hit_in_subtree(dom: &Value, frame: &str, target: u64, hit: u64) -> bool {
    let Some(strings) = dom["strings"].as_array() else {
        return false;
    };
    let Some(docs) = dom["documents"].as_array() else {
        return false;
    };
    let mut docs = docs.iter().filter(|d| {
        d["frameId"]
            .as_u64()
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| strings.get(i))
            .and_then(Value::as_str)
            == Some(frame)
    });
    let Some(doc) = docs.next() else { return false };
    if docs.next().is_some() {
        return false;
    }
    let (Some(ids), Some(parents), Some(types)) = (
        doc["nodes"]["backendNodeId"].as_array(),
        doc["nodes"]["parentIndex"].as_array(),
        doc["nodes"]["nodeType"].as_array(),
    ) else {
        return false;
    };
    if ids.len() > 16384 || ids.len() != parents.len() || ids.len() != types.len() {
        return false;
    }
    let Some(target_index) = ids.iter().position(|n| n.as_u64() == Some(target)) else {
        return false;
    };
    // Never turn a document/text reference into an arbitrary containing element.
    if types[target_index] != 1 {
        return false;
    }
    let Some(mut at) = ids.iter().position(|n| n.as_u64() == Some(hit)) else {
        return false;
    };
    for _ in 0..64 {
        if at == target_index {
            return true;
        }
        let Some(parent) = parents[at].as_u64().and_then(|i| usize::try_from(i).ok()) else {
            return false;
        };
        if parent >= ids.len() || parent == at {
            return false;
        }
        at = parent;
    }
    false
}
pub(super) fn enabled(ax: &Value, node: u64) -> bool {
    ax["nodes"]
        .as_array()
        .and_then(|nodes| {
            nodes
                .iter()
                .find(|n| n["backendDOMNodeId"].as_u64() == Some(node))
        })
        .is_some_and(|n| {
            n["ignored"] == false
                && n["properties"].as_array().is_some_and(|p| {
                    !p.iter()
                        .any(|p| p["name"] == "disabled" && p["value"]["value"] == true)
                })
        })
}
#[derive(Default)]
pub(super) struct Receipt {
    pub attempted: usize,
    pub acknowledged: usize,
}
pub(super) fn collect(
    pipe: &mut Pipe,
    target: &str,
    context: &str,
    args: &ActArgs,
    g: &Grounding,
    node: u64,
    receipt: &mut Receipt,
) -> io::Result<Value> {
    let until = Instant::now() + DEADLINE;
    target_matches(
        &pipe.rpc("Target.getTargetInfo", json!({"targetId":target}), until)?,
        target,
        context,
    )?;
    let a = pipe.rpc(
        "Target.attachToTarget",
        json!({"targetId":target,"flatten":true}),
        until,
    )?;
    let session = a["sessionId"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() < 512)
        .ok_or_else(|| invalid("browser_input_session_missing"))?
        .to_owned();
    let result = (|| {
        let f = frame(&pipe.rpc_on(Some(&session), "Page.getFrameTree", json!({}), until)?)?;
        let root = document(&pipe.rpc_on(
            Some(&session),
            "DOM.getDocument",
            json!({"depth":0,"pierce":false}),
            until,
        )?)?;
        if ["frame_id", "loader_id", "url"]
            .iter()
            .any(|k| f[k] != g.document[k])
            || g.document["backend_document_node"].as_u64() != Some(root)
        {
            return Err(invalid("browser_input_document_changed"));
        }
        let metrics = pipe.rpc_on(Some(&session), "Page.getLayoutMetrics", json!({}), until)?;
        if metrics["cssVisualViewport"] != g.viewport || metrics["visualViewport"] != g.legacy {
            return Err(invalid("browser_input_viewport_changed_take_new_snapshot"));
        }
        let scale = layout_scale(&metrics)?;
        let dom = pipe.rpc_on(
            Some(&session),
            "DOMSnapshot.captureSnapshot",
            json!({"computedStyles":[],"includeDOMRects":true}),
            until,
        )?;
        let fid = f["frame_id"].as_str().unwrap();
        let rects = css_rectangles(&rectangles(&dom, fid)?, scale)?;
        let p = point(
            rects
                .get(&node)
                .ok_or_else(|| invalid("browser_input_element_not_laid_out"))?,
            &g.viewport,
        )?;
        let ax = pipe.rpc_on(
            Some(&session),
            "Accessibility.getPartialAXTree",
            json!({"backendNodeId":node,"fetchRelatives":false}),
            until,
        )?;
        if !enabled(&ax, node) {
            return Err(invalid("browser_input_element_disabled_or_ax_unknown"));
        }
        let hit=pipe.rpc_on(Some(&session),"DOM.getNodeForLocation",json!({"x":p.document[0],"y":p.document[1],"includeUserAgentShadowDOM":false,"ignorePointerEventsNone":false}),until)?;
        if hit["frameId"] != f["frame_id"]
            || !hit["backendNodeId"]
                .as_u64()
                .is_some_and(|id| hit_in_subtree(&dom, fid, node, id))
        {
            return Err(invalid("browser_input_hit_target_changed_or_occluded"));
        }
        // Adjacent identity/viewport reads before first input. This is not an atomic
        // compositor transaction; page script can still react between individual events.
        let f2 = frame(&pipe.rpc_on(Some(&session), "Page.getFrameTree", json!({}), until)?)?;
        let root2 = document(&pipe.rpc_on(
            Some(&session),
            "DOM.getDocument",
            json!({"depth":0,"pierce":false}),
            until,
        )?)?;
        let m2 = pipe.rpc_on(Some(&session), "Page.getLayoutMetrics", json!({}), until)?;
        if f2 != f
            || root2 != root
            || m2["cssVisualViewport"] != g.viewport
            || m2["visualViewport"] != g.legacy
        {
            return Err(invalid("browser_input_preflight_changed"));
        }
        let sequence: &[&str] = if args.action == Action::Hover {
            &["mouseMoved"]
        } else {
            &["mouseMoved", "mousePressed", "mouseReleased"]
        };
        for kind in sequence {
            if Instant::now() >= g.expires {
                return Err(invalid("browser_input_observation_expired"));
            }
            let down = *kind == "mousePressed";
            let mut value = json!({"type":kind,"x":p.viewport[0],"y":p.viewport[1],"modifiers":0,"buttons":if down{1}else{0},"pointerType":"mouse"});
            if *kind != "mouseMoved" {
                value["button"] = json!("left");
                value["clickCount"] = json!(1)
            }
            receipt.attempted += 1;
            pipe.rpc_on(Some(&session), "Input.dispatchMouseEvent", value, until)?;
            receipt.acknowledged += 1;
        }
        Ok(
            json!({"state":"input_sent","action":args.action,"element_ref":args.element_ref,"observation_consumed":true,"point_css_viewport":p.viewport,"hit_css_document":p.document,"input_events_acknowledged":receipt.acknowledged,"atomic":false,"goal_verified":false,"native_input":false,"os_cursor_moved":false,"activation_requested":false}),
        )
    })();
    let detached = if !pipe.poisoned {
        pipe.rpc(
            "Target.detachFromTarget",
            json!({"sessionId":session}),
            Instant::now() + Duration::from_millis(600),
        )
    } else {
        Err(invalid("browser_pipe_poisoned_no_detach_replay"))
    };
    match (result, detached) {
        (Ok(mut v), Ok(_)) => {
            v["debugger_detached"] = json!(true);
            Ok(v)
        }
        (Err(e), _) => Err(e),
        (_, Err(e)) => Err(invalid(format!("browser_input_detach_unknown:{e}"))),
    }
}
impl OwnedChrome {
    pub(super) fn content_act(&mut self, args: &ActArgs, g: &Grounding, node: u64) -> DeviceResult {
        let start = Instant::now();
        let mut receipt = Receipt::default();
        let result = (|| {
            if self.close_started
                || self.content_input_uncertain
                || self.signal.authority_lost
                || !matches!(self.observe_exit(Instant::now()), ExitObservation::Running)
                || birth(self.pid, self.user_uid)? != self.birth
            {
                return Err(invalid(
                    "browser_input_owned_child_unavailable_or_uncertain",
                ));
            }
            let target = self
                .target
                .as_deref()
                .ok_or_else(|| invalid("browser_input_target_missing"))?;
            let context = self
                .context
                .as_deref()
                .ok_or_else(|| invalid("browser_input_context_missing"))?;
            collect(&mut self.pipe, target, context, args, g, node, &mut receipt)
        })();
        let mut r = match result {
            Ok(v) => DeviceResult::value(v),
            Err(e) => {
                if receipt.attempted > 0 {
                    self.content_input_uncertain = true
                }
                DeviceResult::error(
                    if receipt.attempted > 0 {
                        "browser_input_unknown"
                    } else {
                        "browser_input_not_dispatched"
                    },
                    &format!(
                        "{e}; query ORIGINAL browser.session request_id. No release/input replay or Native fallback."
                    ),
                )
            }
        };
        r.data["input_events_attempted"] = json!(receipt.attempted);
        r.data["input_events_acknowledged"] = json!(receipt.acknowledged);
        r.data["input_resource_uncertain"] = json!(self.content_input_uncertain);
        r.data["dispatch_wall_ms"] = json!(start.elapsed().as_millis());
        r
    }
}
