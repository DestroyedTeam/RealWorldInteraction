//! Unwired policy for one explicitly selected experimental mouse click pair.
//! This type does not dispatch input or advertise a capability. Omission keeps
//! legacy click behavior; native actions continue to use their existing value.
use crate::{Result, fault};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Explicit AppKit mouse subtype experiment, not a no-focus guarantee.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClickMouseSubtype {
    #[default]
    Mouse,
    Touch,
}
impl ClickMouseSubtype {
    fn is_mouse(&self) -> bool {
        *self == Self::Mouse
    }
}

/// Source provenance experiment. Factory events remain software generated;
/// selecting them is never an implicit fallback or physical-user-input claim.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClickEventSource {
    #[default]
    InvocationPrivate,
    AppkitFactory,
}
impl ClickEventSource {
    fn is_invocation_private(&self) -> bool {
        *self == Self::InvocationPrivate
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "delivery", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClickDeliveryOptions {
    /// macOS private SPI experiment: no activation, auth, primer or fallback.
    BackgroundSkylightExperimental {},
    /// Same private delivery, but a public AppKit mouse factory constructs the
    /// pair. This is separately negotiated, never an inactive-app promise.
    BackgroundSkylightAppkitExperimental {},
    /// Same plain public AppKit construction, with explicit public PID posting.
    /// Never a downgrade from an unavailable private route or an app guarantee.
    BackgroundPublicAppkitExperimental {
        /// Omit for unchanged ordinary mouse. Touch requires separate capability.
        #[serde(default, skip_serializing_if = "ClickMouseSubtype::is_mouse")]
        mouse_subtype: ClickMouseSubtype,
        /// Omit for the same isolated invocation Private source. Factory is a
        /// separately negotiated ordinary-mouse candidate, not a focus promise.
        #[serde(
            default,
            skip_serializing_if = "ClickEventSource::is_invocation_private"
        )]
        event_source: ClickEventSource,
    },
    /// Explicit macOS zero-event-number experiment, ordinary left pair only.
    /// No modifier, activation, focus restoration, fallback or effect guarantee.
    BackgroundPublicAppkitZeroNumberExperimental {},
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
        let route = &capabilities["pointer_click"][self.delivery_name()];
        let subtype_ready = !matches!(
            self,
            Self::BackgroundPublicAppkitExperimental {
                mouse_subtype: ClickMouseSubtype::Touch,
                ..
            }
        ) || (route["mouse_subtypes"]["touch"]["implemented"] == true
            && route["mouse_subtypes"]["touch"]["available"] == true);
        let source_ready = !self.uses_appkit_factory_source()
            || (matches!(
                self,
                Self::BackgroundPublicAppkitExperimental {
                    mouse_subtype: ClickMouseSubtype::Mouse,
                    ..
                }
            ) && route["event_sources"]["appkit_factory"]["implemented"] == true
                && route["event_sources"]["appkit_factory"]["available"] == true);
        if route["implemented"] != true
            || route["available"] != true
            || !subtype_ready
            || !source_ready
        {
            return Err(fault(
                "capability_unavailable",
                if matches!(self, Self::BackgroundPublicAppkitExperimental { .. }) {
                    "Explicit public AppKit click delivery, subtype or event source is unavailable; no fallback."
                } else {
                    "Experimental macOS SkyLight click delivery is unavailable; no fallback."
                },
            ));
        }
        Ok(())
    }

    pub fn uses_appkit_factory_source(&self) -> bool {
        matches!(
            self,
            Self::BackgroundPublicAppkitExperimental {
                event_source: ClickEventSource::AppkitFactory,
                ..
            }
        )
    }

    /// Stable request identity and the exact selected-window capability key.
    pub fn delivery_name(&self) -> &'static str {
        match self {
            Self::BackgroundSkylightExperimental {} => "background_skylight_experimental",
            Self::BackgroundSkylightAppkitExperimental {} => {
                "background_skylight_appkit_experimental"
            }
            Self::BackgroundPublicAppkitExperimental { .. } => {
                "background_public_appkit_experimental"
            }
            Self::BackgroundPublicAppkitZeroNumberExperimental {} => {
                "background_public_appkit_zero_number_experimental"
            }
        }
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
#[path = "click_delivery_options/zero_number_tests.rs"]
mod zero_number_tests;

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

