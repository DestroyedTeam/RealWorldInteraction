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
    let plain = MovePointerOptions::parse(Some(r#"{"delivery":"background_appkit_experimental"}"#))
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
