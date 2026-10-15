//! Construction-only AppKit/Quartz probe, compiled solely into macOS tests.
//!
//! This is not an input route: it creates no NSApplication or NSWindow, resolves
//! no real window/display, changes no global input state, and posts no event.
//! A synthetic window number is fixture data, not an attested native target.
//! Public CGEvent fields have no window-number accessor; the roundtrip's
//! `windowNumber` must therefore be observed, never filled from the fixture.
//! Likewise, without real window geometry, local/global coordinate conversion
//! is unproven even when two observed point values happen to be equal.
use crate::ffi;
use core_graphics::event::{CGEvent, CGEventFlags, EventField};
use foreign_types::ForeignType;
use objc2::{
    Encoding, RefEncode, msg_send,
    rc::{Retained, autoreleasepool},
    runtime::{AnyClass, AnyObject},
    sel,
};
use objc2_foundation::NSPoint;
use serde::Serialize;
use std::ffi::c_void;
use std::time::Instant;

// The core-graphics opaque type does not implement objc2's encoding trait.
// Match public CGEventRef's opaque struct pointer, rather than misencoding it
// as an Objective-C object or disabling objc2's runtime signature checking.
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
    // Public CGEvent.h APIs missing from the pinned core-graphics Rust wrapper.
    fn CGEventGetTimestamp(event: *const c_void) -> u64;
    fn CGEventGetType(event: *const c_void) -> u32;
    fn CGEventCreateData(allocator: ffi::Ref, event: ffi::Ref) -> ffi::Ref;
    fn CGEventCreateFromData(allocator: ffi::Ref, data: ffi::Ref) -> ffi::Ref;
    fn CGEventSetTimestamp(event: *const c_void, timestamp: u64);
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum MousePhase {
    Down,
    Dragged,
    Up,
}
impl MousePhase {
    fn event_type(self) -> usize {
        // Public NSEventType values from NSEvent.h, also the matching CG types.
        match self {
            Self::Down => 1,
            Self::Dragged => 6,
            Self::Up => 2,
        }
    }
    fn has_delta(self) -> bool {
        matches!(self, Self::Dragged)
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
struct MouseFixture {
    phase: MousePhase,
    local_position: [f64; 2],
    window_number: isize,
    flags: usize,
    timestamp_seconds: f64,
    event_number: isize,
    click_count: isize,
    pressure: f32,
}
impl MouseFixture {
    fn validate(self) -> Result<(), &'static str> {
        if self.local_position.into_iter().any(|v| !v.is_finite())
            || !self.timestamp_seconds.is_finite()
            || self.timestamp_seconds < 0.0
            || !self.pressure.is_finite()
            || !(0.0..=1.0).contains(&self.pressure)
            || self.window_number < 0
            || self.event_number < 0
            || self.click_count < 0
        {
            Err("invalid_mouse_fixture")
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, PartialEq, Serialize)]
struct AppKitSnapshot {
    event_type: usize,
    window_number: isize,
    local_position: [f64; 2],
    flags: usize,
    timestamp_seconds: f64,
    button_number: isize,
    click_count: isize,
    pressure: f32,
    event_number: isize,
    // NSEvent.h only permits deltaX/Y for move/drag/scroll, not down/up.
    delta: Option<[f64; 2]>,
}

#[derive(Debug, PartialEq, Serialize)]
struct QuartzSnapshot {
    event_type: u32,
    global_position: [f64; 2],
    flags: u64,
    timestamp_nanoseconds: u64,
    button_number: i64,
    click_count: i64,
    pressure: f64,
    event_number: i64,
    delta: Option<[i64; 2]>,
}

#[derive(Debug, Serialize)]
struct MouseBridgeSnapshot {
    input: MouseFixture,
    constructed: AppKitSnapshot,
    quartz: QuartzSnapshot,
    roundtrip: AppKitSnapshot,
    stage_milliseconds: [f64; 6],
}

fn event_class() -> Result<&'static AnyClass, &'static str> {
    AnyClass::get(c"NSEvent").ok_or("nsevent_class_unavailable")
}

fn create_mouse(fixture: MouseFixture) -> Result<Retained<AnyObject>, &'static str> {
    fixture.validate()?;
    let class = event_class()?;
    // SAFETY: Public NSEvent factory signature in NSEvent.h; only down/drag/up
    // types, finite fixture values and nil deprecated graphics context are used.
    // The +0 factory result is immediately retained before the pool can drain.
    let raw: *mut AnyObject = unsafe {
        msg_send![class,
            mouseEventWithType: fixture.phase.event_type(),
            location: NSPoint { x: fixture.local_position[0], y: fixture.local_position[1] },
            modifierFlags: fixture.flags,
            timestamp: fixture.timestamp_seconds,
            windowNumber: fixture.window_number,
            context: std::ptr::null_mut::<AnyObject>(),
            eventNumber: fixture.event_number,
            clickCount: fixture.click_count,
            pressure: fixture.pressure]
    };
    unsafe { Retained::retain(raw) }.ok_or("mouse_factory_returned_nil")
}

fn bridge_to_quartz(event: &AnyObject) -> Result<CGEvent, &'static str> {
    // SAFETY: `event` is a retained factory-created NSEvent. CGEvent is nullable
    // and autoreleased under the documented get rule, so add an independent +1
    // before wrapping it with the CGEvent RAII owner (whose Drop releases it).
    let raw: *const AppKitCGEvent = unsafe { msg_send![event, CGEvent] };
    if raw.is_null() {
        return Err("cgevent_bridge_returned_null");
    }
    Ok(unsafe { CGEvent::from_ptr(ffi::CFRetain(raw.cast()).cast_mut().cast()) })
}

