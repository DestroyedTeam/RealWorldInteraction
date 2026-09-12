//! Grounded SendInput only. No focus stealing, clipboard, PostMessage or UIA mutation.
mod trajectory;
use crate::{
    InputGrant, WindowRecord, desktop,
    input_plan::{self, Batch, GuardPhases, Plan, Sink},
    uia::InputScope,
};
use rcu_core::{
    Identity, MutationContext, Resolved, Result, fault,
    operation::{checkpoint, native_call},
};
use std::{
    mem::size_of,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows::Win32::UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*};

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Default, serde::Serialize)]
pub(crate) struct Counters {
    pub sendinput_calls: u64,
    pub events_inserted: u64,
    pub release_cleanup_calls: u64,
    pub release_events_inserted: u64,
    pub last_drag: Option<crate::input_trajectory::DragStats>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch(
    grant: &InputGrant,
    grant_deadline: Instant,
    identity: &Identity,
    window: &WindowRecord,
    target: &Resolved,
    context: Option<&MutationContext>,
    plan: &Plan,
    counters: &mut Counters,
    agent_authority: Option<&crate::agent_binding::AgentInputAuthority>,
) -> Result<Option<bool>> {
    if plan.drag.is_some() {
        counters.last_drag = None;
    }
    grant.validate(identity, now_ms())?;
    if !grant.allows(window) {
        return Err(fault(
            "input_window_not_allocated",
            "Only the granted root and native-owned dialogs may receive input.",
        ));
    }
    let scope = InputScope::new(
        desktop::hwnd(&window.window)?,
        window.pid,
        window
            .native_epoch
            .as_deref()
            .ok_or_else(|| fault("window_epoch_unknown", "Reobserve."))?,
        target.scope.element.as_deref(),
        target.point,
        plan.keyboard,
        plan.text,
    )?;
    let mut sink = NativeSink {
        grant,
        identity,
        window,
        target,
        context,
        first_batch: true,
        current_point: target.point,
        owned_left: false,
        trajectory_clock: Instant::now(),
        scope,
        last_clock: now_ms(),
        grant_deadline,
        counters,
        display_layout: serde_json::to_value(desktop::displays()?).map_err(|_| {
            fault(
                "display_mapping_unavailable",
                "Native display serialization failed.",
            )
        })?,
        agent_selection: agent_authority
            .map(|authority| authority.retain_selection(window))
            .transpose()?,
        feedback: crate::pointer_feedback::Gesture::new(),
    };
    if let Some(path) = &plan.drag {
        let stats = crate::input_trajectory::execute(path, target.point, &mut sink)?;
        sink.counters.last_drag = Some(stats);
    } else {
        input_plan::execute(plan, &mut sink)?;
    }
    sink.feedback.finish();
    // Only an insertion count. The core always returns goal_verified=false.
    Ok(Some(true))
}

struct NativeSink<'a> {
    grant: &'a InputGrant,
    identity: &'a Identity,
    window: &'a WindowRecord,
    target: &'a Resolved,
    context: Option<&'a MutationContext>,
    first_batch: bool,
    current_point: [f64; 2],
    owned_left: bool,
    trajectory_clock: Instant,
    scope: InputScope,
    last_clock: u64,
    grant_deadline: Instant,
    counters: &'a mut Counters,
    display_layout: serde_json::Value,
    agent_selection: Option<crate::agent_binding::AgentInputSelection<'a>>,
    feedback: crate::pointer_feedback::Gesture,
}
impl NativeSink<'_> {
    fn check_allocation_clock(&mut self) -> Result<()> {
        if Instant::now() >= self.grant_deadline {
            return Err(fault(
                "input_grant_expired",
                "Allocated monotonic deadline passed; no further input.",
            ));
        }
        let now = now_ms();
        if now < self.last_clock {
            return Err(fault(
                "clock_rollback",
                "Input slot clock moved backwards; no further input.",
            ));
        }
        self.last_clock = now;
        self.grant.validate(self.identity, now)
    }
}
impl GuardPhases for NativeSink<'_> {
    fn document_precondition(&mut self) -> Result<()> {
        self.check_allocation_clock()?;
        if let Some(selection) = &self.agent_selection {
            selection.verify_root_runtime()?;
        }
        if self.first_batch
            && let Some(context) = self.context
            && let Some(expected_document) = &context.document_state
        {
            let walk = crate::uia::walk(
                desktop::hwnd(&self.window.window)?,
                self.window.pid,
                self.window
                    .native_epoch
                    .as_deref()
                    .ok_or_else(|| fault("window_epoch_unknown", "Reobserve."))?,
            )?;
            if walk.document_state.as_deref() != Some(expected_document) {
                return Err(fault(
                    "input_document_changed",
                    "Document state changed before the first native batch; no input.",
                ));
            }
        }
        Ok(())
    }
    fn exact_input_scope(&mut self) -> Result<()> {
        // The document walk can block or pump provider work. Its return is not
        // proof that the previously focused/hit-tested element is still current.
        self.scope.check()
    }
    fn final_native_state(&mut self) -> Result<()> {
        self.check_allocation_clock()?;
        if &desktop::identity()? != self.identity {
            return Err(fault(
                "session_mismatch",
                "Native desktop/logon changed; no further input.",
            ));
        }
        if serde_json::to_value(desktop::displays()?).ok().as_ref() != Some(&self.display_layout) {
            return Err(fault(
                "input_display_changed",
                "Physical display topology changed; no further input.",
            ));
        }
        let current = desktop::record(desktop::hwnd(&self.window.window)?, 0)?;
        if self
            .context
            .is_some_and(|c| current.title_truncated || current.title != c.window_title)
        {
            return Err(fault(
                "input_window_title_changed",
                "Window context changed before input; no implicit retry.",
            ));
        }
        if !current.foreground
            || !current.enabled
            || !current.visible
            || current.minimized
            || current.cloaked != Some(false)
            || current.native_epoch != self.window.native_epoch
            || current.pid != self.window.pid
            || current.rectangle != self.window.rectangle
            || current.dwm_visible_frame != self.window.dwm_visible_frame
            || current.window_dpi != self.window.window_dpi
            || current.owner_chain != self.window.owner_chain
            || self
                .context
                .is_some_and(|c| current.title_truncated || current.title != c.window_title)
            || !self.grant.allows(&current)
        {
            return Err(fault(
                "input_focus_or_window_changed",
                "Allocated foreground window/epoch/geometry changed; no further input; never refocus automatically.",
            ));
        }
        if desktop::hit_window(self.current_point).as_deref() != Some(&self.window.window) {
            return Err(fault(
                "input_target_occluded",
                "Native hit-test is outside the allocated window; no input.",
            ));
        }
        if let Some(selection) = &self.agent_selection {
            selection.verify_native()?;
        }
        // No further document walk follows the exact UIA/native checks. This
        // closes the avoidable stale-guard interval, not the OS SendInput race.
        if desktop::window_id(unsafe { GetForegroundWindow() }) != self.window.window {
            return Err(fault(
                "input_focus_changed",
                "Foreground changed during the native guard; no input.",
            ));
        }
        self.check_allocation_clock()?;
        physical_input_clear(self.owned_left)
    }
}
impl Sink for NativeSink<'_> {
    fn guard(&mut self) -> Result<()> {
        input_plan::guard_before_batch(self)
    }
    fn send(&mut self, batch: &Batch) -> Result<()> {
        let (events, releases) = events(batch, self.target.point)?;
        checkpoint("before_sendinput")?;
        let call = native_call("sendinput_balanced_gesture")?;
        self.counters.sendinput_calls += 1;
        let inserted = unsafe { SendInput(&events, size_of::<INPUT>() as i32) };
        self.counters.events_inserted += u64::from(inserted);
        drop(call);
        if inserted != events.len() as u32 {
            // Release-only cleanup is intentionally allowed after cancellation.
            // Never retry a down event or replay an uncertain gesture.
            let released = if releases.is_empty() {
                0
            } else {
                self.counters.release_cleanup_calls += 1;
                unsafe { SendInput(&releases, size_of::<INPUT>() as i32) }
            };
            self.counters.release_events_inserted += u64::from(released);
            return Err(fault(
                "input_partial_unconfirmed",
                &format!(
                    "SendInput inserted {inserted}/{} events; release-only cleanup inserted {released}/{}. Effect and UIPI cause are not inferred. No replay.",
                    events.len(),
                    releases.len()
                ),
            ));
        }
        if matches!(batch, Batch::Click { .. } | Batch::Wheel(_)) {
            self.feedback.show(self.window, self.target.point, false);
        }
        self.first_batch = false;
        Ok(())
    }
    fn pause(&mut self) {
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn physical_input_clear(owned_left: bool) -> Result<()> {
    // Do not cancel, release or otherwise alter keys held by the user. Escape
    // additionally provides a local stop signal without registering a hotkey.
    for key in 1..=254 {
        if key == 1 && owned_left {
            continue;
        }
        if unsafe { GetAsyncKeyState(key) } < 0 {
            return Err(fault(
                "user_input_active",
                "A key or mouse button is currently held; stop without changing user input state.",
            ));
        }
    }
    if owned_left && unsafe { GetAsyncKeyState(1) } >= 0 {
        return Err(fault(
            "drag_button_state_changed",
            "The injected left button is no longer held; stop the path and release only.",
        ));
    }
    Ok(())
}

fn key(vk: u16, unicode: Option<u16>, up: bool) -> INPUT {
    let mut flags = if up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    if unicode.is_some() {
        flags |= KEYEVENTF_UNICODE;
    } else if [0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2e].contains(&vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(if unicode.is_some() { 0 } else { vk }),
                wScan: unicode.unwrap_or(0),
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
fn mouse(flags: MOUSE_EVENT_FLAGS, data: u32, x: i32, y: i32) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: x,
                dy: y,
                mouseData: data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
fn move_to(point: [f64; 2]) -> Result<INPUT> {
    let x = input_plan::absolute_axis(
        point[0],
        unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) },
        unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
    )?;
    let y = input_plan::absolute_axis(
        point[1],
        unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) },
        unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) },
    )?;
    Ok(mouse(
        MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
        0,
        x,
        y,
    ))
}
fn events(batch: &Batch, point: [f64; 2]) -> Result<(Vec<INPUT>, Vec<INPUT>)> {
    let mut out = vec![];
    let mut release = vec![];
    match batch {
        Batch::Unicode(units) => {
            for unit in units {
                out.extend([key(0, Some(*unit), false), key(0, Some(*unit), true)]);
                release.push(key(0, Some(*unit), true));
            }
        }
        Batch::Chord(keys) => {
            out.extend(keys.iter().map(|k| key(*k, None, false)));
            release.extend(keys.iter().rev().map(|k| key(*k, None, true)));
            out.extend(release.iter().copied());
        }
        Batch::Click { right } => {
            let (down, up) = if *right {
                (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP)
            } else {
                (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP)
            };
            out.extend([move_to(point)?, mouse(down, 0, 0, 0), mouse(up, 0, 0, 0)]);
            release.push(mouse(up, 0, 0, 0));
        }
        Batch::Wheel(units) => {
            out.extend([
                move_to(point)?,
                mouse(MOUSEEVENTF_WHEEL, *units as u32, 0, 0),
            ]);
        }
    }
    Ok((out, release))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoded_batches_are_balanced_and_no_clipboard_or_focus_api_is_needed() {
        let (unicode, cleanup) = events(&Batch::Unicode(vec![0xd83e, 0xdd80]), [0., 0.]).unwrap();
        assert_eq!(unicode.len(), 4);
        assert_eq!(cleanup.len(), 2);
        assert_eq!(unsafe { unicode[0].Anonymous.ki.wVk }.0, 0);
        assert_eq!(
            unsafe { unicode[1].Anonymous.ki.dwFlags },
            KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
        );
        let (chord, cleanup) = events(&Batch::Chord(vec![0x11, 0x53]), [0., 0.]).unwrap();
        assert_eq!(chord.len(), 4);
        assert_eq!(cleanup.len(), 2);
        assert_eq!(unsafe { chord[2].Anonymous.ki.wVk }.0, 0x53);
        assert_eq!(unsafe { chord[3].Anonymous.ki.wVk }.0, 0x11);
    }
}
