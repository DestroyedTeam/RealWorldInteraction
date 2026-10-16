//! Independent application-local focused-window oracle. Not OS foreground.
//! Binding.focus_window stays selected-window grounding, never a focus observation.
use super::*;

fn identity_matches(
    expected: &str,
    selected_scope: &str,
    selected_window: u32,
    logical: bool,
    selected_cg: Option<u32>,
    focused_cg: Option<u32>,
    exact_cf: bool,
) -> Result<bool> {
    if !logical && selected_cg != Some(selected_window) {
        return Err(fault(
            "focused_window_grounding_unknown",
            "Selected visual AX window lacks the pinned CG identity; focus remains unknown.",
        ));
    }
    if !logical && exact_cf && focused_cg != selected_cg {
        return Err(fault(
            "focused_window_grounding_unknown",
            "Exact AX window has inconsistent CG readback; focus remains unknown.",
        ));
    }
    if !exact_cf && selected_cg.is_some() && focused_cg == selected_cg && !logical {
        return Err(fault(
            "focused_window_identity_conflict",
            "Distinct AX objects share a CG number; do not infer focus from geometry or window number.",
        ));
    }
    Ok(expected == selected_scope && exact_cf)
}

pub(super) fn matched(native: &NativeMac, before: &Metadata, expected: &str) -> Result<bool> {
    let _call = operation::native_call("macos_application_focused_window_read")?;
    let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(native.pid as i32))? };
    let timeout_status = unsafe { ffi::AXUIElementSetMessagingTimeout(application.0, 0.05) };
    if timeout_status != 0 {
        return Err(fault(
            "focused_window_read_timeout_unknown",
            &format!(
                "AX messaging timeout configuration returned {timeout_status}; no focus conclusion."
            ),
        ));
    }
    let row = native.window_record()?;
    let selected = native.resolve_window_element(&application, row.rect)?;
    let focused = application.attr("AXFocusedWindow")?;
    if unsafe {
        ffi::CFGetTypeID(selected.0) != ffi::AXUIElementGetTypeID()
            || ffi::CFGetTypeID(focused.0) != ffi::AXUIElementGetTypeID()
    } {
        return Err(fault(
            "focused_window_native_type_unknown",
            "AXFocusedWindow must be an actual AXUIElement; do not infer false from a type failure.",
        ));
    }
    let selected_owner = selected.pid()?;
    let selected_epoch = delegation::validate_hosted_descendant(native.pid, selected_owner)?;
    let exact_cf = unsafe { ffi::CFEqual(selected.0, focused.0) != 0 };
    if focused.pid()? != native.pid && !exact_cf {
        return Err(fault(
            "focused_window_owner_unknown",
            "An unrelated provider is not evidence that the selected window is unfocused.",
        ));
    }
    let matched = identity_matches(
        expected,
        &native.window_name(),
        native.window,
        native.semantic_only,
        ffi::window_number_for_ax_element(&selected),
        ffi::window_number_for_ax_element(&focused),
        exact_cf,
    )?;
    let after = native.capture_guard_metadata()?;
    if before.identity != after.identity
        || before.binding != after.binding
        || before.blocked != after.blocked
    {
        return Err(fault(
            "focused_window_context_changed",
            "Selected process/session/window/context changed during focus read; discard it.",
        ));
    }
    let selected_after =
        native.resolve_window_element(&application, native.window_record()?.rect)?;
    let focused_after = application.attr("AXFocusedWindow")?;
    if unsafe {
        ffi::CFEqual(selected.0, selected_after.0) == 0
            || ffi::CFEqual(focused.0, focused_after.0) == 0
    } || process_epoch(selected_owner)? != selected_epoch
    {
        return Err(fault(
            "focused_window_changed",
            "Selected AX object/provider or application focus changed during the two reads; unknown, never retry input.",
        ));
    }
    Ok(matched)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn other_actual_window_is_not_selected_binding_success() {
        assert!(
            !identity_matches(
                "cgwindow:10",
                "cgwindow:10",
                10,
                false,
                Some(10),
                Some(11),
                false
            )
            .unwrap()
        );
        assert!(
            identity_matches(
                "cgwindow:10",
                "cgwindow:10",
                10,
                false,
                Some(10),
                Some(10),
                true
            )
            .unwrap()
        );
    }
    #[test]
    fn visual_unknown_or_conflicting_identity_is_not_false() {
        assert!(
            identity_matches("cgwindow:10", "cgwindow:10", 10, false, None, None, true).is_err()
        );
        assert!(
            identity_matches(
                "cgwindow:10",
                "cgwindow:10",
                10,
                false,
                Some(10),
                None,
                true
            )
            .is_err()
        );
        assert!(
            identity_matches(
                "cgwindow:10",
                "cgwindow:10",
                10,
                false,
                Some(10),
                Some(10),
                false
            )
            .is_err()
        );
    }
    #[test]
    fn logical_sheet_matches_only_exact_ax_object_not_parent_cg() {
        assert!(
            identity_matches("cgwindow:10", "cgwindow:10", 10, true, None, None, true).unwrap()
        );
        assert!(
            !identity_matches(
                "cgwindow:10",
                "cgwindow:10",
                10,
                true,
                Some(10),
                Some(10),
                false
            )
            .unwrap()
        );
        assert!(
            !identity_matches("cgwindow:11", "cgwindow:10", 10, true, None, None, true).unwrap()
        );
    }
}
