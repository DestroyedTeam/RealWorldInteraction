//! Explicit Touch-subtype candidate on the existing public PID delivery path.
//! Preparation only: the enclosing guarded sequence owns every actual post.
use super::*;
use rwi_core::ClickMouseSubtype;

pub(super) fn prepare_pair(
    down: &CGEvent,
    up: &CGEvent,
    source: &CGEventSource,
    delivery: &Delivery,
    point: [f64; 2],
    subtype: ClickMouseSubtype,
) -> Result<(PreparedSelectedClickEvent, PreparedSelectedClickEvent)> {
    if subtype == ClickMouseSubtype::Mouse {
        return prepare_public_appkit_click_pair(down, up, source, delivery, point);
    }
    let (down, up) = construct_pair(down, up, source, delivery, point)?;
    let before = [Snapshot::read(&down), Snapshot::read(&up)];
    // NSEventSubtypeTouch / NX_SUBTYPE_MOUSE_TOUCH is 3 in the checked SDK.
    // No Command flag, timestamp override, activation event or primer is added.
    // This is a declared subtype experiment, not a focus-preservation promise.
    for event in [&down, &up] {
        event.set_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE, 3);
    }
    let pair = super::super::public_appkit_click::prepare_pair(&down, &up, delivery, point)?;
    for (event, expected) in [(&down, &before[0]), (&up, &before[1])] {
        let observed = Snapshot::read(event);
        if observed != *expected
            || event.get_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE) != 3
        {
            return Err(snapshot_mismatch(
                "Touch preparation changed another event field or lost the requested subtype; no input.",
                expected,
                &observed,
            ));
        }
    }
    Ok((
        PreparedSelectedClickEvent::Public(pair.0),
        PreparedSelectedClickEvent::Public(pair.1),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn f188_touch_click_prepared_pair_keeps_originals_and_plain_default() {
        let (source, down, up, delivery) = super::super::tests::fixture();
        unsafe {
            CGEventSetTimestamp(down.as_ptr().cast(), 123_456_789_123);
            CGEventSetTimestamp(up.as_ptr().cast(), 123_481_789_123);
        }
        let original = [Snapshot::read(&down), Snapshot::read(&up)];
        for (subtype, expected) in [(ClickMouseSubtype::Mouse, 0), (ClickMouseSubtype::Touch, 3)] {
            let pair = prepare_pair(&down, &up, &source, &delivery, [1012.5, 823.25], subtype);
            if crate::background::pointer_delivery_available() {
                let (d, u) = pair.unwrap();
                for event in [d, u] {
                    match event {
                        PreparedSelectedClickEvent::Public(e) => {
                            assert_eq!(e.subtype_for_test(), expected)
                        }
                        PreparedSelectedClickEvent::Private(_) => {
                            panic!("public candidate selected private carrier")
                        }
                    }
                }
            } else {
                assert_eq!(pair.err().unwrap().code, "background_delivery_unavailable");
            }
            assert_eq!([Snapshot::read(&down), Snapshot::read(&up)], original);
            assert_eq!(
                down.get_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE),
                0
            );
            assert_eq!(
                up.get_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE),
                0
            );
        }
        // Prepared values above are dropped, never posted.
    }
    #[test]
    fn f188_touch_click_invalid_pair_refuses_before_preparation_and_caps_stay_honest() {
        let (source, down, up, delivery) = super::super::tests::fixture();
        down.set_flags(core_graphics::event::CGEventFlags::CGEventFlagCommand);
        assert_eq!(
            prepare_pair(
                &down,
                &up,
                &source,
                &delivery,
                [1012.5, 823.25],
                ClickMouseSubtype::Touch
            )
            .err()
            .unwrap()
            .code,
            "invalid_pointer_delivery"
        );
        for (enabled, semantic) in [(false, false), (true, true), (true, false)] {
            let cap = super::super::super::public_appkit_click::capabilities(enabled, semantic);
            let touch = &cap["mouse_subtypes"]["touch"];
            assert_eq!(touch["available"], cap["available"]);
            assert_eq!(touch["event_subtype"], 3);
            for key in [
                "focus_preservation_verified",
                "application_effect_attested",
                "command_modifier_added",
                "fallback",
            ] {
                assert_eq!(touch[key], false);
            }
            assert_eq!(
                cap["value"],
                serde_json::json!({"delivery":"background_public_appkit_experimental"})
            );
        }
    }
}
