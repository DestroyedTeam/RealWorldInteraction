use super::*;
use core_graphics::event::CGEventFlags;
pub(super) fn fixture() -> (CGEventSource, CGEvent, CGEvent, Delivery) {
    let source = CGEventSource::new(CGEventSourceStateID::Private).unwrap();
    let down = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseDown,
        CGPoint::new(1012.5, 823.25),
        core_graphics::event::CGMouseButton::Left,
    )
    .unwrap();
    let up = CGEvent::new_mouse_event(
        source.clone(),
        CGEventType::LeftMouseUp,
        CGPoint::new(1012.5, 823.25),
        core_graphics::event::CGMouseButton::Left,
    )
    .unwrap();
    for (event, pressure) in [(&down, 1.0), (&up, 0.0)] {
        event.set_flags(CGEventFlags::empty());
        event.set_integer_value_field(EventField::MOUSE_EVENT_NUMBER, 73);
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 1);
        event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, 0);
        event.set_double_value_field(EventField::MOUSE_EVENT_PRESSURE, pressure);
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, 0);
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y, 0);
    }
    let delivery = Delivery {
        pid: 1,
        window: 7,
        frame: crate::geometry::Rect {
            x: 1000.,
            y: 800.,
            w: 660.,
            h: 552.,
        },
    };
    (source, down, up, delivery)
}
#[test]
fn r98_appkit_memory_pair_keeps_source_clock_position_plain_fields_and_originals() {
    let (source, down, up, delivery) = fixture();
    let before = [Snapshot::read(&down), Snapshot::read(&up)];
    let pair = construct_pair(&down, &up, &source, &delivery, [1012.5, 823.25]).unwrap();
    assert_ne!(pair.0.as_ptr(), pair.1.as_ptr());
    for generated in [&pair.0, &pair.1] {
        for original in [&down, &up] {
            assert_ne!(generated.as_ptr(), original.as_ptr());
        }
    }
    assert_eq!([Snapshot::read(&pair.0), Snapshot::read(&pair.1)], before);
    assert_eq!([Snapshot::read(&down), Snapshot::read(&up)], before);
    drop((source, down, up));
    // Copies and their source ownership survive factory-pool/template drops.
    assert_eq!([Snapshot::read(&pair.0), Snapshot::read(&pair.1)], before);
}
#[test]
fn r98_appkit_invalid_plain_pair_and_null_bridge_are_refused_before_posting() {
    let (source, down, up, delivery) = fixture();
    assert_eq!(
        independent_bridge_copy(std::ptr::null())
            .err()
            .unwrap()
            .code,
        "background_delivery_unavailable"
    );
    down.set_flags(CGEventFlags::CGEventFlagCommand);
    assert_eq!(
        construct_pair(&down, &up, &source, &delivery, [1012.5, 823.25])
            .err()
            .unwrap()
            .code,
        "invalid_pointer_delivery"
    );
    down.set_flags(CGEventFlags::empty());
    for d in [
        Delivery { pid: 0, ..delivery },
        Delivery {
            window: 0,
            ..delivery
        },
    ] {
        assert_eq!(
            construct_pair(&down, &up, &source, &d, [1012.5, 823.25])
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
    }
    assert_eq!(
        construct_pair(&up, &down, &source, &delivery, [1012.5, 823.25])
            .err()
            .unwrap()
            .code,
        "invalid_pointer_delivery"
    );
    assert_eq!(
        construct_pair(&down, &up, &source, &delivery, [f64::NAN, 823.25])
            .err()
            .unwrap()
            .code,
        "invalid_pointer_delivery"
    );
    for state in [
        CGEventSourceStateID::HIDSystemState,
        CGEventSourceStateID::CombinedSessionState,
        CGEventSourceStateID::Private,
    ] {
        let foreign = CGEventSource::new(state).unwrap();
        assert_eq!(
            construct_pair(&down, &up, &foreign, &delivery, [1012.5, 823.25])
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
    }
}
#[test]
fn r98_appkit_prepared_pair_preserves_semantics_after_existing_window_stamp() {
    let (source, down, up, delivery) = fixture();
    let options = rwi_core::ClickDeliveryOptions::BackgroundSkylightAppkitExperimental {};
    let result =
        prepare_selected_click_pair(&down, &up, &source, &delivery, [1012.5, 823.25], &options);
    if available() && event_class().is_some() {
        drop(result.unwrap());
    } else {
        assert_eq!(
            result.err().unwrap().code,
            "background_delivery_unavailable"
        );
    }
    // Constructing/dropping prepared memory never invokes .post().
}
#[test]
fn public_appkit_offscreen_memory_pair_keeps_original_fields_and_exact_address_without_post() {
    let (source, down, up, mut delivery) = fixture();
    let point = [-312.5, -220.25];
    delivery.frame = crate::geometry::Rect {
        x: -400.,
        y: -300.,
        w: 660.,
        h: 552.,
    };
    ffi_set_point(&down, point);
    ffi_set_point(&up, point);
    let before = [Snapshot::read(&down), Snapshot::read(&up)];
    let (d, u) = construct_pair(&down, &up, &source, &delivery, point).unwrap();
    let prepared = super::super::public_appkit_click::prepare_pair(&d, &u, &delivery, point);
    if crate::background::pointer_delivery_available() {
        drop(prepared.unwrap());
    } else {
        assert_eq!(
            prepared.err().unwrap().code,
            "background_delivery_unavailable"
        );
    }
    assert_eq!([Snapshot::read(&down), Snapshot::read(&up)], before);
    assert_eq!([Snapshot::read(&d), Snapshot::read(&u)], before);
    if crate::background::pointer_delivery_available() {
        for event in [&d, &u] {
            assert_eq!(
                event.get_integer_value_field(EventField::EVENT_TARGET_UNIX_PROCESS_ID),
                delivery.pid as i64
            );
            assert_eq!(event.get_integer_value_field(51), delivery.window as i64);
            assert_eq!(
                event.get_integer_value_field(EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER),
                delivery.window as i64
            );
            assert_eq!([event.location().x, event.location().y], point);
        }
    }
    // Both posted-object memories were merely dropped; no .post(), window,
    // focus or global pointer API. Negative coordinates grant no authority.
}
fn ffi_set_point(event: &CGEvent, point: [f64; 2]) {
    unsafe {
        ffi::CGEventSetLocation(event.as_ptr().cast(), CGPoint::new(point[0], point[1]));
    }
}

#[test]
fn r98_appkit_capability_is_separate_and_never_claims_inactive_reception() {
    for (enabled, semantic) in [(false, false), (true, true), (true, false)] {
        let cap = appkit_click_capabilities(enabled, semantic);
        assert_eq!(
            cap["value"]["delivery"],
            "background_skylight_appkit_experimental"
        );
        assert_eq!(cap["implemented"], true);
        assert_eq!(
            cap["available"],
            enabled && !semantic && available() && event_class().is_some()
        );
        for field in [
            "fallback",
            "activation_requested",
            "defocus_requested",
            "authentication_requested",
            "command_modifier_added",
            "timestamp_override",
            "cursor_warp_requested",
            "application_effect_attested",
            "target_state_preservation_attested",
            "subtype_override_requested",
        ] {
            assert_eq!(cap[field], false);
        }
        assert_eq!(cap["primer_events"], 0);
    }
}

#[test]
fn f360_factory_source_pair_changes_only_declared_source_and_keeps_originals() {
    let (source, down, up, delivery) = fixture();
    let before = [Snapshot::read(&down), Snapshot::read(&up)];
    let (d, u) = construct_pair_with_source(
        &down,
        &up,
        &source,
        &delivery,
        [1012.5, 823.25],
        rwi_core::ClickEventSource::AppkitFactory,
    )
    .unwrap();
    for (actual, mut expected) in [(&d, before[0].clone()), (&u, before[1].clone())] {
        assert_ne!(actual.as_ptr(), down.as_ptr());
        assert_ne!(actual.as_ptr(), up.as_ptr());
        expected.integers[9] = 0;
        assert_eq!(Snapshot::read(actual), expected);
        assert_eq!(
            actual.get_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE),
            0
        );
    }
    assert_ne!(d.as_ptr(), u.as_ptr());
    assert_eq!([Snapshot::read(&down), Snapshot::read(&up)], before);
    assert_ne!(before[0].integers[9], 0);
    drop((source, down, up));
    assert_eq!(Snapshot::read(&d).integers[9], 0);
}
#[test]
fn f360_factory_source_does_not_accept_foreign_templates_or_modifiers() {
    let (source, down, up, delivery) = fixture();
    down.set_flags(CGEventFlags::CGEventFlagCommand);
    assert_eq!(
        construct_pair_with_source(
            &down,
            &up,
            &source,
            &delivery,
            [1012.5, 823.25],
            rwi_core::ClickEventSource::AppkitFactory
        )
        .err()
        .unwrap()
        .code,
        "invalid_pointer_delivery"
    );
    down.set_flags(CGEventFlags::empty());
    let combined = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).unwrap();
    assert_eq!(
        construct_pair_with_source(
            &down,
            &up,
            &combined,
            &delivery,
            [1012.5, 823.25],
            rwi_core::ClickEventSource::AppkitFactory
        )
        .err()
        .unwrap()
        .code,
        "invalid_pointer_delivery"
    );
}
#[test]
fn f360_factory_source_preparation_and_capability_do_not_claim_focus_or_post() {
    let (source, down, up, delivery) = fixture();
    let policy = rwi_core::ClickDeliveryOptions::parse(Some(
        r#"{"delivery":"background_public_appkit_experimental","event_source":"appkit_factory"}"#,
    ))
    .unwrap()
    .unwrap();
    let pair =
        prepare_selected_click_pair(&down, &up, &source, &delivery, [1012.5, 823.25], &policy);
    if crate::background::pointer_delivery_available() {
        drop(pair.unwrap());
    } else {
        assert_eq!(pair.err().unwrap().code, "background_delivery_unavailable");
    }
    for (enabled, semantic) in [(false, false), (true, true), (true, false)] {
        let cap = super::super::public_appkit_click::capabilities(enabled, semantic);
        let factory = &cap["event_sources"]["appkit_factory"];
        assert_eq!(factory["available"], cap["available"]);
        assert_eq!(factory["source_state_id"], 0);
        for k in [
            "physical_user_input",
            "focus_preservation_verified",
            "application_effect_attested",
            "fallback",
        ] {
            assert_eq!(factory[k], false);
        }
    }
    let bad=rwi_core::ClickDeliveryOptions::parse(Some(r#"{"delivery":"background_public_appkit_experimental","mouse_subtype":"touch","event_source":"appkit_factory"}"#)).unwrap().unwrap();
    assert_eq!(
        prepare_selected_click_pair(&down, &up, &source, &delivery, [1012.5, 823.25], &bad)
            .err()
            .unwrap()
            .code,
        "invalid_pointer_delivery"
    );
}
