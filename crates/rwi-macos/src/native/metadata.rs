//! Native AX metadata, focus and hit-testing. No implicit application activation.
use super::*;

impl NativeMac {
    pub fn new(pid: u32, window: u32, allow_input: bool) -> Result<Self> {
        Self::with_shared_resources(
            pid,
            window,
            allow_input,
            Capturer::default(),
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        )
    }
    pub(crate) fn with_shared_resources(
        pid: u32,
        window: u32,
        allow_input: bool,
        capturer: Capturer,
        dispatches: std::sync::Arc<std::sync::atomic::AtomicU64>,
    ) -> Result<Self> {
        if pid == 0 || pid > i32::MAX as u32 || window == 0 {
            return Err(fault(
                "invalid_target",
                "Pin a positive PID and CGWindowID.",
            ));
        }
        Ok(Self {
            pid,
            window,
            identity: console_identity()?,
            process: process_epoch(pid)?,
            allow_input,
            semantic_only: false,
            capturer,
            hit_diagnostics: std::sync::Mutex::new(Vec::new()),
            dispatches,
            objects: std::sync::Mutex::new(objects::Objects::default()),
            last_observation: std::sync::Mutex::new(serde_json::Value::Null),
        })
    }
    pub(crate) fn window_name(&self) -> String {
        format!("cgwindow:{}", self.window)
    }
    pub(crate) fn require_discovered_process(&self, process: &str) -> Result<()> {
        if self.process != process {
            return Err(fault(
                "window_epoch_changed",
                "The exact process birth changed while constructing the selected native backend.",
            ));
        }
        Ok(())
    }
    pub(super) fn foreground_pid(&self) -> Option<u32> {
        let system = unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide()).ok()? };
        system.attr("AXFocusedApplication").ok()?.pid().ok()
    }
    /// Read native focused-window identity only for a recent-activity conflict.
    /// No AXMain guess, activation, or fallback to a similarly titled window.
    pub(super) fn foreground_input_window(&self) -> (Option<u32>, Option<u32>) {
        let Ok(system) = (unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide()) }) else {
            return (None, None);
        };
        let Ok(application) = system.attr("AXFocusedApplication") else {
            return (None, None);
        };
        let pid = application.pid().ok();
        if pid != Some(self.pid) {
            return (pid, None);
        }
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 0.05);
        }
        let window = application
            .attr("AXFocusedWindow")
            .ok()
            .and_then(|window| ffi::window_number_for_ax_element(&window));
        (pid, window)
    }
    pub(super) fn window_frame(&self) -> Result<Rect> {
        Ok(self.window_record()?.rect)
    }
    pub(super) fn metadata_scene(
        &self,
        meta: Metadata,
        size: [u32; 2],
        png: Vec<u8>,
        content_sha: Option<&str>,
    ) -> Scene {
        let mut binding = meta.binding;
        if let Some(content_sha) = content_sha {
            binding.content = content_sha.to_owned();
        } else if !png.is_empty() {
            binding.content = digest(&png);
        }
        Scene {identity:meta.identity,assurance:"native-attested".into(),binding:Some(binding),
            mutation_ready:self.allow_input&&meta.mapping_ready&&meta.blocked==Some(false)&&unsafe{ffi::CGPreflightPostEventAccess()},
            blocked:meta.blocked,image_size:size,pixel_to_desktop:if meta.mapping_ready{meta.rect.map(size)}else{None},
            elements:meta.elements.into_iter().map(|e|e.element).collect(),
            captured_unix_ms:SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            atomicity:"pre/post native validation; non-atomic; live acceptance pending".into(),
            image_normalization:"CGImage -> device RGB bitmap -> straight RGBA PNG; native dimensions; no resizing; color/alpha normalization, not source-byte identity; full PNG retained, input fingerprint excludes title chrome and bottom one-point compositor border; synthetic pointer paths cannot enter bottom border".into(),png}
    }
    pub(super) fn metadata(&self) -> Result<Metadata> {
        self.metadata_for(None)
    }
    /// Revalidation reads the exact retained target, not unrelated siblings.
    /// Full observations still walk the bounded tree and apply capture guards.
    pub(super) fn metadata_for(&self, target: Option<&Resolved>) -> Result<Metadata> {
        self.metadata_read(target, true)
    }
    /// Snapshot pixels/controls are explicitly non-atomic. The pre-capture
    /// guard needs the native window/context/modal state, not a discarded tree.
    pub(super) fn capture_guard_metadata(&self) -> Result<Metadata> {
        self.metadata_read(None, false)
    }
    fn metadata_read(&self, target: Option<&Resolved>, include_controls: bool) -> Result<Metadata> {
        let _call = operation::native_call("macos_ax_metadata")?;
        let identity = console_identity()?;
        if identity != self.identity || process_epoch(self.pid)? != self.process {
            return Err(fault(
                "session_or_process_changed",
                "Pinned console session/process instance changed. Rebind explicitly.",
            ));
        }
        if unsafe { ffi::AXIsProcessTrusted() } == 0 {
            return Err(fault(
                "accessibility_permission_missing",
                "Grant permission to the native binary through an authorized setup, not this observation call.",
            ));
        }
        let window = self.window_record()?;
        // Background observation: resolve the exact AX window from the target
        // application itself. No focused-application or focused-window
        // requirement; the pinned window does not need to be frontmost.
        let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 0.05);
        }
        let focused = self.resolve_window_element(&application, window.rect)?;
        let ax_rect = ax_rect(&focused)?;
        if !window.rect.same(ax_rect) {
            return Err(fault(
                "window_binding_ambiguous",
                &format!(
                    "Resolved AX window rect (x={:.1},y={:.1},w={:.1},h={:.1}) does not match the pinned CGWindowID/PID/bounds (x={:.1},y={:.1},w={:.1},h={:.1}).",
                    ax_rect.x,
                    ax_rect.y,
                    ax_rect.w,
                    ax_rect.h,
                    window.rect.x,
                    window.rect.y,
                    window.rect.w,
                    window.rect.h
                ),
            ));
        }
        let title = focused
            .attr("AXTitle")
            .ok()
            .and_then(|v| v.string().ok())
            .or_else(|| window.title.clone())
            .unwrap_or_default();
        let modal = focused.attr("AXModal").ok().and_then(|v| v.boolean());
        let blocked = self.modal_state(&application, &focused);
        let (layout, mapping_ready) = display_layout(window.rect)?;
        // This tree belongs to the pinned application. A system-wide hit test
        // would turn another app covering it into a false foreground requirement.
        // Application-scoped AX hit testing still respects that app's window
        // z-order; equality with each exact tree node preserves window identity.
        let mut diagnostics = Vec::new();
        let (elements, coverage) = read_control_metadata(include_controls, || {
            Ok(if let Some(target) = target {
                if target.scope.pid != self.pid || target.scope.window != self.window_name() {
                    return Err(fault(
                        "target_mismatch",
                        "Semantic revalidation belongs to another window or process.",
                    ));
                }
                let element = objects::exact(
                    &focused,
                    self.pid,
                    target.scope.element.as_deref().ok_or_else(|| {
                        fault("semantic_target_missing", "Exact semantic object required.")
                    })?,
                    &self.objects,
                )?;
                (
                    vec![observed_element(
                        &element,
                        &focused,
                        &application,
                        window.rect,
                        blocked,
                        &mut diagnostics,
                        &self.objects,
                    )?],
                    serde_json::json!({"scope":"exact_target_revalidation","complete":false,"input_authority":false}),
                )
            } else {
                ax_tree(
                    &focused,
                    &application,
                    window.rect,
                    blocked,
                    &mut diagnostics,
                    &self.objects,
                )?
            })
        })?;
        if include_controls && let Ok(mut saved) = self.hit_diagnostics.lock() {
            *saved = diagnostics;
        }
        // The application's internal focused element is not window identity: it
        // changes whenever the user or activation chrome moves focus, including
        // while the user works in other applications. Keyboard/text paths verify
        // the exact focused target separately before dispatch.
        let native_epoch = digest(
            format!(
                "{}|{:?}|{}|{:?}|semantic:{}",
                self.process, window.rect, title, modal, self.semantic_only
            )
            .as_bytes(),
        );
        let binding = Binding {
            // Already-read native title, not an invented document-state oracle.
            // Epoch already binds this same title; no new AX reads or weaker gate.
            mutation_context: Some(MutationContext {
                window_title: title.clone(),
                document_state: None,
            }),
            scope: Scope {
                window: self.window_name(),
                pid: self.pid,
                element: None,
            },
            focus_window: self.window_name(),
            layout,
            content: native_epoch.clone(),
            native_epoch,
        };
        if console_identity()? != identity || process_epoch(self.pid)? != self.process {
            return Err(fault(
                "session_or_process_changed",
                "The native context/process changed during AX readback; discard this observation.",
            ));
        }
        Ok(Metadata {
            identity,
            rect: window.rect,
            binding,
            elements,
            title,
            modal,
            blocked,
            mapping_ready,
            coverage,
        })
    }
    /// Exact private-SPI window match first; a strict geometry fallback only
    /// when the private attribute is unavailable. Modal sheets and dialogs are
    /// child elements of a top-level AX window rather than entries in
    /// `AXWindows`, and some sandboxed panels appear only as the application's
    /// focused/main window, so both are covered with the same strict bounds
    /// rule. Ambiguity always fails closed.
    /// Panel/sheet windows often do not expose `AXModal` at all. Unknown
    /// modality is acceptable only for the application's focused window, which
    /// is by definition the current interaction surface (the dialog itself,
    /// not something it blocks); every other unknown state stays fail-closed.
    pub(super) fn modal_state(&self, application: &Cf, window_element: &Cf) -> Option<bool> {
        let focused = application.attr("AXFocusedWindow").ok();
        let focused_here = focused
            .as_ref()
            .is_some_and(|window| unsafe { ffi::CFEqual(window.0, window_element.0) } != 0);
        let modal = |window: &Cf| {
            window
                .attr("AXModal")
                .ok()
                .and_then(|value| value.boolean())
        };
        let window_modal = modal(window_element);
        let focused_modal = focused.as_ref().and_then(modal);
        // Paint S can keep AXFocusedWindow on the document while presenting a
        // separate AXModal dialog. A modal flag says the selected window is a
        // blocker, not that it is itself blocked. Only accept that unfocused
        // dialog when application-scoped native hit testing independently lands
        // in the exact selected AX window/PID/CGWindowID. An unknown hit keeps
        // the input path closed; another focused modal still takes precedence.
        let modal_hit =
            if window_modal == Some(true) && !focused_here && focused_modal != Some(true) {
                let expected = ffi::window_number_for_ax_element(window_element);
                ax_rect(window_element)
                    .ok()
                    .and_then(|bounds| {
                        hit_test(
                            application,
                            [bounds.x + bounds.w / 2., bounds.y + bounds.h / 2.],
                        )
                        .ok()
                    })
                    .and_then(|hit| {
                        let container = if unsafe { ffi::CFEqual(hit.0, window_element.0) } != 0 {
                            Some(hit.retained())
                        } else {
                            hit.attr("AXWindow").ok()
                        }?;
                        Some(
                            hit.pid().ok() == Some(self.pid)
                                && unsafe { ffi::CFEqual(container.0, window_element.0) } != 0
                                && ffi::window_number_for_ax_element(&container) == expected
                                && expected == Some(self.window),
                        )
                    })
            } else {
                None
            };
        // An AXPopover commonly has no AXModal value and leaves AXFocusedWindow
        // on its document. For the separately bound, same-PID CG surface whose
        // rect was already matched above, require an application-scoped hit at
        // its center to descend from this exact AXPopover. Otherwise modality
        // remains unknown and input is refused.
        let popover_hit = if window_modal.is_none()
            && !focused_here
            && focused_modal != Some(true)
            && popover_subrole(window_element)
        {
            ax_rect(window_element)
                .ok()
                .and_then(|bounds| {
                    hit_test(
                        application,
                        [bounds.x + bounds.w / 2., bounds.y + bounds.h / 2.],
                    )
                    .ok()
                })
                .and_then(|hit| hit_descends_from(&hit, window_element, self.pid))
        } else {
            None
        };
        let effective_modal = if popover_hit == Some(true) {
            Some(true)
        } else {
            window_modal
        };
        with_known_attached_sheet(
            modal_blocked(
                effective_modal,
                focused_here,
                focused_modal,
                modal_hit.or(popover_hit),
            ),
            known_direct_sheet_child(window_element),
        )
    }
    /// Native placement: set the window's accessibility position so its
    /// top-left lands on the authorized display. No activation, no synthetic
    /// input; the exact window element is re-resolved first and failures stay
    /// fail-closed.
    /// Native subrole of the resolved pinned window (AXStandardWindow, AXSheet,
    /// AXDialog, AXPopover, ...): dialog awareness for the caller without
    /// classifying by title. Read-only; no activation and no input.
    pub(crate) fn window_subrole(&self) -> Result<Option<String>> {
        let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 0.25);
        }
        let window = self.resolve_window_element(&application, self.window_frame()?)?;
        Ok(window
            .attr("AXSubrole")
            .ok()
            .and_then(|value| value.string().ok()))
    }
    pub(crate) fn place_window_on(&self, display: &crate::native::Display) -> Result<[f64; 2]> {
        // A semantic-only dialog is not a pixel target, but its own focused
        // AX/CG identity can still authorize a window-level AXPosition move.
        // Never substitute its separately numbered rendering surface.
        let expected = self.window_record()?.rect;
        let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 1.0);
        }
        let window = self.resolve_window_element(&application, expected)?;
        let window_rect = ax_rect(&window)?;
        if !window_rect.same(expected) {
            return Err(fault(
                "window_epoch_changed",
                "The exact AX window moved before placement; discover and bind it again.",
            ));
        }
        if self.semantic_only {
            let focused = application.attr("AXFocusedWindow")?;
            if unsafe { ffi::CFEqual(focused.0, window.0) } == 0
                || ffi::window_number_for_ax_element(&focused) != Some(self.window)
                || focused.pid()? != self.pid
            {
                return Err(fault(
                    "semantic_dialog_changed",
                    "The exact logical dialog lost focus before placement; no move sent.",
                ));
            }
        }
        if !window.attribute_settable("AXPosition")? {
            return Err(fault(
                "placement_unavailable",
                "The exact AX window does not allow position changes; no move sent.",
            ));
        }
        if window_rect.w > display.rect.w || window_rect.h > display.rect.h {
            return Err(fault(
                "placement_impossible",
                "The window is larger than the authorized display; it was not moved.",
            ));
        }
        let (x, y) = if !self.semantic_only && dialog_subrole(&window) {
            // A background panel may remain behind its application's document
            // after AXPosition. Put it in available space instead of moving all
            // windows onto the same origin; this does not raise or activate it.
            let obstacles = windows()?
                .into_iter()
                .filter(|other| other.pid == self.pid && other.id != self.window)
                .map(|other| other.rect)
                .collect::<Vec<_>>();
            panel_placement_position(window_rect, display.rect, &obstacles)
        } else {
            crate::native::placement_position(window_rect, display.rect)
        };
        let point = core_graphics::geometry::CGPoint::new(x, y);
        let value = unsafe {
            ffi::AXValueCreate(
                1,
                (&point as *const core_graphics::geometry::CGPoint).cast(),
            )
        };
        let value = unsafe { Cf::owned(value)? };
        use core_foundation::base::TCFType;
        let attribute = core_foundation::string::CFString::new("AXPosition");
        // AXSet may take effect even if its reply is lost. Count entry into the
        // native mutation before calling it so an error cannot be mislabeled as
        // a proven pre-dispatch rejection by the durable move receipt.
        self.dispatches
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let code = unsafe {
            ffi::AXUIElementSetAttributeValue(
                window.0,
                attribute.as_concrete_TypeRef().cast(),
                value.0,
            )
        };
        if code != 0 {
            return Err(fault(
                "placement_rejected",
                "Native AXPosition returned an error; the window may still have moved. Discover its current bounds before any new request.",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
        Ok([x, y])
    }
    pub(super) fn resolve_window_element(&self, application: &Cf, expected: Rect) -> Result<Cf> {
        let value = application.attr("AXWindows")?;
        let mut windows = unsafe { ffi::array(value.0, 64)? };
        let mut exact = Vec::new();
        let mut geometric = Vec::new();
        for (index, window) in windows.iter().enumerate() {
            if ffi::window_number_for_ax_element(window) == Some(self.window) {
                exact.push(index);
            } else if ax_rect(window).is_ok_and(|r| r.same(expected)) {
                geometric.push(index);
            }
        }
        match (exact.len(), geometric.len()) {
            (1, _) => return Ok(windows.swap_remove(exact[0])),
            (0, 1) => return Ok(windows.swap_remove(geometric[0])),
            (0, 0) => (),
            _ => {
                return Err(fault(
                    "window_binding_ambiguous",
                    "Multiple AX windows match the pinned identity or bounds; rebind explicitly.",
                ));
            }
        }
        // Sheets and popovers can be descendants of an AXWindows entry without
        // being AXWindows entries themselves. Paint S's color popover is one
        // such separate CG surface; one level of AXChildren is not sufficient.
        // This is still an exact PID/CGWindowID/bounds selection, never a title
        // or nearest-window fallback.
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 0.25);
        }
        let mut child_exact = Vec::new();
        let mut child_geometric = Vec::new();
        let mut dialog_candidates = Vec::new();
        let mut pending = VecDeque::new();
        for window in &windows {
            pending.push_back((window.retained(), 0usize));
        }
        let search_start = Instant::now();
        let mut searched = 0usize;
        let mut search_truncated = false;
        while let Some((node, depth)) = pending.pop_front() {
            operation::checkpoint("macos_ax_dialog_search")?;
            if searched >= 192 || search_start.elapsed() > Duration::from_millis(500) {
                search_truncated = true;
                break;
            }
            searched += 1;
            if depth > 0 && node.pid().ok() == Some(self.pid) {
                let native_id = ffi::window_number_for_ax_element(&node);
                let rect = ax_rect(&node).ok();
                let is_dialog = dialog_subrole(&node);
                if is_dialog && dialog_candidates.len() < 8 {
                    dialog_candidates.push(serde_json::json!({
                        "window":native_id,
                        "rect":rect.map(|r| [r.x,r.y,r.w,r.h]),
                        "depth":depth
                    }));
                }
                if rect.is_some_and(|r| r.same(expected)) {
                    let list = if native_id == Some(self.window) {
                        Some(&mut child_exact)
                    } else if is_dialog {
                        Some(&mut child_geometric)
                    } else {
                        None
                    };
                    if let Some(list) = list
                        && !list
                            .iter()
                            .any(|prior: &Cf| unsafe { ffi::CFEqual(prior.0, node.0) } != 0)
                    {
                        list.push(node.retained());
                    }
                }
            }
            if depth >= 6 {
                continue;
            }
            let Ok(children_value) = node.attr("AXChildren") else {
                continue;
            };
            let Ok(children) = (unsafe { ffi::array(children_value.0, 64) }) else {
                continue;
            };
            for child in children {
                pending.push_back((child, depth + 1));
            }
        }
        match (child_exact.len(), child_geometric.len()) {
            (1, _) => return Ok(child_exact.remove(0)),
            (0, 1) => return Ok(child_geometric.remove(0)),
            (0, 0) => (),
            _ => {
                return Err(fault(
                    "window_binding_ambiguous",
                    "Multiple AX child sheets/dialogs match the pinned identity or bounds; rebind explicitly.",
                ));
            }
        }
        // Some apps (e.g. the sandboxed open panel) expose the sheet only as
        // the application's focused or main window.
        let mut relay = Vec::new();
        for key in ["AXFocusedWindow", "AXMainWindow"] {
            let Ok(element) = application.attr(key) else {
                continue;
            };
            if ffi::window_number_for_ax_element(&element) == Some(self.window) {
                return Ok(element);
            }
            if dialog_subrole(&element) && ax_rect(&element).is_ok_and(|r| r.same(expected)) {
                relay.push(element);
            }
        }
        match relay.len() {
            1 => Ok(relay.remove(0)),
            0 => {
                let mut error = fault(
                    "window_missing",
                    "Pinned CGWindowID/PID has no matching AX window; do not substitute another window.",
                );
                error.diagnostic = Some(Box::new(serde_json::json!({
                    "ax_descendants_checked":searched,
                    "ax_search_truncated":search_truncated,
                    "dialog_candidates":dialog_candidates,
                    "expected_rect":[expected.x,expected.y,expected.w,expected.h],
                    "expected_window":self.window
                })));
                Err(error)
            }
            _ => Err(fault(
                "window_binding_ambiguous",
                "Multiple AX focused/main windows match the pinned identity or bounds; rebind explicitly.",
            )),
        }
    }
}

