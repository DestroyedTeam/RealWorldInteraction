//! Opt-in public AppKit construction only; delivery remains the existing guarded
//! SLEventPostToPid pair. Neither availability nor posting proves app receipt.
use super::{
    Delivery, available,
    click::{PreparedClickEvent, click_capabilities, prepare_click_pair},
    window_local,
};
use crate::ffi;
use core_graphics::{
    event::{CGEvent, CGEventType, EventField},
    event_source::{CGEventSource, CGEventSourceStateID},
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
    // Same pointer alias as all existing crate declarations (FFI Ref=*const).
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
fn unavailable() -> rwi_core::Fault {
    fault(
        "background_delivery_unavailable",
        "Public AppKit mouse construction or independent CGEvent copy is unavailable; no fallback or input.",
    )
}
const FIELDS: [u32; 10] = [
    EventField::MOUSE_EVENT_NUMBER,
    EventField::MOUSE_EVENT_CLICK_STATE,
    EventField::MOUSE_EVENT_BUTTON_NUMBER,
    EventField::MOUSE_EVENT_DELTA_X,
    EventField::MOUSE_EVENT_DELTA_Y,
    EventField::EVENT_SOURCE_UNIX_PROCESS_ID,
    EventField::EVENT_SOURCE_USER_DATA,
    EventField::EVENT_SOURCE_USER_ID,
    EventField::EVENT_SOURCE_GROUP_ID,
    EventField::EVENT_SOURCE_STATE_ID,
];
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
struct Snapshot {
    kind: u32,
    point: [f64; 2],
    flags: u64,
    timestamp: u64,
    pressure: f64,
    integers: [i64; 10],
}
impl Snapshot {
    fn read(event: &CGEvent) -> Self {
        let location = event.location();
        Self {
            kind: event.get_type() as u32,
            point: [location.x, location.y],
            flags: event.get_flags().bits(),
            // SAFETY: the RAII event owns a live CGEventRef during this getter.
            timestamp: unsafe { CGEventGetTimestamp(event.as_ptr().cast()) },
            pressure: event.get_double_value_field(EventField::MOUSE_EVENT_PRESSURE),
            integers: FIELDS.map(|field| event.get_integer_value_field(field)),
        }
    }
}
fn validate_templates(
    down: &CGEvent,
    up: &CGEvent,
    source: &CGEventSource,
    delivery: &Delivery,
    point: [f64; 2],
) -> Result<[Snapshot; 2]> {
    let snapshots = [Snapshot::read(down), Snapshot::read(up)];
    let [d, u] = &snapshots;
    // The source is a borrow from this same MouseGesture, not a new HID/combined
    // table. GetSourceStateID returns the allocated private table's UNIQUE ID,
    // not the -1 enum parameter used to create it (CGEventSource.h).
    let state = unsafe { CGEventSourceGetSourceStateID(source.as_ptr().cast()) };
    // Match this exact table and reject public shared states; another invocation's
    // Private source must also fail, despite having the same creation enum.
    if down.as_ptr() == up.as_ptr()
        || d.kind != CGEventType::LeftMouseDown as u32
        || u.kind != CGEventType::LeftMouseUp as u32
        || d.point != point
        || u.point != point
        || d.flags != 0
        || u.flags != 0
        || d.pressure != 1.0
        || u.pressure != 0.0
        || d.integers[0] <= 0
        || d.integers[0] != u.integers[0]
        || [&d.integers, &u.integers]
            .into_iter()
            .any(|i| i[1] != 1 || i[2] != 0 || i[3] != 0 || i[4] != 0 || i[9] != i64::from(state))
        || state == CGEventSourceStateID::CombinedSessionState as i32
        || state == CGEventSourceStateID::HIDSystemState as i32
        || delivery.pid == 0
        || delivery.pid > i32::MAX as u32
        || delivery.window == 0
        || !delivery.frame.valid()
        || !point.iter().all(|v| v.is_finite())
        || !delivery.frame.contains(point)
    {
        return Err(fault(
            "invalid_pointer_delivery",
            "One exact plain Private-source single left pair and finite selected window are required; no input.",
        ));
    }
    Ok(snapshots)
}
fn independent_bridge_copy(raw: *const AppKitCGEvent) -> Result<CGEvent> {
    if raw.is_null() {
        return Err(unavailable());
    }
    // SAFETY: private caller keeps the retained factory NSEvent (and its +0 CG
    // getter result) alive. The public create-rule copy is independent, not clone
    // (which is CFRetain), and its one owned reference is released by CGEvent Drop.
    let copy = unsafe { CGEventCreateCopy(raw.cast()) };
    if copy.is_null() {
        return Err(unavailable());
    }
    let event = unsafe { CGEvent::from_ptr(copy.cast()) };
    if event.as_ptr().cast::<c_void>() == raw.cast_mut().cast() {
        return Err(fault(
            "event_constructor_mismatch",
            "The public copy did not produce independent memory; no input.",
        ));
    }
    Ok(event)
}
fn construct_one(
    original: &CGEvent,
    snapshot: &Snapshot,
    source: &CGEventSource,
    delivery: &Delivery,
    event_source: rwi_core::ClickEventSource,
) -> Result<CGEvent> {
    let class = event_class().ok_or_else(unavailable)?;
    let local = window_local(snapshot.point, delivery.frame);
    // AppKit window-base y is bottom-up; Quartz addressing below remains the
    // identical global point and existing top-down window-local setter.
    let base = [local[0], delivery.frame.h - local[1]];
    let event_number = isize::try_from(snapshot.integers[0]).map_err(|_| {
        fault(
            "invalid_pointer_delivery",
            "Event number exceeds NSInteger; no input.",
        )
    })?;
    // SAFETY: exact public NSEvent.h selector, NSInteger/NSUInteger/double/float
    // ABI, only valid down/up phases and nil deprecated context. Retain +0 before
    // the local autorelease pool drains. No NSApplication/NSWindow is created.
    let raw: *mut AnyObject = unsafe {
        msg_send![class,
        mouseEventWithType:snapshot.kind as usize,
        location:NSPoint{x:base[0],y:base[1]}, modifierFlags:0usize,
        timestamp:snapshot.timestamp as f64/1_000_000_000.0,
        windowNumber:delivery.window as isize,
        context:std::ptr::null_mut::<AnyObject>(), eventNumber:event_number,
        clickCount:1isize, pressure:snapshot.pressure as f32]
    };
    let appkit = unsafe { Retained::retain(raw) }.ok_or_else(unavailable)?;
    let borrowed: *const AppKitCGEvent = unsafe { msg_send![&*appkit, CGEvent] };
    let event = independent_bridge_copy(borrowed)?;
    if event.as_ptr() == original.as_ptr() {
        return Err(fault(
            "event_constructor_mismatch",
            "AppKit event aliases the Quartz template; no input.",
        ));
    }
    // Public factory has no source parameter. The unchanged default restores
    // the SAME invocation Private source; the explicit candidate retains the
    // factory software source. Both restore exact position and nanoseconds
    // (seconds ABI may round), never a new clock or integer field58 override.
    unsafe {
        if event_source == rwi_core::ClickEventSource::InvocationPrivate {
            CGEventSetSource(event.as_ptr().cast(), source.as_ptr().cast());
        }
        ffi::CGEventSetLocation(
            event.as_ptr().cast(),
            CGPoint::new(snapshot.point[0], snapshot.point[1]),
        );
        CGEventSetTimestamp(event.as_ptr().cast(), snapshot.timestamp);
    }
    let mut expected = snapshot.clone();
    if event_source == rwi_core::ClickEventSource::AppkitFactory {
        // This explicit software-event candidate retains factory state 0.
        // Every other source, clock, point and flag field must still match.
        expected.integers[9] = 0;
    }
    let observed = Snapshot::read(&event);
    if observed != expected {
        return Err(snapshot_mismatch(
            "Public mouse semantics/source/clock/point changed during AppKit construction; no input.",
            &expected,
            &observed,
        ));
    }
    // Factory-derived internal fields are not manually copied or overridden:
    // constructor provenance is the one explicit variable under investigation.
    Ok(event)
}
fn construct_pair(
    down: &CGEvent,
    up: &CGEvent,
    source: &CGEventSource,
    delivery: &Delivery,
    point: [f64; 2],
) -> Result<(CGEvent, CGEvent)> {
    construct_pair_with_source(
        down,
        up,
        source,
        delivery,
        point,
        rwi_core::ClickEventSource::InvocationPrivate,
    )
}
fn construct_pair_with_source(
    down: &CGEvent,
    up: &CGEvent,
    source: &CGEventSource,
    delivery: &Delivery,
    point: [f64; 2],
    event_source: rwi_core::ClickEventSource,
) -> Result<(CGEvent, CGEvent)> {
    let before = validate_templates(down, up, source, delivery, point)?;
    autoreleasepool(|_| {
        let first = construct_one(down, &before[0], source, delivery, event_source)?;
        if [Snapshot::read(down), Snapshot::read(up)] != before {
            return Err(fault(
                "event_constructor_mismatch",
                "Original pair changed after first construction; no input.",
            ));
        }
        let second = construct_one(up, &before[1], source, delivery, event_source)?;
        if first.as_ptr() == second.as_ptr() || [Snapshot::read(down), Snapshot::read(up)] != before
        {
            return Err(fault(
                "event_constructor_mismatch",
                "Independent pair memory or original templates changed; no input.",
            ));
        }
        // BOTH retained copies exist before any guarded down; no post here.
        Ok((first, second))
    })
}
fn snapshot_mismatch(reason: &str, expected: &Snapshot, observed: &Snapshot) -> rwi_core::Fault {
    let mut error = fault("event_constructor_mismatch", reason);
    error.diagnostic = Some(Box::new(
        serde_json::json!({"scope":"local_event_memory_only","input_attempted":false,"expected":expected,"observed":observed,"integer_fields":FIELDS}),
    ));
    error
}
pub(crate) enum PreparedSelectedClickEvent {
    Private(PreparedClickEvent),
    Public(super::public_appkit_click::PreparedPublicClickEvent),
}
impl PreparedSelectedClickEvent {
    pub(crate) fn post(&self) {
        match self {
            Self::Private(event) => event.post(),
            Self::Public(event) => event.post(),
        }
    }
}
pub(super) fn public_factory_available() -> bool {
    event_class().is_some()
}

pub(crate) fn prepare_selected_click_pair(
    down: &CGEvent,
    up: &CGEvent,
    source: &CGEventSource,
    delivery: &Delivery,
    point: [f64; 2],
    options: &rwi_core::ClickDeliveryOptions,
) -> Result<(PreparedSelectedClickEvent, PreparedSelectedClickEvent)> {
    let pair = match options {
        rwi_core::ClickDeliveryOptions::BackgroundSkylightExperimental {} => {
            prepare_click_pair(down, up, delivery, point)?
        }
        rwi_core::ClickDeliveryOptions::BackgroundSkylightAppkitExperimental {} => {
            prepare_appkit_click_pair(down, up, source, delivery, point)?
        }
        rwi_core::ClickDeliveryOptions::BackgroundPublicAppkitExperimental {
            mouse_subtype,
            event_source,
        } => {
            if *event_source == rwi_core::ClickEventSource::AppkitFactory {
                if *mouse_subtype != rwi_core::ClickMouseSubtype::Mouse {
                    return Err(fault(
                        "invalid_pointer_delivery",
                        "Factory-source candidate requires ordinary mouse; no input or fallback.",
                    ));
                }
                return prepare_public_appkit_pair_with_source(
                    down,
                    up,
                    source,
                    delivery,
                    point,
                    *event_source,
                );
            }
            return touch::prepare_pair(down, up, source, delivery, point, *mouse_subtype);
        }
        rwi_core::ClickDeliveryOptions::BackgroundPublicAppkitZeroNumberExperimental {} => {
            return zero_number::prepare_pair(down, up, source, delivery, point);
        }
    };
    Ok((
        PreparedSelectedClickEvent::Private(pair.0),
        PreparedSelectedClickEvent::Private(pair.1),
    ))
}
fn prepare_public_appkit_click_pair(
    down: &CGEvent,
    up: &CGEvent,
    source: &CGEventSource,
    delivery: &Delivery,
    point: [f64; 2],
) -> Result<(PreparedSelectedClickEvent, PreparedSelectedClickEvent)> {
    prepare_public_appkit_pair_with_source(
        down,
        up,
        source,
        delivery,
        point,
        rwi_core::ClickEventSource::InvocationPrivate,
    )
}
fn prepare_public_appkit_pair_with_source(
    down: &CGEvent,
    up: &CGEvent,
    source: &CGEventSource,
    delivery: &Delivery,
    point: [f64; 2],
    event_source: rwi_core::ClickEventSource,
) -> Result<(PreparedSelectedClickEvent, PreparedSelectedClickEvent)> {
    // Neither variant falls back to a private post. Original Private-template
    // validation and the same counted Sequence guards remain mandatory.
    let (down, up) = construct_pair_with_source(down, up, source, delivery, point, event_source)?;
    let before = [Snapshot::read(&down), Snapshot::read(&up)];
    let pair = super::public_appkit_click::prepare_pair(&down, &up, delivery, point)?;
    for (event, expected) in [(&down, &before[0]), (&up, &before[1])] {
        let observed = Snapshot::read(event);
        if observed != *expected {
            return Err(snapshot_mismatch(
                "Public PID addressing changed plain mouse fields/source/clock/point; no input.",
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
fn prepare_appkit_click_pair(
    down: &CGEvent,
    up: &CGEvent,
    source: &CGEventSource,
    delivery: &Delivery,
    point: [f64; 2],
) -> Result<(PreparedClickEvent, PreparedClickEvent)> {
    if !available() {
        return Err(unavailable());
    }
    let (down, up) = construct_pair(down, up, source, delivery, point)?;
    // Existing PID/window stamp, local setter and prepared-release ownership are
    // unchanged. Actual input still goes through the SAME Sequence::pair guards.
    let before = [Snapshot::read(&down), Snapshot::read(&up)];
    let prepared = prepare_click_pair(&down, &up, delivery, point)?;
    // The legacy preparation uses retained clones of these owned events, so we
    // can validate its final stamp/local setter without exposing raw prepared
    // event fields. This prevents a constructor-specific point/clock mutation.
    for (event, expected) in [(&down, &before[0]), (&up, &before[1])] {
        let observed = Snapshot::read(event);
        if observed != *expected {
            return Err(snapshot_mismatch(
                "Existing window stamp changed public mouse semantics/source/clock/point; no input.",
                expected,
                &observed,
            ));
        }
    }
    Ok(prepared)
}
pub(crate) fn appkit_click_capabilities(
    input_enabled: bool,
    semantic_only: bool,
) -> serde_json::Value {
    let mut cap = click_capabilities(input_enabled, semantic_only);
    let factory = event_class().is_some();
    cap["available"] = serde_json::json!(cap["available"] == true && factory);
    cap["value"] = serde_json::json!({"delivery":"background_skylight_appkit_experimental"});
    cap["constructor"] = serde_json::json!(
        "public NSEvent mouse factory + independent CGEventCreateCopy; same invocation Private source"
    );
    cap["timestamp_preserved_from_quartz_template"] = serde_json::json!(true);
    cap["timestamp_normalization"] =
        serde_json::json!("same original nanoseconds after public seconds bridge; no new clock");
    cap["constructor_owned_internal_fields_may_differ"] = serde_json::json!(true);
    cap["subtype_override_requested"] = serde_json::json!(false);
    if !factory {
        cap["reason"] =
            serde_json::json!("NSEvent factory or CGEvent getter unavailable; no fallback");
    }
    cap
}

#[cfg(test)]
#[path = "appkit_click/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "appkit_click/subtype_probe_tests.rs"]
mod subtype_probe_tests;

mod touch;
mod zero_number;
