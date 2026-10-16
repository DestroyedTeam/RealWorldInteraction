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
