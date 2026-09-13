//! Explicit read-only resource reconciliation; never sends release events.
use crate::{ffi, session};
use rcu_core::{Result, fault, operation};
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
    if states
        .iter()
        .any(|s| !s.keys.is_empty() || !s.buttons.is_empty() || s.flags & MOMENTARY_FLAGS != 0)
    {
        return Err(fault(
            "native_input_resources_held",
            "Native keys, buttons or momentary modifiers are still down. Do not inject cleanup or clear uncertainty; wait for an explicitly authorized fresh resource check.",
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
        "states":states.iter().map(|s|json!({"state_id":s.source,"flags":s.flags.to_string()})).collect::<Vec<_>>(),
        "keycode_range":[0,255],"button_range":[0,31],"momentary_modifier_mask":MOMENTARY_FLAGS.to_string(),
        "coverage":"bounded HID and combined-session samples, non-atomic; not proof that every OS event queue is empty or that no human will input next",
        "automatic_release_events":false,"old_operations_replayable":false
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn either_state_source_or_momentary_modifier_prevents_false_release_proof() {
        for source in [0, 1] {
            for held in 0..3 {
                let mut state = State {
                    source,
                    keys: vec![],
                    buttons: vec![],
                    flags: 0,
                };
                match held {
                    0 => state.keys.push(36),
                    1 => state.buttons.push(0),
                    _ => state.flags = 1 << 20,
                }
                assert!(require_released(&[state]).is_err());
            }
        }
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
