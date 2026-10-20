use super::*;

// A nonzero AX status is not proof that the application made no change.
pub(super) fn native_action_failure(action: &str, status: i32) -> rwi_core::Fault {
    let mut error = fault(
        "native_action_unconfirmed",
        &format!(
            "{action} returned AXError {status}; effect unknown. Inspect the original operation and fresh state; never replay or send fallback input."
        ),
    );
    error.diagnostic = Some(Box::new(serde_json::json!({
        "api": "AXUIElementPerformAction", "action": action,
        "native_status": status, "effect": "unknown", "replay_allowed": false,
        // This path invokes one AX action, not a key/button down/up sequence.
        // Its uncertain application effect must not demand global HID idle.
        "held_input_resources": false
    })));
    error
}

pub(super) fn needs_background_activation(
    intent: &Intent,
    native_press: bool,
    explicit_action: bool,
    foreground_pid: Option<u32>,
    target_pid: u32,
) -> bool {
    matches!(intent, Intent::Text | Intent::KeyChord)
        && !native_press
        && !explicit_action
        && foreground_pid != Some(target_pid)
}

pub(super) fn validate_keyboard_payload(intent: &Intent, value: Option<&str>) -> Result<()> {
    match intent {
        Intent::Text => {
            value.ok_or_else(|| fault("value_required", "Text intent requires Unicode value."))?;
        }
        Intent::KeyChord => {
            let payload = rwi_core::KeyChordPayload::parse(value.ok_or_else(|| {
                fault("value_required", "Key chord requires a supported chord.")
            })?)?;
            parse_chord(&payload.key)?;
        }
        _ => {}
    }
    Ok(())
}
