//! Explicit public legacy tracking carrier. No default MouseMoved changes.
//! An opaque tag may be address-shaped inside AppKit. Never decode it as an
//! ObjC object, borrow foreign userData, guess modern area tags or auto-fallback.
use super::{Delivery, pointer_delivery_available, window_local};
use crate::ffi;
use core_graphics::{
    event::{CGEvent, CGEventFlags, EventField},
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
use rwi_core::{MovePointerOptions, Result, TrackingPhase, fault};
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
    // Raw UInt32 is intentional: core-graphics CGEventType lacks NSEvent kinds 8/9.
    fn CGEventGetType(event: *const c_void) -> u32;
    fn CGEventCreateCopy(event: *const c_void) -> *mut c_void;
    fn CGEventGetTimestamp(event: *const c_void) -> u64;
    fn CGEventSetTimestamp(event: *const c_void, timestamp: u64);
    fn CGEventSetSource(event: *mut c_void, source: *mut c_void);
    fn CGEventSourceGetSourceStateID(source: *const c_void) -> i32;
}
fn event_class() -> Option<&'static AnyClass> {
    AnyClass::get(c"NSEvent").filter(|class| {
        class.class_method(sel!(enterExitEventWithType:location:modifierFlags:timestamp:windowNumber:context:eventNumber:trackingNumber:userData:)).is_some()
        && class.class_method(sel!(eventWithCGEvent:)).is_some()
        && [sel!(CGEvent),sel!(type),sel!(windowNumber),sel!(locationInWindow),sel!(trackingNumber),sel!(userData)].into_iter().all(|s| class.instance_method(s).is_some())
    })
}
fn unavailable() -> rwi_core::Fault {
    fault(
        "background_delivery_unavailable",
        "Public tracking construction/addressing or current registered owner authority is unavailable; no fallback or input.",
    )
}
pub(crate) fn available() -> bool {
    // Conservative until an explicit controlled registration/lifetime gate is
    // chosen for the Native experiment. A parsed caller scalar alone is not proof.
    false
}
pub(crate) fn capabilities() -> serde_json::Value {
    serde_json::json!({"implemented":true,"available":available(),"opt_in_required":true,
        "factory_available":event_class().is_some(),"window_addressing_available":pointer_delivery_available(),
        "delivery":"background_appkit_tracking_experimental","phases":["entered","exited"],
        "tracking_number_encoding":"canonical nonzero signed i64 decimal string (opaque NSInteger)",
        "registration":"caller must establish exact currently registered legacy owner/tag/process/window lifetime",
        "opaque_tag_may_encode_target_address":true,"registration_attested_by_scalar_parse":false,
        "user_data":"NULL only","foreign_objc_pointer_decode":false,
        "modern_tracking_area_tag_discovery":false,"fallback":false,
        "activation_requested":false,"cursor_warp_requested":false,"native_acknowledgement":false,
        "application_effect_attested":false,"generic_cold_hover_verified":false,
        "scope":"experimental registered legacy owner calibration only; not generic modern tracking/tooltip"})
}
#[derive(Debug, Clone, PartialEq)]
struct Metadata {
    kind: usize,
    window: isize,
    point: [f64; 2],
    tag: isize,
    null_data: bool,
}
fn metadata(event: &AnyObject, kind: u32) -> Result<Metadata> {
    let actual: usize = unsafe { msg_send![event, type] };
    // These getters are legal only after actual type 8/9 is established.
    if actual != kind as usize || !matches!(actual, 8 | 9) {
        return Err(fault(
            "event_constructor_mismatch",
            "Actual tracking type differs; no tracking getters or input.",
        ));
    }
    unsafe {
        let point: NSPoint = msg_send![event, locationInWindow];
        let data: *mut c_void = msg_send![event, userData];
        Ok(Metadata {
            kind: actual,
            window: msg_send![event, windowNumber],
            point: [point.x, point.y],
            tag: msg_send![event, trackingNumber],
            null_data: data.is_null(),
        })
    }
}
fn copy_bridge(event: &AnyObject) -> Result<CGEvent> {
    let raw: *const AppKitCGEvent = unsafe { msg_send![event, CGEvent] };
    if raw.is_null() {
        return Err(unavailable());
    }
    let copied = unsafe { CGEventCreateCopy(raw.cast()) };
    if copied.is_null() {
        return Err(unavailable());
    }
    let owned = unsafe { CGEvent::from_ptr(copied.cast()) };
    if owned.as_ptr().cast::<c_void>() == raw.cast_mut().cast() {
        return Err(fault(
            "event_constructor_mismatch",
            "Tracking copy aliases the borrowed bridge; no input.",
        ));
    }
    Ok(owned)
}
fn roundtrip(event: &CGEvent, kind: u32) -> Result<Metadata> {
    let class = event_class().ok_or_else(unavailable)?;
    let raw: *mut AnyObject =
        unsafe { msg_send![class, eventWithCGEvent:event.as_ptr().cast::<AppKitCGEvent>()] };
    let owned = unsafe { Retained::retain(raw) }.ok_or_else(unavailable)?;
    metadata(&owned, kind)
}

