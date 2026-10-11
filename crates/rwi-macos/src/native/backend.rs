//! Single native-window Backend implementation; policy bodies unchanged.
use super::*;

impl Backend for NativeMac {
    fn local_pixel_anchor(&self) -> bool {
        !self.semantic_only
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        Some(self.dispatches.load(std::sync::atomic::Ordering::Acquire))
    }
    fn identity(&self) -> Identity {
        self.identity.clone()
    }
    fn capabilities(&self) -> serde_json::Value {
        let mut caps = crate::desktop::native_capabilities(
            self.allow_input,
            crate::session::permissions(),
            crate::session::current_facts()
                .ok()
                .and_then(|f| f.console_seat),
        );
        caps["current_foreground"] = activation::foreground_state()
            .ok()
            .and_then(|f| serde_json::to_value(f).ok())
            .unwrap_or(serde_json::Value::Null);
        caps["observation_consistency"] = self
            .last_observation
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default();
        caps["target_scope"] = serde_json::json!(
            "pinned PID + CGWindowID + process start; no implicit foreground requirement"
        );
        // The legacy pinned constructor does not gain dynamic catalog operations.
        caps["window_discovery"]["implemented"] = serde_json::json!(false);
        caps["pointer_feedback"] = crate::overlay::status_json();
        caps["user_input_recent_seconds"] = match crate::activity::seconds_since_user_input() {
            Some(seconds) => serde_json::json!(seconds),
            None => serde_json::Value::Null,
        };
        caps
    }
    fn capture_visual(&mut self, options: VisualOptions) -> Result<VisualCapture> {
        self.visual_capture(options)
    }
    fn observe(&mut self) -> Result<Scene> {
        // Never reuse a previous snapshot's coverage/timings for a failed or
        // visual-only read. Exact input revalidation does not touch this report.
        if let Ok(mut saved) = self.last_observation.lock() {
            *saved = serde_json::Value::Null;
        }
        if self.semantic_only {
            let before = self.capture_guard_metadata()?;
            let after = self.metadata()?;
            if before.binding != after.binding || before.blocked != after.blocked {
                return Err(fault(
                    "semantic_binding_changed",
                    "The exact focused dialog changed during observation.",
                ));
            }
            if let Ok(mut saved) = self.last_observation.lock() {
                *saved = serde_json::json!({"scope":"last_observe","window_context_stable":true,
                    "pixel_semantic_atomic":false,"controls_source":"post_native_metadata",
                    "input_revalidation_required":true,"ax_coverage_before":before.coverage,
                    "ax_coverage_after":after.coverage});
            }
            let mut scene = self.metadata_scene(after, [0, 0], Vec::new(), None);
            scene.pixel_to_desktop = None;
            scene.image_normalization="semantic-only AX dialog; no pixels, visual mapping, or inferred rendering-window identity".into();
            return Ok(scene);
        }
        if !unsafe { ffi::CGPreflightScreenCaptureAccess() } {
            return Err(fault(
                "screen_capture_permission_missing",
                "Screen capture must already be permitted; no permission request is triggered.",
            ));
        }
        // Observation is read-only, including for background applications.
        // Sending an AppKit activation event here can change key-window/chrome
        // state or dismiss a transient panel, despite not moving OS foreground.
        // Any input preparation belongs to an explicitly leased input operation.
        // A window that is mid-animation repaints between the pre/post reads even
        // though nothing actionable changed. Retry the read-only capture a bounded
        // number of times with a short settle wait; the returned frame is still
        // fully revalidated and no input is ever inferred or replayed.
        let mut settled = None;
        for attempt in 0..3 {
            let result = (|| -> Result<Scene> {
                let phase = Instant::now();
                let before = self.capture_guard_metadata()?;
                let before_ms = phase.elapsed().as_millis() as u64;
                let phase = Instant::now();
                let capture = self.capturer.capture(self.pid, self.window, before.rect)?;
                let capture_ms = phase.elapsed().as_millis() as u64;
                let phase = Instant::now();
                let after = self.metadata()?;
                let after_ms = phase.elapsed().as_millis() as u64;
                // A second full tree was previously discarded even though only
                // window/context/modal changes gate read-only capture. Do not
                // claim semantic equality when that comparison is not collected.
                let semantic_content_changed: Option<bool> = None;
                if let Some(error) = capture_consistency_failure(
                    before.binding != after.binding,
                    before.blocked != after.blocked,
                    semantic_content_changed,
                    [before.elements.len(), after.elements.len()],
                    attempt + 1,
                ) {
                    return Err(error);
                }
                if let Ok(mut saved) = self.last_observation.lock() {
                    *saved = serde_json::json!({"scope":"last_observe","window_context_stable":true,
                        "pixel_semantic_atomic":false,"semantic_content_changed":semantic_content_changed,
                        "controls_source":"post_capture_native_metadata","input_revalidation_required":true,
                        "attempts":attempt+1,"before_elements":null,"after_elements":after.elements.len(),"control_tree_walks":1,
                        "metadata_before_ms":before_ms,"capture_ms":capture_ms,"metadata_after_ms":after_ms,
                        "ax_coverage_before":before.coverage,"ax_coverage_after":after.coverage});
                }
                let mut scene = self.metadata_scene(
                    after,
                    capture.size,
                    capture.png,
                    Some(&capture.content_sha),
                );
                scene.atomicity = "native window/context pre/post guarded; pixels precede controls; one post-capture tree, semantic comparison not collected; non-atomic, every input revalidates its original target".into();
                Ok(scene)
            })();
            match result {
                Ok(scene) => return Ok(scene),
                Err(error) if visual::ax_unavailable_for_visual(&error) => {
                    return self.observe_visual_only(&error);
                }
                Err(error)
                    if attempt < 2
                        && (error.code == "capture_binding_changed"
                            || ffi::observation_read_can_settle(&error)) =>
                {
                    settled = Some(error);
                    std::thread::sleep(std::time::Duration::from_millis(150));
                }
                Err(error) => return Err(error),
            }
        }
        Err(settled.expect("capture settle attempts exhausted"))
    }
    fn revalidate(&mut self, reference: &Resolved) -> Result<Scene> {
        if reference.source_region.is_some() {
            return Err(fault(
                "capability_unavailable",
                "Native source-region capture/revalidation is not implemented on macOS; no full-window substitution.",
            ));
        }
        if reference.scope.element.is_none() {
            return self.observe();
        }
        Ok(self.metadata_scene(
            self.metadata_for(Some(reference))?,
            [0, 0],
            Vec::new(),
            None,
        ))
    }

