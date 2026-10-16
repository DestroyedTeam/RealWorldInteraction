use super::*;

pub(crate) fn foreground_state() -> Result<ForegroundState> {
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
    let focused = foreground_object_timeout(app.attr("AXFocusedWindow")?)?;
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
    let after_window = foreground_object_timeout(after_app.attr("AXFocusedWindow")?)?;
    require_public_foreground_unchanged(PublicForegroundReadback {
        original_pid: pid,
        public_pid: after_public.pid,
        ax_pid: after_app.pid()?,
        birth_unchanged: process_epoch(pid)? == process_start,
        public_application_equal: after_public.same_previous,
        ax_application_equal: unsafe { ffi::CFEqual(app.0, after_app.0) } != 0,
        ax_window_equal: unsafe { ffi::CFEqual(focused.0, after_window.0) } != 0,
        geometry_equal: ax_rect(&focused)?.same(row.rect),
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
