//! Main-document CDP observation only. No JavaScript, navigation, activation or Input commands.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use rwi_core::device::{DeviceImage, DeviceResult, browser::SnapshotArgs};
use std::collections::BTreeMap;
const MAX_PIXELS: u64 = 8_388_608;
const MAX_DECODE: usize = 32 * 1024 * 1024;

fn target_matches(v: &Value, target: &str, context: &str) -> io::Result<()> {
    let v = &v["targetInfo"];
    if v["targetId"] != target
        || v["browserContextId"] != context
        || v["type"] != "page"
        || v.get("subtype").is_some()
    {
        return Err(invalid("browser_snapshot_target_context_changed"));
    }
    Ok(())
}
fn frame(v: &Value) -> io::Result<Value> {
    let f = &v["frameTree"]["frame"];
    for k in ["id", "loaderId", "url"] {
        if !f[k]
            .as_str()
            .is_some_and(|s| !s.is_empty() && s.len() <= 8192)
        {
            return Err(invalid("browser_snapshot_frame_identity_missing"));
        }
    }
    if f.get("parentId").is_some() {
        return Err(invalid("browser_snapshot_not_root_frame"));
    }
    Ok(json!({"frame_id":f["id"],"loader_id":f["loaderId"],"url":f["url"]}))
}
fn document(v: &Value) -> io::Result<u64> {
    let n = &v["root"];
    if n["nodeType"] != 9 {
        return Err(invalid("browser_snapshot_document_root_missing"));
    }
    n["backendNodeId"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or_else(|| invalid("browser_snapshot_document_identity_missing"))
}
pub(super) fn original_png(v: &Value) -> io::Result<(String, [u32; 2], String)> {
    let b64 = v["data"]
        .as_str()
        .filter(|s| s.len() <= 6 * CAP)
        .ok_or_else(|| invalid("browser_snapshot_image_size_or_shape"))?;
    let raw = STANDARD
        .decode(b64)
        .map_err(|_| invalid("browser_snapshot_image_base64"))?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(&raw));
    decoder.set_limits(png::Limits { bytes: MAX_DECODE });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|_| invalid("browser_snapshot_image_header"))?;
    let size = [reader.info().width, reader.info().height];
    if size.contains(&0)
        || u64::from(size[0]) * u64::from(size[1]) > MAX_PIXELS
        || reader.info().animation_control.is_some()
    {
        return Err(invalid("browser_snapshot_image_dimensions_or_animation"));
    }
    let n = reader
        .output_buffer_size()
        .filter(|n| *n <= MAX_DECODE)
        .ok_or_else(|| invalid("browser_snapshot_image_decode_budget"))?;
    reader
        .next_frame(&mut vec![0; n])
        .map_err(|_| invalid("browser_snapshot_image_decode"))?;
    reader
        .finish()
        .map_err(|_| invalid("browser_snapshot_image_end"))?;
    // Validate once, but return the exact original PNG bytes: no resize/re-encode/upscale.
    Ok((b64.into(), size, rwi_core::digest(&raw)))
}
pub(super) fn rectangles(v: &Value, frame_id: &str) -> io::Result<BTreeMap<u64, Value>> {
    let docs = v["documents"]
        .as_array()
        .ok_or_else(|| invalid("browser_snapshot_layout_documents_missing"))?;
    let strings = v["strings"]
        .as_array()
        .ok_or_else(|| invalid("browser_snapshot_layout_strings_missing"))?;
    let matches: Vec<_> = docs
        .iter()
        .filter(|d| {
            d["frameId"]
                .as_u64()
                .and_then(|i| usize::try_from(i).ok())
                .and_then(|i| strings.get(i))
                .and_then(Value::as_str)
                == Some(frame_id)
        })
        .collect();
    if matches.len() != 1 {
        return Err(invalid("browser_snapshot_layout_frame_mismatch"));
    }
    let d = matches[0];
    let ids = d["nodes"]["backendNodeId"]
        .as_array()
        .ok_or_else(|| invalid("browser_snapshot_layout_nodes_missing"))?;
    let indices = d["layout"]["nodeIndex"]
        .as_array()
        .ok_or_else(|| invalid("browser_snapshot_layout_indices_missing"))?;
    let bounds = d["layout"]["bounds"]
        .as_array()
        .ok_or_else(|| invalid("browser_snapshot_layout_bounds_missing"))?;
    if ids.len() > 16384 || indices.len() != bounds.len() {
        return Err(invalid("browser_snapshot_layout_budget_or_shape"));
    }
    let mut result = BTreeMap::new();
    for (i, b) in indices.iter().zip(bounds) {
        let id = i
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .and_then(|n| ids.get(n))
            .and_then(Value::as_u64)
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("browser_snapshot_layout_node_index"))?;
        let a = b
            .as_array()
            .filter(|a| a.len() == 4)
            .ok_or_else(|| invalid("browser_snapshot_layout_rect_shape"))?;
        if !a.iter().all(|n| n.as_f64().is_some_and(f64::is_finite))
            || a[2].as_f64().unwrap() < 0.
            || a[3].as_f64().unwrap() < 0.
        {
            return Err(invalid("browser_snapshot_layout_rect_invalid"));
        }
        result.entry(id).or_insert_with(|| b.clone());
    }
    Ok(result)
}
/// Chromium emits DOMSnapshot bounds in physical layout pixels. The unrounded
/// legacy/CSS visual-viewport dimensions expose LayoutZoomFactor (DPR + page
/// zoom, not pinch scale). Never infer it from PNG dimensions or OS DPI.
pub(super) fn layout_scale(metrics: &Value) -> io::Result<f64> {
    let dimension = |section: &str, key: &str| {
        metrics[section][key]
            .as_f64()
            .filter(|n| n.is_finite() && *n > 0.)
            .ok_or_else(|| invalid("browser_snapshot_layout_units_unavailable"))
    };
    let x = dimension("visualViewport", "clientWidth")?
        / dimension("cssVisualViewport", "clientWidth")?;
    let y = dimension("visualViewport", "clientHeight")?
        / dimension("cssVisualViewport", "clientHeight")?;
    if !x.is_finite() || !y.is_finite() || x <= 0. || y <= 0. || (x - y).abs() > x.max(y) * 1e-6 {
        return Err(invalid("browser_snapshot_layout_scale_inconsistent"));
    }
    Ok(x)
}
pub(super) fn css_rectangles(
    raw: &BTreeMap<u64, Value>,
    scale: f64,
) -> io::Result<BTreeMap<u64, Value>> {
    if !scale.is_finite() || scale <= 0. {
        return Err(invalid("browser_snapshot_layout_scale_invalid"));
    }
    raw.iter()
        .map(|(id, b)| {
            let rect = b
                .as_array()
                .filter(|a| a.len() == 4)
                .ok_or_else(|| invalid("browser_snapshot_layout_rect_shape"))?;
            let v: io::Result<Vec<f64>> = rect
                .iter()
                .map(|n| {
                    n.as_f64()
                        .map(|n| n / scale)
                        .filter(|n| n.is_finite())
                        .ok_or_else(|| invalid("browser_snapshot_css_rect_invalid"))
                })
                .collect();
            Ok((*id, json!(v?)))
        })
        .collect()
}
fn text(v: &Value, key: &str, truncated: &mut Vec<String>) -> Value {
    let Some(s) = v[key]["value"].as_str() else {
        return v[key]
            .get("value")
            .filter(|v| v.is_boolean() || v.is_number())
            .cloned()
            .unwrap_or(Value::Null);
    };
    if s.len() <= 1024 {
        return json!(s);
    }
    let mut n = 1024;
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    truncated.push(key.into());
    json!(&s[..n])
}
pub(super) fn nodes(v: &Value, rects: &BTreeMap<u64, Value>, limit: u16) -> io::Result<Value> {
    let raw = v["nodes"]
        .as_array()
        .ok_or_else(|| invalid("browser_snapshot_ax_nodes_missing"))?;
    let mut rows = Vec::new();
    let mut eligible = 0;
    for n in raw.iter().filter(|n| n["ignored"] != true) {
        eligible += 1;
        if rows.len() >= usize::from(limit) {
            continue;
        }
        let mut shortened = Vec::new();
        let role = text(n, "role", &mut shortened);
        let name = text(n, "name", &mut shortened);
        let value = text(n, "value", &mut shortened);
        let description = text(n, "description", &mut shortened);
        let id = n["backendDOMNodeId"].as_u64();
        let mut states = BTreeMap::new();
        if let Some(p) = n["properties"].as_array() {
            for p in p.iter().take(64) {
                if let Some(k) = p["name"].as_str().filter(|s| s.len() <= 64) {
                    let val = text(p, "value", &mut shortened);
                    // AXProperty.value is itself an AXValue; text() reads its .value.
                    if !val.is_null() {
                        states.insert(k.to_owned(), val);
                    }
                }
            }
        }
        rows.push(json!({"backend_node_id":id,"role":role,"name":name,"value":value,"description":description,"states":states,"bounds_css_document":id.and_then(|id|rects.get(&id)),"truncated_fields":shortened}));
    }
    Ok(
        json!({"nodes":rows,"coverage":{"main_frame_only":true,"complete":false,"max_ax_depth":12,"raw_ax_nodes":raw.len(),"eligible_nodes":eligible,"returned_nodes":rows.len(),"node_limit_reached":eligible>rows.len(),"max_nodes":limit,"ignored_nodes_excluded":true}}),
    )
}
pub(super) fn collect(
    pipe: &mut Pipe,
    target: &str,
    context: &str,
    args: &SnapshotArgs,
) -> io::Result<DeviceResult> {
    let until = Instant::now() + DEADLINE;
    target_matches(
        &pipe.rpc("Target.getTargetInfo", json!({"targetId":target}), until)?,
        target,
        context,
    )?;
    let attached = pipe.rpc(
        "Target.attachToTarget",
        json!({"targetId":target,"flatten":true}),
        until,
    )?;
    let session = attached["sessionId"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() < 512)
        .ok_or_else(|| invalid("browser_snapshot_session_missing"))?
        .to_owned();
    let observed = (|| {
        let before = frame(&pipe.rpc_on(Some(&session), "Page.getFrameTree", json!({}), until)?)?;
        let root = document(&pipe.rpc_on(
            Some(&session),
            "DOM.getDocument",
            json!({"depth":0,"pierce":false}),
            until,
        )?)?;
        let metrics = pipe.rpc_on(Some(&session), "Page.getLayoutMetrics", json!({}), until)?;
        let scale = layout_scale(&metrics)?;
        let ax = pipe.rpc_on(
            Some(&session),
            "Accessibility.getFullAXTree",
            json!({"depth":12,"frameId":before["frame_id"]}),
            until,
        )?;
        let dom = pipe.rpc_on(
            Some(&session),
            "DOMSnapshot.captureSnapshot",
            json!({"computedStyles":[],"includeDOMRects":true}),
            until,
        )?;
        let raw_rects = rectangles(&dom, before["frame_id"].as_str().unwrap())?;
        let rects = css_rectangles(&raw_rects, scale)?;
        let image = pipe.rpc_on(
            Some(&session),
            "Page.captureScreenshot",
            json!({"format":"png","fromSurface":true,"captureBeyondViewport":false}),
            until,
        )?;
        let (b64, size, hash) = original_png(&image)?;
        let root_after = document(&pipe.rpc_on(
            Some(&session),
            "DOM.getDocument",
            json!({"depth":0,"pierce":false}),
            until,
        )?)?;
        let after = frame(&pipe.rpc_on(Some(&session), "Page.getFrameTree", json!({}), until)?)?;
        let metrics_after =
            pipe.rpc_on(Some(&session), "Page.getLayoutMetrics", json!({}), until)?;
        target_matches(
            &pipe.rpc("Target.getTargetInfo", json!({"targetId":target}), until)?,
            target,
            context,
        )?;
        if root != root_after || before != after {
            return Err(invalid(
                "browser_snapshot_document_changed_observation_discarded",
            ));
        }
        if metrics["visualViewport"] != metrics_after["visualViewport"]
            || metrics["cssVisualViewport"] != metrics_after["cssVisualViewport"]
        {
            return Err(invalid(
                "browser_snapshot_viewport_changed_observation_discarded",
            ));
        }
        let mut data = nodes(&ax, &rects, args.max_nodes)?;
        for node in data["nodes"].as_array_mut().unwrap() {
            node["bounds_document_physical"] = node["backend_node_id"]
                .as_u64()
                .and_then(|id| raw_rects.get(&id))
                .cloned()
                .unwrap_or(Value::Null);
        }
        data["document"] = before;
        data["document"]["backend_document_node"] = json!(root);
        data["viewport_css"] = metrics["cssVisualViewport"].clone();
        data["viewport_legacy"] = metrics["visualViewport"].clone();
        data["coordinate_mapping"] = json!({"source":"Page.getLayoutMetrics legacy/CSS visual-viewport client dimensions","physical_per_css_pixel":scale,"raw_space":"Chromium DOMSnapshot physical document pixels","bounds_css_document":"raw document bounds divided by physical_per_css_pixel","screenshot_or_OS_scale_inferred":false});
        data["image"] = json!({"mime":"image/png","size":size,"sha256":hash,"source":"CDP.Page.captureScreenshot","original_bytes":true,"resampled":false,"browser_chrome_or_OS_dialogs":false});
        data["consistency"] = json!({"document_pre_post_equal":true,"viewport_pre_post_equal":true,"atomic":false,"semantic_content_changed":null,"order":"identity/viewport; AX/layout/PNG; identity/viewport checks"});
        data["input_authority"] = json!(false);
        data["activation_requested"] = json!(false);
        data["goal_verified"] = json!(false);
        let mut r = DeviceResult::value(data);
        r.images.push(DeviceImage {
            mime: "image/png".into(),
            base64: b64,
        });
        Ok(r)
    })();
    // One owned debugger attachment is retired even on an ordinary observation error.
    // An ambiguous/poisoned pipe never sends a detach or alternate command.
    let detach = if !pipe.poisoned {
        pipe.rpc(
            "Target.detachFromTarget",
            json!({"sessionId":session}),
            Instant::now() + Duration::from_millis(600),
        )
    } else {
        Err(invalid("browser_pipe_poisoned_no_detach_replay"))
    };
    match (observed, detach) {
        (Ok(mut r), Ok(_)) => {
            r.data["debugger_detached"] = json!(true);
            Ok(r)
        }
        (Err(e), _) => Err(e),
        (_, Err(e)) => Err(invalid(format!("browser_snapshot_detach_unknown:{e}"))),
    }
}
impl OwnedChrome {
    pub(super) fn content_snapshot(&mut self, args: &SnapshotArgs) -> DeviceResult {
        let result = (|| {
            if self.close_started
                || self.signal.authority_lost
                || !matches!(self.observe_exit(Instant::now()), ExitObservation::Running)
                || birth(self.pid, self.user_uid)? != self.birth
            {
                return Err(invalid("browser_snapshot_owned_child_unavailable"));
            }
            let target = self
                .target
                .as_deref()
                .ok_or_else(|| invalid("browser_snapshot_target_missing"))?;
            let context = self
                .context
                .as_deref()
                .ok_or_else(|| invalid("browser_snapshot_context_missing"))?;
            collect(&mut self.pipe, target, context, args)
        })();
        result.unwrap_or_else(|e| {
            DeviceResult::error(
                "browser_snapshot_failed",
                &format!("{e}; no native fallback, activation, input or automatic replay."),
            )
        })
    }
}
