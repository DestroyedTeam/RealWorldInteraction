//! Headless per-user desktop backend; selection is an explicit later read-only call.
use crate::{
    capture::Capturer,
    catalog::{Catalog, WindowRecord, matches_query},
    native::{NativeMac, process_epoch, window_inventory, windows},
    session,
};
use rwi_core::*;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

pub struct NativeMacDesktop {
    identity: Identity,
    uid: u32,
    allow_input: bool,
    selected: Option<NativeMac>,
    catalog: Catalog,
    capturer: Capturer,
    dispatches: Arc<AtomicU64>,
    authorized_display: Option<crate::catalog::AuthorizedDisplay>,
}
impl NativeMacDesktop {
    /// Reads only native process/console identity, never windows, AX, capture or input.
    pub fn new(
        uid: u32,
        allow_input: bool,
        authorized_display: Option<crate::AuthorizedDisplay>,
    ) -> Result<Self> {
        Ok(Self::from_identity(
            session::identity_for_uid(uid)?,
            uid,
            allow_input,
            authorized_display,
        ))
    }
    fn from_identity(
        identity: Identity,
        uid: u32,
        allow_input: bool,
        authorized_display: Option<crate::AuthorizedDisplay>,
    ) -> Self {
        Self {
            identity,
            uid,
            allow_input,
            selected: None,
            catalog: Catalog::default(),
            capturer: Capturer::default(),
            dispatches: Arc::new(AtomicU64::new(0)),
            authorized_display,
        }
    }
    fn check_identity(&self) -> Result<()> {
        if session::identity_for_uid(self.uid)? != self.identity {
            return Err(fault(
                "session_mismatch",
                "The native macOS user/security session/boot/seat changed; open a new context, never reuse its lease or frames.",
            ));
        }
        Ok(())
    }
    fn selected(&mut self) -> Result<&mut NativeMac> {
        self.selected.as_mut().ok_or_else(||fault("window_selection_required","Discover and explicitly bind an exact CGWindowID/PID/process fingerprint. No foreground window is chosen automatically."))
    }
}

pub(crate) fn native_capabilities(
    allow_input: bool,
    permissions: Value,
    console_set: Option<i64>,
) -> Value {
    json!({
        "adapter":"rust-macos-native","capture":"ScreenCaptureKit.SCScreenshotManager",
        "input":"CoreGraphics.CGEvent","oracle":"Accessibility.AXUIElement",
        "checked_context_policy":"native_element","input_opt_in":allow_input,
        "permissions":permissions,"permission_prompts":false,
        "evidence":"native_api_observed","hardware_attestation":false,
        "seat_kind":"logical_on_console_domain","native_console_set_id":console_set,"native_hardware_seat_observed":console_set.is_some(),
        "window_discovery":{"implemented":true,"scope":"on_screen_only_same_native_user","max_native_rows":2048,"max_page_rows":8,"max_page_bytes":12288,"catalog_ttl_ms":10000},
        "binding_identity":"fresh CGWindowID + PID + process start + context + observed bounds/title fingerprint; no window creation-time or continuous lifetime attestation",
        "ax_tree":"bounded_partial","mapping":"single containing unrotated display only",
        "pointer_drag":{"implemented":true,"button":"left","coordinate_space":"offset_from_grounded_reference_in_desktop_points","max_waypoints":rwi_core::trajectory::MAX_WAYPOINTS,"max_duration_ms":rwi_core::trajectory::MAX_DURATION_MS,"sample_interval_ms":crate::native::NATIVE_SAMPLE_INTERVAL_MS,"sampling_policy":"bounded interpolation; all requested vertices and times retained","max_lateness_ms":rwi_core::input_trajectory::MAX_LATENESS_MS,"scope":"pinned window; every sample must stay inside the observed frame; background postToPid delivery, no cursor movement"},"pointer_feedback":{"implemented":false},
        "capture_visual":{"implemented":false},"native_roi":{"implemented":false},
        "lifecycle":{"implemented":false},"recovery":"read-only HID + combined session resource state, requiring listen-event preflight and agent idle writer proof",
        "unsupported":["activation","launch","native owner-chain transition","minimized/hidden window discovery","mixed-display windows","rotated displays","Pointer feedback","capture_visual","native ROI","clipboard","unattended unlock","modal-window input while AXModal is true/unknown"],
        "text_input":"only the exact focused, explicitly classified non-secure AX text element; unknown classifications remain unavailable",
        "background_operation":"observation resolves the exact AX window from the target application; input uses CGEventPostToPid with window addressing and window-local coordinates; the system cursor does not move and no application is activated or focused; the target window does not need to be frontmost",
        "interruption_coverage":"per-batch focus, hit-test, layout, session, permissions, lease and cancellation; not a complete global human-input observer",
        "lock_state":"unknown","live_acceptance":"pending",
        "atomicity":"pre/post validation; neither CGEventPost nor property equality proves the business goal"
    })
}

