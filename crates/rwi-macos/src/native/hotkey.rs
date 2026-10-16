//! Explicit global foreground keys. No postToPid, activation, text, or fallback.
use super::*;
use core_graphics::event::CGEventTapLocation;
use std::sync::atomic::{AtomicU64, Ordering};

/// Only releases allocated ups paired with this sequence's posted downs.
/// Drop also unwinds partial sequences after cancellation/lease/native guards.
struct Held<'a, T, P: FnMut(&T)> {
    releases: Vec<&'a T>,
    post: P,
}
impl<T, P: FnMut(&T)> Drop for Held<'_, T, P> {
    fn drop(&mut self) {
        while let Some(up) = self.releases.pop() {
            (self.post)(up);
        }
    }
}
fn balanced<T>(
    pairs: &[(T, T)],
    mut check: impl FnMut() -> Result<()>,
    post: impl FnMut(&T),
    mut pause: impl FnMut(),
) -> Result<()> {
    let mut held = Held {
        releases: Vec::with_capacity(pairs.len()),
        post,
    };
    for (down, up) in pairs {
        check()?;
        held.releases.push(up);
        (held.post)(down);
        pause();
    }
    Ok(())
}

fn chord_events(key: u16, flags: CGEventFlags) -> Result<Vec<(CGEvent, CGEvent)>> {
    // One private source shared by the entire chord, not hardware-derived flags.
    let source = keyboard::source()?;
    let mut current = CGEventFlags::empty();
    let mut pairs = Vec::with_capacity(5);
    let event = |code, down, state, modifier| -> Result<CGEvent> {
        let event = CGEvent::new_keyboard_event(source.clone(), code, down).map_err(|_| {
            fault(
                "input_allocation_failed",
                "Entire chord must allocate before input.",
            )
        })?;
        event.set_flags(state);
        if modifier {
            event.set_type(CGEventType::FlagsChanged);
        }
        Ok(event)
    };
    for (flag, code) in [
        (CGEventFlags::CGEventFlagControl, 59),
        (CGEventFlags::CGEventFlagAlternate, 58),
        (CGEventFlags::CGEventFlagShift, 56),
        (CGEventFlags::CGEventFlagCommand, 55),
    ] {
        if flags.contains(flag) {
            let previous = current;
            current |= flag;
            pairs.push((
                event(code, true, current, true)?,
                event(code, false, previous, true)?,
            ));
        }
    }
    pairs.push((
        event(key, true, flags, false)?,
        event(key, false, flags, false)?,
    ));
    Ok(pairs)
}

pub(crate) fn post(
    identity: &Identity,
    uid: u32,
    counter: &AtomicU64,
    key_name: &str,
    expected: &ForegroundState,
) -> Result<serde_json::Value> {
    expected.validate()?;
    let (key, flags) = keyboard::parse_chord(key_name)?;
    let pairs = chord_events(key, flags)?;
    let check = || -> Result<()> {
        operation::checkpoint("foreground_keyboard_guard")?;
        if crate::session::identity_for_uid(uid)? != *identity {
            return Err(fault(
                "session_mismatch",
                "Native user/session changed; no global fallback.",
            ));
        }
        if !unsafe { ffi::CGPreflightPostEventAccess() } {
            return Err(fault(
                "input_permission_missing",
                "System event posting requires existing permission.",
            ));
        }
        if !unsafe { ffi::CGPreflightListenEventAccess() } {
            return Err(fault(
                "listen_event_permission_missing",
                "Global input requires existing listen authorization for cooperative yield.",
            ));
        }
        if crate::activity::seconds_since_hid_input().is_none_or(|s| s < 1.2) {
            return Err(fault(
                "user_active",
                "Global keys yield to recent/unknown HID activity; no replay.",
            ));
        }
        if activation::foreground_state()? != *expected {
            return Err(fault(
                "foreground_changed",
                "Global keyboard receiver changed; no further down event.",
            ));
        }
        Ok(())
    };
    check()?;
    // This is a global operation, unlike isolated background input. Do not
    // combine this chord with already-held human/unknown keys or buttons.
    crate::recovery::input_resources(uid)?;
    let _call = operation::native_call("macos_foreground_keyboard_sequence")?;
    balanced(
        &pairs,
        check,
        |event| {
            let _ = counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                Some(n.saturating_add(1))
            });
            event.post(CGEventTapLocation::HID);
        },
        || std::thread::sleep(Duration::from_millis(8)),
    )?;
    Ok(serde_json::json!({
        "route":"CGEventPost(HID)","posted_events":pairs.len()*2,
        "paired_release_posted":true,"before":expected,
        "after":activation::foreground_state().ok(),"goal_verified":false,
        "coverage":"sampled session/foreground/HID guards; synthetic input, not continuous physical-human attestation",
        "cleanup":"own paired key-up events posted; OS queue and receiver unacknowledged"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_hotkey_balanced_sequence_releases_only_posted_keys_after_every_partial_guard() {
        let pairs = [
            ("cmd_down", "cmd_up"),
            ("shift_down", "shift_up"),
            ("tab_down", "tab_up"),
        ];
        for fail_at in 0..=3 {
            let mut checks = 0;
            let mut posted = Vec::new();
            let result = balanced(
                &pairs,
                || {
                    let n = checks;
                    checks += 1;
                    if n == fail_at {
                        Err(fault("foreground_changed", "stop"))
                    } else {
                        Ok(())
                    }
                },
                |e| posted.push(*e),
                || {},
            );
            assert_eq!(result.is_ok(), fail_at == 3);
            let mut expected = pairs.iter().take(fail_at).map(|p| p.0).collect::<Vec<_>>();
            expected.extend(pairs.iter().take(fail_at).rev().map(|p| p.1));
            assert_eq!(posted, expected);
        }
    }
    #[test]
    fn global_hotkey_cancellation_after_modifier_still_posts_its_own_release() {
        let op = operation::Registry::default().begin(None).unwrap();
        let mut posted = Vec::new();
        let result = operation::run(&op, || {
            balanced(
                &[("cmd_down", "cmd_up"), ("tab_down", "tab_up")],
                || operation::checkpoint("guard"),
                |e| {
                    posted.push(*e);
                    op.cancel();
                },
                || {},
            )
        });
        assert!(result.is_err());
        assert_eq!(posted, vec!["cmd_down", "cmd_up"]);
        op.finish();
        assert!(op.status().resources_released);
    }
    #[test]
    fn global_hotkey_chord_parser_retains_exact_physical_system_shortcuts() {
        assert_eq!(
            keyboard::parse_chord("cmd+tab").unwrap(),
            (48, CGEventFlags::CGEventFlagCommand)
        );
        assert_eq!(
            keyboard::parse_chord("command+shift+tab").unwrap(),
            (
                48,
                CGEventFlags::CGEventFlagCommand | CGEventFlags::CGEventFlagShift
            )
        );
        for value in ["", "cmd", "tab+escape", "echo hello", "cmd+\n"] {
            assert!(keyboard::parse_chord(value).is_err());
        }
    }
}
