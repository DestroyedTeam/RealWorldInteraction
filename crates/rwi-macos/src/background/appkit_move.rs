//! Opt-in public NSEvent MouseMoved construction only. Posting stays in the
//! existing guarded CGEventPostToPid route; no tracking messages are fabricated.
use super::{Delivery, pointer_delivery_available, window_local};
use crate::ffi;
use core_graphics::{
    event::{CGEvent, CGEventType, EventField},
    event_source::CGEventSource,
    geometry::CGPoint,
};
use foreign_types::ForeignType;
use objc2::{
    Encoding, RefEncode, msg_send,
    rc::{Retained, autoreleasepool},
    runtime::{AnyClass, AnyObject},
    sel,
};
use objc2_foundation::NSPoint;
use rwi_core::{Result, fault};
use std::ffi::c_void;

#[repr(C)]
struct AppKitCGEvent {
    _opaque: [u8; 0],
}
unsafe impl RefEncode for AppKitCGEvent {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CGEvent", &[]));
}
#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateCopy(event: *const c_void) -> *mut c_void;
    fn CGEventGetTimestamp(event: *const c_void) -> u64;
    fn CGEventSetTimestamp(event: *const c_void, timestamp: u64);
    fn CGEventSetSource(event: *mut c_void, source: *mut c_void);
    fn CGEventSourceGetSourceStateID(source: *const c_void) -> i32;
}
fn event_class() -> Option<&'static AnyClass> {
    AnyClass::get(c"NSEvent").filter(|class| {
        class.class_method(sel!(mouseEventWithType:location:modifierFlags:timestamp:windowNumber:context:eventNumber:clickCount:pressure:)).is_some()
            && class.instance_method(sel!(CGEvent)).is_some()
    })
}
pub(crate) fn available() -> bool {
    event_class().is_some() && pointer_delivery_available()
}
fn unavailable() -> rwi_core::Fault {
    fault(
        "background_delivery_unavailable",
        "Public AppKit move construction or window addressing is unavailable; no fallback or input.",
    )
}
const FIELDS: [u32; 7] = [
    EventField::MOUSE_EVENT_DELTA_X,
    EventField::MOUSE_EVENT_DELTA_Y,
    EventField::EVENT_SOURCE_STATE_ID,
    EventField::EVENT_SOURCE_UNIX_PROCESS_ID,
    EventField::EVENT_SOURCE_USER_ID,
    EventField::EVENT_SOURCE_GROUP_ID,
    EventField::EVENT_SOURCE_USER_DATA,
];
#[derive(Debug, PartialEq)]
struct Snapshot {
    kind: u32,
    point: [f64; 2],
    flags: u64,
    timestamp: u64,
    integers: [i64; 7],
}
impl Snapshot {
    fn read(event: &CGEvent) -> Self {
        let point = event.location();
        Self {
            kind: event.get_type() as u32,
            point: [point.x, point.y],
            flags: event.get_flags().bits(),
            // SAFETY: CGEvent's RAII reference remains live through this getter.
            timestamp: unsafe { CGEventGetTimestamp(event.as_ptr().cast()) },
            integers: FIELDS.map(|field| event.get_integer_value_field(field)),
        }
    }
}
fn independent_copy(raw: *const AppKitCGEvent) -> Result<CGEvent> {
    if raw.is_null() {
        return Err(unavailable());
    }
    // SAFETY: caller retains the factory NSEvent throughout this get-rule bridge.
    // The public create-rule copy owns independent memory and one CF reference.
    let copied = unsafe { CGEventCreateCopy(raw.cast()) };
    if copied.is_null() {
        return Err(unavailable());
    }
    let event = unsafe { CGEvent::from_ptr(copied.cast()) };
    if event.as_ptr().cast::<c_void>() == raw.cast_mut().cast() {
        return Err(fault(
            "event_constructor_mismatch",
            "Factory bridge copy aliases its input; no input.",
        ));
    }
    Ok(event)
}
pub(crate) fn construct(
    original: &CGEvent,
    source: &CGEventSource,
    delivery: &Delivery,
    point: [f64; 2],
) -> Result<CGEvent> {
    let before = Snapshot::read(original);
    // The borrowed source belongs to this same move invocation. Private tables
    // have unique IDs, not the -1 constructor enum. Never borrow a shared table.
    let state = unsafe { CGEventSourceGetSourceStateID(source.as_ptr().cast()) };
    if before.kind != CGEventType::MouseMoved as u32
        || before.point != point
        || before.flags != 0
        || before.integers[..2] != [0, 0]
        || matches!(state, 0 | 1)
        || before.integers[2] != i64::from(state)
        || delivery.pid == 0
        || delivery.pid > i32::MAX as u32
        || delivery.window == 0
        || !delivery.frame.valid()
        || !point.iter().all(|v| v.is_finite())
        || !delivery.frame.contains(point)
    {
        return Err(fault(
            "invalid_pointer_delivery",
            "One plain finite MouseMoved with its exact private source and window is required; no input.",
        ));
    }
    let class = event_class().ok_or_else(unavailable)?;
    let local = window_local(point, delivery.frame);
    let base = NSPoint::new(local[0], delivery.frame.h - local[1]);
    autoreleasepool(|_| {
        // Public NSEvent.h selector and exact ABI. For MouseMoved, mandatory
        // button-factory arguments are 0; invalid move getters are never called.
        // Retain the +0 object before this local autorelease pool is drained.
        let raw: *mut AnyObject = unsafe {
            msg_send![class,
            mouseEventWithType:5usize, location:base, modifierFlags:0usize,
            timestamp:before.timestamp as f64/1_000_000_000.0, windowNumber:delivery.window as isize,
            context:std::ptr::null_mut::<AnyObject>(), eventNumber:0isize,
            clickCount:0isize, pressure:0.0f32]
        };
        let appkit = unsafe { Retained::retain(raw) }.ok_or_else(unavailable)?;
        let bridge: *const AppKitCGEvent = unsafe { msg_send![&*appkit, CGEvent] };
        let event = independent_copy(bridge)?;
        if event.as_ptr() == original.as_ptr() {
            return Err(fault(
                "event_constructor_mismatch",
                "Factory copy aliases the Quartz template; no input.",
            ));
        }
        // Public factory has no source argument. Preserve THIS private source,
        // exact Quartz point and nanosecond clock; seconds may round. No source
        // PID spoof, field58 override, tracking userdata, or unknown-field copy.
        unsafe {
            CGEventSetSource(event.as_ptr().cast(), source.as_ptr().cast());
            ffi::CGEventSetLocation(event.as_ptr().cast(), CGPoint::new(point[0], point[1]));
            CGEventSetTimestamp(event.as_ptr().cast(), before.timestamp);
        }
        if Snapshot::read(&event) != before || Snapshot::read(original) != before {
            return Err(fault(
                "event_constructor_mismatch",
                "MouseMoved source/clock/flags/delta/point changed during factory construction; no input.",
            ));
        }
        // Existing deliver_pointer stamps/revalidates the same exact addressing
        // at the guarded post. This function creates memory only and never posts.
        Ok(event)
    })
}
/// Public bridge selectors only. This feature is separately advertised so an
/// old AppKit constructor capability never authorizes caller-declared deltas.
fn delta_event_class() -> Option<&'static AnyClass> {
    event_class().filter(|class| {
        class.class_method(sel!(eventWithCGEvent:)).is_some()
            && class.instance_method(sel!(type)).is_some()
            && class.instance_method(sel!(deltaX)).is_some()
            && class.instance_method(sel!(deltaY)).is_some()
    })
}
fn public_move_delta(event: &CGEvent) -> Result<[f64; 2]> {
    if event.get_type() as u32 != CGEventType::MouseMoved as u32 {
        return Err(fault(
            "invalid_pointer_delivery",
            "Only MouseMoved has this explicit delta contract; no input.",
        ));
    }
    let class = delta_event_class().ok_or_else(unavailable)?;
    autoreleasepool(|_| {
        let raw: *mut AnyObject =
            unsafe { msg_send![class, eventWithCGEvent:event.as_ptr().cast::<AppKitCGEvent>()] };
        // Public factory is get-rule. Retain while both the owning CG event and
        // local autorelease pool are alive; no borrowed pointer escapes this scope.
        let appkit = unsafe { Retained::retain(raw) }.ok_or_else(unavailable)?;
        let kind: usize = unsafe { msg_send![&*appkit, type] };
        if kind != 5 {
            return Err(fault(
                "event_constructor_mismatch",
                "Public bridge did not return MouseMoved; delta getters not read and no input.",
            ));
        }
        let delta: [f64; 2] = unsafe { [msg_send![&*appkit, deltaX], msg_send![&*appkit, deltaY]] };
        if !delta.iter().all(|v| v.is_finite()) {
            return Err(fault(
                "event_constructor_mismatch",
                "Public bridge delta is unknown/nonfinite; no input.",
            ));
        }
        Ok(delta)
    })
}
/// Mutate only the two public integer delta fields of the already independent
/// private-source AppKit constructor copy. No previous physical position is
/// guessed; no button, source PID, clock, point, tracking or transport is changed.
pub(crate) fn apply_explicit_delta(event: &CGEvent, delta: [i32; 2]) -> Result<()> {
    rwi_core::MovePointerOptions::validate_explicit_delta(delta)?;
    let mut expected = Snapshot::read(event);
    if expected.kind != CGEventType::MouseMoved as u32
        || expected.flags != 0
        || expected.integers[..2] != [0, 0]
        || matches!(expected.integers[2], 0 | 1)
        || !expected.point.iter().all(|v| v.is_finite())
    {
        return Err(fault(
            "invalid_pointer_delivery",
            "One plain zero-delta private MouseMoved copy is required; no input.",
        ));
    }
    delta_event_class().ok_or_else(unavailable)?;
    for (field, value) in [
        EventField::MOUSE_EVENT_DELTA_X,
        EventField::MOUSE_EVENT_DELTA_Y,
    ]
    .into_iter()
    .zip(delta)
    {
        event.set_integer_value_field(field, i64::from(value));
    }
    expected.integers[0] = i64::from(delta[0]);
    expected.integers[1] = i64::from(delta[1]);
    let actual = public_move_delta(event)?;
    if Snapshot::read(event) != expected || actual != delta.map(f64::from) {
        return Err(fault(
            "event_constructor_mismatch",
            "Public CG integer delta / NSEvent delta bridge changed fields or values; no input.",
        ));
    }
    Ok(())
}
pub(crate) fn capabilities() -> serde_json::Value {
    serde_json::json!({"implemented":true,"available":available(),"opt_in_required":true,
        "route":"public NSEvent MouseMoved constructor + existing CGEventPostToPid/window-addressed",
        "value":{"delivery":"background_appkit_experimental"},"fallback":false,
        "explicit_delta":{"implemented":true,"available":available() && delta_event_class().is_some(),
            "value":{"delta":[17,-9]},"each_axis_min":-128,"each_axis_max":128,"nonzero_required":true,
            "meaning":"caller-declared CG integer fields; previous physical motion unknown",
            "public_bridge_readback_required":true,"native_tracking_attested":false,"fallback":false},
        "native_acknowledgement":false,"application_effect_attested":false,
        "activation_requested":false,"cursor_warp_requested":false,"authentication_requested":false,
        "scope":"experimental macOS constructor-only; exact native binding/lease/permissions still required"})
}
#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::{
        event::{CGEventFlags, CGMouseButton},
        event_source::CGEventSourceStateID,
    };
    fn fixture() -> (CGEventSource, CGEvent, Delivery, [f64; 2]) {
        let source = CGEventSource::new(CGEventSourceStateID::Private).unwrap();
        let point = [1012.5, 823.25];
        let event = CGEvent::new_mouse_event(
            source.clone(),
            CGEventType::MouseMoved,
            CGPoint::new(point[0], point[1]),
            CGMouseButton::Left,
        )
        .unwrap();
        event.set_flags(CGEventFlags::empty());
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, 0);
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y, 0);
        let delivery = Delivery {
            pid: 42,
            window: 1_048_573,
            frame: crate::geometry::Rect {
                x: 700.0,
                y: 600.0,
                w: 600.0,
                h: 400.0,
            },
        };
        (source, event, delivery, point)
    }
    #[test]
    fn f05_factory_move_memory_keeps_same_private_source_clock_point_and_original() {
        let (source, original, delivery, point) = fixture();
        let before = Snapshot::read(&original);
        let candidate = construct(&original, &source, &delivery, point).unwrap();
        assert_ne!(original.as_ptr(), candidate.as_ptr());
        assert_eq!(Snapshot::read(&candidate), before);
        assert_eq!(Snapshot::read(&original), before);
        assert_eq!(candidate.get_type() as u32, 5);
        assert!(!matches!(
            candidate.get_integer_value_field(EventField::EVENT_SOURCE_STATE_ID),
            0 | 1
        ));
        // Neither a synthetic window number nor these pure getters attest input.
        println!(
            "{}",
            serde_json::json!({"probe":"f05_factory_move_memory","constructor":"public_nsevent",
            "same_private_source_clock_point":true,"original_unchanged":true,
            "post_calls":0,"NSApplication_or_NSWindow_created":false,
            "native_receiving_or_tracking_verified":false})
        );
    }
    #[test]
    fn f05_factory_move_bad_source_kind_flags_delta_point_or_window_is_predispatch() {
        let (source, event, delivery, point) = fixture();
        let another = CGEventSource::new(CGEventSourceStateID::Private).unwrap();
        assert_eq!(
            construct(&event, &another, &delivery, point)
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
        assert_eq!(
            construct(
                &event,
                &source,
                &Delivery {
                    window: 0,
                    ..delivery
                },
                point
            )
            .err()
            .unwrap()
            .code,
            "invalid_pointer_delivery"
        );
        assert_eq!(
            construct(&event, &source, &delivery, [f64::NAN, point[1]])
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
        event.set_flags(CGEventFlags::CGEventFlagCommand);
        assert_eq!(
            construct(&event, &source, &delivery, point)
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
        event.set_flags(CGEventFlags::empty());
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, 1);
        assert_eq!(
            construct(&event, &source, &delivery, point)
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, 0);
        event.set_type(CGEventType::LeftMouseDown);
        assert_eq!(
            construct(&event, &source, &delivery, point)
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
    }
    #[test]
    fn f05_factory_move_null_bridge_and_capability_never_claim_delivery_or_goal() {
        assert_eq!(
            independent_copy(std::ptr::null()).err().unwrap().code,
            "background_delivery_unavailable"
        );
        let cap = capabilities();
        assert_eq!(cap["value"]["delivery"], "background_appkit_experimental");
        assert_eq!(cap["application_effect_attested"], false);
        assert_eq!(cap["fallback"], false);
        assert_eq!(cap["activation_requested"], false);
        assert_eq!(cap["cursor_warp_requested"], false);
    }
    #[test]
    fn f05_explicit_delta_public_memory_bridge_changes_only_two_fields() {
        for delta in [[17, -9], [-128, 128], [0, 1]] {
            let (source, original, delivery, point) = fixture();
            let before = Snapshot::read(&original);
            let event = construct(&original, &source, &delivery, point).unwrap();
            assert_ne!(event.as_ptr(), original.as_ptr());
            let mut expected = Snapshot::read(&event);
            apply_explicit_delta(&event, delta).unwrap();
            expected.integers[0] = i64::from(delta[0]);
            expected.integers[1] = i64::from(delta[1]);
            assert_eq!(Snapshot::read(&event), expected);
            assert_eq!(public_move_delta(&event).unwrap(), delta.map(f64::from));
            assert_eq!(Snapshot::read(&original), before);
        }
    }
    #[test]
    fn f05_explicit_delta_invalid_input_is_before_mutation_or_getters() {
        for delta in [[0, 0], [129, 0], [0, -129], [i32::MIN, 1]] {
            let (_, event, _, _) = fixture();
            let before = Snapshot::read(&event);
            assert_eq!(
                apply_explicit_delta(&event, delta).unwrap_err().code,
                "invalid_input_value"
            );
            assert_eq!(Snapshot::read(&event), before);
        }
        let (_, event, _, _) = fixture();
        event.set_type(CGEventType::LeftMouseDown);
        let before = Snapshot::read(&event);
        assert_eq!(
            apply_explicit_delta(&event, [17, -9]).unwrap_err().code,
            "invalid_pointer_delivery"
        );
        assert_eq!(Snapshot::read(&event), before);
    }
    #[test]
    fn f05_explicit_delta_capability_never_attests_native_tracking_or_physical_motion() {
        let cap = capabilities();
        let feature = &cap["explicit_delta"];
        assert_eq!(feature["implemented"], true);
        assert_eq!(feature["native_tracking_attested"], false);
        assert_eq!(feature["each_axis_min"], -128);
        assert_eq!(feature["each_axis_max"], 128);
        assert_eq!(
            cap["value"],
            serde_json::json!({"delivery":"background_appkit_experimental"})
        );
        assert_eq!(cap["cursor_warp_requested"], false);
        assert_eq!(cap["activation_requested"], false);
    }
    #[test]
    fn h2_after_stamp_public_decode_is_sender_memory_only_without_post() {
        fn cg_sample(event: &CGEvent) -> serde_json::Value {
            let value = Snapshot::read(event);
            serde_json::json!({"kind":value.kind,"cg_screen_point":value.point,
                "flags":value.flags,"timestamp_ns":value.timestamp,
                "delta":[value.integers[0],value.integers[1]],
                "source_state_id":value.integers[2],"source_unix_pid":value.integers[3],
                "source_user_id":value.integers[4],"source_group_id":value.integers[5],
                "source_user_data_is_zero":value.integers[6]==0,
                "target_unix_pid":event.get_integer_value_field(EventField::EVENT_TARGET_UNIX_PROCESS_ID),
                "addressed_window_field_51":event.get_integer_value_field(super::super::TARGET_WINDOW_NUMBER_FIELD),
                "window_under_pointer_field_91":event.get_integer_value_field(EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER),
                "window_can_handle_field_92":event.get_integer_value_field(EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER_THAT_CAN_HANDLE_THIS_EVENT)})
        }
        let (source, original, delivery, point) = fixture();
        let original_before = Snapshot::read(&original);
        let original_selected_before = cg_sample(&original);
        let candidate = construct(&original, &source, &delivery, point).unwrap();
        let constructed = Snapshot::read(&candidate);
        let factory_copy = cg_sample(&candidate);
        let class = AnyClass::get(c"NSEvent")
            .filter(|c| c.class_method(sel!(eventWithCGEvent:)).is_some())
            .expect("public eventWithCGEvent selector");
        for selector in [
            sel!(type),
            sel!(windowNumber),
            sel!(locationInWindow),
            sel!(modifierFlags),
            sel!(timestamp),
            sel!(deltaX),
            sel!(deltaY),
        ] {
            assert!(class.instance_method(selector).is_some());
        }
        // Same memory stamp/setter as deliver_pointer, deliberately NOT post().
        super::super::stamp(&candidate, &delivery, true);
        let after_stamp = cg_sample(&candidate);
        let local = window_local(point, delivery.frame);
        assert!(ffi::set_window_location(
            candidate.as_ptr().cast(),
            CGPoint::new(local[0], local[1])
        ));
        let after_window_location = cg_sample(&candidate);
        assert_eq!(Snapshot::read(&candidate), constructed);
        assert_eq!(Snapshot::read(&original), original_before);
        assert_eq!(
            candidate.get_integer_value_field(EventField::EVENT_TARGET_UNIX_PROCESS_ID),
            i64::from(delivery.pid)
        );
        assert_eq!(
            candidate.get_integer_value_field(super::super::TARGET_WINDOW_NUMBER_FIELD),
            i64::from(delivery.window)
        );
        autoreleasepool(|_| {
            let raw: *mut AnyObject = unsafe {
                msg_send![class, eventWithCGEvent:candidate.as_ptr().cast::<AppKitCGEvent>()]
            };
            let decoded = unsafe { Retained::retain(raw) }.expect("public CG bridge");
            let kind: usize = unsafe { msg_send![&*decoded, type] };
            assert_eq!(kind, 5); // Never call invalid move button/eventNumber getters.
            let p: NSPoint = unsafe { msg_send![&*decoded, locationInWindow] };
            let window: isize = unsafe { msg_send![&*decoded, windowNumber] };
            let flags: usize = unsafe { msg_send![&*decoded, modifierFlags] };
            let timestamp: f64 = unsafe { msg_send![&*decoded, timestamp] };
            let delta: [f64; 2] =
                unsafe { [msg_send![&*decoded, deltaX], msg_send![&*decoded, deltaY]] };
            assert!(
                p.x.is_finite()
                    && p.y.is_finite()
                    && timestamp.is_finite()
                    && delta.into_iter().all(f64::is_finite)
            );
            assert_eq!(flags, 0);
            println!(
                "{}",
                serde_json::json!({
                    "probe":"h2_after_stamp_public_decode", "kind":kind,
                    "quartz_screen_point":point, "declared_topdown_window_local":local,
                    "declared_factory_window_base":[local[0],delivery.frame.h-local[1]],
                    "decoded_sender_location_in_window":[p.x,p.y],
                    "decoded_sender_window_number":window, "timestamp_seconds":timestamp,
                    "decoded_sender_delta":delta, "factory_copy_before_stamp":factory_copy,
                    "after_address_stamp":after_stamp,
                    "after_window_location_setter":after_window_location,
                    "source_user_data_address_exported":false,
                    "original_selected_fields_unchanged":cg_sample(&original)==original_selected_before,
                    "fixture_window_is_synthetic":true, "explicit_NSWindow_create_calls":0,
                    "input_post_calls":0, "activation_calls":0, "pointer_warp_calls":0,
                    "recipient_NSWindow_context_known":false,
                    "recipient_CocoaMain_key_hitTest_tracking_or_Chrome_effect_verified":false,
                    "production_coordinate_rule_changed":false
                })
            );
        });
        assert_eq!(Snapshot::read(&original), original_before);
        assert_eq!(cg_sample(&original), original_selected_before);
    }
}