/// Positive, exact-parent evidence only; never an application-wide PID gate.
/// AppKit document sheets are AXChildren of their owner even when AXModal=false
/// and AXFocusedWindow remains on the owner. A separately selected sheet has
/// buttons as children and therefore does not block itself. A remote provider's
/// sheet can also block this parent; its PID never confers input authority here.
fn known_direct_sheet_child(window: &Cf) -> bool {
    let Ok(value) = window.attr("AXChildren") else {
        return false;
    };
    let Ok(children) = (unsafe { ffi::array(value.0, 64) }) else {
        return false;
    };
    children.iter().any(|child| {
        if unsafe { ffi::CFGetTypeID(child.0) != ffi::AXUIElementGetTypeID() } {
            return false;
        }
        child
            .attr("AXRole")
            .ok()
            .and_then(|v| v.string().ok())
            .as_deref()
            == Some("AXSheet")
            && child.attr("AXParent").ok().is_some_and(|parent| unsafe {
                ffi::CFEqual(parent.0, window.0) != 0 && ffi::CFEqual(child.0, window.0) == 0
            })
            && child.attr("AXHidden").ok().and_then(|v| v.boolean()) != Some(true)
    })
}
fn with_known_attached_sheet(base: Option<bool>, attached: bool) -> Option<bool> {
    if attached { Some(true) } else { base }
}