fn bridge_to_appkit(event: &CGEvent) -> Result<Retained<AnyObject>, &'static str> {
    let class = event_class()?;
    // SAFETY: Public eventWithCGEvent: takes a valid, live CGEventRef. It retains
    // that CGEvent itself. Retain the nullable +0 NSEvent result as well, allowing
    // either owner and the autorelease pool to be dropped independently.
    let raw: *mut AnyObject =
        unsafe { msg_send![class, eventWithCGEvent: event.as_ptr().cast::<AppKitCGEvent>()] };
    unsafe { Retained::retain(raw) }.ok_or("nsevent_roundtrip_returned_nil")
}

fn appkit_snapshot(event: &AnyObject, phase: MousePhase) -> AppKitSnapshot {
    // SAFETY: The private callers use only live, retained NSEvents of the given
    // mouse phase. Property signatures/validity follow the public NSEvent.h.
    unsafe {
        let point: NSPoint = msg_send![event, locationInWindow];
        AppKitSnapshot {
            event_type: msg_send![event, type],
            window_number: msg_send![event, windowNumber],
            local_position: [point.x, point.y],
            flags: msg_send![event, modifierFlags],
            timestamp_seconds: msg_send![event, timestamp],
            button_number: msg_send![event, buttonNumber],
            click_count: msg_send![event, clickCount],
            pressure: msg_send![event, pressure],
            event_number: msg_send![event, eventNumber],
            delta: phase
                .has_delta()
                .then(|| [msg_send![event, deltaX], msg_send![event, deltaY]]),
        }
    }
}

fn quartz_snapshot(event: &CGEvent, phase: MousePhase) -> QuartzSnapshot {
    let point = event.location();
    QuartzSnapshot {
        event_type: event.get_type() as u32,
        global_position: [point.x, point.y],
        flags: event.get_flags().bits(),
        // SAFETY: The RAII owner keeps the CGEventRef live for this public read.
        timestamp_nanoseconds: unsafe { CGEventGetTimestamp(event.as_ptr().cast()) },
        button_number: event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER),
        click_count: event.get_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE),
        pressure: event.get_double_value_field(EventField::MOUSE_EVENT_PRESSURE),
        event_number: event.get_integer_value_field(EventField::MOUSE_EVENT_NUMBER),
        delta: phase.has_delta().then(|| {
            [
                event.get_integer_value_field(EventField::MOUSE_EVENT_DELTA_X),
                event.get_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y),
            ]
        }),
    }
}

fn probe_mouse(fixture: MouseFixture) -> Result<MouseBridgeSnapshot, &'static str> {
    autoreleasepool(|_| {
        let mut elapsed = [0.0; 6];
        let mut at = Instant::now();
        let appkit = create_mouse(fixture)?;
        elapsed[0] = at.elapsed().as_secs_f64() * 1000.0;
        at = Instant::now();
        let constructed = appkit_snapshot(&appkit, fixture.phase);
        elapsed[1] = at.elapsed().as_secs_f64() * 1000.0;
        at = Instant::now();
        let quartz = bridge_to_quartz(&appkit)?;
        elapsed[2] = at.elapsed().as_secs_f64() * 1000.0;
        at = Instant::now();
        let quartz_snapshot = quartz_snapshot(&quartz, fixture.phase);
        elapsed[3] = at.elapsed().as_secs_f64() * 1000.0;
        at = Instant::now();
        let roundtrip = bridge_to_appkit(&quartz)?;
        elapsed[4] = at.elapsed().as_secs_f64() * 1000.0;
        at = Instant::now();
        let roundtrip = appkit_snapshot(&roundtrip, fixture.phase);
        elapsed[5] = at.elapsed().as_secs_f64() * 1000.0;
        Ok(MouseBridgeSnapshot {
            input: fixture,
            constructed,
            quartz: quartz_snapshot,
            roundtrip,
            stage_milliseconds: elapsed,
        })
    })
}