#[cfg(test)]
mod appkit_click_policy_tests {
    use super::*;
    const VALUE: &str = r#"{"delivery":"background_skylight_appkit_experimental"}"#;

    #[test]
    fn r98_appkit_click_is_explicit_strict_and_separate_from_the_old_route() {
        assert_eq!(ClickDeliveryOptions::from_click_value(None).unwrap(), None);
        let route = ClickDeliveryOptions::parse(Some(VALUE)).unwrap().unwrap();
        assert_eq!(
            route,
            ClickDeliveryOptions::BackgroundSkylightAppkitExperimental {}
        );
        assert_eq!(serde_json::to_string(&route).unwrap(), VALUE);
        assert_eq!(
            route.delivery_name(),
            "background_skylight_appkit_experimental"
        );
        assert_eq!(
            route.event_pair(),
            [
                ExperimentalClickPhase::LeftMouseDown,
                ExperimentalClickPhase::LeftMouseUp
            ]
        );
        let old_only = serde_json::json!({"pointer_click":{"background_skylight_experimental":{"implemented":true,"available":true}}});
        assert_eq!(
            route.require_capability(&old_only).unwrap_err().code,
            "capability_unavailable"
        );
        for (implemented, available) in [(false, true), (true, false)] {
            let cap = serde_json::json!({"pointer_click":{"background_skylight_appkit_experimental":{"implemented":implemented,"available":available}}});
            assert_eq!(
                route.require_capability(&cap).unwrap_err().code,
                "capability_unavailable"
            );
        }
        route.require_capability(&serde_json::json!({"pointer_click":{"background_skylight_appkit_experimental":{"implemented":true,"available":true}}})).unwrap();
        for key in [
            "constructor",
            "activation",
            "modifiers",
            "subtype",
            "timestamp",
            "fallback",
            "field58",
        ] {
            let mut v = serde_json::from_str::<serde_json::Value>(VALUE).unwrap();
            v.as_object_mut()
                .unwrap()
                .insert(key.into(), serde_json::json!(true));
            assert_eq!(
                ClickDeliveryOptions::parse(Some(&v.to_string()))
                    .unwrap_err()
                    .code,
                "invalid_input_value"
            );
        }
    }

    #[test]
    fn r98_appkit_click_json_cannot_become_ax_action_or_other_intent() {
        assert!(
            ClickDeliveryOptions::from_click_value(Some(VALUE))
                .unwrap()
                .is_some()
        );
        assert_eq!(ClickDeliveryOptions::native_action_value(Some(VALUE)), None);
        assert_eq!(
            crate::Intent::DoubleClick
                .validate(Some(VALUE))
                .unwrap_err()
                .code,
            "invalid_input_value"
        );
        assert_eq!(
            crate::Intent::ContextMenu
                .validate(Some(VALUE))
                .unwrap_err()
                .code,
            "invalid_input_value"
        );
        crate::Intent::Text.validate(Some(VALUE)).unwrap();
    }
}

#[cfg(test)]
mod public_pid_appkit_tests {
    use super::*;
    const VALUE: &str = r#"{"delivery":"background_public_appkit_experimental"}"#;
    #[test]
    fn public_pid_appkit_click_is_explicit_strict_and_separately_negotiated() {
        assert_eq!(ClickDeliveryOptions::from_click_value(None).unwrap(), None);
        assert_eq!(
            ClickDeliveryOptions::native_action_value(Some("AXPress")),
            Some("AXPress")
        );
        let policy = ClickDeliveryOptions::from_click_value(Some(VALUE))
            .unwrap()
            .unwrap();
        assert_eq!(
            policy.delivery_name(),
            "background_public_appkit_experimental"
        );
        assert_eq!(serde_json::to_string(&policy).unwrap(), VALUE);
        assert_eq!(
            policy.event_pair(),
            [
                ExperimentalClickPhase::LeftMouseDown,
                ExperimentalClickPhase::LeftMouseUp
            ]
        );
        for cap in [
            serde_json::json!({}),
            serde_json::json!({"pointer_move":{"background_public_appkit_experimental":{"implemented":true,"available":true}}}),
            serde_json::json!({"pointer_click":{"background_skylight_appkit_experimental":{"implemented":true,"available":true},"background_skylight_experimental":{"implemented":true,"available":true}}}),
        ] {
            assert_eq!(
                policy.require_capability(&cap).unwrap_err().code,
                "capability_unavailable"
            );
        }
        for available in [false, true] {
            let cap = serde_json::json!({"pointer_click":{"background_public_appkit_experimental":{"implemented":true,"available":available}}});
            assert_eq!(policy.require_capability(&cap).is_ok(), available);
        }
        for invalid in [
            "null",
            r#"{"delivery":"background_public_appkit_experimental","fallback":true}"#,
            r#"{"delivery":"background_public_appkit_experimental","source_pid":1}"#,
        ] {
            assert_eq!(
                ClickDeliveryOptions::from_click_value(Some(invalid))
                    .unwrap_err()
                    .code,
                "invalid_input_value"
            );
        }
        assert!(
            ClickDeliveryOptions::from_click_value(Some(
                r#"{"delivery":"background_skylight_appkit_experimental"}"#
            ))
            .unwrap()
            .is_some()
        );
        assert!(
            ClickDeliveryOptions::from_click_value(Some(
                r#"{"delivery":"background_skylight_experimental"}"#
            ))
            .unwrap()
            .is_some()
        );
    }
}

