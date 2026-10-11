//! Explicit WindowServer movement, not a background fallback or activation.
use super::*;

pub(super) fn guard(native: &NativeMac, expected: &rwi_core::ForegroundState) -> Result<()> {
    expected.validate()?;
    operation::checkpoint("foreground_pointer_guard")?;
    selected_scope(expected, native.pid, &native.window_name(), &native.process)?;
    if console_identity()? != native.identity || process_epoch(native.pid)? != native.process {
        return Err(fault(
            "session_or_process_changed",
            "Pinned native identity changed; no global event.",
        ));
    }
    if !unsafe { ffi::CGPreflightPostEventAccess() }
        || !unsafe { ffi::CGPreflightListenEventAccess() }
    {
        return Err(fault(
            "input_permission_missing",
            "Foreground pointer requires existing post/listen permissions.",
        ));
    }
    if crate::activity::seconds_since_hid_input().is_none_or(|seconds| seconds < 1.2) {
        return Err(fault(
            "user_active",
            "Foreground pointer yields to recent or unknown HID activity; no replay.",
        ));
    }
    if activation::foreground_state()? != *expected {
        return Err(fault(
            "foreground_changed",
            "Exact foreground receiver changed; no global event or background fallback.",
        ));
    }
    crate::recovery::input_resources(unsafe { libc::geteuid() })?;
    Ok(())
}

fn selected_scope(
    expected: &rwi_core::ForegroundState,
    pid: u32,
    window: &str,
    process: &str,
) -> Result<()> {
    if expected.pid != pid || expected.window != window || expected.process_start != process {
        return Err(fault(
            "foreground_scope_mismatch",
            "Foreground pointer must address the exact selected window/process; no global event.",
        ));
    }
    Ok(())
}

pub(super) fn post(event: &CGEvent) {
    event.post(core_graphics::event::CGEventTapLocation::HID);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreground_pointer_exact_window_and_process_do_not_inherit_pid_authority() {
        let expected = rwi_core::ForegroundState {
            pid: 42,
            window: "cgwindow:7".into(),
            process_start: "42:1:2".into(),
            native_epoch: "a".repeat(64),
        };
        assert!(selected_scope(&expected, 42, "cgwindow:7", "42:1:2").is_ok());
        for (pid, window, birth) in [
            (43, "cgwindow:7", "42:1:2"),
            (42, "cgwindow:8", "42:1:2"),
            (42, "cgwindow:7", "42:2:3"),
        ] {
            assert_eq!(
                selected_scope(&expected, pid, window, birth)
                    .unwrap_err()
                    .code,
                "foreground_scope_mismatch"
            );
        }
    }
    #[test]
    fn foreground_pointer_route_is_explicit_and_does_not_replace_background_delivery() {
        let source = include_str!("../input.rs");
        assert!(source.contains("MovePointerOptions::parse(value)"));
        assert!(source.contains("foreground_move::guard(self, foreground)"));
        assert!(source.contains("foreground_move::post(&event)"));
        assert!(
            source.contains("crate::background::deliver_pointer(&event, &delivery, target.point)")
        );
        assert!(source.contains("pointer && !foreground_pointer"));
        let event = move_pointer::event([50., 75.]).unwrap();
        assert_eq!(event.get_type() as u32, CGEventType::MouseMoved as u32);
        assert_eq!(event.get_flags(), CGEventFlags::empty());
        // Construction/wiring only: no posting, cursor movement or hover claim.
    }
}