#[derive(Debug, Serialize)]
struct AppKitMoveSnapshot {
    event_type: usize,
    window_number: isize,
    window_position: [f64; 2],
    flags: usize,
    timestamp_seconds: f64,
    subtype: i16,
    delta: [f64; 2],
}

#[derive(Debug, Serialize)]
struct QuartzMoveSnapshot {
    event_type: u32,
    cg_location: [f64; 2],
    flags: u64,
    timestamp_nanoseconds: u64,
    subtype: i64,
    delta: [i64; 2],
    source_state_id: i64,
    source_unix_pid: i64,
    target_unix_pid: i64,
}

fn appkit_move_snapshot(event: &AnyObject) -> AppKitMoveSnapshot {
    // NSEvent.h permits these getters for MouseMoved. Deliberately do not reuse
    // appkit_snapshot: eventNumber/clickCount/buttonNumber/pressure are not valid
    // MouseMoved getters. Reading a subtype does not endorse an undocumented one.
    unsafe {
        let point: NSPoint = msg_send![event, locationInWindow];
        AppKitMoveSnapshot {
            event_type: msg_send![event, type],
            window_number: msg_send![event, windowNumber],
            window_position: [point.x, point.y],
            flags: msg_send![event, modifierFlags],
            timestamp_seconds: msg_send![event, timestamp],
            subtype: msg_send![event, subtype],
            delta: [msg_send![event, deltaX], msg_send![event, deltaY]],
        }
    }
}

fn quartz_move_snapshot(event: &CGEvent) -> QuartzMoveSnapshot {
    // CGEventTypes.h's public kCGMouseEventSubtype = 7 is not exposed by the
    // pinned Rust wrapper. No private window-addressing fields are read or set.
    const MOUSE_EVENT_SUBTYPE: u32 = 7;
    let point = event.location();
    QuartzMoveSnapshot {
        event_type: event.get_type() as u32,
        cg_location: [point.x, point.y],
        flags: event.get_flags().bits(),
        timestamp_nanoseconds: unsafe { CGEventGetTimestamp(event.as_ptr().cast()) },
        subtype: event.get_integer_value_field(MOUSE_EVENT_SUBTYPE),
        delta: [
            event.get_integer_value_field(EventField::MOUSE_EVENT_DELTA_X),
            event.get_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y),
        ],
        source_state_id: event.get_integer_value_field(EventField::EVENT_SOURCE_STATE_ID),
        source_unix_pid: event.get_integer_value_field(EventField::EVENT_SOURCE_UNIX_PROCESS_ID),
        target_unix_pid: event.get_integer_value_field(EventField::EVENT_TARGET_UNIX_PROCESS_ID),
    }
}

