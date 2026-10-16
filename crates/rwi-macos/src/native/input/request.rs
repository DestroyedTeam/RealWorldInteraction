//! Distinguish an explicit click delivery object from an advertised AX action.
//! Classification never posts input, changes the target, or permits fallback.
use rwi_core::{Intent, Result, fault};

pub(super) fn native_action_request<'a>(
    intent: &Intent,
    marked: Option<&'a str>,
    value: Option<&str>,
) -> Result<Option<&'a str>> {
    if marked.is_none()
        && matches!(intent, Intent::Click)
        && rwi_core::ClickDeliveryOptions::from_click_value(value)?.is_some()
    {
        // Only an explicit parsed pointer route uses JSON. An advertised native
        // action reference still requires its unchanged matching action string.
        return Ok(None);
    }
    match (marked, intent, value) {
        (Some(action), Intent::Click, Some(value)) if value == action => Ok(Some(action)),
        (Some(_), _, _) | (None, Intent::Click, Some(_)) => Err(fault(
            "native_action_mismatch",
            "Use the exact checked native-action reference and matching action. A pointer reference cannot be promoted by its value; no input sent.",
        )),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod r92_click_delivery_tests {
    use super::super::require_surface_action;
    use super::*;
    const EXPERIMENTAL: &str = r#"{"delivery":"background_skylight_experimental"}"#;
    #[test]
    fn explicit_click_route_does_not_promote_or_replace_native_action_authority() {
        assert!(
            native_action_request(&Intent::Click, None, Some(EXPERIMENTAL))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            native_action_request(&Intent::Click, Some("AXPress"), Some("AXPress")).unwrap(),
            Some("AXPress")
        );
        for (marked, value) in [
            (None, "AXPress"),
            (Some("AXPress"), EXPERIMENTAL),
            (Some("AXPress"), "AXCancel"),
        ] {
            assert_eq!(
                native_action_request(&Intent::Click, marked, Some(value))
                    .unwrap_err()
                    .code,
                "native_action_mismatch"
            );
        }
        assert!(
            native_action_request(&Intent::Click, None, None)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            native_action_request(
                &Intent::Click,
                None,
                Some(r#"{"delivery":"background_skylight_experimental","fallback":true}"#)
            )
            .unwrap_err()
            .code,
            "invalid_input_value"
        );
        assert_eq!(
            require_surface_action(true, &Intent::Click, None)
                .unwrap_err()
                .code,
            "semantic_action_required"
        );
    }
}