    fn feedback(&mut self, visible: bool, point: Option<[f64; 2]>) -> Result<serde_json::Value> {
        if visible {
            let point = match point {
                Some(point) => point,
                None => crate::overlay::last_point().ok_or_else(|| {
                    fault(
                        "invalid_arguments",
                        "Provide point on the first visible request.",
                    )
                })?,
            };
            if point.iter().any(|v| !v.is_finite()) {
                return Err(fault("invalid_arguments", "Point must be finite."));
            }
            crate::overlay::show_at(point);
        } else {
            crate::overlay::hide();
        }
        Ok(crate::overlay::status_json())
    }
    fn popups(&mut self, _scope: &rwi_core::Scope) -> Result<Vec<rwi_core::Popup>> {
        Ok(crate::native::popups_for_pid(self.pid))
    }
    fn focus_window(
        &mut self,
        r: &Resolved,
        f: &rwi_core::ForegroundState,
    ) -> Result<serde_json::Value> {
        self.focus_exact_window(r, f)
    }
    fn move_window(
        &mut self,
        reference: &Resolved,
        display: &rwi_core::DisplayTarget,
    ) -> Result<serde_json::Value> {
        if reference.scope.pid != self.pid || reference.scope.window != self.window_name() {
            return Err(fault(
                "target_mismatch",
                "Reference belongs to another native window; observe this exact one.",
            ));
        }
        let target = crate::native::displays()?
            .into_iter()
            .find(|candidate| {
                display.matches(candidate.built_in, candidate.id, candidate.uuid.as_deref())
            })
            .ok_or_else(|| {
                fault(
                    "display_not_found",
                    "No active display matches the requested target; nothing was moved.",
                )
            })?;
        let requested_origin = self.place_window_on(&target)?;
        Ok(serde_json::json!({
            "window": self.window_name(),
            "display": {"id": target.id, "uuid": target.uuid, "built_in": target.built_in},
            "requested_origin": requested_origin,
            "input_sent": false,
            "activated": false,
        }))
    }
    fn act(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        intent: &Intent,
        value: Option<&str>,
    ) -> Result<Option<bool>> {
        let result = self.dispatch_input(expected, target, intent, value);
        if result.is_ok() {
            // Purely visual feedback for the user; never input and never the
            // real cursor. Failure to draw is not an input failure.
            crate::overlay::show_at(target.point);
        }
        result
    }
    fn inspect(&mut self, scope: &Scope, predicate: &Predicate, expected: &str) -> Result<Oracle> {
        if scope.pid != self.pid || scope.window != self.window_name() {
            return Err(fault(
                "verification_scope_mismatch",
                "Oracle is pinned to another window/PID.",
            ));
        }
        let meta = self.metadata()?;
        let matched = match predicate {
            Predicate::WindowTitle => meta.title == expected,
            Predicate::FocusedWindow => meta.binding.focus_window == expected,
            Predicate::ElementValue => {
                let id = scope
                    .element
                    .as_ref()
                    .ok_or_else(|| fault("element_required", "Select an observed element."))?;
                let el = meta
                    .elements
                    .iter()
                    .find(|e| &e.element.id == id)
                    .ok_or_else(|| {
                        fault(
                            "element_changed",
                            "Exact element is absent; same value elsewhere is not success.",
                        )
                    })?;
                verify_value(el, expected)?
            }
            Predicate::Modal => match meta.modal {
                Some(true) => meta.title == expected,
                Some(false) => expected == "none",
                None => {
                    return Err(fault(
                        "modal_state_unknown",
                        "Native AXModal is unavailable; do not infer success from absence.",
                    ));
                }
            },
        };
        Ok(Oracle {
            identity: meta.identity,
            scope: scope.clone(),
            assurance: "native-attested".into(),
            matched,
            source: "independent-AX-readback; no screenshot/input receipt inference".into(),
        })
    }
}
