//! Explicit delivery selection; never promote background movement implicitly.
use crate::{ForegroundState, Result, fault};
use serde::{Deserialize, Serialize};

/// Only public NSEvent tracking kinds; never a CGEventType enum conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrackingPhase {
    Entered,
    Exited,
}
impl TrackingPhase {
    pub fn native_kind(self) -> u32 {
        match self {
            Self::Entered => 8,
            Self::Exited => 9,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "delivery", rename_all = "snake_case", deny_unknown_fields)]
pub enum MovePointerOptions {
    BackgroundWindow {},
    /// macOS private SPI experiment; never the default or an automatic retry.
    BackgroundSkylightExperimental {},
    /// Public AppKit constructor experiment; unchanged public PID transport.
    #[serde(rename = "background_appkit_experimental")]
    BackgroundAppKitExperimental {
        /// Caller-declared CG integer deltas, not inferred physical motion.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_explicit_delta"
        )]
        delta: Option<[i32; 2]>,
    },
    /// Registered public legacy tag carrier, not modern-area pointer discovery.
    #[serde(rename = "background_appkit_tracking_experimental")]
    BackgroundAppKitTrackingExperimental {
        phase: TrackingPhase,
        /// Full-width opaque NSInteger in canonical signed decimal; no JS Number.
        tracking_number: String,
    },
    /// Authenticated cooperative owner only; no public tag or trusted witness input.
    BackgroundCooperativeTrackingExperimental {
        phase: TrackingPhase,
        owner_endpoint: String,
        registration_id: String,
    },
    ForegroundSystem {
        expected_foreground: ForegroundState,
    },
}
fn deserialize_explicit_delta<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<[i32; 2]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    <[i32; 2]>::deserialize(deserializer).map(Some)
}

