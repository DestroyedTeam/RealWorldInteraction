//! A logical AX dialog is independently selectable. It is never equated with a
//! differently numbered compositor surface, even when title/PID/bounds look alike.
use super::*;

pub(crate) fn focused_dialog_window(pid: u32) -> Result<Option<Window>> {
    let _call = operation::native_call("macos_focused_ax_dialog")?;
    let birth = process_epoch(pid)?;
    if unsafe { ffi::AXIsProcessTrusted() } == 0 {
        return Err(fault(
            "accessibility_permission_missing",
            "Focused-dialog discovery needs existing AX permission.",
        ));
    }
    let app = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(pid as i32))? };
    unsafe {
        ffi::AXUIElementSetMessagingTimeout(app.0, 0.05);
    }
    match app.attr("AXHidden").ok().and_then(|v| v.boolean()) {
        Some(true) => return Ok(None),
        Some(false) => (),
        None => {
            return Err(fault(
                "semantic_app_visibility_unknown",
                "Application hidden state is unknown; no logical dialog is offered.",
            ));
        }
    }
    let Ok(focused_window) = app.attr("AXFocusedWindow") else {
        return Ok(None);
    };
    if focused_window.pid()? != pid {
        return Ok(None);
    }
    // Some applications keep AXFocusedWindow on the document while their
    // focused control belongs to a separate dialog (observed: Paint's Picked
    // Color panel). Follow that native AXWindow association, not an AXWindows
    // title/bounds guess. This is a semantic-only dialog, never pixel authority.
    let (dialog, focus_owner) = if metadata::dialog_subrole(&focused_window) {
        (focused_window.retained(), None)
    } else {
        let Ok(focused_element) = app.attr("AXFocusedUIElement") else {
            return Ok(None);
        };
        let Ok(owner) = focused_element.attr("AXWindow") else {
            return Ok(None);
        };
        if focused_element.pid()? != pid
            || owner.pid()? != pid
            || !distinct_focused_control_dialog(
                metadata::dialog_subrole(&owner),
                ffi::window_number_for_ax_element(&owner),
                ffi::window_number_for_ax_element(&focused_window),
            )
        {
            return Ok(None);
        }
        (owner, Some(focused_element))
    };
    if dialog.pid()? != pid
        || !metadata::dialog_subrole(&dialog)
        || dialog.attr("AXMinimized").ok().and_then(|v| v.boolean()) == Some(true)
    {
        return Ok(None);
    }
    let Some(id) = ffi::window_number_for_ax_element(&dialog) else {
        return Ok(None);
    };
    let bounds = ax_rect(&dialog)?;
    let role = dialog.attr("AXRole")?.string()?;
    let direct_sheet = if role == "AXSheet" && focus_owner.is_none() {
        let main = app.attr("AXMainWindow")?;
        main.pid()? == pid
            && direct_focused_sheet(&role, id, ffi::window_number_for_ax_element(&main))
    } else {
        false
    };
    let hit_grounded = (|| -> Result<bool> {
        let hit = hit_test(&app, [bounds.x + bounds.w / 2., bounds.y + bounds.h / 2.])?;
        let containing = if unsafe { ffi::CFEqual(hit.0, dialog.0) } != 0 {
            hit.retained()
        } else {
            hit.attr("AXWindow")?
        };
        Ok(unsafe { ffi::CFEqual(containing.0, dialog.0) } != 0
            && ffi::window_number_for_ax_element(&containing) == Some(id)
            && hit.pid()? == pid)
    })()
    .unwrap_or(false);
    // App-level point hit-testing can land on the underlying document even
    // while a genuine AXSheet is focused (Paint S Save prompt). Only that
    // exact sheet role may use direct native AX actions without a visual/pixel
    // surrogate; its own CG ID/PID/rect and focus are checked below and again
    // at bind/action time. A distinct dialog owning the exact focused control
    // has a separate native focus proof, also rechecked below. Other generic
    // AXWindows still need the matching hit.
    if !hit_grounded && !direct_sheet && focus_owner.is_none() {
        return Err(fault(
            "semantic_dialog_not_hit",
            "The dialog lacks an exact application hit-test receiver, focused AXSheet, or focused-control owner proof.",
        ));
    }
    let inventory = window_inventory_with_options(0)?;
    let matches = inventory
        .windows
        .into_iter()
        .filter(|w| w.id == id)
        .collect::<Vec<_>>();
    let window = validate_logical_window(matches, pid, id, bounds)?;
    let after = app.attr("AXFocusedWindow")?;
    if process_epoch(pid)? != birth || unsafe { ffi::CFEqual(after.0, focused_window.0) } == 0 {
        return Err(fault(
            "semantic_dialog_changed",
            "Logical dialog changed during independent native readback.",
        ));
    }
    let after_dialog = if let Some(before_element) = focus_owner {
        let after_element = app.attr("AXFocusedUIElement")?;
        let owner = after_element.attr("AXWindow")?;
        if unsafe { ffi::CFEqual(before_element.0, after_element.0) } == 0
            || unsafe { ffi::CFEqual(dialog.0, owner.0) } == 0
            || after_element.pid()? != pid
            || owner.pid()? != pid
            || !metadata::dialog_subrole(&owner)
            || ffi::window_number_for_ax_element(&owner) != Some(id)
        {
            return Err(fault(
                "semantic_dialog_changed",
                "Exact focused control or its native dialog owner changed during readback.",
            ));
        }
        owner
    } else {
        after
    };
    if !ax_rect(&after_dialog)?.same(bounds) {
        return Err(fault(
            "semantic_dialog_changed",
            "Logical dialog geometry changed during native focus readback.",
        ));
    }
    Ok(Some(window))
}
fn distinct_focused_control_dialog(
    dialog: bool,
    owner_id: Option<u32>,
    focused_window_id: Option<u32>,
) -> bool {
    dialog
        && matches!((owner_id, focused_window_id), (Some(owner), Some(window)) if owner != 0 && window != 0 && owner != window)
}
fn direct_focused_sheet(role: &str, sheet_id: u32, main_id: Option<u32>) -> bool {
    role == "AXSheet" && main_id.is_some_and(|id| id != sheet_id)
}
fn validate_logical_window(
    mut matches: Vec<Window>,
    pid: u32,
    id: u32,
    bounds: Rect,
) -> Result<Window> {
    if matches.len() != 1
        || matches[0].id != id
        || matches[0].pid != pid
        || !matches[0].rect.same(bounds)
        || !matches[0].rect.valid()
    {
        return Err(fault(
            "semantic_window_mismatch",
            "Logical AX window must independently match its own CG ID, PID and geometry; no visual surrogate.",
        ));
    }
    Ok(matches.remove(0))
}
impl NativeMac {
    pub(crate) fn window_record(&self) -> Result<Window> {
        if self.semantic_only {
            return focused_dialog_window(self.pid)?
                .filter(|w| w.id == self.window)
                .ok_or_else(|| {
                    fault(
                        "semantic_dialog_changed",
                        "The exact bound logical dialog is no longer the focused native surface.",
                    )
                });
        }
        let found = windows()?
            .into_iter()
            .find(|w| w.id == self.window && w.pid == self.pid);
        if let Some(window) = found {
            return Ok(window);
        }
        // An inactive shared panel can retain its native record while being
        // removed from the onscreen list. Explain that distinction without
        // returning an offscreen Window, input binding, or activation fallback.
        let probe = window_record_probe::read(self.window);
        operation::checkpoint("macos_missing_window_probe_complete")?;
        Err(window_record_probe::missing(self.pid, self.window, probe))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn window() -> Window {
        Window {
            id: 3849,
            pid: 42,
            rect: Rect {
                x: 625.,
                y: 205.,
                w: 262.,
                h: 259.,
            },
            alpha: 1.,
            title: Some(String::new()),
        }
    }
    #[test]
    fn only_a_distinct_focused_ax_sheet_can_skip_unreliable_app_hit_testing() {
        assert!(direct_focused_sheet("AXSheet", 5614, Some(5100)));
        assert!(!direct_focused_sheet("AXWindow", 5614, Some(5100)));
        assert!(!direct_focused_sheet("AXSheet", 5614, Some(5614)));
        assert!(!direct_focused_sheet("AXSheet", 5614, None));
    }
    #[test]
    fn focused_control_can_ground_its_own_distinct_dialog_not_an_ordinary_window() {
        assert!(distinct_focused_control_dialog(
            true,
            Some(9763),
            Some(2189)
        ));
        assert!(!distinct_focused_control_dialog(
            false,
            Some(9763),
            Some(2189)
        ));
        assert!(!distinct_focused_control_dialog(
            true,
            Some(2189),
            Some(2189)
        ));
    }
    #[test]
    fn focused_control_owner_needs_two_known_nonzero_native_window_ids() {
        for (owner, window) in [
            (None, Some(2189)),
            (Some(9763), None),
            (None, None),
            (Some(0), Some(2189)),
            (Some(9763), Some(0)),
        ] {
            assert!(!distinct_focused_control_dialog(true, owner, window));
        }
    }
    #[test]
    fn logical_window_never_borrows_a_render_surface_number_or_geometry() {
        let w = window();
        let rect = w.rect;
        assert!(validate_logical_window(vec![w.clone()], 42, 3849, rect).is_ok());
        assert!(validate_logical_window(vec![w.clone()], 41, 3849, rect).is_err());
        assert!(validate_logical_window(vec![w.clone()], 42, 3850, rect).is_err());
        assert!(validate_logical_window(vec![w.clone(), w.clone()], 42, 3849, rect).is_err());
        assert!(validate_logical_window(vec![], 42, 3849, rect).is_err());
        assert!(
            validate_logical_window(
                vec![w],
                42,
                3849,
                Rect {
                    x: 550.,
                    y: 130.,
                    w: 412.,
                    h: 409.
                }
            )
            .is_err()
        );
    }
}

#[cfg(test)]
#[test]
fn missing_window_has_read_only_exact_server_probe_wiring() {
    let production = include_str!("semantic.rs")
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    assert!(
        production.contains("window_record_probe"),
        "missing exact WindowServer read-only diagnostic; onscreen absence is not destruction"
    );
}
