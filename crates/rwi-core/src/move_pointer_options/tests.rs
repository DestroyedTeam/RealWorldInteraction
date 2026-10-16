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
        let value = serde_json::json!({"delivery":"background_skylight_experimental",extra:true});
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
    let options = MovePointerOptions::parse(Some(r#"{"delivery":"background_window"}"#)).unwrap();
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