fn modal_blocked(
    window_modal: Option<bool>,
    focused_here: bool,
    focused_modal: Option<bool>,
    modal_hit: Option<bool>,
) -> Option<bool> {
    // A modal surface does not block itself; it does block a different window.
    // A different ordinary/nonmodal focus is not a foreground prerequisite.
    if focused_here {
        Some(false)
    } else if focused_modal == Some(true) {
        Some(true)
    } else if window_modal == Some(true) {
        modal_hit.map(|hit| !hit)
    } else {
        window_modal
    }
}

/// Bounded best-effort placement. All same-application windows remain obstacles,
/// even when only the first 64 contribute candidate edges. Native input still
/// checks actual hit ownership after the move; free geometry is not authority.
fn panel_placement_position(window: Rect, display: Rect, obstacles: &[Rect]) -> (f64, f64) {
    let base = crate::native::placement_position(window, display);
    let max_x = display.x + display.w - window.w;
    let max_y = display.y + display.h - window.h;
    let mut candidates = vec![base, (max_x, base.1), (base.0, max_y), (max_x, max_y)];
    for rect in obstacles.iter().filter(|rect| rect.valid()).take(64) {
        candidates.extend([
            (rect.x + rect.w + 24.0, base.1),
            (base.0, rect.y + rect.h + 24.0),
            (rect.x - window.w - 24.0, base.1),
            (base.0, rect.y - window.h - 24.0),
        ]);
    }
    candidates
        .into_iter()
        .filter(|&(x, y)| {
            x >= display.x
                && x <= max_x
                && y >= display.y
                && y <= max_y
                && obstacles.iter().filter(|rect| rect.valid()).all(|rect| {
                    x + window.w <= rect.x
                        || x >= rect.x + rect.w
                        || y + window.h <= rect.y
                        || y >= rect.y + rect.h
                })
        })
        .min_by(|a, b| {
            let distance = |p: &(f64, f64)| (p.0 - base.0).powi(2) + (p.1 - base.1).powi(2);
            distance(a).total_cmp(&distance(b))
        })
        .unwrap_or(base)
}

