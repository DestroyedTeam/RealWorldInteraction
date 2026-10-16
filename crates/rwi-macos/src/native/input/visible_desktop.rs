//! Explicit physical hover over an exposed window; never activate or click it.
use super::*;

pub(super) fn guard(
    native: &NativeMac,
    expected_foreground: &rwi_core::ForegroundState,
    point: [f64; 2],
) -> Result<()> {
    expected_foreground.validate()?;
    operation::checkpoint("visible_desktop_pointer_guard")?;
    foreground_move::require_display_point(&displays()?, point)?;
    if console_identity()? != native.identity || process_epoch(native.pid)? != native.process {
        return Err(fault(
            "session_or_process_changed",
            "Pinned target changed; no system event.",
        ));
    }
    if !unsafe {
        ffi::AXIsProcessTrusted() != 0
            && ffi::CGPreflightPostEventAccess()
            && ffi::CGPreflightListenEventAccess()
    } {
        return Err(fault(
            "input_permission_missing",
            "Existing AX/post/listen permissions required; no prompt.",
        ));
    }
    if crate::activity::seconds_since_hid_input().is_none_or(|seconds| seconds < 1.2) {
        return Err(fault(
            "user_active",
            "Physical hover yields to recent or unknown HID activity; no replay.",
        ));
    }
    // Unlike an application-scoped AX hit, a SYSTEM-WIDE hit must identify the
    // actual exposed receiver. Never configure the global AX messaging timeout.
    let app = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(native.pid as i32))? };
    let window = native.resolve_window_element(&app, native.window_record()?.rect)?;
    let system = unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide())? };
    let hit = hit_test(&system, point)?;
    let same_window = unsafe { ffi::CFEqual(hit.0, window.0) != 0 }
        || hit
            .attr("AXWindow")
            .ok()
            .is_some_and(|w| unsafe { ffi::CFEqual(w.0, window.0) != 0 });
    if !physical_hit_matches(hit.pid()? == native.pid, same_window, ax_rect(&hit)?, point) {
        return Err(fault(
            "visible_desktop_hit_unproven",
            "System-wide hit is not the exact exposed target; no cursor move or fallback.",
        ));
    }
    require_nonsecure_input(&element_description(&hit)?)?;
    unchanged_foreground(expected_foreground, &activation::foreground_state()?)?;
    crate::recovery::input_resources(unsafe { libc::geteuid() })?;
    Ok(())
}

fn physical_hit_matches(owner: bool, window: bool, bounds: Rect, point: [f64; 2]) -> bool {
    owner
        && window
        && bounds.valid()
        && point.iter().all(|p| p.is_finite())
        && bounds.contains(point)
}
fn unchanged_foreground(
    expected: &rwi_core::ForegroundState,
    actual: &rwi_core::ForegroundState,
) -> Result<()> {
    if expected != actual {
        return Err(fault(
            "foreground_changed",
            "Observed foreground changed; no system move or fallback.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_desktop_physical_hit_never_inherits_application_tree_authority() {
        let bounds = Rect {
            x: 20.,
            y: 30.,
            w: 40.,
            h: 50.,
        };
        assert!(physical_hit_matches(true, true, bounds, [40., 50.]));
        for (owner, window, point) in [
            (false, true, [40., 50.]),
            (true, false, [40., 50.]),
            (true, true, [90., 50.]),
            (true, true, [f64::NAN, 50.]),
        ] {
            assert!(!physical_hit_matches(owner, window, bounds, point));
        }
    }
    #[test]
    fn visible_desktop_pins_every_foreground_identity_field_without_selected_pid_aliasing() {
        let before = rwi_core::ForegroundState {
            pid: 20,
            window: "cgwindow:3".into(),
            process_start: "20:1:2".into(),
            native_epoch: "a".repeat(64),
        };
        assert!(unchanged_foreground(&before, &before).is_ok());
        for actual in [
            rwi_core::ForegroundState {
                pid: 21,
                ..before.clone()
            },
            rwi_core::ForegroundState {
                window: "cgwindow:4".into(),
                ..before.clone()
            },
            rwi_core::ForegroundState {
                process_start: "20:2:3".into(),
                ..before.clone()
            },
            rwi_core::ForegroundState {
                native_epoch: "b".repeat(64),
                ..before.clone()
            },
        ] {
            assert_eq!(
                unchanged_foreground(&before, &actual).unwrap_err().code,
                "foreground_changed"
            );
        }
    }
}