/// Compare a caller's actual production MouseMoved constructor with a public
/// NSEvent factory/CG bridge. Compiled into tests only (lib.rs gates this module).
/// No event is mutated/posted and no native window is resolved or created.
pub(crate) fn probe_move_constructor(
    current: &CGEvent,
    synthetic_window_number: isize,
) -> Result<serde_json::Value, &'static str> {
    if current.get_type() as u32 != 5 || synthetic_window_number < 0 {
        return Err("invalid_move_constructor_fixture");
    }
    let current_quartz = quartz_move_snapshot(current);
    if !current_quartz.cg_location.into_iter().all(f64::is_finite) {
        return Err("invalid_move_constructor_fixture");
    }
    autoreleasepool(|_| {
        let original_ns = bridge_to_appkit(current)?;
        let current_appkit = appkit_move_snapshot(&original_ns);
        // Use the same numeric point/flags/time without pretending to know a
        // local/global frame mapping. The factory determines its own event source
        // and bridge fields: observe differences, rather than forcing equality.
        let class = event_class()?;
        let timestamp = current_quartz.timestamp_nanoseconds as f64 / 1_000_000_000.0;
        let raw: *mut AnyObject = unsafe {
            msg_send![class,
                mouseEventWithType:5usize,
                location:NSPoint::new(current_quartz.cg_location[0], current_quartz.cg_location[1]),
                modifierFlags:current_quartz.flags as usize,
                timestamp:timestamp, windowNumber:synthetic_window_number,
                context:std::ptr::null_mut::<AnyObject>(),
                eventNumber:0isize, clickCount:0isize, pressure:0.0f32]
        };
        let appkit = unsafe { Retained::retain(raw) }.ok_or("move_factory_returned_nil")?;
        let constructed = appkit_move_snapshot(&appkit);
        let bridged = bridge_to_quartz(&appkit)?;
        let factory_quartz = quartz_move_snapshot(&bridged);
        let roundtrip = bridge_to_appkit(&bridged)?;
        let roundtrip = appkit_move_snapshot(&roundtrip);
        let current_value = serde_json::to_value(&current_quartz)
            .map_err(|_| "move_snapshot_serialization_failed")?;
        let factory_value = serde_json::to_value(&factory_quartz)
            .map_err(|_| "move_snapshot_serialization_failed")?;
        let mut equal_fields = Vec::new();
        let mut differing_fields = Vec::new();
        for field in [
            "event_type",
            "cg_location",
            "flags",
            "timestamp_nanoseconds",
            "subtype",
            "delta",
            "source_state_id",
            "source_unix_pid",
            "target_unix_pid",
        ] {
            if current_value[field] == factory_value[field] {
                equal_fields.push(field);
            } else {
                differing_fields.push(field);
            }
        }
        Ok(serde_json::json!({
            "synthetic_window_number":synthetic_window_number,
            "factory_timestamp_seconds":timestamp,
            "current":{"quartz":current_value,"appkit":current_appkit},
            "public_nsevent":{"constructed":constructed,"quartz":factory_value,
                "roundtrip":roundtrip},
            "comparison":{"equal_public_fields":equal_fields,
                "differing_public_fields":differing_fields}
        }))
    })
}

// Legacy tracking metadata belongs to these two legal factory types only.
// This static Rust cookie remains alive for every retained event; never dereference
// an event's opaque userData, export its address, or interpret it as an ObjC object.
static TRACKING_MEMORY_COOKIE: u8 = 0x73;

fn tracking_memory_snapshot(event: &AnyObject) -> serde_json::Value {
    let kind: usize = unsafe { msg_send![event, type] };
    if !matches!(kind, 8 | 9) {
        return serde_json::json!({
            "status":"observed_wrong_kind", "event_type":kind,
            "window_number":null,"window_position":null,"tracking_number":null,
            "user_data_matches_static_cookie":null,"tracking_getters_read":false
        });
    }
    // Public NSEvent.h permits these scalar/pointer getters for Entered/Exited.
    // Deliberately do not query trackingArea, event.window, pressure or button data.
    let (window, point, tracking, data): (isize, NSPoint, isize, *mut c_void) = unsafe {
        (
            msg_send![event, windowNumber],
            msg_send![event, locationInWindow],
            msg_send![event, trackingNumber],
            msg_send![event, userData],
        )
    };
    serde_json::json!({
        "status":"observed", "event_type":kind, "window_number":window,
        "window_position":[point.x,point.y],"tracking_number":tracking,
        "user_data_matches_static_cookie":std::ptr::eq(
            data.cast_const(), std::ptr::addr_of!(TRACKING_MEMORY_COOKIE).cast::<c_void>()),
        "tracking_getters_read":true
    })
}

