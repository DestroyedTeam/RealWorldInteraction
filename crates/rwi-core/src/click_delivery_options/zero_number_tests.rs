use super::*;
use serde_json::json;
const VALUE: &str = r#"{"delivery":"background_public_appkit_zero_number_experimental"}"#;

#[test]
fn f235_zero_number_policy_is_explicit_strict_and_not_a_native_action() {
    let value = ClickDeliveryOptions::parse(Some(VALUE)).unwrap().unwrap();
    assert_eq!(serde_json::to_string(&value).unwrap(), VALUE);
    assert_eq!(ClickDeliveryOptions::parse(None).unwrap(), None);
    assert_eq!(ClickDeliveryOptions::native_action_value(Some(VALUE)), None);
    assert_eq!(value.event_pair().len(), 2);
    for field in [
        "mouse_subtype",
        "event_number",
        "modifiers",
        "fallback",
        "activate",
    ] {
        let mut raw: serde_json::Value = serde_json::from_str(VALUE).unwrap();
        raw[field] = json!(0);
        assert!(ClickDeliveryOptions::parse(Some(&raw.to_string())).is_err());
    }
    for intent in [crate::Intent::DoubleClick, crate::Intent::ContextMenu] {
        assert!(intent.validate(Some(VALUE)).is_err());
    }
    let ordinary = r#"{"delivery":"background_public_appkit_experimental"}"#;
    assert_eq!(
        serde_json::to_string(
            &ClickDeliveryOptions::parse(Some(ordinary))
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        ordinary
    );
}

#[test]
fn f235_zero_number_policy_requires_its_own_native_capability() {
    let value = ClickDeliveryOptions::parse(Some(VALUE)).unwrap().unwrap();
    let mut caps = json!({"pointer_click":{"background_public_appkit_experimental":{"implemented":true,"available":true}}});
    assert!(value.require_capability(&caps).is_err());
    for (implemented, available) in [
        (json!(true), json!(false)),
        (json!(false), json!(true)),
        (json!("true"), json!(true)),
        (json!(true), json!(true)),
    ] {
        caps["pointer_click"][value.delivery_name()] =
            json!({"implemented":implemented,"available":available});
        assert_eq!(
            value.require_capability(&caps).is_ok(),
            implemented == true && available == true
        );
    }
}
