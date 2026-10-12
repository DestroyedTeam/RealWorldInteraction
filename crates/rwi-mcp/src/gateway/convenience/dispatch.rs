use super::*;

impl Gateway {
    pub(in crate::gateway) async fn convenience(
        &self,
        name: &str,
        mut args: Value,
        context: RequestContext<RoleServer>,
    ) -> DeviceResult {
        let started = Instant::now();
        let include_timings = match take_include_timings(&mut args) {
            Ok(value) => value,
            Err(error) => return error,
        };
        let progress = progress::Progress::new(context.clone());
        let timings = CallTimings::default();
        progress.emit("Request accepted").await;
        let mut result = self
            .convenience_inner(name, args, context, &progress, &timings)
            .await;
        progress
            .emit(if result.is_error {
                "Request ended with an error; inspect receipt"
            } else {
                "Request returned; verify application state"
            })
            .await;
        let timing_rows = timings.rows();
        result.data["stage_timings"] = timing_rows;
        result.data["elapsed_ms"] = json!(started.elapsed().as_millis() as u64);
        let include_timing_output = expose_timings(name, &result, include_timings);
        // Retain full diagnostics before shaping the default reply. Successful
        // callers can request inline timing; errors/uncertainty keep it by default.
        if let Ok(mut cache) = self.presentation.lock() {
            let reference = cache.retain(name, result.data.clone());
            if compactable_receipt(&result)
                && !matches!(name, "computer.snapshot" | "computer.controls")
            {
                result.data = compact_action_reply(&result.data, reference);
            } else {
                result.data["details_ref"] = json!(reference);
            }
        }
        if !include_timing_output {
            trim_timing_fields(&mut result.data);
        }
        result
    }
    async fn convenience_inner(
        &self,
        name: &str,
        args: Value,
        context: RequestContext<RoleServer>,
        progress: &progress::Progress,
        timings: &CallTimings,
    ) -> DeviceResult {
        let call = |tool, args, cleanup: bool| {
            let context = context.clone();
            async move {
                if !cleanup {
                    progress.emit(progress::stage(tool)).await;
                }
                let began = Instant::now();
                let result = self
                    .invoke(tool, args, if cleanup { None } else { Some(context) })
                    .await;
                timings.record(tool, began.elapsed());
                if cleanup {
                    progress
                        .emit("Lease release request returned; inspect receipt")
                        .await;
                }
                result
            }
        };
        match name {
            "computer.hover" => {
                let args = match serde_json::from_value::<HoverArgs>(args) {
                    Ok(args) => args,
                    Err(error) => return invalid(&error.to_string()),
                };
                hover(args, call).await
            }
            "computer.scroll" => {
                let args = match serde_json::from_value::<ScrollArgs>(args) {
                    Ok(args) => args,
                    Err(error) => return invalid(&error.to_string()),
                };
                scroll(args, call).await
            }
            "computer.place_window" => {
                let args = match serde_json::from_value::<PlaceWindowArgs>(args) {
                    Ok(args) => args,
                    Err(error) => return invalid(&error.to_string()),
                };
                place_window(args, call).await
            }
            "computer.controls" => {
                let a = match serde_json::from_value::<ControlsArgs>(args) {
                    Ok(a)
                        if a.query.len() <= 512
                            && (1..=128).contains(&a.limit)
                            && a.offset <= 65536 =>
                    {
                        a
                    }
                    _ => return invalid("Use query <=512 bytes, limit 1..128 and bounded offset."),
                };
                let mut cache = match self.presentation.lock() {
                    Ok(cache) => cache,
                    Err(_) => {
                        return DeviceResult::error(
                            "presentation_unavailable",
                            "Retained observation cache unavailable.",
                        );
                    }
                };
                let rows = match cache.observation_rows(&a.observation_ref, &a.frame) {
                    Ok(rows) => rows,
                    Err(error) => return error,
                };
                control_page(rows, &a)
            }
            "computer.snapshot" => {
                let a = match serde_json::from_value::<SnapshotArgs>(args) {
                    Ok(a) if (64..=2048).contains(&a.max_edge) && (1..=128).contains(&a.limit) => a,
                    _ => return invalid("Use max_edge 64..2048 and limit 1..128."),
                };
                let observed = call(
                    "computer.observe",
                    json!({"target":a.target,"inline_image":false}),
                    false,
                )
                .await;
                if observed.is_error {
                    if !read_only_ax_visual_candidate(&observed) {
                        return observed;
                    }
                    // A separate exact-current-binding visual read. It neither
                    // repairs AX, retries input, nor creates a usable input frame.
                    let visual = call(
                        "computer.capture_visual",
                        json!({"target":a.target,"max_edge":a.max_edge,"feedback":"exclude"}),
                        false,
                    )
                    .await;
                    return degraded_ax_snapshot(&a.target, observed, visual);
                }
                let d = &observed.data;
                let details = self
                    .presentation
                    .lock()
                    .ok()
                    .and_then(|mut c| c.retain("computer.observe", d.clone()));
                let image = if d["artifact_id"].is_null()
                    && d["scene"]["image_size"] == json!([0, 0])
                {
                    DeviceResult::value(
                        json!({"view_id":null,"mapping":null,"input_reference_available":false}),
                    )
                } else {
                    call("computer.image", json!({"target":a.target,"frame":d["frame"],"artifact_id":d["artifact_id"],"max_edge":a.max_edge}), false).await
                };
                if image.is_error {
                    return DeviceResult {
                        is_error: true,
                        data: json!({"code":"snapshot_preview_failed","observation_ref":details,"frame":d["frame"],"preview_error":image.data,"goal_verified":false}),
                        images: vec![],
                    };
                }
                let rows = d["scene"]["elements"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                let (elements, total) = compact_controls(&rows, a.limit);
                let omitted = total.saturating_sub(elements.len());
                let (view_id, input_reference_available) =
                    snapshot_input_view(&d["scene"], &image.data);
                DeviceResult {
                    is_error: false,
                    data: json!({"target":a.target,"window":snapshot_window(d),"consistency":d["adapter_status"]["observation_consistency"],"frame":d["frame"],"view_id":view_id,"mapping":image.data["mapping"],"visual_available":!d["artifact_id"].is_null(),"visual_only":d["scene"]["binding"].is_null() && !d["artifact_id"].is_null(),"assurance":d["scene"]["assurance"],"input_reference_available":input_reference_available,"elements":elements,"available_controls":total,"omitted_controls":omitted,"control_projection":"preferred controls; unnamed unavailable containers may be collapsed; computer.controls keeps full original pages","coverage":d["adapter_status"]["observation_consistency"]["ax_coverage_after"],"native_complete":snapshot_native_complete(d),"reference_max_age_ms":d["adapter_status"]["input_proposal_max_age_ms"],"observation_ref":details,"blocked":d["scene"]["blocked"],"mutation_ready":snapshot_mutation_ready(d),"input_domain_fenced":d["input_domain_fenced"]}),
                    images: image.images,
                }
            }
            "computer.click" => {
                let a = match serde_json::from_value::<ClickArgs>(args) {
                    Ok(a) => a,
                    Err(e) => return invalid(&e.to_string()),
                };
                let has_action = matches!(
                    &a.at,
                    ClickTarget::Element {
                        action: Some(_),
                        ..
                    }
                );
                let intent = match click_intent(a.click_count, has_action) {
                    Ok(intent) => intent,
                    Err(error) => return error,
                };
                let (tool, args) = match a.at {
                    ClickTarget::Element {
                        frame,
                        element,
                        action,
                    } => (
                        "computer.act_checked",
                        json!({"frame":frame,"element":element,"intent":intent,"value":action}),
                    ),
                    ClickTarget::View {
                        view_id,
                        x,
                        y,
                        red_marker_cluster,
                    } => {
                        if !x.is_finite() || !y.is_finite() {
                            return invalid("Coordinates must be finite.");
                        }
                        let r = call(
                            "computer.resolve_view",
                            view_resolution_args(&a.target, &view_id, x, y, red_marker_cluster),
                            false,
                        )
                        .await;
                        if r.is_error {
                            return r;
                        }
                        (
                            "computer.act",
                            json!({"reference":r.data["reference"],"intent":intent}),
                        )
                    }
                };
                leased(&a.target, &a.request_id, tool, args, call).await
            }
            "computer.reveal" => {
                let a = match serde_json::from_value::<RevealArgs>(args) {
                    Ok(a) => a,
                    Err(e) => return invalid(&e.to_string()),
                };
                reveal(a, call).await
            }
            "computer.type_text" => {
                let a = match serde_json::from_value::<TextArgs>(args) {
                    Ok(a) => a,
                    Err(e) => return invalid(&e.to_string()),
                };
                let args = match keyboard_action(a.frame, a.element, a.text, "text", 4096) {
                    Ok(args) => args,
                    Err(e) => return e,
                };
                leased(&a.target, &a.request_id, "computer.act_checked", args, call).await
            }
            "computer.press_key" => {
                let a = match serde_json::from_value::<KeyArgs>(args) {
                    Ok(a) => a,
                    Err(e) => return invalid(&e.to_string()),
                };
                let args = match keyboard_action(a.frame, a.element, a.key, "key_chord", 128) {
                    Ok(args) => args,
                    Err(e) => return e,
                };
                leased(&a.target, &a.request_id, "computer.act_checked", args, call).await
            }
            "computer.drag" => {
                let a = match serde_json::from_value::<DragArgs>(args) {
                    Ok(a) => a,
                    Err(e) => return invalid(&e.to_string()),
                };
                if !(16..=5000).contains(&a.duration_ms)
                    || !a.from.iter().chain(a.to.iter()).all(|n| n.is_finite())
                {
                    return invalid("Use finite view coordinates and duration_ms 16..5000.");
                }
                let mut points = Vec::new();
                let mut reference = Value::Null;
                for point in [a.from, a.to] {
                    let r = call(
                        "computer.resolve_view",
                        json!({"target":a.target,"view_id":a.view_id,"x":point[0],"y":point[1]}),
                        false,
                    )
                    .await;
                    if r.is_error {
                        return r;
                    }
                    let p = match serde_json::from_value::<[f64; 2]>(r.data["point"].clone()) {
                        Ok(p) if p.iter().all(|n| n.is_finite()) => p,
                        _ => {
                            return DeviceResult::error(
                                "invalid_mapping_reply",
                                "No input dispatched.",
                            );
                        }
                    };
                    if points.is_empty() {
                        reference = r.data["reference"].clone();
                    }
                    points.push(p);
                }
                let offset = [points[1][0] - points[0][0], points[1][1] - points[0][1]];
                leased(&a.target,&a.request_id,"computer.act",json!({"reference":reference,"intent":{"drag":{"waypoints":[{"at_ms":0,"offset":[0,0]},{"at_ms":a.duration_ms,"offset":offset}]}}}),call).await
            }
            "computer.drag_between" => {
                let a: DragBetweenArgs = match serde_json::from_value(args) {
                    Ok(a) => a,
                    Err(_) => return invalid("Use the advertised paired endpoint schema."),
                };
                if let Err(error) = validate_drag_between(&a) {
                    return error;
                }
                if let Err(error) = self.paired_device(&a) {
                    return error;
                }
                paired_drag(a, call).await
            }
            _ => invalid("Unknown convenience tool."),
        }
    }
}

// Invocation-local bounded labels, never arguments/text or a global history.
// This is adapter wall time around invoke, not native CPU/capture time.
#[derive(Default)]
pub(super) struct CallTimings(Mutex<Vec<(&'static str, u64, std::time::Duration)>>);
impl CallTimings {
    pub(super) fn record(&self, tool: &'static str, elapsed: std::time::Duration) {
        if let Ok(mut rows) = self.0.lock() {
            if let Some(row) = rows.iter_mut().find(|r| r.0 == tool) {
                row.1 += 1;
                row.2 += elapsed;
            } else {
                rows.push((tool, 1, elapsed));
            }
        }
    }
    pub(super) fn rows(&self) -> Value {
        match self.0.lock() {
            Ok(rows) => {
                json!(rows.iter().map(|(tool, calls, elapsed)| {
                json!({"tool":tool,"calls":calls,"elapsed_ms":elapsed.as_millis() as u64})
            }).collect::<Vec<_>>())
            }
            Err(_) => Value::Null,
        }
    }
}
