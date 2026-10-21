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
    // Preserve the existing policy: HID-system keys block, while buttons and
    // momentary flags block from either table. Combined-session ordinary keys
    // alone do not block (historically observed session keycode 118).
    // Explicit global hotkeys also exist: insertion location is not a source
    // table identity or evidence of which actor owns a reported down state.
    if states.iter().any(|s| {
        !s.buttons.is_empty()
            || s.flags & MOMENTARY_FLAGS != 0
            || (s.source == 1 && !s.keys.is_empty())
    }) {
        let mut error = fault(
            "native_input_resources_held",
            "Global keys/buttons/modifiers are down; this does not identify an RWI-owned input. This legacy interrupted-event reconciliation cannot prove release. Do not inject cleanup or replay; require scoped ownership evidence or a fresh resource check.",
        );
        error.diagnostic = Some(Box::new(held_resource_diagnostic(states)));
        return Err(error);
    }
    Ok(())
}

/// Only format values already sampled by input_resources; never query or post.
fn held_resource_diagnostic(states: &[State]) -> Value {
    let rows = states.iter().take(2).map(|s| {
        let modifiers = [(1 << 17, "shift"), (1 << 18, "control"),
            (1 << 19, "option"), (1 << 20, "command"), (1 << 23, "fn")]
            .into_iter().filter_map(|(mask,name)| (s.flags & mask != 0).then_some(name)).collect::<Vec<_>>();
        json!({"state_id":s.source,
            "keys":s.keys.iter().take(256).copied().collect::<Vec<_>>(),
            "key_count":s.keys.len(),"keys_truncated":s.keys.len()>256,
            "buttons":s.buttons.iter().take(32).copied().collect::<Vec<_>>(),
            "button_count":s.buttons.len(),"buttons_truncated":s.buttons.len()>32,
            "flags":s.flags.to_string(),"momentary_flags":(s.flags & MOMENTARY_FLAGS).to_string(),
            "modifiers":modifiers,
            "blocking":{"keys":s.source==1 && !s.keys.is_empty(),"buttons":!s.buttons.is_empty(),"modifiers":s.flags & MOMENTARY_FLAGS != 0}})
    }).collect::<Vec<_>>();
    json!({"scope":"current_input_resource_sample","atomic":false,
        "source":"CGEventSourceKeyState/ButtonState/FlagsState",
        "query_error_status":"not exposed by these native state queries",
        "input_ownership":"unknown","resource_probe_sent_input":false,
        "cleanup_authority":false,"replay_allowed":false,
        "momentary_modifier_mask":MOMENTARY_FLAGS.to_string(),
        "state_count":states.len(),"states_truncated":states.len()>2,"states":rows})
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
    fn f408_resource_diagnostic_retains_exact_values_without_ownership() {
        let error = require_released(&[
            State {
                source: 0,
                keys: vec![124],
                buttons: vec![0],
                flags: 1 << 20,
            },
            State {
                source: 1,
                keys: vec![123],
                buttons: vec![],
                flags: 0,
            },
        ])
        .unwrap_err();
        assert_eq!(error.code, "native_input_resources_held");
        let d = error.diagnostic.unwrap();
        assert_eq!(d["states"][0]["keys"], json!([124]));
        assert_eq!(d["states"][0]["buttons"], json!([0]));
        assert_eq!(d["states"][0]["flags"], "1048576");
        assert_eq!(d["states"][0]["modifiers"], json!(["command"]));
        assert_eq!(
            d["states"][0]["blocking"],
            json!({"keys":false,"buttons":true,"modifiers":true})
        );
        assert_eq!(d["states"][1]["state_id"], 1);
        assert_eq!(d["states"][1]["blocking"]["keys"], true);
        assert_eq!(d["input_ownership"], "unknown");
        for key in [
            "resource_probe_sent_input",
            "cleanup_authority",
            "replay_allowed",
            "atomic",
        ] {
            assert_eq!(d[key], false);
        }
    }

    #[test]
    fn f408_resource_diagnostic_keeps_existing_policy_matrix() {
        for source in [0, 1, 9] {
            for keys in [vec![], vec![124]] {
                for buttons in [vec![], vec![0]] {
                    for flags in [0, 1 << 16, 1 << 20, 1 << 23, 1 << 15] {
                        let blocked = !buttons.is_empty()
                            || flags & MOMENTARY_FLAGS != 0
                            || (source == 1 && !keys.is_empty());
                        let result = require_released(&[State {
                            source,
                            keys: keys.clone(),
                            buttons: buttons.clone(),
                            flags,
                        }]);
                        assert_eq!(result.is_err(), blocked);
                        if let Err(error) = result {
                            assert_eq!(error.code, "native_input_resources_held");
                            assert_eq!(
                                error.next_step,
                                "Global keys/buttons/modifiers are down; this does not identify an RWI-owned input. This legacy interrupted-event reconciliation cannot prove release. Do not inject cleanup or replay; require scoped ownership evidence or a fresh resource check."
                            );
                            assert!(error.diagnostic.is_some());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn f408_resource_diagnostic_bounds_rows_arrays_and_full_width_flags() {
        let state = || State {
            source: 1,
            keys: (0..300).collect(),
            buttons: (0..40).collect(),
            flags: u64::MAX,
        };
        let d = require_released(&[state(), state(), state()])
            .unwrap_err()
            .diagnostic
            .unwrap();
        assert_eq!(d["state_count"], 3);
        assert_eq!(d["states_truncated"], true);
        assert_eq!(d["states"].as_array().unwrap().len(), 2);
        assert_eq!(d["states"][0]["keys"].as_array().unwrap().len(), 256);
        assert_eq!(d["states"][0]["key_count"], 300);
        assert_eq!(d["states"][0]["keys_truncated"], true);
        assert_eq!(d["states"][0]["buttons"].as_array().unwrap().len(), 32);
        assert_eq!(d["states"][0]["buttons_truncated"], true);
        assert_eq!(d["states"][0]["flags"], u64::MAX.to_string());
        assert!(serde_json::to_vec(&d).unwrap().len() < 8192);
    }
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
