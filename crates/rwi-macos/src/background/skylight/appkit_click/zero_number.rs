//! One explicit zero-number pair. No post, focus/tag change or modifier here.
use super::*;

pub(super) fn prepare_pair(
    down: &CGEvent,
    up: &CGEvent,
    source: &CGEventSource,
    delivery: &Delivery,
    point: [f64; 2],
) -> Result<(PreparedSelectedClickEvent, PreparedSelectedClickEvent)> {
    // Validate the SAME positive Private-source templates first; mutate only the
    // independent factory copies, never the caller's pair or a shared source.
    let (down, up) = construct_pair(down, up, source, delivery, point)?;
    let mut expected = [Snapshot::read(&down), Snapshot::read(&up)];
    for (event, snapshot) in [&down, &up].into_iter().zip(expected.iter_mut()) {
        if event.get_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE) != 0 {
            return Err(fault(
                "event_constructor_mismatch",
                "Zero-number experiment requires ordinary mouse subtype; no input.",
            ));
        }
        event.set_integer_value_field(EventField::MOUSE_EVENT_NUMBER, 0);
        snapshot.integers[0] = 0;
    }
    let pair =
        super::super::public_appkit_click::prepare_zero_number_pair(&down, &up, delivery, point)?;
    for (event, snapshot) in [(&down, &expected[0]), (&up, &expected[1])] {
        if Snapshot::read(event) != *snapshot
            || event.get_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE) != 0
        {
            return Err(snapshot_mismatch(
                "Zero-number preparation changed another field; no input.",
                snapshot,
                &Snapshot::read(event),
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
    fn f235_zero_number_memory_pair_preserves_originals_and_positive_default() {
        let (source, down, up, delivery) = super::super::tests::fixture();
        let before = [Snapshot::read(&down), Snapshot::read(&up)];
        for (zero, pair) in [
            (
                false,
                prepare_public_appkit_click_pair(&down, &up, &source, &delivery, [1012.5, 823.25]),
            ),
            (
                true,
                prepare_pair(&down, &up, &source, &delivery, [1012.5, 823.25]),
            ),
        ] {
            if crate::background::pointer_delivery_available() {
                let (d, u) = pair.unwrap();
                for event in [d, u] {
                    match event {
                        PreparedSelectedClickEvent::Public(e) => {
                            assert_eq!(
                                e.number_for_test(),
                                if zero { 0 } else { before[0].integers[0] }
                            );
                            assert_eq!(e.subtype_for_test(), 0);
                        }
                        PreparedSelectedClickEvent::Private(_) => panic!("not public route"),
                    }
                }
            } else {
                assert_eq!(pair.err().unwrap().code, "background_delivery_unavailable");
            }
            assert_eq!([Snapshot::read(&down), Snapshot::read(&up)], before);
        }
        // Prepared values are dropped, never posted.
    }
    #[test]
    fn f235_zero_number_invalid_template_refuses_and_capability_never_attests_focus() {
        let (source, down, up, delivery) = super::super::tests::fixture();
        down.set_flags(core_graphics::event::CGEventFlags::CGEventFlagCommand);
        assert_eq!(
            prepare_pair(&down, &up, &source, &delivery, [1012.5, 823.25])
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
        for (enabled, semantic) in [(false, false), (true, true), (true, false)] {
            let cap = super::super::super::public_appkit_click::zero_number_capabilities(
                enabled, semantic,
            );
            let old = super::super::super::public_appkit_click::capabilities(enabled, semantic);
            assert_eq!(cap["available"], old["available"]);
            assert_eq!(cap["event_number"], 0);
            for key in [
                "focus_preservation_verified",
                "application_effect_attested",
                "activation_requested",
                "command_modifier_added",
                "fallback",
            ] {
                assert_eq!(cap[key], false);
            }
            assert_eq!(
                old["value"],
                serde_json::json!({"delivery":"background_public_appkit_experimental"})
            );
        }
    }
}
