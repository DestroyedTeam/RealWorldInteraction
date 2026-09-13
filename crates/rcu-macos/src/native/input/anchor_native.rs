//! AX continuation objects stay on the synchronous input worker's stack.
use super::*;
use continuation::{Facts, Kind};

pub(super) struct AxObject(Cf);
impl PartialEq for AxObject {
    fn eq(&self, other: &Self) -> bool {
        unsafe { ffi::CFEqual(self.0.0, other.0.0) != 0 }
    }
}

impl NativeMac {
    pub(super) fn input_anchor_facts(
        &self,
        target: &Resolved,
        intent: &Intent,
    ) -> Result<Facts<AxObject>> {
        let _call = operation::native_call("macos_input_anchor_read")?;
        let permitted =
            || unsafe { ffi::AXIsProcessTrusted() != 0 && ffi::CGPreflightPostEventAccess() };
        if !permitted() {
            return Err(fault(
                "input_permission_missing",
                "Native AX or posting permission changed; no prompt or input.",
            ));
        }
        let identity = console_identity()?;
        let process = process_epoch(self.pid)?;
        if identity != self.identity || process != self.process {
            return Err(fault(
                "session_or_process_changed",
                "No more input in a replaced native context or process.",
            ));
        }
        let system = unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide()) }?;
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(system.0, 0.05);
        }
        let app = system.attr("AXFocusedApplication")?;
        if app.pid()? != self.pid {
            return Err(fault(
                "focus_mismatch",
                "The selected application is no longer foreground.",
            ));
        }
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(app.0, 0.05);
        }
        let focused_window = app.attr("AXFocusedWindow")?;
        let rect = ax_rect(&focused_window)?;
        let all = windows()?;
        let index = all
            .iter()
            .position(|w| w.id == self.window && w.pid == self.pid && w.rect.same(rect))
            .ok_or_else(|| {
                fault(
                    "window_binding_changed",
                    "The exact pinned CGWindowID/PID/AX window bounds changed.",
                )
            })?;
        if all
            .iter()
            .filter(|w| w.pid == self.pid && w.rect.same(rect))
            .count()
            != 1
        {
            return Err(fault(
                "window_binding_ambiguous",
                "AX window no longer uniquely maps to the pinned native CGWindowID.",
            ));
        }
        let hit = hit_test(&system, target.point)?;
        if hit.pid()? != self.pid
            || unsafe { ffi::CFEqual(hit.attr("AXWindow")?.0, focused_window.0) } == 0
        {
            return Err(fault(
                "target_occluded",
                "The original action point belongs to another native window or process.",
            ));
        }
        let hit_description = element_description(&hit)?;
        require_nonsecure_input(&hit_description)?;
        let kind = Kind::of(intent);
        if matches!(kind, Kind::Text | Kind::Keyboard) {
            require_keyboard_target(&hit_description, target.scope.element.as_deref())?;
            if matches!(kind, Kind::Text) {
                require_text_target(&hit_description, target.scope.element.as_deref())?;
            }
        }
        let focused = if kind.needs_focus() {
            let element = app.attr("AXFocusedUIElement")?;
            if element.pid()? != self.pid
                || unsafe { ffi::CFEqual(element.attr("AXWindow")?.0, focused_window.0) } == 0
            {
                return Err(fault(
                    "focus_changed",
                    "The focused native element belongs to another window/context.",
                ));
            }
            Some(AxObject(element))
        } else {
            None
        };
        let (layout, mapping_ready) = display_layout(rect)?;
        let facts = Facts {
            identity,
            process,
            scope: target.scope.clone(),
            window_rect: rect,
            hit_rect: ax_rect(&hit)?,
            hit_id: hit_description.element.id,
            focused,
            point: target.point,
            layout,
            mapping_ready,
            permissions: true,
            modal: focused_window
                .attr("AXModal")
                .ok()
                .and_then(|v| v.boolean()),
            occluded: all[..index]
                .iter()
                .any(|w| w.alpha > 0.01 && w.rect.overlaps(rect)),
            enabled: hit.attr("AXEnabled").ok().and_then(|v| v.boolean()),
            writable: if matches!(kind, Kind::Text) {
                Some(hit.attribute_settable("AXValue")?)
            } else {
                None
            },
            class: hit_description.text_class,
            window: AxObject(focused_window),
            hit: AxObject(hit),
        };
        if console_identity()? != facts.identity
            || process_epoch(self.pid)? != facts.process
            || !permitted()
        {
            return Err(fault(
                "input_boundary_changed",
                "Native context/process or TCC changed during continuation readback.",
            ));
        }
        Ok(facts)
    }
}