/// Test-only detached Entered/Exited construction and same-object bridge readback.
/// No window, event queue, delivery, tracking owner or application effect is created.
/// Nullable bridges are observations, not assertions that background hover is impossible.
pub(crate) fn probe_tracking_constructor_roundtrip(
    kind: usize,
) -> Result<serde_json::Value, &'static str> {
    if !matches!(kind, 8 | 9) {
        return Err("invalid_tracking_memory_kind");
    }
    autoreleasepool(|_| {
        let mut result = serde_json::json!({
            "probe":"tracking_constructor_roundtrip_memory_only",
            "fixture":{"event_type":kind,"window_number":0,
                "point":[10.25,20.75],"tracking_number":73,"event_number":37},
            "constructed":{"status":"not_reached"},
            "quartz":{"status":"not_reached"},
            "roundtrip":{"status":"not_reached"},
            "selected_metadata_equal":null,
            "scope":"same-event bridge only; serialized/cross-process preservation unknown",
            "tracking_area_read":false,"pointer_address_exported":false,
            "user_data_dereferenced":false,"tracking_owner_established":false,
            "receiver_effect_verified":false,"window_created":false,
            "activation_calls":0,"post_calls":0,"warp_calls":0,"capture_calls":0
        });
        let class = match event_class() {
            Ok(class) => class,
            Err(reason) => {
                result["constructed"] = serde_json::json!({"status":"unavailable","reason":reason});
                return Ok(result);
            }
        };
        if class.class_method(sel!(enterExitEventWithType:location:modifierFlags:timestamp:windowNumber:context:eventNumber:trackingNumber:userData:)).is_none() {
            result["constructed"] = serde_json::json!({"status":"unavailable","reason":"tracking_factory_selector_unavailable"});
            return Ok(result);
        }
        // Exact public NSEvent.h ABI, kind 8/9, window 0 and nil deprecated context.
        // userData is an opaque pointer to this process's static cookie only.
        let raw: *mut AnyObject = unsafe {
            msg_send![class,
                enterExitEventWithType:kind,
                location:NSPoint::new(10.25,20.75),
                modifierFlags:0usize,
                timestamp:1234.125f64,
                windowNumber:0isize,
                context:std::ptr::null_mut::<AnyObject>(),
                eventNumber:37isize,
                trackingNumber:73isize,
                userData:std::ptr::addr_of!(TRACKING_MEMORY_COOKIE).cast::<c_void>().cast_mut()]
        };
        let Some(constructed) = (unsafe { Retained::retain(raw) }) else {
            result["constructed"] =
                serde_json::json!({"status":"nil","reason":"tracking_factory_returned_nil"});
            return Ok(result);
        };
        let before = tracking_memory_snapshot(&constructed);
        result["constructed"] = before.clone();
        let cg = match bridge_to_quartz(&constructed) {
            Ok(cg) => cg,
            Err(reason) => {
                result["quartz"] = serde_json::json!({"status":"unavailable","reason":reason});
                return Ok(result);
            }
        };
        // The public CG enum has no tracking kinds 8/9. Never use get_type()
        // or convert this opaque event's discriminator to a Rust CGEventType.
        let raw_kind = unsafe { CGEventGetType(cg.as_ptr().cast()) };
        result["quartz"] = serde_json::json!({"status":"observed","raw_event_type_u32":raw_kind});
        let roundtrip = match bridge_to_appkit(&cg) {
            Ok(event) => event,
            Err(reason) => {
                result["roundtrip"] = serde_json::json!({"status":"unavailable","reason":reason});
                return Ok(result);
            }
        };
        let observed = tracking_memory_snapshot(&roundtrip);
        result["selected_metadata_equal"] = serde_json::json!(before == observed);
        result["roundtrip"] = observed;
        Ok(result)
    })
}

fn tracking_serialization_quartz_snapshot(event: &CGEvent) -> serde_json::Value {
    let point = event.location();
    // The owned CGEvent stays alive. Never decode opaque tracking kinds using
    // core-graphics' Rust CGEventType enum, which has no variants for 8 or 9.
    serde_json::json!({
        "status":"observed",
        "raw_event_type_u32":unsafe { CGEventGetType(event.as_ptr().cast()) },
        "location":[point.x,point.y],"flags":event.get_flags().bits(),
        "timestamp_nanoseconds":unsafe { CGEventGetTimestamp(event.as_ptr().cast()) }
    })
}

