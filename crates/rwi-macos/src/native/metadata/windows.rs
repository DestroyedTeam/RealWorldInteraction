//! Exact native window/modal methods; no PID/geometry input authority.
use super::*;

impl NativeMac {
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
    pub(in crate::native) fn modal_state(
        &self,
        application: &Cf,
        window_element: &Cf,
    ) -> Option<bool> {
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
    pub(in crate::native) fn resolve_window_element(
        &self,
        application: &Cf,
        expected: Rect,
    ) -> Result<Cf> {
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
