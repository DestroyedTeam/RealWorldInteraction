//! Explicit read-only resource reconciliation; never sends release events.
use crate::{ffi, session};
use rwi_core::{Result, fault, operation};
use serde_json::{Value, json};

// Shift, Control, Option, Command and Fn are momentary; Caps Lock is not held.
const MOMENTARY_FLAGS: u64 = (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20) | (1 << 23);

#[derive(Debug)]
struct State {
    source: i32,
    keys: Vec<u16>,
    buttons: Vec<u32>,
    flags: u64,
}
fn require_released(states: &[State]) -> Result<()> {
    // Legacy conservative reconciliation for potentially interrupted CGEvent
    // sequences, not a precondition for background input. These global tables
    // do not attribute a user's keys in another app to an RWI operation.
    // Native AX actions acquire no held key/button resources and must not
    // enter this recovery path merely because their business effect is unknown.
    // The hardware source (HIDSystemState, id 1) is the authority on physical
    // input. A key that only the combined-session state still reports down is
    // residue (e.g. a key-up swallowed by another process): our injected
    // events are process-targeted and never touch the hardware source, and
    // treating such residue as held deadlocks recovery forever (observed
    // live: keycode 118 stuck in the session state while hardware was clean,
    // fencing the input domain across agent restarts). Buttons and momentary
    // modifiers still block from either source because they can represent an
    // in-flight injected drag or a mid-chord state of ours.
    if states.iter().any(|s| {
        !s.buttons.is_empty()
            || s.flags & MOMENTARY_FLAGS != 0
            || (s.source == 1 && !s.keys.is_empty())
    }) {
        return Err(fault(
            "native_input_resources_held",
            "Global keys/buttons/modifiers are down; this does not identify an RWI-owned input. This legacy interrupted-event reconciliation cannot prove release. Do not inject cleanup or replay; require scoped ownership evidence or a fresh resource check.",
        ));
    }
    Ok(())
}
/// The caller must also hold the agent domain gate and prove all native writers idle.
pub fn input_resources(uid: u32) -> Result<Value> {
    let before = session::identity_for_uid(uid)?;
    if !unsafe { ffi::CGPreflightListenEventAccess() } {
        return Err(fault(
            "reconciliation_unavailable",
            "Input-monitoring preflight denies reliable resource-state proof. No permission is requested and no existing input authorization is changed.",
        ));
    }
    let _call = operation::native_call("macos_input_resource_readback")?;
    let mut states = Vec::new();
    for source in [0, 1] {
        // CGEventSource.h: CombinedSessionState, HIDSystemState.
        let mut state = State {
            source,
            keys: vec![],
            buttons: vec![],
            flags: 0,
        };
        for key in 0..=255u16 {
            operation::checkpoint("macos_resource_key_state")?;
            if unsafe { ffi::CGEventSourceKeyState(source, key) } {
                state.keys.push(key);
            }
        }
        for button in 0..32u32 {
            if unsafe { ffi::CGEventSourceButtonState(source, button) } {
                state.buttons.push(button);
            }
        }
        state.flags = unsafe { ffi::CGEventSourceFlagsState(source) };
        states.push(state);
    }
    if before != session::identity_for_uid(uid)? || !unsafe { ffi::CGPreflightListenEventAccess() }
    {
        return Err(fault(
            "reconciliation_context_changed",
            "Native context or resource read permission changed; keep the input domain fenced.",
        ));
    }
    require_released(&states)?;
    Ok(json!({
        "source":"CoreGraphics.CGEventSourceKeyState/ButtonState/FlagsState",
        "evidence":"native_api_observed","identity":before,"input_sent":false,
        "listen_event_preflight":true,"pressed_keys":0,"pressed_buttons":0,
        "stale_session_keys":states.iter().filter(|s|s.source==0).flat_map(|s|s.keys.clone()).collect::<Vec<_>>(),
        "states":states.iter().map(|s|json!({"state_id":s.source,"flags":s.flags.to_string()})).collect::<Vec<_>>(),
        "keycode_range":[0,255],"button_range":[0,31],"momentary_modifier_mask":MOMENTARY_FLAGS.to_string(),
        "coverage":"bounded HID and combined-session samples, non-atomic; not proof that every OS event queue is empty or that no human will input next",
        "automatic_release_events":false,"old_operations_replayable":false
    }))
}

#[cfg(test)]
mod dump_tests {
    use super::*;
    #[test]
    #[ignore = "Explicit live resource-state dump; read-only, no input"]
    fn dump_input_resource_state_diagnostic() {
        for source in [0, 1] {
            let keys: Vec<u16> = (0..=255u16)
                .filter(|key| unsafe { ffi::CGEventSourceKeyState(source, *key) })
                .collect();
            let buttons: Vec<u32> = (0..32u32)
                .filter(|button| unsafe { ffi::CGEventSourceButtonState(source, *button) })
                .collect();
            let flags = unsafe { ffi::CGEventSourceFlagsState(source) };
            println!("state_id={source} keys={keys:?} buttons={buttons:?} flags=0x{flags:016x}");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hardware_keys_buttons_and_momentary_modifiers_block_but_stale_session_keys_do_not() {
        let state = |source: i32, keys: Vec<u16>, buttons: Vec<u32>, flags: u64| State {
            source,
            keys,
            buttons,
            flags,
        };
        // Hardware-source keys block.
        assert!(require_released(&[state(1, vec![36], vec![], 0)]).is_err());
        // Buttons block from either source: they can be an in-flight drag of ours.
        for source in [0, 1] {
            assert!(require_released(&[state(source, vec![], vec![0], 0)]).is_err());
        }
        // Momentary modifiers block from either source.
        for source in [0, 1] {
            assert!(require_released(&[state(source, vec![], vec![], 1 << 20)]).is_err());
        }
        // A key that only the combined-session state still reports is residue:
        // our injected events are process-targeted, so it cannot be ours and it
        // must not deadlock recovery (observed live: keycode 118).
        assert!(require_released(&[state(0, vec![118], vec![], 0)]).is_ok());
        assert!(
            require_released(&[state(0, vec![], vec![], 0), state(1, vec![], vec![], 0)]).is_ok()
        );
    }
    #[test]
    fn caps_lock_is_not_reported_as_a_held_modifier() {
        assert!(
            require_released(&[State {
                source: 0,
                keys: vec![],
                buttons: vec![],
                flags: 1 << 16
            }])
            .is_ok()
        );
    }
}