impl Backend for NativeMacDesktop {
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
        caps["window_selected"] = json!(self.selected.is_some());
        caps["pointer_feedback"] = crate::overlay::status_json();
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
        self.check_identity()?;
        self.catalog.replace(rows,json!({
            "identity":self.identity,"filter":query,"native_windows_enumerated":count,
            "unknown_or_invalid_rows":unknown,"unavailable_native_titles":missing_titles,
            "enumeration_complete":unknown==0 && (query.title_contains.is_none() || missing_titles==0),
            "coverage":"bounded on-screen CG rows with independently known same-UID process identity; hidden/minimized windows are outside scope; missing titles may omit title-filter matches",
            "native_api":"CGWindowListCopyWindowInfo + proc_pidinfo; no AX content or screenshot",
            "epoch_assurance":"observed_fingerprint_not_native_window_birth",
            "captured_unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis().to_string(),
            "input_sent":false,"activated":false
        }))
    }
    fn bind_window(&mut self, selection: &WindowSelection) -> Result<Value> {
        self.check_identity()?;
        let selected = self.catalog.select(selection)?;
        let all = windows()?;
        let candidates = all
            .iter()
            .filter(|w| format!("cgwindow:{}", w.id) == selected.window)
            .collect::<Vec<_>>();
        if candidates.len() != 1 || candidates[0].pid != selected.pid {
            return Err(fault(
                "window_epoch_changed",
                "The exact discovered window/PID is absent or ambiguous; no replacement by title.",
            ));
        }
        let current = candidates[0];
        let mut display = crate::native::displays()
            .ok()
            .and_then(|list| crate::native::containing_display(&list, current.rect).cloned());
        selected.require_same(&WindowRecord::from_native(
            &self.identity,
            current,
            process_epoch(current.pid)?,
            display.as_ref(),
        )?)?;
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
        let backend = NativeMac::with_shared_resources(
            current.pid,
            current.id,
            self.allow_input,
            self.capturer.clone(),
            self.dispatches.clone(),
        )?;
        backend.require_discovered_process(&selected.process_start)?;
        if backend.identity() != self.identity {
            return Err(fault(
                "session_mismatch",
                "Native context changed while binding.",
            ));
        }
        self.check_identity()?;
        let subrole = backend.window_subrole().ok().flatten();
        self.selected = Some(backend);
        let placed_json = placed.map(|rect| {
            json!({"bounds":[rect.x,rect.y,rect.w,rect.h],"display":display.as_ref().map(crate::catalog::DisplayRecord::from_native)})
        });
        Ok(
            json!({"bound":selected,"relation":selection.relation,"subrole":subrole,"input_authorized":self.allow_input,"input_sent":false,"activated":false,"placed":placed_json,"next":"Observe this exact window. Background observation and input are supported; the window does not need to be frontmost. Engine invalidates all prior frames/references; unsupported owner relations are not inferred."}),
        )
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
                    display
                        .built_in
                        .is_none_or(|built_in| candidate.built_in == built_in)
                        && display.id.is_none_or(|id| candidate.id == id)
                })
                .ok_or_else(|| {
                    fault(
                        "display_not_found",
                        "No active display matches the requested target; nothing was moved.",
                    )
                })?
        };
        {
            let native = self.selected()?;
            native.place_window_on(&target)?;
        }
        self.check_identity()?;
        let moved = windows()?
            .into_iter()
            .find(|w| format!("cgwindow:{}", w.id) == reference.scope.window)
            .ok_or_else(|| {
                fault(
                    "window_missing",
                    "The window disappeared during the explicit move.",
                )
            })?;
        let placed_display = crate::native::displays()
            .ok()
            .and_then(|list| crate::native::containing_display(&list, moved.rect).cloned())
            .map(|d| crate::catalog::DisplayRecord::from_native(&d));
        Ok(json!({
            "window": reference.scope.window,
            "bounds": [moved.rect.x, moved.rect.y, moved.rect.w, moved.rect.h],
            "display": placed_display,
            "requested": display,
            "input_sent": false,
            "activated": false,
        }))
    }
    fn observe(&mut self) -> Result<Scene> {
        self.selected()?.observe()
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
    fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> Result<Oracle> {
        self.selected()?.inspect(s, p, e)
    }
}