#[cfg(test)]
#[test]
fn panel_placement_avoids_covered_origin_without_moving_other_windows() {
    let rect = |x, y, w, h| Rect { x, y, w, h };
    let display = rect(1512.0, 0.0, 1920.0, 1080.0);
    let panel = rect(18.0, 52.0, 341.0, 367.0);
    let obstacles = [
        rect(1536.0, 24.0, 1434.0, 660.0),
        rect(1536.0, 24.0, 1220.0, 784.0),
    ];
    assert_eq!(
        panel_placement_position(panel, display, &obstacles),
        (2994.0, 24.0)
    );
    assert_eq!(
        panel_placement_position(panel, display, &[]),
        (1536.0, 24.0)
    );
    assert_eq!(
        panel_placement_position(panel, display, &[display]),
        (1536.0, 24.0)
    );
    let negative = rect(-1920.0, -1080.0, 1920.0, 1080.0);
    assert_eq!(
        panel_placement_position(panel, negative, &[rect(-1896.0, -1056.0, 1434.0, 784.0)]),
        (-438.0, -1056.0)
    );
}

fn popover_subrole(element: &Cf) -> bool {
    ["AXRole", "AXSubrole"].into_iter().any(|key| {
        element
            .attr(key)
            .ok()
            .and_then(|value| value.string().ok())
            .as_deref()
            == Some("AXPopover")
    })
}

