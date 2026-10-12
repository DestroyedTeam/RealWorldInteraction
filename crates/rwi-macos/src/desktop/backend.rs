//! Native desktop trait dispatch; selection helpers remain in the owner module.
use super::*;

impl Backend for NativeMacDesktop {
    fn local_pixel_anchor(&self) -> bool {
        self.selected
            .as_ref()
            .is_some_and(|selected| !selected.semantic_only)
    }
    fn identity(&self) -> Identity {
        self.identity.clone()
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        Some(self.dispatches.load(Ordering::Acquire))
    }
    fn capabilities(&self) -> Value {
        let mut caps = native_capabilities(
            self.allow_input,
            session::permissions(),
            session::current_facts().ok().and_then(|f| f.console_seat),
        );
        caps["target_scope"] =
            json!("configured UID + current native graphical security session, no initial window");
        caps["window_attachment"] = json!({"implemented":!self.fixed_attachment,"fixed_binding":self.fixed_attachment,
            "selection":"fresh exact window/PID/process fingerprint","activation":false,"placement":false,
            "independent_references":true,"physical_lease_shared":true});
        caps["window_selected"] = json!(self.selected.is_some());
        caps["foreground_keyboard"] = json!({
            "implemented":!self.fixed_attachment,
            "authorized":self.allow_input,"route":"explicit CGEventPost(HID)",
            "scope":"original desktop + current native foreground tuple + shared input lease",
            "background_fallback":false,"paired_modifier_key_release":true,
            "native_acknowledgement":false,"hardware_human_origin_attested":false
        });
        caps["semantic_only"] = json!(
            self.selected
                .as_ref()
                .is_some_and(|selected| selected.semantic_only)
        );
        if let Some(selected) = &self.selected {
            caps["observation_consistency"] = selected
                .last_observation
                .lock()
                .map(|v| v.clone())
                .unwrap_or_default();
        }
        if let Some(selected) = &self.selected
            && let Ok(rows) = selected.hit_diagnostics.lock()
        {
            caps["ax_hit_diagnostics"] =
                json!({"scope":"last_metadata_read", "limit":8,"mismatches":*rows});
        }
        caps["pointer_feedback"] = crate::overlay::status_json();
        caps["current_foreground"] = crate::native::activation::foreground_state()
            .ok()
            .and_then(|f| serde_json::to_value(f).ok())
            .unwrap_or(Value::Null);
        caps["user_input_recent_seconds"] = match crate::activity::seconds_since_user_input() {
            Some(seconds) => json!(seconds),
            None => Value::Null,
        };
        caps
    }
    fn runtime_status(&self) -> Value {
        session::process_status(self.uid)
    }
    fn feedback(&mut self, visible: bool, point: Option<[f64; 2]>) -> Result<Value> {
        feedback_overlay(visible, point)
    }
    fn popups(&mut self, scope: &rwi_core::Scope) -> Result<Vec<rwi_core::Popup>> {
        let mut rows = crate::native::popups_for_pid(scope.pid);
        // Fill AX gaps with the window server's own title/geometry; never guess.
        let native = windows().unwrap_or_default();
        for row in &mut rows {
            let Some(id) = row
                .window
                .as_deref()
                .and_then(|w| w.strip_prefix("cgwindow:"))
                .and_then(|n| n.parse::<u32>().ok())
            else {
                continue;
            };
            if let Some(window) = native.iter().find(|w| w.id == id && w.pid == scope.pid) {
                if row.title.is_none() {
                    row.title = window.title.clone();
                }
                if row.bounds.is_none() {
                    row.bounds = Some([window.rect.x, window.rect.y, window.rect.w, window.rect.h]);
                }
            }
        }
        let displays = crate::native::displays().ok();
        for row in &mut rows {
            if let (Some(bounds), Some(list)) = (row.bounds, displays.as_ref()) {
                let rect = crate::geometry::Rect {
                    x: bounds[0],
                    y: bounds[1],
                    w: bounds[2],
                    h: bounds[3],
                };
                row.display = crate::native::containing_display(list, rect).map(|display| {
                    serde_json::to_value(crate::catalog::DisplayRecord::from_native(display))
                        .unwrap_or(Value::Null)
                });
            }
        }
        Ok(rows)
    }
    fn discover(&mut self) -> Result<Value> {
        self.discover_with(&DiscoveryQuery::default())
    }
    fn discover_with(&mut self, query: &DiscoveryQuery) -> Result<Value> {
        query.validate()?;
        self.check_identity()?;
        if query.cursor.is_some() {
            return self.catalog.page(query.cursor.as_deref());
        }
        let inventory = window_inventory()?;
        let count = inventory.native_rows;
        let native = inventory.windows;
        let display_list = crate::native::displays().ok();
        let display_records = display_list.as_ref().map(|list| {
            list.iter()
                .map(crate::catalog::DisplayRecord::from_native)
                .collect::<Vec<_>>()
        });
        let display_uuid_complete = display_list
            .as_ref()
            .is_some_and(|list| list.iter().all(|display| display.uuid.is_some()));
        let missing_titles = native.iter().filter(|w| w.title.is_none()).count();
        let mut rows = Vec::new();
        let mut unknown = inventory.invalid_rows;
        for window in native {
            operation::checkpoint("macos_discovery_row")?;
            if !matches_query(&window, query) {
                continue;
            }
            let display = display_list
                .as_ref()
                .and_then(|list| crate::native::containing_display(list, window.rect));
            match process_epoch(window.pid).and_then(|process| {
                WindowRecord::from_native(&self.identity, &window, process, display)
            }) {
                Ok(row) => rows.push(row),
                Err(error) if error.code == "process_user_mismatch" => (),
                Err(_) => unknown += 1,
            }
        }
        let mut semantic_probe = Value::Null;
        if let Some(pid) = query.pid {
            match crate::native::focused_dialog_window(pid) {
                Ok(Some(window))
                    if !rows
                        .iter()
                        .any(|row| row.window == format!("cgwindow:{}", window.id))
                        && matches_query(&window, query) =>
                {
                    let display = display_list
                        .as_ref()
                        .and_then(|list| crate::native::containing_display(list, window.rect));
                    rows.push(
                        WindowRecord::from_native(
                            &self.identity,
                            &window,
                            process_epoch(pid)?,
                            display,
                        )?
                        .logical(),
                    );
                    semantic_probe =
                        json!({"complete":true,"focused_dialog_added":true,"pixels":false});
                }
                Ok(_) => semantic_probe = json!({"complete":true,"focused_dialog_added":false}),
                Err(error) => semantic_probe = json!({"complete":false,"code":error.code}),
            }
        }
        self.check_identity()?;
        self.catalog.replace(rows,json!({
            "identity":self.identity,"filter":query,"native_windows_enumerated":count,
            "displays":display_records,"display_catalog_complete":display_list.is_some(),
            "display_uuid_complete":display_uuid_complete,
            "semantic_dialog_probe":semantic_probe,
            "unknown_or_invalid_rows":unknown,"unavailable_native_titles":missing_titles,
            "enumeration_complete":unknown==0 && (query.title_contains.is_none() || missing_titles==0),
            "coverage":"bounded on-screen CG rows with independently known same-UID process identity; hidden/minimized windows are outside scope; missing titles may omit title-filter matches",
            "native_api":"CGWindowListCopyWindowInfo + proc_pidinfo + active displays; exact-pid queries additionally ground the focused AX dialog without pixels",
            "epoch_assurance":"observed_fingerprint_not_native_window_birth",
            "captured_unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis().to_string(),
            "input_sent":false,"activated":false
        }))
    }
    fn bind_window(&mut self, selection: &WindowSelection) -> Result<Value> {
        if self.fixed_attachment {
            return Err(fault(
                "fixed_window_target",
                "An attachment cannot be rebound; attach a fresh window through the original desktop target.",
            ));
        }
        let (selected, current, mut display) = self.validate_selection(selection)?;
        let mut placed = None;
        // Selection itself never moves a window: with implicit placement off,
        // binding a window that sits on another display succeeds without a
        // native side effect, and an explicit move is what changes placement.
        if let Some(policy) = &self.authorized_display
            && !policy.allows(
                display
                    .as_ref()
                    .map(crate::catalog::DisplayRecord::from_native)
                    .as_ref(),
            )
            && policy.place_on_bind
        {
            if selected.semantic_only {
                return Err(fault(
                    "semantic_action_required",
                    "A logical AX dialog is not a movable visual surface; it was not moved or rebound.",
                ));
            }
            let list = crate::native::displays()?;
            let target = list
                .iter()
                .find(|candidate| {
                    policy.allows(
                        Some(crate::catalog::DisplayRecord::from_native(candidate)).as_ref(),
                    )
                })
                .ok_or_else(|| {
                    fault(
                        "unauthorized_display",
                        "No active display matches the authorized-display policy; the window could not be placed.",
                    )
                })?;
            let placement = NativeMac::with_shared_resources(
                current.pid,
                current.id,
                self.allow_input,
                self.capturer.clone(),
                self.dispatches.clone(),
            )?;
            placement.require_discovered_process(&selected.process_start)?;
            placement.place_window_on(target)?;
            let moved_rect = windows()?
                .iter()
                .find(|w| w.id == current.id && w.pid == current.pid)
                .map(|w| w.rect)
                .ok_or_else(|| {
                    fault(
                        "window_missing",
                        "The window disappeared during native placement.",
                    )
                })?;
            display = crate::native::displays()
                .ok()
                .and_then(|list| crate::native::containing_display(&list, moved_rect).cloned());
            if !policy.allows(
                display
                    .as_ref()
                    .map(crate::catalog::DisplayRecord::from_native)
                    .as_ref(),
            ) {
                return Err(fault(
                    "unauthorized_display",
                    "Native placement did not move the window onto the authorized display; not bound.",
                ));
            }
            placed = Some(moved_rect);
        }
        let backend = self.selected_backend(&selected, &current)?;
        let subrole = backend.window_subrole().ok().flatten();
        if let Some(live) = &self.live
            && live.active()
            && !selected.semantic_only
        {
            match backend.visual_copy() {
                Ok(source) => live.source(source),
                Err(error) => live.source_failed(error.code),
            }
        }
        self.selected = Some(backend);
        let placed_json = placed.map(|rect| {
            json!({"bounds":[rect.x,rect.y,rect.w,rect.h],"display":display.as_ref().map(crate::catalog::DisplayRecord::from_native)})
        });
        Ok(
            json!({"bound":selected,"relation":selection.relation,"subrole":subrole,"input_authorized":self.allow_input,"input_sent":false,"activated":false,"placed":placed_json,"next":if selected.semantic_only{"Observe exact AX controls without pixels. Only explicit advertised native actions are allowed. Live keeps the previous explicit visual source; no relationship to a rendering window is inferred."}else{"Observe this exact window. Background observation and input are supported; the window does not need to be frontmost. Engine invalidates all prior frames/references; unsupported owner relations are not inferred."}}),
        )
    }
    fn attach_window(&mut self, selection: &WindowSelection) -> Result<WindowAttachment> {
        if self.fixed_attachment {
            return Err(fault(
                "attachment_source_required",
                "Use the original desktop discovery target.",
            ));
        }
        let (selected, current, _) = self.validate_selection(selection)?;
        let backend = self.selected_backend(&selected, &current)?;
        // Each attachment has its own selected AX registry and Live session,
        // but capture resources, event counter and physical lease identity are shared.
        let mut attached = Self::from_identity(
            self.identity.clone(),
            self.uid,
            self.allow_input,
            self.authorized_display,
        );
        attached.capturer = self.capturer.clone();
        attached.dispatches = self.dispatches.clone();
        attached.selected = Some(backend);
        attached.fixed_attachment = true;
        Ok(WindowAttachment {
            backend: Box::new(attached),
            details: json!({"bound":selected,
            "relation":selection.relation,"fixed_binding":true,"placed":null,
            "source_live_changed":false,"input_sent":false,"activated":false}),
        })
    }
    fn focus_window(&mut self, r: &Resolved, f: &rwi_core::ForegroundState) -> Result<Value> {
        self.check_identity()?;
        self.selected()?.focus_exact_window(r, f)
    }
    fn hotkey(&mut self, key: &str, f: &rwi_core::ForegroundState) -> Result<Value> {
        self.check_identity()?;
        if !self.allow_input || self.fixed_attachment {
            return Err(fault(
                "foreground_scope_unavailable",
                "System keyboard input requires an input-authorized original desktop, not a fixed window attachment.",
            ));
        }
        crate::native::hotkey::post(&self.identity, self.uid, &self.dispatches, key, f)
    }
    fn move_window(
        &mut self,
        reference: &Resolved,
        display: &rwi_core::DisplayTarget,
    ) -> Result<Value> {
        self.check_identity()?;
        let target = {
            let native = self.selected()?;
            if reference.scope.pid != native.pid || reference.scope.window != native.window_name() {
                return Err(fault(
                    "target_mismatch",
                    "Reference belongs to another native window; observe this exact one.",
                ));
            }
            crate::native::displays()?
                .into_iter()
                .find(|candidate| {
                    display.matches(candidate.built_in, candidate.id, candidate.uuid.as_deref())
                })
                .ok_or_else(|| {
                    fault(
                        "display_not_found",
                        "No active display matches the requested target; nothing was moved.",
                    )
                })?
        };
        let requested_origin = {
            let native = self.selected()?;
            native.place_window_on(&target)?
        };
        let mut readback = self.settle_window_placement(
            reference.scope.pid,
            &reference.scope.window,
            &target,
            requested_origin,
        )?;
        readback["requested"] = json!(display);
        Ok(readback)
    }
    fn observe(&mut self) -> Result<Scene> {
        self.selected()?.observe()
    }
    fn capture_visual(&mut self, options: VisualOptions) -> Result<VisualCapture> {
        self.selected()?.visual_capture(options)
    }
    fn live(&mut self, action: LiveAction, options: Option<LiveOptions>) -> Result<Value> {
        if action == LiveAction::Start {
            if let Some(live) = &self.live
                && live.active()
            {
                let mut status = live.status();
                status["start"] = json!("already_running; stop before reconfiguring");
                return Ok(status);
            }
            self.check_identity()?;
            let source = self.selected()?.visual_copy()?;
            let options =
                options.ok_or_else(|| fault("invalid_live_options", "Start requires options."))?;
            self.live = Some(crate::live::Session::start(source, options)?);
        }
        Ok(self.live.as_ref().map(|live| live.control_native(action))
            .unwrap_or_else(|| json!({"status":"stopped","capture_pending":false,"presenter_visible":false,"input_sent":false})))
    }
    fn live_view(&mut self, update: LiveViewUpdate) -> Result<Value> {
        self.check_identity()?;
        self.live
            .as_ref()
            .ok_or_else(|| {
                fault(
                    "live_not_started",
                    "Start a preview before publishing visibility.",
                )
            })?
            .view_update(update)
    }
    fn live_controller(&self) -> Result<Arc<dyn LiveController>> {
        Ok(Arc::new(DesktopLiveControl {
            identity: self.identity.clone(),
            uid: self.uid,
            session: self.live.as_ref().map(crate::live::Session::controller),
        }))
    }
    fn revalidate(&mut self, r: &Resolved) -> Result<Scene> {
        self.selected()?.revalidate(r)
    }
    fn act(
        &mut self,
        b: &Binding,
        r: &Resolved,
        i: &Intent,
        v: Option<&str>,
    ) -> Result<Option<bool>> {
        self.selected()?.act(b, r, i, v)
    }
    fn drag_between(
        &mut self,
        pair: &rwi_core::drag_between::Pair,
        destination: &mut dyn Backend,
    ) -> Result<Option<bool>> {
        self.check_identity()?;
        self.selected()?.dispatch_drag_between(pair, destination)
    }
    fn drag_surface(
        &mut self,
        endpoint: &rwi_core::drag_between::Endpoint,
        initial: bool,
    ) -> Result<rwi_core::drag_between::Surface> {
        self.check_identity()?;
        self.selected()?.paired_drag_surface(endpoint, initial)
    }
    fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> Result<Oracle> {
        self.selected()?.inspect(s, p, e)
    }
}
