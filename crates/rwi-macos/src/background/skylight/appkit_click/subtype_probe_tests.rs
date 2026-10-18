//! S2 sender-memory experiment only. No post, application/window or input API.
//! AppKit names mouse subtype 3 Touch; it is not a documented no-focus flag.
use super::*;
use serde_json::{Value, json};

const TOUCH_SUBTYPE: i64 = 3;

fn fixture() -> (CGEventSource, CGEvent, CGEvent, Delivery) {
    let (source, down, up, delivery) = super::tests::fixture();
    // Nonzero, different clocks expose normalization/reset bugs hidden by the
    // platform factory's zero timestamp default. No event is posted here.
    unsafe {
        CGEventSetTimestamp(down.as_ptr().cast(), 123_456_789_123);
        CGEventSetTimestamp(up.as_ptr().cast(), 123_481_789_123);
    }
    (source, down, up, delivery)
}

fn independent_variant(original: &CGEvent, subtype: i64) -> CGEvent {
    // Copy ownership is independent; changing this experiment must not mutate
    // the production factory event, its paired phase, or the shared source.
    let event = independent_bridge_copy(original.as_ptr().cast()).unwrap();
    assert_ne!(event.as_ptr(), original.as_ptr());
    event.set_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE, subtype);
    event
}

fn appkit_decode(event: &CGEvent) -> Value {
    autoreleasepool(|_| {
        let class = event_class().unwrap();
        assert!(class.class_method(sel!(eventWithCGEvent:)).is_some());
        // SAFETY: NSEvent.h takes a borrowed CGEventRef, returns a nullable +0
        // object retaining it. The RAII copy outlives this autorelease pool.
        let raw: *mut AnyObject =
            unsafe { msg_send![class, eventWithCGEvent:event.as_ptr().cast::<AppKitCGEvent>()] };
        let Some(decoded) = (unsafe { Retained::retain(raw) }) else {
            return json!({"decoded":false});
        };
        // All queried getters are documented for the mouse down/up types used
        // here. In the SDK NSEventSubtype is short, not NSInteger.
        let kind: usize = unsafe { msg_send![&*decoded, type] };
        let subtype: i16 = unsafe { msg_send![&*decoded, subtype] };
        let flags: usize = unsafe { msg_send![&*decoded, modifierFlags] };
        let window: isize = unsafe { msg_send![&*decoded, windowNumber] };
        let number: isize = unsafe { msg_send![&*decoded, eventNumber] };
        let clicks: isize = unsafe { msg_send![&*decoded, clickCount] };
        let pressure: f32 = unsafe { msg_send![&*decoded, pressure] };
        let point: NSPoint = unsafe { msg_send![&*decoded, locationInWindow] };
        let borrowed: *const AppKitCGEvent = unsafe { msg_send![&*decoded, CGEvent] };
        let roundtrip = independent_bridge_copy(borrowed).unwrap();
        json!({"decoded":true,"type":kind,"subtype":subtype,"flags":flags,
            "window_number":window,"event_number":number,"click_count":clicks,
            "pressure":pressure,"location_in_window":[point.x,point.y],
            "bridge_subtype":roundtrip.get_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE),
            "bridge_snapshot":Snapshot::read(&roundtrip)})
    })
}

#[test]
fn f187_subtype3_independent_memory_keeps_other_fields_and_original_pair() {
    let (source, down, up, delivery) = fixture();
    let pair = construct_pair(&down, &up, &source, &delivery, [1012.5, 823.25]).unwrap();
    let originals = [Snapshot::read(&pair.0), Snapshot::read(&pair.1)];
    for event in [&pair.0, &pair.1] {
        let subtype = event.get_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE);
        let variant = independent_variant(event, TOUCH_SUBTYPE);
        assert_eq!(Snapshot::read(&variant), Snapshot::read(event));
        assert_eq!(
            variant.get_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE),
            TOUCH_SUBTYPE
        );
        assert_eq!(
            event.get_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE),
            subtype
        );
        assert_eq!(event.get_flags().bits(), 0);
    }
    assert_eq!(
        [Snapshot::read(&pair.0), Snapshot::read(&pair.1)],
        originals
    );
    assert_eq!([Snapshot::read(&down), Snapshot::read(&up)], originals);
}

#[test]
fn f187_subtype3_roundtrip_records_receiver_decode_not_focus_effect() {
    let (source, down, up, delivery) = fixture();
    let pair = construct_pair(&down, &up, &source, &delivery, [1012.5, 823.25]).unwrap();
    let mut observations = Vec::new();
    for (phase, event) in [("down", &pair.0), ("up", &pair.1)] {
        for requested in [0, TOUCH_SUBTYPE] {
            let variant = independent_variant(event, requested);
            let before = Snapshot::read(&variant);
            let decoded = appkit_decode(&variant);
            assert_eq!(
                Snapshot::read(&variant),
                before,
                "decoding changed the original event"
            );
            // These assertions concern memory, not whether a third-party app
            // consumes Touch the same way as a default mouse subtype.
            if decoded["decoded"] == true {
                assert_eq!(decoded["type"], before.kind);
                assert_eq!(decoded["flags"], 0);
                assert_eq!(decoded["event_number"], 73);
                assert_eq!(decoded["click_count"], 1);
                assert_eq!(decoded["subtype"], requested);
                assert_eq!(decoded["bridge_subtype"], requested);
                assert_eq!(
                    decoded["bridge_snapshot"],
                    serde_json::to_value(&before).unwrap()
                );
            }
            observations.push(json!({"phase":phase,"requested_subtype":requested,
                "source_snapshot":before,"appkit":decoded}));
        }
    }
    println!(
        "F187_SUBTYPE_OBSERVATION {}",
        json!({
        "scope":"sender_owned_event_memory_only","input_post_count":0,
        "native_target_effect_observed":false,"foreground_preservation_observed":false,
        "appkit_sdk_name_for_3":"NSEventSubtypeTouch","rows":observations})
    );
}

#[test]
fn f187_subtype3_existing_addressing_is_memory_only_and_not_auto_selected() {
    let (source, down, up, delivery) = fixture();
    let pair = construct_pair(&down, &up, &source, &delivery, [1012.5, 823.25]).unwrap();
    assert_eq!(
        pair.0
            .get_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE),
        0
    );
    assert_eq!(
        pair.1
            .get_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE),
        0
    );
    let d = independent_variant(&pair.0, TOUCH_SUBTYPE);
    let u = independent_variant(&pair.1, TOUCH_SUBTYPE);
    let before = [Snapshot::read(&d), Snapshot::read(&u)];
    let prepared =
        super::super::public_appkit_click::prepare_pair(&d, &u, &delivery, [1012.5, 823.25]);
    if crate::background::pointer_delivery_available() {
        drop(prepared.unwrap()); // Deliberately never call post().
        for event in [&d, &u] {
            assert_eq!(
                event.get_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE),
                TOUCH_SUBTYPE
            );
            assert_eq!(
                event.get_integer_value_field(51),
                i64::from(delivery.window)
            );
            assert_eq!(
                event.get_integer_value_field(EventField::EVENT_TARGET_UNIX_PROCESS_ID),
                i64::from(delivery.pid)
            );
        }
    } else {
        assert_eq!(
            prepared.err().unwrap().code,
            "background_delivery_unavailable"
        );
    }
    assert_eq!([Snapshot::read(&d), Snapshot::read(&u)], before);
    assert_eq!(
        appkit_click_capabilities(true, false)["subtype_override_requested"],
        false
    );
}