fn hit_descends_from(hit: &Cf, ancestor: &Cf, pid: u32) -> Option<bool> {
    if hit.pid().ok() != Some(pid) {
        return Some(false);
    }
    let mut node = hit.retained();
    for _ in 0..8 {
        if unsafe { ffi::CFEqual(node.0, ancestor.0) } != 0 {
            return Some(true);
        }
        node = node.attr("AXParent").ok()?;
    }
    Some(false)
}

#[cfg(test)]
#[test]
fn focused_modal_dialog_is_actionable_but_parent_is_blocked() {
    assert_eq!(
        modal_blocked(Some(true), true, Some(true), None),
        Some(false)
    );
    assert_eq!(modal_blocked(None, true, None, None), Some(false));
    assert_eq!(
        modal_blocked(Some(false), false, Some(true), None),
        Some(true)
    );
    assert_eq!(
        modal_blocked(Some(true), false, Some(true), Some(true)),
        Some(true)
    );
    assert_eq!(
        modal_blocked(Some(true), false, Some(false), Some(true)),
        Some(false)
    );
    assert_eq!(
        modal_blocked(Some(true), false, Some(false), Some(false)),
        Some(true)
    );
    assert_eq!(modal_blocked(Some(true), false, Some(false), None), None);
    assert_eq!(
        modal_blocked(Some(false), false, Some(false), None),
        Some(false)
    );
    assert_eq!(modal_blocked(Some(false), false, None, None), Some(false));
    assert_eq!(modal_blocked(None, false, None, None), None);
    assert_eq!(
        modal_blocked(Some(true), false, None, Some(true)),
        Some(false)
    );
}

