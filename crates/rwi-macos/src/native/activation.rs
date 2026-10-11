//! Explicit AX foreground route; never invoked by observe or background input.
//! Public AXRaise + writable AXFrontmost, once each, no key/cursor fallback.
use super::*;
use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};
use rwi_core::ForegroundState;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

pub(crate) fn foreground_state() -> Result<ForegroundState> {
    let _call = operation::native_call("macos_foreground_metadata")?;
    if unsafe { ffi::AXIsProcessTrusted() } == 0 {
        return Err(fault(
            "accessibility_permission_missing",
            "Foreground identity requires existing AX permission.",
        ));
    }
    let system = unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide())? };
    let app = system.attr("AXFocusedApplication")?;
    unsafe {
        ffi::AXUIElementSetMessagingTimeout(app.0, 0.05);
    }
    let pid = app.pid()?;
    let process_start = process_epoch(pid)?;
    let focused = app.attr("AXFocusedWindow")?;
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
    let after_app = system.attr("AXFocusedApplication")?;
    let after_window = after_app.attr("AXFocusedWindow")?;
    if process_epoch(pid)? != process_start
        || !ax_rect(&focused)?.same(row.rect)
        || after_app.pid()? != pid
        || unsafe { ffi::CFEqual(app.0, after_app.0) } == 0
        || unsafe { ffi::CFEqual(focused.0, after_window.0) } == 0
    {
        return Err(fault(
            "foreground_changed",
            "Foreground identity changed while reading it.",
        ));
    }
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
impl NativeMac {
    pub(crate) fn focus_exact_window(
        &self,
        reference: &Resolved,
        expected: &ForegroundState,
    ) -> Result<serde_json::Value> {
        expected.validate()?;
        if !self.allow_input
            || self.semantic_only
            || !reference.placement_only
            || reference.scope.pid != self.pid
            || reference.scope.window != self.window_name()
        {
            return Err(fault(
                "foreground_scope_unavailable",
                "Explicit foreground requires an input-authorized exact visual window-level reference.",
            ));
        }
        let _call = operation::native_call("macos_explicit_foreground")?;
        let check = || -> Result<()> {
            operation::checkpoint("foreground_guard")?;
            if console_identity()? != self.identity || process_epoch(self.pid)? != self.process {
                return Err(fault(
                    "session_or_process_changed",
                    "Pinned session/process changed; no fallback.",
                ));
            }
            // AX owns this route. CGEvent posting permission is unrelated; only
            // the existing input-observation permission is needed for HID yield.
            if !unsafe { ffi::CGPreflightListenEventAccess() } {
                return Err(fault(
                    "listen_event_permission_missing",
                    "Foreground requires existing listen authorization for cooperative HID yield.",
                ));
            }
            // Foreground changes affect the global keyboard receiver, unlike scoped
            // background pointer actions. Yield to recent HID input everywhere.
            if crate::activity::seconds_since_hid_input().is_none_or(|s| s < 1.2) {
                return Err(fault(
                    "user_active",
                    "Explicit foreground yields to recent/unknown HID input; never replay.",
                ));
            }
            Ok(())
        };
        check()?;
        let before = foreground_state()?;
        if &before != expected {
            return Err(fault(
                "foreground_changed",
                "Foreground changed after the supplied observation; no focus request sent.",
            ));
        }
        let row = self.window_record()?;
        let app = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(app.0, 0.05);
        }
        let window = self.resolve_window_element(&app, row.rect)?;
        if window.pid()? != self.pid
            || !ax_rect(&window)?.same(row.rect)
            || ffi::window_number_for_ax_element(&window) != Some(self.window)
        {
            return Err(fault(
                "window_epoch_changed",
                "Exact AX/CG target identity is not stable.",
            ));
        }
        let is_target = |f: &ForegroundState| {
            f.pid == self.pid && f.window == self.window_name() && f.process_start == self.process
        };
        if is_target(&before) {
            return Ok(
                serde_json::json!({"state":"already_foreground","foreground":before,"native_requests":0,"goal_verified":false,"input_authority_granted":false,"coverage":"sampled_native_identity; not continuous human-input attestation"}),
            );
        }
        if !action_names(&window).iter().any(|a| a == "AXRaise")
            || !app.attribute_settable("AXFrontmost")?
        {
            return Err(fault(
                "foreground_api_unavailable",
                "Exact window must advertise AXRaise and application AXFrontmost must be writable; no shortcut or activation fallback.",
            ));
        }
        let dispatch = |object: &Cf, action: bool| -> Result<()> {
            check()?;
            // Recheck both receivers at each side-effect boundary. These are
            // sequential observations, not an atomic or continuous input proof.
            if foreground_state()? != before {
                return Err(fault(
                    "foreground_changed",
                    "Foreground changed at the dispatch boundary; no further request sent.",
                ));
            }
            let current_row = self.window_record()?;
            let current_window = self.resolve_window_element(&app, current_row.rect)?;
            if !current_row.rect.same(row.rect)
                || unsafe { ffi::CFEqual(window.0, current_window.0) } == 0
            {
                return Err(fault(
                    "window_epoch_changed",
                    "Target changed at the dispatch boundary; no further request sent.",
                ));
            }
            self.dispatches.fetch_add(1, Ordering::AcqRel);
            let code = unsafe {
                if action {
                    let name = CFString::new("AXRaise");
                    ffi::AXUIElementPerformAction(object.0, name.as_concrete_TypeRef().cast())
                } else {
                    let name = CFString::new("AXFrontmost");
                    let yes = CFBoolean::true_value();
                    ffi::AXUIElementSetAttributeValue(
                        object.0,
                        name.as_concrete_TypeRef().cast(),
                        yes.as_concrete_TypeRef().cast(),
                    )
                }
            };
            if code != 0 {
                return Err(fault(
                    "foreground_native_error",
                    "A native foreground request returned an error; an effect may still occur. Observe, never retry.",
                ));
            }
            Ok(())
        };
        dispatch(&window, true)?;
        let after_raise = foreground_state()?;
        if !is_target(&after_raise) && after_raise != before {
            return Err(fault(
                "foreground_changed",
                "Another foreground appeared after raise; no further request sent.",
            ));
        }
        if !is_target(&after_raise) {
            dispatch(&app, false)?;
        }
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            check()?;
            let now = foreground_state()?;
            let current_row = self.window_record()?;
            let current_window = self.resolve_window_element(&app, current_row.rect)?;
            if !current_row.rect.same(row.rect)
                || unsafe { ffi::CFEqual(window.0, current_window.0) } == 0
            {
                return Err(fault(
                    "window_epoch_changed",
                    "Target changed after a foreground request; never replay.",
                ));
            }
            if is_target(&now) {
                return Ok(
                    serde_json::json!({"state":"foreground_observed","before_foreground":before,"after_foreground":now,
                    "goal_verified":false,"input_authority_granted":false,"coverage":"sampled_native_identity; not continuous human-input attestation",
                    "route":"public_AXRaise_then_AXFrontmost_once; no keyboard/warp/unhide/restore"}),
                );
            }
            if now != before {
                return Err(fault(
                    "foreground_changed",
                    "Another foreground appeared; yield without restoration or replay.",
                ));
            }
            if Instant::now() >= deadline {
                return Err(fault(
                    "foreground_observation_timeout",
                    "Foreground not observed within 500ms; effect may arrive later, no retry.",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
