use super::*;

pub(super) mod settlement;

pub(crate) fn foreground_state() -> Result<ForegroundState> {
    read_foreground(|object, attribute, _| object.attr(attribute))
}

/// Only the observation after an explicit foreground request may settle one
/// failed read. The caller retains its original operation and target guards.
pub(super) fn post_dispatch_foreground_state(
    phase: &'static str,
    policy: &mut settlement::Settlement,
    check: &impl Fn() -> Result<()>,
) -> Result<ForegroundState> {
    let clock_start = std::cell::Cell::new(None);
    read_foreground(|object, attribute, site| {
        policy.read(
            settlement::ReadSite {
                phase,
                site,
                attribute,
            },
            || object.attr(attribute),
            check,
            |delay| {
                std::thread::sleep(delay);
                Ok(())
            },
            || {
                let now = Instant::now();
                let start = clock_start.get().unwrap_or(now);
                clock_start.set(Some(start));
                now.saturating_duration_since(start)
            },
        )
    })
}

fn read_foreground(
    mut attr: impl FnMut(&Cf, &'static str, &'static str) -> Result<Cf>,
) -> Result<ForegroundState> {
    let _call = operation::native_call("macos_foreground_metadata")?;
    if unsafe { ffi::AXIsProcessTrusted() } == 0 {
        return Err(fault(
            "accessibility_permission_missing",
            "Foreground identity requires existing AX permission.",
        ));
    }
    let public = public_foreground_application(None)?;
    let pid = public.pid;
    let process_start = process_epoch(pid)?;
    let app = foreground_object_timeout(unsafe {
        Cf::owned(ffi::AXUIElementCreateApplication(pid as i32))?
    })?;
    if app.pid()? != pid {
        return Err(fault(
            "foreground_identity_unknown",
            "Public PID and exact AX application disagree.",
        ));
    }
    let focused = foreground_object_timeout(attr(&app, "AXFocusedWindow", "initial_application")?)?;
    if focused.pid()? != pid {
        return Err(fault(
            "foreground_identity_unknown",
            "Focused window belongs to another provider.",
        ));
    }
    let id = ffi::window_number_for_ax_element(&focused).ok_or_else(|| {
        fault(
            "foreground_identity_unknown",
            "No exact foreground window number; do not guess.",
        )
    })?;
    let row = windows()?
        .into_iter()
        .find(|w| w.pid == pid && w.id == id)
        .ok_or_else(|| {
            fault(
                "foreground_identity_unknown",
                "Foreground has no current exact on-screen native window.",
            )
        })?;
    let after_public = public_foreground_application(Some(public.clone()))?;
    if after_public.pid != pid || after_public.same_previous != Some(true) {
        return Err(fault(
            "foreground_changed",
            "Public frontmost application changed; no replacement-app probe.",
        ));
    }
    let after_app = foreground_object_timeout(unsafe {
        Cf::owned(ffi::AXUIElementCreateApplication(after_public.pid as i32))?
    })?;
    let after_window = foreground_object_timeout(attr(
        &after_app,
        "AXFocusedWindow",
        "independent_application",
    )?)?;
    require_public_foreground_unchanged(PublicForegroundReadback {
        original_pid: pid,
        public_pid: after_public.pid,
        ax_pid: after_app.pid()?,
        birth_unchanged: process_epoch(pid)? == process_start,
        public_application_equal: after_public.same_previous,
        ax_application_equal: unsafe { ffi::CFEqual(app.0, after_app.0) } != 0,
        ax_window_equal: unsafe { ffi::CFEqual(focused.0, after_window.0) } != 0,
        geometry_equal: read_rect(&focused, &mut attr)?.same(row.rect),
    })?;
    let native_epoch = digest(
        format!(
            "{pid}:{process_start}:{id}:{:?}:{}",
            row.rect,
            digest(row.title.as_deref().unwrap_or("").as_bytes())
        )
        .as_bytes(),
    );
    Ok(ForegroundState {
        pid,
        window: format!("cgwindow:{id}"),
        process_start,
        native_epoch,
    })
}

fn read_rect(
    element: &Cf,
    attr: &mut impl FnMut(&Cf, &'static str, &'static str) -> Result<Cf>,
) -> Result<Rect> {
    let p = attr(element, "AXPosition", "focused_geometry")?.point()?;
    let s = attr(element, "AXSize", "focused_geometry")?.size()?;
    let rect = Rect {
        x: p.x,
        y: p.y,
        w: s.width,
        h: s.height,
    };
    if !rect.valid() {
        return Err(fault(
            "ax_geometry_unknown",
            "Invalid native element bounds.",
        ));
    }
    Ok(rect)
}
