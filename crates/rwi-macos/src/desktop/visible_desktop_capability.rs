use serde_json::{Value, json};

pub(super) fn describe(input_enabled: bool) -> Value {
    json!({
        "implemented":true,"available":input_enabled,"opt_in_required":true,
        "moves_system_cursor":true,"activation_requested":false,"button_transition":false,
        "fallback":false,"native_acknowledgement":false,"application_effect_attested":false,
        "scope":"exposed exact window only; does not activate a background target",
        "value":{"delivery":"visible_desktop_system","expected_foreground":"fresh exact current_foreground tuple, may differ from selected target"},
        "requires":"system-wide positive exact-window AX hit + mapped point + unchanged foreground + idle HID + existing permissions + shared lease",
        "goal_verification":"independent; not generic cold tracking or tooltip proof"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_desktop_capability_is_explicit_physical_motion_not_background_equivalence() {
        let cap = describe(true);
        for key in [
            "implemented",
            "available",
            "opt_in_required",
            "moves_system_cursor",
        ] {
            assert_eq!(cap[key], true);
        }
        for key in [
            "activation_requested",
            "button_transition",
            "fallback",
            "application_effect_attested",
        ] {
            assert_eq!(cap[key], false);
        }
        assert_eq!(describe(false)["available"], false);
    }
}