/// Test-only public flatten/rebuild discriminator, entirely within this process.
/// CFData may contain opaque pointer bits: never read/export its bytes or move it
/// to another process. Cookie equality is diagnostic, not a usable tracking owner.
/// Nil or changed metadata is recorded literally; no tracking/input success implied.
pub(crate) fn probe_tracking_serialization_roundtrip(
    kind: usize,
) -> Result<serde_json::Value, &'static str> {
    if !matches!(kind, 8 | 9) {
        return Err("invalid_tracking_serialization_kind");
    }
    autoreleasepool(|_| {
        let mut result = serde_json::json!({
            "probe":"tracking_serialization_roundtrip_memory_only",
            "fixture":{"event_type":kind,"window_number":0,
                "point":[10.25,20.75],"tracking_number":73,"event_number":37},
            "constructed":{"status":"not_reached"},
            "constructed_fixture_fields_match":null,
            "quartz_original":{"status":"not_reached"},
            "same_event_roundtrip":{"status":"not_reached"},
            "same_event_selected_metadata_equal":null,
            "serialization":{"status":"not_reached"},
            "quartz_rebuilt":{"status":"not_reached"},
            "rebuilt_event_distinct_from_original":null,
            "quartz_selected_metadata_equal":null,
            "serialized_roundtrip":{"status":"not_reached"},
            "serialized_selected_metadata_equal":null,
            "scope":"public serialization/rebuild in this process only; cross-process ownership and delivery unknown",
            "data_bytes_exported":false,"data_address_exported":false,
            "pointer_address_exported":false,"user_data_dereferenced":false,
            "foreign_pointer_sent":false,"tracking_area_read":false,
            "tracking_owner_established":false,"receiver_effect_verified":false,
            "window_created":false,"activation_calls":0,"post_calls":0,
            "warp_calls":0,"capture_calls":0
        });
        let class = match event_class() {
            Ok(class) => class,
            Err(reason) => {
                result["constructed"] = serde_json::json!({"status":"unavailable","reason":reason});
                return Ok(result);
            }
        };
        if class.class_method(sel!(enterExitEventWithType:location:modifierFlags:timestamp:windowNumber:context:eventNumber:trackingNumber:userData:)).is_none() {
            result["constructed"] = serde_json::json!({"status":"unavailable","reason":"tracking_factory_selector_unavailable"});
            return Ok(result);
        }
        // Same legal public factory fixture as Source5, never a real window or
        // foreign address. Retain its nullable +0 return before the pool drains.
        let raw: *mut AnyObject = unsafe {
            msg_send![class,
                enterExitEventWithType:kind,
                location:NSPoint::new(10.25,20.75),
                modifierFlags:0usize,
                timestamp:1234.125f64,
                windowNumber:0isize,
                context:std::ptr::null_mut::<AnyObject>(),
                eventNumber:37isize,
                trackingNumber:73isize,
                userData:std::ptr::addr_of!(TRACKING_MEMORY_COOKIE).cast::<c_void>().cast_mut()]
        };
        let Some(constructed) = (unsafe { Retained::retain(raw) }) else {
            result["constructed"] =
                serde_json::json!({"status":"nil","reason":"tracking_factory_returned_nil"});
            return Ok(result);
        };
        let before = tracking_memory_snapshot(&constructed);
        result["constructed"] = before.clone();
        let constructed_matches = before["status"] == "observed"
            && before["event_type"] == kind
            && before["window_number"] == 0
            && before["window_position"] == serde_json::json!([10.25, 20.75])
            && before["tracking_number"] == 73
            && before["tracking_getters_read"] == true
            && before["user_data_matches_static_cookie"] == true;
        result["constructed_fixture_fields_match"] = serde_json::json!(constructed_matches);
        if !constructed_matches {
            result["serialization"] = serde_json::json!({"status":"not_attempted","reason":"constructed_fixture_fields_not_observed"});
            return Ok(result);
        }
        let original = match bridge_to_quartz(&constructed) {
            Ok(event) => event,
            Err(reason) => {
                result["quartz_original"] =
                    serde_json::json!({"status":"unavailable","reason":reason});
                return Ok(result);
            }
        };
        let original_snapshot = tracking_serialization_quartz_snapshot(&original);
        result["quartz_original"] = original_snapshot.clone();
        match bridge_to_appkit(&original) {
            Ok(event) => {
                let sample = tracking_memory_snapshot(&event);
                result["same_event_selected_metadata_equal"] = serde_json::json!(before == sample);
                result["same_event_roundtrip"] = sample;
            }
            Err(reason) => {
                result["same_event_roundtrip"] =
                    serde_json::json!({"status":"unavailable","reason":reason});
            }
        }
        // CGEvent.h: nullable Create-rule CFData +1. Never call a non-null Rust
        // wrapper until NULL is checked; Cf releases this one ownership at exit.
        let data_raw = unsafe { CGEventCreateData(std::ptr::null(), original.as_ptr().cast()) };
        if data_raw.is_null() {
            result["serialization"] =
                serde_json::json!({"status":"nil","reason":"cgevent_create_data_returned_null"});
            return Ok(result);
        }
        let data = ffi::Cf(data_raw);
        result["serialization"] = serde_json::json!({"status":"created","retained_in_memory_only":true,"bytes_exported":false});
        // Nullable Create-rule CGEvent +1. The original remains alive throughout,
        // so pointer inequality cannot be an allocator reuse after its release.
        let rebuilt_raw = unsafe { CGEventCreateFromData(std::ptr::null(), data.0) };
        if rebuilt_raw.is_null() {
            result["quartz_rebuilt"] = serde_json::json!({"status":"nil","reason":"cgevent_create_from_data_returned_null"});
            return Ok(result);
        }
        let rebuilt = unsafe { CGEvent::from_ptr(rebuilt_raw.cast_mut().cast()) };
        result["rebuilt_event_distinct_from_original"] =
            serde_json::json!(!std::ptr::eq(original.as_ptr(), rebuilt.as_ptr()));
        let rebuilt_snapshot = tracking_serialization_quartz_snapshot(&rebuilt);
        result["quartz_selected_metadata_equal"] =
            serde_json::json!(original_snapshot == rebuilt_snapshot);
        result["quartz_rebuilt"] = rebuilt_snapshot;
        match bridge_to_appkit(&rebuilt) {
            Ok(event) => {
                // First read actual NSEvent type; tracking getters only for 8/9.
                let sample = tracking_memory_snapshot(&event);
                result["serialized_selected_metadata_equal"] = serde_json::json!(before == sample);
                result["serialized_roundtrip"] = sample;
            }
            Err(reason) => {
                result["serialized_roundtrip"] =
                    serde_json::json!({"status":"unavailable","reason":reason});
            }
        }
        Ok(result)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::geometry::CGPoint;
    use serde_json::json;

    fn fixture(phase: MousePhase, window_number: isize) -> MouseFixture {
        let (local_position, timestamp_seconds, pressure) = match phase {
            MousePhase::Down => ([12.25, 34.5], 1234.125, 0.5),
            MousePhase::Dragged => ([29.25, 25.5], 1234.25, 0.625),
            MousePhase::Up => ([29.25, 25.5], 1234.375, 0.0),
        };
        MouseFixture {
            phase,
            local_position,
            window_number,
            flags: (CGEventFlags::CGEventFlagShift | CGEventFlags::CGEventFlagAlternate).bits()
                as usize,
            timestamp_seconds,
            event_number: 73,
            click_count: 2,
            pressure,
        }
    }

    #[test]
    fn mouse_bridge_snapshot() {
        let mut snapshots = Vec::new();
        // window 0 and a synthetic nonzero value do not resolve/create windows.
        for window_number in [0, 1_048_573] {
            for phase in [MousePhase::Down, MousePhase::Dragged, MousePhase::Up] {
                let input = fixture(phase, window_number);
                let snapshot = probe_mouse(input).expect("public mouse construction/bridge");
                assert_eq!(snapshot.constructed.event_type, phase.event_type());
                assert_eq!(snapshot.constructed.window_number, input.window_number);
                assert_eq!(snapshot.constructed.local_position, input.local_position);
                assert_eq!(snapshot.constructed.flags, input.flags);
                assert_eq!(
                    snapshot.constructed.timestamp_seconds,
                    input.timestamp_seconds
                );
                assert_eq!(snapshot.constructed.button_number, 0);
                assert_eq!(snapshot.constructed.event_number, input.event_number);
                assert_eq!(snapshot.constructed.click_count, input.click_count);
                assert_eq!(snapshot.constructed.pressure, input.pressure);
                assert_eq!(snapshot.quartz.event_type as usize, phase.event_type());
                assert_eq!(snapshot.quartz.flags, input.flags as u64);
                assert_eq!(snapshot.quartz.button_number, 0);
                assert_eq!(snapshot.quartz.click_count, input.click_count as i64);
                assert_eq!(snapshot.quartz.event_number, input.event_number as i64);
                assert_eq!(
                    snapshot.quartz.timestamp_nanoseconds,
                    (input.timestamp_seconds * 1_000_000_000.0) as u64
                );
                assert_eq!(snapshot.roundtrip.event_type, phase.event_type());
                assert_eq!(snapshot.roundtrip.window_number, input.window_number);
                assert_eq!(snapshot.roundtrip.local_position, input.local_position);
                assert_eq!(snapshot.roundtrip.flags, input.flags);
                assert_eq!(
                    snapshot.roundtrip.timestamp_seconds,
                    input.timestamp_seconds
                );
                assert_eq!(snapshot.roundtrip.button_number, 0);
                assert_eq!(snapshot.roundtrip.click_count, input.click_count);
                assert_eq!(snapshot.roundtrip.event_number, input.event_number);
                assert_eq!(snapshot.roundtrip.pressure, snapshot.quartz.pressure as f32);
                // Deliberately do not assert that unspecified frame conversion,
                // real window identity or input pressure precision survives it.
                snapshots.push(snapshot);
            }
        }
        println!(
            "{}",
            json!({
                "probe":"nsevent_mouse_constructor_cg_roundtrip",
                "scope":"construction_only_no_gui_no_input_delivery",
                "local_global_frame_mapping":"unproven_without_real_window_geometry",
                "cg_public_window_number":"no_public_accessor_used",
                "down_up_appkit_delta":"not_read_property_not_valid_for_these_types",
                "stage_order":["create_appkit", "read_appkit", "bridge_to_quartz",
                    "read_quartz", "bridge_to_appkit", "read_roundtrip"],
                "snapshots":snapshots
            })
        );
    }

    #[test]
    fn public_cg_field_updates_roundtrip_without_private_window_fields() {
        autoreleasepool(|_| {
            let input = fixture(MousePhase::Dragged, 1_048_573);
            let event = create_mouse(input).unwrap();
            let cg = bridge_to_quartz(&event).unwrap();
            cg.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, 17);
            cg.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y, -9);
            cg.set_integer_value_field(EventField::MOUSE_EVENT_NUMBER, 777);
            cg.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 3);
            cg.set_double_value_field(EventField::MOUSE_EVENT_PRESSURE, 0.75);
            cg.set_flags(CGEventFlags::CGEventFlagControl);
            // SAFETY: Public setters operate on the owned event only, no post.
            unsafe {
                ffi::CGEventSetLocation(cg.as_ptr().cast(), CGPoint::new(512.5, -128.25));
                CGEventSetTimestamp(cg.as_ptr().cast(), 1_234_500_000_000);
            }
            let ns = bridge_to_appkit(&cg).unwrap();
            let quartz = quartz_snapshot(&cg, MousePhase::Dragged);
            let roundtrip = appkit_snapshot(&ns, MousePhase::Dragged);
            println!(
                "{}",
                json!({"probe":"public_cg_field_updates", "quartz":quartz,
                    "roundtrip":roundtrip, "requested_pressure":0.75,
                    "window_geometry_resolved":false, "input_delivery_attempted":false})
            );
            assert_eq!(quartz.delta, Some([17, -9]));
            assert_eq!(quartz.event_number, 777);
            assert_eq!(quartz.click_count, 3);
            // CG's NSEvent-backed event can quantize even an explicit public
            // pressure setter. Record the literal value; do not require a
            // lossless double representation or hard-code one OS's 8-bit result.
            assert!((quartz.pressure - 0.75).abs() <= 1.0 / 255.0);
            assert_eq!(quartz.global_position, [512.5, -128.25]);
            assert_eq!(quartz.timestamp_nanoseconds, 1_234_500_000_000);
            assert_eq!(quartz.flags, CGEventFlags::CGEventFlagControl.bits());
            assert_eq!(roundtrip.event_type, 6);
            assert_eq!(roundtrip.window_number, input.window_number);
            assert_eq!(roundtrip.delta, Some([17.0, -9.0]));
            assert_eq!(roundtrip.event_number, 777);
            assert_eq!(roundtrip.click_count, 3);
            assert_eq!(roundtrip.button_number, 0);
            assert_eq!(roundtrip.flags as u64, quartz.flags);
            assert_eq!(roundtrip.timestamp_seconds, 1234.5);
            assert_eq!(roundtrip.pressure, quartz.pressure as f32);
        });
    }

    #[test]
    fn retained_objects_outlive_creation_autorelease_pool() {
        let input = fixture(MousePhase::Dragged, 0);
        let (event, cg, roundtrip) = autoreleasepool(|_| {
            let event = create_mouse(input).unwrap();
            let cg = bridge_to_quartz(&event).unwrap();
            let roundtrip = bridge_to_appkit(&cg).unwrap();
            (event, cg, roundtrip)
        });
        assert_eq!(
            appkit_snapshot(&event, input.phase).local_position,
            input.local_position
        );
        assert_eq!(quartz_snapshot(&cg, input.phase).event_type, 6);
        let before = appkit_snapshot(&roundtrip, input.phase);
        drop(event);
        drop(cg);
        assert_eq!(appkit_snapshot(&roundtrip, input.phase), before);
    }

    #[test]
    fn invalid_fixtures_fail_before_native_construction() {
        let good = fixture(MousePhase::Down, 0);
        let invalid = [
            MouseFixture {
                local_position: [f64::NAN, 0.0],
                ..good
            },
            MouseFixture {
                timestamp_seconds: f64::INFINITY,
                ..good
            },
            MouseFixture {
                pressure: 1.5,
                ..good
            },
            MouseFixture {
                window_number: -1,
                ..good
            },
            MouseFixture {
                event_number: -1,
                ..good
            },
            MouseFixture {
                click_count: -1,
                ..good
            },
        ];
        for input in invalid {
            assert_eq!(input.validate(), Err("invalid_mouse_fixture"));
            assert_eq!(create_mouse(input).unwrap_err(), "invalid_mouse_fixture");
        }
    }
}
