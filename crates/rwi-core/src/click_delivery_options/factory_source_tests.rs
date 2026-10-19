use super::*;
const PLAIN: &str = r#"{"delivery":"background_public_appkit_experimental"}"#;
const FACTORY: &str =
    r#"{"delivery":"background_public_appkit_experimental","event_source":"appkit_factory"}"#;
#[test]
fn f360_factory_source_omission_keeps_original_wire_and_private_default() {
    let p = ClickDeliveryOptions::parse(Some(PLAIN)).unwrap().unwrap();
    assert!(!p.uses_appkit_factory_source());
    assert_eq!(serde_json::to_string(&p).unwrap(), PLAIN);
    assert_eq!(ClickDeliveryOptions::parse(None).unwrap(), None);
}
#[test]
fn f360_factory_source_roundtrip_is_not_an_ax_action_or_other_route() {
    let p = ClickDeliveryOptions::parse(Some(FACTORY)).unwrap().unwrap();
    assert!(p.uses_appkit_factory_source());
    assert_eq!(serde_json::to_string(&p).unwrap(), FACTORY);
    assert_eq!(
        p.event_pair(),
        [
            ExperimentalClickPhase::LeftMouseDown,
            ExperimentalClickPhase::LeftMouseUp
        ]
    );
    assert_eq!(
        ClickDeliveryOptions::native_action_value(Some(FACTORY)),
        None
    );
    assert!(
        ClickDeliveryOptions::from_click_value(Some("AXPress"))
            .unwrap()
            .is_none()
    );
}
#[test]
fn f360_factory_source_refuses_unknown_null_and_other_routes() {
    for source in [
        serde_json::Value::Null,
        serde_json::json!("hid"),
        serde_json::json!("combined"),
        serde_json::json!(true),
    ] {
        let v = serde_json::json!({"delivery":"background_public_appkit_experimental","event_source":source});
        assert_eq!(
            ClickDeliveryOptions::parse(Some(&v.to_string()))
                .unwrap_err()
                .code,
            "invalid_input_value"
        );
    }
    for route in [
        "background_skylight_experimental",
        "background_skylight_appkit_experimental",
        "background_public_appkit_zero_number_experimental",
    ] {
        let v = serde_json::json!({"delivery":route,"event_source":"appkit_factory"});
        assert_eq!(
            ClickDeliveryOptions::parse(Some(&v.to_string()))
                .unwrap_err()
                .code,
            "invalid_input_value"
        );
    }
}
#[test]
fn f360_factory_source_requires_exact_capability_and_refuses_touch() {
    let p = ClickDeliveryOptions::parse(Some(FACTORY)).unwrap().unwrap();
    let mut cap = serde_json::json!({"pointer_click":{"background_public_appkit_experimental":{"implemented":true,"available":true,"mouse_subtypes":{"touch":{"implemented":true,"available":true}}}}});
    assert_eq!(
        p.require_capability(&cap).unwrap_err().code,
        "capability_unavailable"
    );
    cap["pointer_click"][p.delivery_name()]["event_sources"] =
        serde_json::json!({"appkit_factory":{"implemented":true,"available":true}});
    p.require_capability(&cap).unwrap();
    let touch = FACTORY.replace(
        "\"event_source\"",
        "\"mouse_subtype\":\"touch\",\"event_source\"",
    );
    assert_eq!(
        ClickDeliveryOptions::parse(Some(&touch))
            .unwrap()
            .unwrap()
            .require_capability(&cap)
            .unwrap_err()
            .code,
        "capability_unavailable"
    );
}
