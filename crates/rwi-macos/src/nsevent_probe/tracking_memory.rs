use super::*;

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