impl MovePointerOptions {
    /// A tag is an opaque API scalar, potentially address-shaped, never an ObjC
    /// object to decode. Parsing does NOT prove registration or current ownership.
    pub fn parse_tracking_number(value: &str) -> Result<i64> {
        let number = value.parse::<i64>().map_err(|_| {
            fault(
                "invalid_input_value",
                "Tracking tag must be a nonzero canonical signed i64 decimal string; no input.",
            )
        })?;
        if number == 0 || number.to_string() != value {
            return Err(fault(
                "invalid_input_value",
                "Tracking tag must be canonical, nonzero and lossless; no input.",
            ));
        }
        Ok(number)
    }
    pub fn validate_cooperative_selector(endpoint: &str, registration: &str) -> Result<()> {
        // macOS peer path syntax, independent of the MCP Host's platform.
        if endpoint.len() > 100
            || endpoint.as_bytes().contains(&0)
            || !endpoint.starts_with('/')
            || endpoint
                .split('/')
                .skip(1)
                .any(|part| part.is_empty() || part == "." || part == "..")
            || !crate::device::valid_id(registration)
        {
            return Err(fault(
                "invalid_input_value",
                "Use a short absolute owner socket and exact opaque registration_id; no tag/pointer/witness input.",
            ));
        }
        Ok(())
    }
    /// Policy bound for this opt-in experiment, not a measured motion history.
    pub fn validate_explicit_delta(delta: [i32; 2]) -> Result<()> {
        if delta == [0, 0] || delta.iter().any(|v| !(-128..=128).contains(v)) {
            return Err(fault(
                "invalid_input_value",
                "Explicit AppKit delta requires two signed integers, each -128..128, with at least one nonzero; no input.",
            ));
        }
        Ok(())
    }
    pub fn parse(value: Option<&str>) -> Result<Self> {
        let Some(value) = value else {
            return Ok(Self::BackgroundWindow {});
        };
        if value.len() > 4096 {
            return Err(fault(
                "invalid_input_value",
                "Bound pointer delivery options to 4096 bytes.",
            ));
        }
        let options: Self = serde_json::from_str(value).map_err(|_| {
            fault(
                "invalid_input_value",
                "Use explicit pointer delivery options, never text, a button or an action.",
            )
        })?;
        if let Self::ForegroundSystem {
            expected_foreground,
        } = &options
        {
            expected_foreground.validate()?;
        }
        if let Self::BackgroundAppKitExperimental { delta: Some(delta) } = &options {
            Self::validate_explicit_delta(*delta)?;
        }
        if let Self::BackgroundAppKitTrackingExperimental {
            tracking_number, ..
        } = &options
        {
            Self::parse_tracking_number(tracking_number)?;
        }
        if let Self::BackgroundCooperativeTrackingExperimental {
            owner_endpoint,
            registration_id,
            ..
        } = &options
        {
            Self::validate_cooperative_selector(owner_endpoint, registration_id)?;
        }
        Ok(options)
    }
    pub fn requires_foreground(&self) -> bool {
        matches!(self, Self::ForegroundSystem { .. })
    }
    pub fn require_capability(&self, capabilities: &serde_json::Value) -> Result<()> {
        if let Self::BackgroundCooperativeTrackingExperimental {
            owner_endpoint,
            registration_id,
            ..
        } = self
        {
            Self::validate_cooperative_selector(owner_endpoint, registration_id)?;
            let cap = &capabilities["pointer_move"]["background_cooperative_tracking_experimental"];
            if cap["implemented"] != true || cap["available"] != true {
                return Err(fault(
                    "capability_unavailable",
                    "Authenticated cooperative tracking is unavailable; no foreign-tag/direct-post fallback.",
                ));
            }
        }
        if let Self::BackgroundAppKitTrackingExperimental {
            tracking_number, ..
        } = self
        {
            Self::parse_tracking_number(tracking_number)?;
            let route = &capabilities["pointer_move"]["background_appkit_tracking_experimental"];
            if route["implemented"] != true || route["available"] != true {
                return Err(fault(
                    "capability_unavailable",
                    "Registered legacy AppKit tracking experiment is unavailable; no fallback.",
                ));
            }
        }
        if matches!(self, Self::BackgroundSkylightExperimental {}) {
            let route = &capabilities["pointer_move"]["background_skylight_experimental"];
            if route["implemented"] != true || route["available"] != true {
                return Err(fault(
                    "capability_unavailable",
                    "The experimental macOS SkyLight pointer route is unavailable; no fallback.",
                ));
            }
        }
        if matches!(self, Self::BackgroundAppKitExperimental { .. }) {
            let route = &capabilities["pointer_move"]["background_appkit_experimental"];
            if route["implemented"] != true || route["available"] != true {
                return Err(fault(
                    "capability_unavailable",
                    "The experimental macOS AppKit pointer constructor is unavailable; no fallback.",
                ));
            }
        }
        if let Self::BackgroundAppKitExperimental { delta: Some(delta) } = self {
            Self::validate_explicit_delta(*delta)?;
            let feature =
                &capabilities["pointer_move"]["background_appkit_experimental"]["explicit_delta"];
            if feature["implemented"] != true || feature["available"] != true {
                return Err(fault(
                    "capability_unavailable",
                    "This backend does not advertise explicit AppKit delta; no fallback.",
                ));
            }
        }
        // Explicit background selection is not the same as omitting options.
        // An unadvertised device must never reinterpret it as its native route.
        if matches!(self, Self::BackgroundWindow {})
            && (capabilities["pointer_move"]["implemented"] != true
                || capabilities["pointer_move"]["default_delivery"] != "background_window")
        {
            return Err(fault(
                "capability_unavailable",
                "This backend does not advertise window-addressed background pointer delivery; no fallback.",
            ));
        }
        if self.requires_foreground()
            && capabilities["pointer_move"]["foreground_system"]["implemented"] != true
        {
            return Err(fault(
                "capability_unavailable",
                "This backend does not advertise explicit foreground pointer delivery; no fallback.",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn foreground() -> serde_json::Value {
        serde_json::json!({"delivery":"foreground_system","expected_foreground":{
            "pid":42,"window":"cgwindow:7","process_start":"42:1:2","native_epoch":"a".repeat(64)}})
    }
    #[test]
    fn experimental_skylight_is_explicit_strict_and_not_foreground() {
        let options =
            MovePointerOptions::parse(Some(r#"{"delivery":"background_skylight_experimental"}"#))
                .unwrap();
        assert_eq!(
            options,
            MovePointerOptions::BackgroundSkylightExperimental {}
        );
        assert!(!options.requires_foreground());
        assert_ne!(MovePointerOptions::parse(None).unwrap(), options);
        assert!(
            crate::Intent::MovePointer
                .validate(Some(r#"{"delivery":"background_skylight_experimental"}"#))
                .is_ok()
        );
        for extra in ["button", "fallback", "activate", "authentication"] {
            let value =
                serde_json::json!({"delivery":"background_skylight_experimental",extra:true});
            assert!(MovePointerOptions::parse(Some(&value.to_string())).is_err());
        }
    }
    #[test]
    fn experimental_skylight_cannot_inherit_other_routes_or_missing_availability() {
        let options = MovePointerOptions::BackgroundSkylightExperimental {};
        for route in [
            serde_json::json!({}),
            serde_json::json!({"implemented":true}),
            serde_json::json!({"implemented":true,"available":false}),
            serde_json::json!({"implemented":false,"available":true}),
        ] {
            let caps = serde_json::json!({"pointer_move":{
                "implemented":true,"default_delivery":"background_window",
                "foreground_system":{"implemented":true},
                "background_skylight_experimental":route}});
            assert_eq!(
                options.require_capability(&caps).unwrap_err().code,
                "capability_unavailable"
            );
        }
        assert!(
            options
                .require_capability(&serde_json::json!({"pointer_move":{
            "background_skylight_experimental":{"implemented":true,"available":true}}}))
                .is_ok()
        );
    }
    #[cfg(feature = "fixture")]
    #[test]
    fn experimental_skylight_unadvertised_fixture_never_dispatches() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = crate::fixture::Fixture::default();
        let mut engine = crate::Engine::new(dir.path().into()).unwrap();
        engine
            .register("fixture".into(), Box::new(fixture.clone()))
            .unwrap();
        let frame = engine.observe("fixture").unwrap().frame;
        let reference = engine
            .resolve(
                "fixture",
                &frame,
                crate::Locator::Element { id: "Input".into() },
            )
            .unwrap();
        let lease = engine.claim("fixture", 5000).unwrap();
        let result = engine.act(
            "fixture",
            &reference.reference,
            &lease,
            "unsupported-skylight",
            crate::Intent::MovePointer,
            Some(r#"{"delivery":"background_skylight_experimental"}"#.into()),
        );
        assert_eq!(result.unwrap_err().code, "capability_unavailable");
        assert_eq!(fixture.0.lock().unwrap().calls, 0);
        engine.release(&lease).unwrap();
    }
    #[test]
    fn pointer_options_default_is_unchanged_and_foreground_requires_explicit_exact_tuple() {
        assert_eq!(
            MovePointerOptions::parse(None).unwrap(),
            MovePointerOptions::BackgroundWindow {}
        );
        let value = foreground().to_string();
        assert!(
            MovePointerOptions::parse(Some(&value))
                .unwrap()
                .requires_foreground()
        );
        assert!(crate::Intent::MovePointer.validate(Some(&value)).is_ok());
        assert!(MovePointerOptions::parse(Some("{\"delivery\":\"foreground_system\"}")).is_err());
    }
    #[test]
    fn pointer_options_reject_unknown_null_text_buttons_and_invalid_foreground() {
        for value in [
            "",
            "null",
            "text",
            "{}",
            "{\"delivery\":\"auto\"}",
            "{\"delivery\":\"background_window\",\"button\":\"left\"}",
        ] {
            assert!(MovePointerOptions::parse(Some(value)).is_err(), "{value}");
        }
        let mut value = foreground();
        value["expected_foreground"]["pid"] = serde_json::json!(0);
        assert!(MovePointerOptions::parse(Some(&value.to_string())).is_err());
    }
    #[test]
    fn pointer_options_foreground_capability_is_not_inherited_from_background_support() {
        let value = foreground().to_string();
        let options = MovePointerOptions::parse(Some(&value)).unwrap();
        assert!(
            options
                .require_capability(&serde_json::json!({"pointer_move":{"implemented":true}}))
                .is_err()
        );
        assert!(
            options
                .require_capability(
                    &serde_json::json!({"pointer_move":{"foreground_system":{"implemented":true}}})
                )
                .is_ok()
        );
        assert!(
            MovePointerOptions::BackgroundWindow {}
                .require_capability(&serde_json::json!({"pointer_move":{
                    "implemented":true,"default_delivery":"background_window"}}))
                .is_ok()
        );
    }
    #[cfg(feature = "fixture")]
    #[test]
    fn pointer_options_unadvertised_foreground_is_refused_before_backend_input() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = crate::fixture::Fixture::default();
        let mut engine = crate::Engine::new(dir.path().into()).unwrap();
        engine
            .register("fixture".into(), Box::new(fixture.clone()))
            .unwrap();
        let frame = engine.observe("fixture").unwrap().frame;
        let reference = engine
            .resolve(
                "fixture",
                &frame,
                crate::Locator::Element { id: "Input".into() },
            )
            .unwrap();
        let lease = engine.claim("fixture", 5000).unwrap();
        let result = engine.act(
            "fixture",
            &reference.reference,
            &lease,
            "unsupported-foreground",
            crate::Intent::MovePointer,
            Some(foreground().to_string()),
        );
        assert_eq!(result.unwrap_err().code, "capability_unavailable");
        assert_eq!(fixture.0.lock().unwrap().calls, 0);
        engine.release(&lease).unwrap();
    }
    #[test]
    fn pointer_options_explicit_background_needs_its_own_advertised_route() {
        let options =
            MovePointerOptions::parse(Some(r#"{"delivery":"background_window"}"#)).unwrap();
        for unadvertised in [
            serde_json::json!({}),
            serde_json::json!({"pointer_move":{"implemented":true}}),
            serde_json::json!({"pointer_move":{"implemented":true,"default_delivery":"foreground_system"}}),
            serde_json::json!({"pointer_move":{"implemented":false,"default_delivery":"background_window"}}),
        ] {
            assert_eq!(
                options.require_capability(&unadvertised).unwrap_err().code,
                "capability_unavailable"
            );
        }
    }
    #[cfg(feature = "fixture")]
    #[test]
    fn pointer_options_explicit_background_refuses_but_bare_move_keeps_legacy_backend_route() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = crate::fixture::Fixture::default();
        let mut engine = crate::Engine::new(dir.path().into()).unwrap();
        engine
            .register("fixture".into(), Box::new(fixture.clone()))
            .unwrap();
        let frame = engine.observe("fixture").unwrap().frame;
        let reference = engine
            .resolve(
                "fixture",
                &frame,
                crate::Locator::Element { id: "Input".into() },
            )
            .unwrap();
        let lease = engine.claim("fixture", 5000).unwrap();
        let refused = engine.act(
            "fixture",
            &reference.reference,
            &lease,
            "explicit-background",
            crate::Intent::MovePointer,
            Some(r#"{"delivery":"background_window"}"#.into()),
        );
        assert_eq!(refused.unwrap_err().code, "capability_unavailable");
        assert_eq!(fixture.0.lock().unwrap().calls, 0);
        let bare = engine
            .act(
                "fixture",
                &reference.reference,
                &lease,
                "bare-default",
                crate::Intent::MovePointer,
                None,
            )
            .unwrap();
        assert_eq!(bare.dispatch, "returned");
        assert!(!bare.goal_verified);
        assert_eq!(fixture.0.lock().unwrap().calls, 1);
        engine.release(&lease).unwrap();
    }
}

#[cfg(test)]
mod appkit_move_option_tests {
    use super::*;

    #[test]
    fn f05_appkit_move_is_strict_opt_in_and_never_inherits_another_route() {
        let value = r#"{"delivery":"background_appkit_experimental"}"#;
        let selected = MovePointerOptions::parse(Some(value)).unwrap();
        assert_eq!(
            selected,
            MovePointerOptions::BackgroundAppKitExperimental { delta: None }
        );
        assert!(!selected.requires_foreground());
        assert_ne!(MovePointerOptions::parse(None).unwrap(), selected);
        assert!(crate::Intent::MovePointer.validate(Some(value)).is_ok());
        for field in [
            "fallback",
            "button",
            "activate",
            "authentication",
            "dwell_ms",
        ] {
            let input = serde_json::json!({"delivery":"background_appkit_experimental",field:true});
            assert!(MovePointerOptions::parse(Some(&input.to_string())).is_err());
        }
        for route in [
            serde_json::json!({}),
            serde_json::json!({"implemented":true}),
            serde_json::json!({"implemented":true,"available":false}),
            serde_json::json!({"implemented":false,"available":true}),
        ] {
            let capabilities = serde_json::json!({"pointer_move":{
                "implemented":true,"default_delivery":"background_window",
                "background_skylight_experimental":{"implemented":true,"available":true},
                "background_appkit_experimental":route}});
            assert_eq!(
                selected.require_capability(&capabilities).unwrap_err().code,
                "capability_unavailable"
            );
        }
        assert!(
            selected
                .require_capability(&serde_json::json!({"pointer_move":{
            "background_appkit_experimental":{"implemented":true,"available":true}}}))
                .is_ok()
        );
    }

    #[cfg(feature = "fixture")]
    #[test]
    fn f05_appkit_move_unadvertised_device_is_refused_before_backend_input() {
        let directory = tempfile::tempdir().unwrap();
        let fixture = crate::fixture::Fixture::default();
        let mut engine = crate::Engine::new(directory.path().into()).unwrap();
        engine
            .register("fixture".into(), Box::new(fixture.clone()))
            .unwrap();
        let frame = engine.observe("fixture").unwrap().frame;
        let reference = engine
            .resolve(
                "fixture",
                &frame,
                crate::Locator::Element { id: "Input".into() },
            )
            .unwrap();
        let lease = engine.claim("fixture", 5000).unwrap();
        let result = engine.act(
            "fixture",
            &reference.reference,
            &lease,
            "explicit-appkit-move",
            crate::Intent::MovePointer,
            Some(r#"{"delivery":"background_appkit_experimental"}"#.into()),
        );
        assert_eq!(result.unwrap_err().code, "capability_unavailable");
        assert_eq!(fixture.0.lock().unwrap().calls, 0);
        engine.release(&lease).unwrap();
    }
}

#[cfg(test)]
mod explicit_delta_tests {
    use super::*;
    fn value(delta: serde_json::Value) -> String {
        serde_json::json!({"delivery":"background_appkit_experimental","delta":delta}).to_string()
    }
    #[test]
    fn f05_explicit_delta_is_bounded_integer_pair_and_never_inferred() {
        let plain = r#"{"delivery":"background_appkit_experimental"}"#;
        let selected = MovePointerOptions::parse(Some(plain)).unwrap();
        assert_eq!(
            serde_json::to_value(selected).unwrap(),
            serde_json::json!({"delivery":"background_appkit_experimental"})
        );
        assert_eq!(
            MovePointerOptions::parse(None).unwrap(),
            MovePointerOptions::BackgroundWindow {}
        );
        for delta in [[17, -9], [-128, 128], [0, 1]] {
            let raw = value(serde_json::json!(delta));
            let selected = MovePointerOptions::parse(Some(&raw)).unwrap();
            assert!(!selected.requires_foreground());
            assert_eq!(
                serde_json::to_value(selected).unwrap(),
                serde_json::json!({"delivery":"background_appkit_experimental","delta":delta})
            );
        }
        for delta in [
            serde_json::json!(null),
            serde_json::json!([0, 0]),
            serde_json::json!([129, 0]),
            serde_json::json!([0, -129]),
            serde_json::json!([1.0, 0]),
            serde_json::json!([1]),
            serde_json::json!([1, 2, 3]),
            serde_json::json!(["1", 0]),
            serde_json::json!([true, 0]),
        ] {
            assert_eq!(
                MovePointerOptions::parse(Some(&value(delta)))
                    .unwrap_err()
                    .code,
                "invalid_input_value"
            );
        }
        for delivery in [
            "background_window",
            "background_skylight_experimental",
            "foreground_system",
        ] {
            let raw = serde_json::json!({"delivery":delivery,"delta":[17,-9]}).to_string();
            assert!(MovePointerOptions::parse(Some(&raw)).is_err());
        }
        let raw = r#"{"delivery":"background_appkit_experimental","delta":[17,-9],"delta":[1,2]}"#;
        assert!(MovePointerOptions::parse(Some(raw)).is_err());
        assert!(MovePointerOptions::parse(Some(&"x".repeat(4097))).is_err());
    }
    #[test]
    fn f05_explicit_delta_needs_its_own_feature_and_cannot_inherit_legacy_caps() {
        let options = MovePointerOptions::parse(Some(&value(serde_json::json!([17, -9])))).unwrap();
        let plain =
            MovePointerOptions::parse(Some(r#"{"delivery":"background_appkit_experimental"}"#))
                .unwrap();
        for feature in [
            serde_json::json!({}),
            serde_json::json!({"implemented":true}),
            serde_json::json!({"implemented":true,"available":false}),
            serde_json::json!({"implemented":false,"available":true}),
        ] {
            let cap = serde_json::json!({"pointer_move":{"background_appkit_experimental":{"implemented":true,"available":true,"explicit_delta":feature}}});
            assert_eq!(
                options.require_capability(&cap).unwrap_err().code,
                "capability_unavailable"
            );
            assert!(plain.require_capability(&cap).is_ok());
        }
        let cap = serde_json::json!({"pointer_move":{"background_appkit_experimental":{"implemented":true,"available":true,"explicit_delta":{"implemented":true,"available":true}}}});
        assert!(options.require_capability(&cap).is_ok());
        assert!(
            options
                .require_capability(
                    &serde_json::json!({"pointer_move":{"foreground_system":{"implemented":true}}})
                )
                .is_err()
        );
        assert!(
            MovePointerOptions::BackgroundAppKitExperimental {
                delta: Some([0, 0])
            }
            .require_capability(&cap)
            .is_err()
        );
    }
    #[test]
    fn f05_registered_tracking_full_width_string_is_strict_and_default_unchanged() {
        assert_eq!(
            MovePointerOptions::parse(None).unwrap(),
            MovePointerOptions::BackgroundWindow {}
        );
        for phase in [TrackingPhase::Entered, TrackingPhase::Exited] {
            for tag in [
                "9007199254740993",
                "-9007199254740993",
                "9223372036854775807",
                "-9223372036854775808",
            ] {
                let value = serde_json::json!({"delivery":"background_appkit_tracking_experimental","phase":phase,"tracking_number":tag});
                let parsed = MovePointerOptions::parse(Some(&value.to_string())).unwrap();
                assert!(!parsed.requires_foreground());
                assert_eq!(serde_json::to_value(parsed).unwrap(), value);
            }
        }
        for tag in [
            serde_json::json!(0),
            serde_json::json!(73),
            serde_json::json!(73.0),
            serde_json::Value::Null,
            serde_json::json!("0"),
            serde_json::json!("-0"),
            serde_json::json!("+73"),
            serde_json::json!("073"),
            serde_json::json!(" 73"),
            serde_json::json!("73 "),
            serde_json::json!("1e3"),
            serde_json::json!("9223372036854775808"),
            serde_json::json!("-9223372036854775809"),
        ] {
            let value = serde_json::json!({"delivery":"background_appkit_tracking_experimental","phase":"entered","tracking_number":tag});
            assert!(
                MovePointerOptions::parse(Some(&value.to_string())).is_err(),
                "{value}"
            );
        }
        for value in [
            serde_json::json!({"delivery":"background_appkit_tracking_experimental","phase":"moved","tracking_number":"73"}),
            serde_json::json!({"delivery":"background_appkit_tracking_experimental","phase":"entered","tracking_number":"73","delta":[17,-9]}),
            serde_json::json!({"delivery":"background_appkit_tracking_experimental","phase":"entered","tracking_number":"73","user_data":"x"}),
            serde_json::json!({"delivery":"background_appkit_experimental","phase":"entered","tracking_number":"73"}),
        ] {
            assert!(MovePointerOptions::parse(Some(&value.to_string())).is_err());
        }
        assert_eq!(TrackingPhase::Entered.native_kind(), 8);
        assert_eq!(TrackingPhase::Exited.native_kind(), 9);
    }
    #[test]
    fn f05_registered_tracking_requires_own_capability_no_inherited_route() {
        let opts = MovePointerOptions::BackgroundAppKitTrackingExperimental {
            phase: TrackingPhase::Entered,
            tracking_number: "9007199254740993".into(),
        };
        for value in [
            serde_json::json!({}),
            serde_json::json!({"implemented":true}),
            serde_json::json!({"implemented":true,"available":false}),
            serde_json::json!({"implemented":false,"available":true}),
        ] {
            let cap = serde_json::json!({"pointer_move":{"background_appkit_experimental":{"implemented":true,"available":true},"background_appkit_tracking_experimental":value}});
            assert_eq!(
                opts.require_capability(&cap).unwrap_err().code,
                "capability_unavailable"
            );
        }
        assert!(opts.require_capability(&serde_json::json!({"pointer_move":{"background_appkit_tracking_experimental":{"implemented":true,"available":true}}})).is_ok());
    }
    #[test]
    fn f05_cooperative_tracking_selector_is_strict_and_not_a_trusted_witness() {
        let v = serde_json::json!({"delivery":"background_cooperative_tracking_experimental","phase":"entered",
            "owner_endpoint":"/tmp/own-private/owner.sock","registration_id":"opaque-owned-1"});
        let parsed = MovePointerOptions::parse(Some(&v.to_string())).unwrap();
        assert!(!parsed.requires_foreground());
        assert!(parsed.require_capability(&serde_json::json!({"pointer_move":{"background_appkit_tracking_experimental":{"implemented":true,"available":true}}})).is_err());
        assert!(parsed.require_capability(&serde_json::json!({"pointer_move":{"background_cooperative_tracking_experimental":{"implemented":true,"available":true}}})).is_ok());
        for field in [
            "tracking_number",
            "user_data",
            "pid",
            "process_start",
            "native_epoch",
            "trusted_witness",
            "delta",
        ] {
            let mut bad = v.clone();
            bad[field] = serde_json::json!("not-authority");
            assert!(MovePointerOptions::parse(Some(&bad.to_string())).is_err());
        }
        for path in ["relative.sock", "/tmp/../other.sock", "/tmp/a\0sock"] {
            let mut bad = v.clone();
            bad["owner_endpoint"] = serde_json::json!(path);
            assert!(MovePointerOptions::parse(Some(&bad.to_string())).is_err());
        }
        assert_eq!(
            MovePointerOptions::parse(None).unwrap(),
            MovePointerOptions::BackgroundWindow {}
        );
    }
}