/// Factory + independent CG carrier only. No post, native owner decoding,
/// activation, cursor warp, field58 override or unknown field mutation here.
pub(crate) fn construct(
    original: &CGEvent,
    source: &CGEventSource,
    delivery: &Delivery,
    point: [f64; 2],
    phase: TrackingPhase,
    declared_tag: &str,
) -> Result<CGEvent> {
    let tag = isize::try_from(MovePointerOptions::parse_tracking_number(declared_tag)?).map_err(
        |_| {
            fault(
                "invalid_input_value",
                "Public tag does not fit native NSInteger; no truncation or input.",
            )
        },
    )?;
    let kind = phase.native_kind();
    let source_state = unsafe { CGEventSourceGetSourceStateID(source.as_ptr().cast()) };
    let timestamp = unsafe { CGEventGetTimestamp(original.as_ptr().cast()) };
    let original_point = original.location();
    if unsafe { CGEventGetType(original.as_ptr().cast()) } != 5
        || [original_point.x, original_point.y] != point
        || original.get_flags().bits() != 0
        || matches!(source_state, 0 | 1)
        || delivery.pid == 0
        || delivery.pid > i32::MAX as u32
        || delivery.window == 0
        || !delivery.frame.valid()
        || !delivery.frame.contains(point)
        || !point.into_iter().all(f64::is_finite)
    {
        return Err(fault(
            "invalid_pointer_delivery",
            "Tracking construction requires exact private-source point/window addressing; no input.",
        ));
    }
    let class = event_class().ok_or_else(unavailable)?;
    let local = window_local(point, delivery.frame);
    let base = NSPoint::new(local[0], delivery.frame.h - local[1]);
    autoreleasepool(|_| {
        let raw: *mut AnyObject = unsafe {
            msg_send![class, enterExitEventWithType:kind as usize, location:base,
                modifierFlags:0usize, timestamp:timestamp as f64/1_000_000_000.0,
                windowNumber:delivery.window as isize, context:std::ptr::null_mut::<AnyObject>(),
                eventNumber:0isize, trackingNumber:tag, userData:std::ptr::null_mut::<c_void>()]
        };
        let appkit = unsafe { Retained::retain(raw) }.ok_or_else(unavailable)?;
        let before = metadata(&appkit, kind)?;
        if before.window != delivery.window as isize
            || before.point != [base.x, base.y]
            || before.tag != tag
            || !before.null_data
        {
            return Err(fault(
                "event_constructor_mismatch",
                "Public tracking factory fields differ; no input.",
            ));
        }
        let event = copy_bridge(&appkit)?;
        if event.as_ptr() == original.as_ptr()
            || unsafe { CGEventGetType(event.as_ptr().cast()) } != kind
        {
            return Err(fault(
                "event_constructor_mismatch",
                "Tracking bridge aliases template or raw kind differs; no input.",
            ));
        }
        // Preserve invocation-private source/clock/global point. No typed get_type
        // and no MouseMoved-only delta/pressure getter may see this 8/9 carrier.
        unsafe {
            CGEventSetSource(event.as_ptr().cast(), source.as_ptr().cast());
            ffi::CGEventSetLocation(event.as_ptr().cast(), CGPoint::new(point[0], point[1]));
            CGEventSetTimestamp(event.as_ptr().cast(), timestamp);
        }
        event.set_flags(CGEventFlags::empty());
        let after = roundtrip(&event, kind)?;
        let actual = event.location();
        if unsafe { CGEventGetType(event.as_ptr().cast()) } != kind
            || [actual.x, actual.y] != point
            || event.get_flags().bits() != 0
            || unsafe { CGEventGetTimestamp(event.as_ptr().cast()) } != timestamp
            || event.get_integer_value_field(EventField::EVENT_SOURCE_STATE_ID)
                != i64::from(source_state)
            || after.window != before.window
            || after.tag != before.tag
            || !after.null_data
            || !after.point.into_iter().all(f64::is_finite)
        {
            return Err(fault(
                "event_constructor_mismatch",
                "Tracking kind/window/tag/NULL source-clock-point contract changed; no input.",
            ));
        }
        Ok(event)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::{
        event::{CGEventType, CGMouseButton},
        event_source::CGEventSourceStateID,
    };
    fn memory(phase: TrackingPhase) {
        let source = CGEventSource::new(CGEventSourceStateID::Private).unwrap();
        let point = [440.0, 224.0];
        let original = CGEvent::new_mouse_event(
            source.clone(),
            CGEventType::MouseMoved,
            CGPoint::new(point[0], point[1]),
            CGMouseButton::Left,
        )
        .unwrap();
        original.set_flags(CGEventFlags::empty());
        let delivery = Delivery {
            pid: 514,
            window: 1_048_573,
            frame: crate::geometry::Rect {
                x: 0.0,
                y: 0.0,
                w: 880.0,
                h: 448.0,
            },
        };
        let event = construct(
            &original,
            &source,
            &delivery,
            point,
            phase,
            "9007199254740993",
        )
        .unwrap();
        assert_eq!(
            unsafe { CGEventGetType(event.as_ptr().cast()) },
            phase.native_kind()
        );
        let fields = autoreleasepool(|_| roundtrip(&event, phase.native_kind())).unwrap();
        assert_eq!(fields.tag, 9_007_199_254_740_993isize);
        assert!(fields.null_data);
        assert_eq!(fields.window, 1_048_573);
        assert_ne!(event.as_ptr(), original.as_ptr());
        println!(
            "F05_REGISTERED_TRACKING_MEMORY {}",
            serde_json::json!({"kind":phase.native_kind(),
            "tracking_tag_decimal":"9007199254740993","NULL_user_data":fields.null_data,
            "window_number":fields.window,"window_position":fields.point,
            "CG_position":point,"independent_copy":true,"native_post_count":0,"window_create_count":0,
            "synthetic_tag_registration_verified":false,"application_effect_verified":false})
        );
    }
    #[test]
    fn f05_registered_tracking_entered_carrier_memory() {
        memory(TrackingPhase::Entered);
    }
    #[test]
    fn f05_registered_tracking_exited_carrier_memory() {
        memory(TrackingPhase::Exited);
    }
    #[test]
    fn f05_registered_tracking_capability_is_separate_and_finite() {
        let cap = capabilities();
        assert_eq!(cap["available"], false);
        assert_eq!(cap["opt_in_required"], true);
        assert_eq!(cap["fallback"], false);
        assert_eq!(cap["native_acknowledgement"], false);
        assert_eq!(cap["application_effect_attested"], false);
        assert_eq!(cap["user_data"], "NULL only");
    }
}