fn feedback_overlay(visible: bool, point: Option<[f64; 2]>) -> Result<Value> {
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

#[cfg(test)]
mod tests {
    use super::*;
    fn backend() -> NativeMacDesktop {
        NativeMacDesktop::from_identity(
            Identity {
                host: "synthetic".into(),
                guest: None,
                session: "uid:501:security:100:boot:1".into(),
                seat: "console:1".into(),
                desktop: "quartz-console".into(),
            },
            501,
            false,
            None,
        )
    }
    #[test]
    fn unselected_backend_constructs_and_rejects_observation_without_reading_native_ui() {
        let mut backend = backend();
        assert_eq!(backend.input_dispatch_count(), Some(0));
        assert_eq!(
            backend.observe().unwrap_err().code,
            "window_selection_required"
        );
        assert!(!backend.allow_input);
        assert!(backend.selected.is_none());
    }
    #[test]
    fn capabilities_separate_native_implementation_permissions_and_unsupported_tools() {
        let cap = native_capabilities(
            false,
            json!({"accessibility":false,"screen_capture":false,"post_event":false,"listen_event":false}),
            None,
        );
        assert_eq!(cap["window_discovery"]["implemented"], true);
        assert_eq!(cap["input_opt_in"], false);
        assert_eq!(cap["pointer_drag"]["implemented"], true);
        assert_eq!(cap["pointer_drag"]["button"], "left");
        assert_eq!(
            cap["pointer_drag"]["max_duration_ms"],
            rwi_core::trajectory::MAX_DURATION_MS
        );
        for key in [
            "pointer_feedback",
            "capture_visual",
            "native_roi",
            "lifecycle",
        ] {
            assert_eq!(cap[key]["implemented"], false);
        }
        assert_eq!(cap["lock_state"], "unknown");
        assert_eq!(cap["hardware_attestation"], false);
        assert_eq!(cap["seat_kind"], "logical_on_console_domain");
        assert_eq!(cap["native_hardware_seat_observed"], false);
        assert!(cap["native_console_set_id"].is_null());
        assert_eq!(cap["live_acceptance"], "pending");
    }
    #[test]
    #[ignore = "Explicit process-only native diagnostic: identity and non-prompting permission booleans; no window or UI access"]
    fn process_only_native_diagnostic_without_windows_or_ui_access() {
        let uid = unsafe { libc::getuid() };
        let status = session::process_status(uid);
        let mut report = json!({"diagnostic":"process_only_no_ui","process":status});
        match NativeMacDesktop::new(uid, false, None) {
            Ok(mut backend) => {
                assert!(backend.selected.is_none());
                assert_eq!(
                    backend.observe().unwrap_err().code,
                    "window_selection_required"
                );
                let dir = tempfile::tempdir().unwrap();
                let mut engine = Engine::new(dir.path().into()).unwrap();
                engine
                    .register("process-only-native-desktop".into(), Box::new(backend))
                    .unwrap();
                report["targets"] = engine.targets();
                report["unselected_constructor"] = json!("available");
            }
            Err(error) => {
                report["unselected_constructor"] = json!("unavailable");
                report["error"] = json!(error);
            }
        }
        assert_eq!(report["process"]["screen_captured"], false);
        assert_eq!(report["process"]["window_list_read"], false);
        assert_eq!(report["process"]["input_sent"], false);
        println!("{}", serde_json::to_string(&report).unwrap());
    }
}