/// Sheets and dialogs are native child windows of their parent AX window.
/// Advisory per-process window inventory for popup delivery. Covers the same
/// AX surface as binding (AXWindows plus child sheets/dialogs and the
/// focused/main relays) and reports unknown purposes verbatim.
pub(super) fn popup_inventory(pid: u32) -> Vec<rwi_core::Popup> {
    let mut out: Vec<rwi_core::Popup> = Vec::new();
    let push = |element: &Cf, out: &mut Vec<rwi_core::Popup>| {
        if out.len() >= 16 {
            return;
        }
        let subrole = element
            .attr("AXSubrole")
            .ok()
            .and_then(|value| value.string().ok());
        let role = element
            .attr("AXRole")
            .ok()
            .and_then(|value| value.string().ok());
        let title = element
            .attr("AXTitle")
            .ok()
            .and_then(|value| value.string().ok())
            .filter(|title| !title.is_empty());
        let window =
            ffi::window_number_for_ax_element(element).map(|number| format!("cgwindow:{number}"));
        if out
            .iter()
            .any(|existing| existing.window.is_some() && existing.window == window)
        {
            return;
        }
        let kind = rwi_core::popup_kind(subrole.as_deref()).to_string();
        out.push(rwi_core::Popup {
            window,
            title,
            role,
            subrole,
            kind,
            bounds: ax_rect(element).ok().map(|r| [r.x, r.y, r.w, r.h]),
            display: None,
        });
    };
    let Ok(application) = (unsafe { Cf::owned(ffi::AXUIElementCreateApplication(pid as i32)) })
    else {
        return out;
    };
    unsafe {
        ffi::AXUIElementSetMessagingTimeout(application.0, 0.25);
    }
    if let Ok(value) = application.attr("AXWindows")
        && let Ok(windows) = unsafe { ffi::array(value.0, 64) }
    {
        for window in &windows {
            push(window, &mut out);
            if let Ok(children_value) = window.attr("AXChildren")
                && let Ok(children) = unsafe { ffi::array(children_value.0, 32) }
            {
                for child in &children {
                    if dialog_subrole(child) {
                        push(child, &mut out);
                    }
                }
            }
        }
    }
    for key in ["AXFocusedWindow", "AXMainWindow"] {
        if let Ok(element) = application.attr(key) {
            push(&element, &mut out);
        }
    }
    out
}
pub(super) fn dialog_subrole(element: &Cf) -> bool {
    // AXSheet may be exposed as the role itself with no AXSubrole (observed
    // Paint S unsaved-document sheet). Keep both forms, but never classify a
    // generic AXWindow with unknown role/subrole as a dialog.
    let role = element
        .attr("AXRole")
        .ok()
        .and_then(|value| value.string().ok());
    let subrole = element
        .attr("AXSubrole")
        .ok()
        .and_then(|value| value.string().ok());
    dialog_semantics(role.as_deref(), subrole.as_deref())
}
fn dialog_semantics(role: Option<&str>, subrole: Option<&str>) -> bool {
    [role, subrole].into_iter().flatten().any(|value| {
        matches!(
            value,
            "AXSheet" | "AXDialog" | "AXSystemDialog" | "AXPopover"
        )
    })
}
#[cfg(test)]
#[test]
fn ax_sheet_role_without_subrole_is_a_groundable_dialog() {
    assert!(dialog_semantics(Some("AXSheet"), None));
    assert!(dialog_semantics(Some("AXWindow"), Some("AXDialog")));
    assert!(!dialog_semantics(Some("AXWindow"), None));
    assert!(!dialog_semantics(None, None));
}

