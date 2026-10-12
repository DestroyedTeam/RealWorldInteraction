//! Unwired policy for one explicitly selected experimental mouse click pair.
//! This type does not dispatch input or advertise a capability. Omission keeps
//! legacy click behavior; native actions continue to use their existing value.
use crate::{Result, fault};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "delivery", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClickDeliveryOptions {
    /// macOS private SPI experiment: no activation, auth, primer or fallback.
    BackgroundSkylightExperimental {},
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExperimentalClickPhase {
    LeftMouseDown,
    LeftMouseUp,
}

impl ClickDeliveryOptions {
    /// Interpret only a click's overloaded value. Plain native-action names keep
    /// their existing exact advertised-action checks; JSON never becomes an AX
    /// action. Do not call this to reinterpret text/chord payload contents.
    pub fn from_click_value(value: Option<&str>) -> Result<Option<Self>> {
        Self::parse(value.filter(|value| Self::json_like(value)))
    }

    /// Classify an already validated Click value without changing its bytes.
    /// The retained element/backend must still advertise this exact action.
    pub fn native_action_value(value: Option<&str>) -> Option<&str> {
        value.filter(|value| !Self::json_like(value))
    }

    fn json_like(value: &str) -> bool {
        let value = value.trim_start();
        matches!(
            value.as_bytes().first(),
            Some(b'{' | b'[' | b'"' | b'-' | b'0'..=b'9')
        ) || matches!(value, "null" | "true" | "false")
    }

    /// Parse a separate optional delivery field, never an AX action/text value.
    pub fn parse(value: Option<&str>) -> Result<Option<Self>> {
        let Some(value) = value else {
            return Ok(None);
        };
        if value.len() > 4096 {
            return Err(fault(
                "invalid_input_value",
                "Bound explicit click delivery options to 4096 bytes.",
            ));
        }
        serde_json::from_str(value).map(Some).map_err(|_| {
            fault(
                "invalid_input_value",
                "Use the explicit experimental click delivery object; no extra options or fallback.",
            )
        })
    }

    /// Capability is specific to clicks: movement support cannot authorize it.
    /// This check is a policy prerequisite, not native authority or app evidence.
    pub fn require_capability(&self, capabilities: &serde_json::Value) -> Result<()> {
        let route = &capabilities["pointer_click"]["background_skylight_experimental"];
        if route["implemented"] != true || route["available"] != true {
            return Err(fault(
                "capability_unavailable",
                "Experimental macOS SkyLight click delivery is unavailable; no fallback.",
            ));
        }
        Ok(())
    }

    /// Exactly one left down/up pair, no move/primer, double-click or retry.
    /// A future native adapter must independently validate each counted post.
    pub fn event_pair(&self) -> [ExperimentalClickPhase; 2] {
        [
            ExperimentalClickPhase::LeftMouseDown,
            ExperimentalClickPhase::LeftMouseUp,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const EXPLICIT: &str = r#"{"delivery":"background_skylight_experimental"}"#;

    #[test]
    fn r92_click_policy_omission_never_enables_experimental_delivery() {
        assert_eq!(ClickDeliveryOptions::parse(None).unwrap(), None);
        let explicit = ClickDeliveryOptions::parse(Some(EXPLICIT))
            .unwrap()
            .unwrap();
        assert_eq!(
            explicit,
            ClickDeliveryOptions::BackgroundSkylightExperimental {}
        );
        assert_eq!(serde_json::to_string(&explicit).unwrap(), EXPLICIT);
    }

    #[test]
    fn r92_click_policy_refuses_unknown_null_oversized_and_semantic_action_values() {
        for value in [
            "",
            "null",
            "{}",
            "AXPress",
            "text",
            r#"{"delivery":"auto"}"#,
            r#"{"delivery":"background_window"}"#,
            r#"{"delivery":"foreground_system"}"#,
        ] {
            assert_eq!(
                ClickDeliveryOptions::parse(Some(value)).unwrap_err().code,
                "invalid_input_value"
            );
        }
        for key in [
            "fallback",
            "activate",
            "authentication",
            "button",
            "click_count",
            "expected_foreground",
            "primer",
            "modifiers",
        ] {
            let mut value = serde_json::json!({"delivery":"background_skylight_experimental"});
            value
                .as_object_mut()
                .unwrap()
                .insert(key.into(), serde_json::json!(true));
            let value = value.to_string();
            assert_eq!(
                ClickDeliveryOptions::parse(Some(&value)).unwrap_err().code,
                "invalid_input_value"
            );
        }
        let oversized = " ".repeat(4097);
        assert_eq!(
            ClickDeliveryOptions::parse(Some(&oversized))
                .unwrap_err()
                .code,
            "invalid_input_value"
        );
    }

    #[test]
    fn r92_click_policy_cannot_inherit_move_or_public_route_capabilities() {
        let policy = ClickDeliveryOptions::BackgroundSkylightExperimental {};
        for capabilities in [
            serde_json::json!({}),
            serde_json::json!({"pointer_move":{"background_skylight_experimental":{"implemented":true,"available":true}}}),
            serde_json::json!({"pointer_click":{"implemented":true,"default_delivery":"background_window"}}),
        ] {
            assert_eq!(
                policy.require_capability(&capabilities).unwrap_err().code,
                "capability_unavailable"
            );
        }
        for route in [
            serde_json::json!({}),
            serde_json::json!({"implemented":true}),
            serde_json::json!({"implemented":true,"available":false}),
            serde_json::json!({"implemented":false,"available":true}),
            serde_json::json!({"implemented":"true","available":true}),
        ] {
            let capabilities =
                serde_json::json!({"pointer_click":{"background_skylight_experimental":route}});
            assert_eq!(
                policy.require_capability(&capabilities).unwrap_err().code,
                "capability_unavailable"
            );
        }
        assert!(
            policy
                .require_capability(&serde_json::json!({"pointer_click":{
            "background_skylight_experimental":{"implemented":true,"available":true}}}))
                .is_ok()
        );
    }

    #[test]
    fn r92_click_policy_contains_one_down_up_pair_only() {
        let policy = ClickDeliveryOptions::BackgroundSkylightExperimental {};
        assert_eq!(
            policy.event_pair(),
            [
                ExperimentalClickPhase::LeftMouseDown,
                ExperimentalClickPhase::LeftMouseUp
            ]
        );
    }

    #[test]
    fn r92_click_policy_schema_remains_explicit_and_rejects_extra_fields() {
        let schema = serde_json::to_value(schemars::schema_for!(ClickDeliveryOptions)).unwrap();
        let raw = serde_json::to_string(&schema).unwrap();
        assert!(raw.contains("background_skylight_experimental"));
        assert!(raw.contains("\"additionalProperties\":false"));
    }

    #[test]
    fn r92_click_policy_shared_click_value_separates_delivery_from_exact_action_names() {
        for value in [
            None,
            Some("AXPress"),
            Some("AXCancel"),
            Some("AXPick"),
            Some("AXUnknown"),
            Some("unknown_action"),
        ] {
            assert_eq!(ClickDeliveryOptions::from_click_value(value).unwrap(), None);
            assert_eq!(ClickDeliveryOptions::native_action_value(value), value);
        }
        assert!(
            ClickDeliveryOptions::from_click_value(Some(EXPLICIT))
                .unwrap()
                .is_some()
        );
        assert_eq!(
            ClickDeliveryOptions::native_action_value(Some(EXPLICIT)),
            None
        );
        for value in [
            "{",
            "[1]",
            "\"AXPress\"",
            "null",
            "true",
            "false",
            "1",
            "-1",
            "  {\"delivery\":\"auto\"}",
        ] {
            assert_eq!(
                ClickDeliveryOptions::from_click_value(Some(value))
                    .unwrap_err()
                    .code,
                "invalid_input_value"
            );
            assert_eq!(ClickDeliveryOptions::native_action_value(Some(value)), None);
        }
    }
}

#[cfg(test)]
mod engine_tests;