#[cfg(test)]
mod touch_policy_tests {
    use super::*;
    use serde_json::json;
    const PLAIN: &str = r#"{"delivery":"background_public_appkit_experimental"}"#;
    const TOUCH: &str =
        r#"{"delivery":"background_public_appkit_experimental","mouse_subtype":"touch"}"#;
    #[test]
    fn f188_touch_click_is_explicit_and_preserves_legacy_default_encoding() {
        let plain = ClickDeliveryOptions::parse(Some(PLAIN)).unwrap().unwrap();
        assert_eq!(serde_json::to_string(&plain).unwrap(), PLAIN);
        let explicit_mouse = ClickDeliveryOptions::parse(Some(
            r#"{"delivery":"background_public_appkit_experimental","mouse_subtype":"mouse"}"#,
        ))
        .unwrap()
        .unwrap();
        assert_eq!(plain, explicit_mouse);
        let touch = ClickDeliveryOptions::parse(Some(TOUCH)).unwrap().unwrap();
        assert_ne!(plain, touch);
        assert_eq!(serde_json::to_string(&touch).unwrap(), TOUCH);
        assert_eq!(touch.event_pair(), plain.event_pair());
        for subtype in [json!(3), json!(null), json!("auto"), json!(false)] {
            let value =
                json!({"delivery":"background_public_appkit_experimental","mouse_subtype":subtype})
                    .to_string();
            assert!(ClickDeliveryOptions::parse(Some(&value)).is_err());
        }
        assert!(
            ClickDeliveryOptions::parse(Some(
                r#"{"delivery":"background_skylight_appkit_experimental","mouse_subtype":"touch"}"#
            ))
            .is_err()
        );
    }
    #[test]
    fn f188_touch_click_requires_both_route_and_exact_subtype_capability() {
        let plain = ClickDeliveryOptions::parse(Some(PLAIN)).unwrap().unwrap();
        let touch = ClickDeliveryOptions::parse(Some(TOUCH)).unwrap().unwrap();
        let mut cap = json!({"pointer_click":{"background_public_appkit_experimental":{"implemented":true,"available":true}}});
        assert!(plain.require_capability(&cap).is_ok());
        assert!(touch.require_capability(&cap).is_err());
        for (implemented, available) in [
            (json!(true), json!(false)),
            (json!(false), json!(true)),
            (json!("true"), json!(true)),
            (json!(true), json!(true)),
        ] {
            cap["pointer_click"]["background_public_appkit_experimental"]["mouse_subtypes"]["touch"] =
                json!({"implemented":implemented,"available":available});
            assert_eq!(
                touch.require_capability(&cap).is_ok(),
                implemented == true && available == true
            );
        }
        cap["pointer_click"]["background_public_appkit_experimental"]["available"] = json!(false);
        assert!(touch.require_capability(&cap).is_err());
        for name in ["modifiers", "fallback", "activate", "subtype"] {
            let mut value: serde_json::Value = serde_json::from_str(TOUCH).unwrap();
            value[name] = json!(true);
            assert!(ClickDeliveryOptions::parse(Some(&value.to_string())).is_err());
        }
    }
}

#[cfg(test)]
#[path = "click_delivery_options/factory_source_tests.rs"]
mod factory_source_tests;