/// Window guards are not control observations and cannot manufacture authority.
fn read_control_metadata<T>(
    include: bool,
    read: impl FnOnce() -> Result<(Vec<T>, serde_json::Value)>,
) -> Result<(Vec<T>, serde_json::Value)> {
    if include {
        read()
    } else {
        Ok((
            Vec::new(),
            serde_json::json!({"scope":"window_context_only","complete":false,
            "controls_read":false,"input_authority":false,"absence_proven":false}),
        ))
    }
}

#[test]
fn capture_guard_skips_only_the_control_walk_not_exact_target_revalidation() {
    let mut reads = 0;
    let (elements, coverage) = read_control_metadata::<u8>(false, || {
        reads += 1;
        Ok((vec![1], serde_json::Value::Null))
    })
    .unwrap();
    assert_eq!(reads, 0);
    assert!(elements.is_empty());
    assert_eq!(coverage["controls_read"], false);
    assert_eq!(coverage["complete"], false);
    assert_eq!(coverage["input_authority"], false);
    let (elements, _) = read_control_metadata(true, || {
        reads += 1;
        Ok((vec![1], serde_json::Value::Null))
    })
    .unwrap();
    assert_eq!(reads, 1);
    assert_eq!(elements, vec![1]);
    assert!(
        read_control_metadata::<u8>(true, || Err(fault("target_replaced", "exact read refused")))
            .is_err()
    );
}

#[cfg(test)]
mod attached_sheet_policy_contract {
    use super::*;
    #[test]
    fn direct_document_sheet_blocks_only_its_parent_surface() {
        assert_eq!(with_known_attached_sheet(Some(false), true), Some(true));
        assert_eq!(with_known_attached_sheet(Some(false), false), Some(false));
        assert_eq!(with_known_attached_sheet(None, false), None);
        assert_eq!(with_known_attached_sheet(None, true), Some(true));
        assert_eq!(with_known_attached_sheet(Some(true), false), Some(true));
    }
}
