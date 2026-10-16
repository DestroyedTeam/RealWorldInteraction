//! Explicit WindowServer movement, not a background fallback or activation.
use super::*;

pub(super) fn guard(
    native: &NativeMac,
    expected: &rwi_core::ForegroundState,
    point: [f64; 2],
) -> Result<()> {
    expected.validate()?;
    operation::checkpoint("foreground_pointer_guard")?;
    selected_scope(expected, native.pid, &native.window_name(), &native.process)?;
    require_display_point(&displays()?, point)?;
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
    anchor_native::require_foreground_pointer_hit(native, point)?;
    if activation::foreground_state()? != *expected {
        return Err(fault(
            "foreground_changed",
            "Exact foreground receiver changed; no global event or background fallback.",
        ));
    }
    crate::recovery::input_resources(unsafe { libc::geteuid() })?;
    Ok(())
}

pub(super) fn require_display_point(displays: &[Display], point: [f64; 2]) -> Result<()> {
    let mut matching = displays.iter().filter(|d| d.rect.contains(point));
    let first = matching.next();
    if point.iter().any(|v| !v.is_finite())
        || first.is_none_or(|d| !d.rotation.is_finite() || d.rotation.abs() >= 0.001)
        || matching.next().is_some()
    {
        return Err(fault(
            "foreground_point_unmapped",
            "A system-cursor event requires exactly one unrotated active display at its point; no clamp, warp fallback or background substitution.",
        ));
    }
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
        assert!(source.contains("foreground_move::guard(self, foreground, target.point)"));
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
    #[test]
    fn window_local_reference_does_not_authorize_an_offscreen_system_cursor() {
        let screen = Display {
            id: 4,
            uuid: None,
            built_in: false,
            rect: Rect {
                x: 1800.,
                y: 0.,
                w: 1920.,
                h: 1080.,
            },
            pixels: [1920, 1080],
            rotation: 0.,
        };
        assert!(require_display_point(std::slice::from_ref(&screen), [3300., 700.]).is_ok());
        for point in [[3800., 700.], [f64::NAN, 700.], [3720., 700.]] {
            assert_eq!(
                require_display_point(std::slice::from_ref(&screen), point)
                    .unwrap_err()
                    .code,
                "foreground_point_unmapped"
            );
        }
        let mut rotated = screen.clone();
        rotated.rotation = 90.;
        assert!(require_display_point(&[rotated], [3300., 700.]).is_err());
        assert!(require_display_point(&[screen.clone(), screen], [3300., 700.]).is_err());
        assert!(require_display_point(&[], [3300., 700.]).is_err());
    }
}
