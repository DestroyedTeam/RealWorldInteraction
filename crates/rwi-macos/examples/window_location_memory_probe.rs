//! Construction-only comparison. Never post, activate, create a window, query
//! the desktop or change field 58. Synthetic geometry/identity is fixture data.
#[cfg(target_os = "macos")]
mod probe {
    use core_graphics::{
        event::{CGEvent, CGEventFlags, CGEventType, CGMouseButton, EventField},
        event_source::{CGEventSource, CGEventSourceStateID},
        geometry::CGPoint,
    };
    use foreign_types::ForeignType;
    use objc2::{
        Encoding, RefEncode, msg_send,
        rc::{Retained, autoreleasepool},
        runtime::{AnyClass, AnyObject},
    };
    use objc2_foundation::NSPoint;
    use serde_json::{Value, json};
    use std::ffi::c_void;

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventGetTimestamp(event: *const c_void) -> u64;
        fn CGEventCreateCopy(event: *const c_void) -> *mut c_void;
    }
    #[repr(C)]
    struct AppKitCGEvent {
        _opaque: [u8; 0],
    }
    unsafe impl RefEncode for AppKitCGEvent {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CGEvent", &[]));
    }
    type SetWindowLocation = unsafe extern "C" fn(*const c_void, CGPoint);
    struct Library(*mut c_void);
    impl Drop for Library {
        fn drop(&mut self) {
            unsafe {
                libc::dlclose(self.0);
            }
        }
    }

    // Deliberately nonzero origins; no real display/window is looked up.
    const FRAME: [f64; 4] = [3036., 504., 660., 552.];
    const POINT: [f64; 2] = [3186., 804.];
    const SYNTHETIC_WINDOW: i64 = 91_234_567;
    fn local_point() -> [f64; 2] {
        [POINT[0] - FRAME[0], POINT[1] - FRAME[1]]
    }
    fn variants() -> [(&'static str, Option<[f64; 2]>); 3] {
        [
            ("no_setter", None),
            ("window_local", Some(local_point())),
            ("screen", Some(POINT)),
        ]
    }
    fn independent_copy(event: &CGEvent) -> Result<CGEvent, &'static str> {
        // core-graphics 0.24 CGEvent.clone() only CFRetains the same pointer.
        // This public create-rule API returns a separately owned event copy.
        let raw = unsafe { CGEventCreateCopy(event.as_ptr().cast()) };
        if raw.is_null() {
            return Err("event_copy_allocation_failed");
        }
        let copy = unsafe { CGEvent::from_ptr(raw.cast()) };
        if copy.as_ptr() == event.as_ptr() {
            return Err("event_copy_aliased_original");
        }
        Ok(copy)
    }
    fn protected(event: &CGEvent) -> Value {
        let integer = |field| event.get_integer_value_field(field);
        json!({
            "type":event.get_type() as u32, "flags":event.get_flags().bits(),
            "timestamp_ns":unsafe { CGEventGetTimestamp(event.as_ptr().cast()) },
            "button":integer(EventField::MOUSE_EVENT_BUTTON_NUMBER),
            "click_count":integer(EventField::MOUSE_EVENT_CLICK_STATE),
            "event_number":integer(EventField::MOUSE_EVENT_NUMBER),
            "pressure":event.get_double_value_field(EventField::MOUSE_EVENT_PRESSURE),
            "delta":[integer(EventField::MOUSE_EVENT_DELTA_X),integer(EventField::MOUSE_EVENT_DELTA_Y)],
            "subtype":integer(7u32), "source_state_id":integer(EventField::EVENT_SOURCE_STATE_ID),
            "source_pid":integer(EventField::EVENT_SOURCE_UNIX_PROCESS_ID),
            "target_pid":integer(EventField::EVENT_TARGET_UNIX_PROCESS_ID),
            "synthetic_window_number":integer(51u32),
            "window_under_pointer":integer(EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER),
            "window_can_handle":integer(EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER_THAT_CAN_HANDLE_THIS_EVENT)
        })
    }
    fn appkit(event: &CGEvent) -> Result<Value, &'static str> {
        let class = AnyClass::get(c"NSEvent").ok_or("nsevent_class_unavailable")?;
        // Same public CGEventRef opaque encoding used by src/nsevent_probe.rs.
        let raw: *mut AnyObject =
            unsafe { msg_send![class, eventWithCGEvent:event.as_ptr().cast::<AppKitCGEvent>()] };
        let event = unsafe { Retained::retain(raw) }.ok_or("nsevent_bridge_returned_nil")?;
        unsafe {
            let point: NSPoint = msg_send![&*event, locationInWindow];
            let event_type: usize = msg_send![&*event, type];
            let window_number: isize = msg_send![&*event, windowNumber];
            let flags: usize = msg_send![&*event, modifierFlags];
            let timestamp: f64 = msg_send![&*event, timestamp];
            let event_number: isize = msg_send![&*event, eventNumber];
            let click_count: isize = msg_send![&*event, clickCount];
            let button: isize = msg_send![&*event, buttonNumber];
            let pressure: f32 = msg_send![&*event, pressure];
            Ok(json!({
                "type":event_type,"window_number":window_number,
                "location_in_window":[point.x,point.y],
                "flags":flags,"timestamp_seconds":timestamp,
                "event_number":event_number,"click_count":click_count,
                "button":button,"pressure":pressure
            }))
        }
    }
    fn original_snapshot(event: &CGEvent) -> Result<Value, &'static str> {
        let point = event.location();
        Ok(json!({"protected_fields":protected(event),
            "quartz_location":[point.x,point.y],
            "appkit":autoreleasepool(|_| appkit(event))?}))
    }
    fn original_memory_unchanged(before: &Value, after: &Value) -> bool {
        before["protected_fields"] == after["protected_fields"]
            && before["quartz_location"] == after["quartz_location"]
    }
    fn mouse_pair() -> Result<Vec<(&'static str, CGEvent)>, &'static str> {
        let source = CGEventSource::new(CGEventSourceStateID::Private)
            .map_err(|_| "source_allocation_failed")?;
        let mut originals = Vec::new();
        for (phase, kind, pressure) in [
            ("down", CGEventType::LeftMouseDown, 1.0),
            ("up", CGEventType::LeftMouseUp, 0.0),
        ] {
            let original = CGEvent::new_mouse_event(
                source.clone(),
                kind,
                CGPoint::new(POINT[0], POINT[1]),
                CGMouseButton::Left,
            )
            .map_err(|_| "event_allocation_failed")?;
            original.set_flags(CGEventFlags::empty());
            for (field, value) in [
                (EventField::MOUSE_EVENT_BUTTON_NUMBER, 0),
                (EventField::MOUSE_EVENT_NUMBER, 73),
                (EventField::MOUSE_EVENT_CLICK_STATE, 1),
                (
                    EventField::EVENT_TARGET_UNIX_PROCESS_ID,
                    i64::from(std::process::id()),
                ),
                (51u32, SYNTHETIC_WINDOW),
                (
                    EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER,
                    SYNTHETIC_WINDOW,
                ),
                (
                    EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER_THAT_CAN_HANDLE_THIS_EVENT,
                    SYNTHETIC_WINDOW,
                ),
            ] {
                original.set_integer_value_field(field, value);
            }
            original.set_double_value_field(EventField::MOUSE_EVENT_PRESSURE, pressure);
            original.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, 0);
            original.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y, 0);
            originals.push((phase, original));
        }
        if originals[0].1.as_ptr() == originals[1].1.as_ptr() {
            return Err("original_down_up_aliased");
        }
        Ok(originals)
    }
    pub(super) fn inspect() -> Result<Value, Box<dyn std::error::Error>> {
        let raw = unsafe {
            libc::dlopen(
                c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight".as_ptr(),
                libc::RTLD_LOCAL | libc::RTLD_NOW,
            )
        };
        let mut output = json!({
            "scope":"event_memory_only_no_native_window",
            "comparison_complete":false,"input_posted":false,"window_created":false,
            "activation_requested":false,"cursor_changed":false,"authentication_requested":false,
            "field58_written":false,"constructor_only":true,"Chrome_effect_attested":false,
            "fixture":{"frame":FRAME,"point":POINT,"local":local_point(),
                       "synthetic_window":SYNTHETIC_WINDOW,"native_identity_attested":false}
        });
        if raw.is_null() {
            output["setter_available"] = json!(false);
            return Ok(output);
        }
        let library = Library(raw);
        let pointer = unsafe { libc::dlsym(library.0, c"CGEventSetWindowLocation".as_ptr()) };
        if pointer.is_null() {
            output["setter_available"] = json!(false);
            return Ok(output);
        }
        // Pinned exact signature used by ffi.rs. No post/auth symbol is resolved.
        let setter: SetWindowLocation = unsafe { std::mem::transmute(pointer) };
        output["setter_available"] = json!(true);
        // Keep both originals and all six copies alive until every final check.
        // Distinct-address checks cannot be fooled by freed-pointer reuse.
        let originals = mouse_pair()?;
        let mut before_all = Vec::new();
        for (_, original) in &originals {
            before_all.push(original_snapshot(original)?);
        }
        let mut copies: Vec<CGEvent> = Vec::new();
        let mut phases = Vec::new();
        for (phase_index, (phase, original)) in originals.iter().enumerate() {
            let baseline = protected(original);
            let mut rows = Vec::new();
            for (name, point) in variants() {
                let original_before = original_snapshot(original)?;
                if !original_memory_unchanged(&before_all[phase_index], &original_before) {
                    return Err("original_mutated_before_variant".into());
                }
                let event = independent_copy(original)?;
                if originals
                    .iter()
                    .any(|(_, other)| other.as_ptr() == event.as_ptr())
                    || copies.iter().any(|other| other.as_ptr() == event.as_ptr())
                {
                    return Err("variant_event_memory_aliased".into());
                }
                let copy_index = copies.len();
                copies.push(event);
                let event = &copies[copy_index];
                let copy_before = original_snapshot(event)?;
                if !original_memory_unchanged(&original_before, &copy_before) {
                    return Err("copy_initial_point_or_fields_changed".into());
                }
                if let Some(point) = point {
                    unsafe {
                        setter(event.as_ptr().cast(), CGPoint::new(point[0], point[1]));
                    }
                }
                let after = protected(event);
                let changed_fields: Vec<_> = after
                    .as_object()
                    .unwrap()
                    .keys()
                    .filter(|field| after[*field] != baseline[*field])
                    .cloned()
                    .collect();
                let quartz = event.location();
                let observed = autoreleasepool(|_| appkit(event))?;
                let original_after = original_snapshot(original)?;
                if !original_memory_unchanged(&before_all[phase_index], &original_after) {
                    return Err("original_mutated_after_variant".into());
                }
                rows.push(json!({"variant":name,"setter_input":point,
                    "copy_ordinal":copy_index,"memory_distinct_from_originals_and_prior_copies":true,
                    "copy_before_setter":copy_before,
                    "copy_initial_quartz_and_fields_match_original":true,
                    "quartz_location":[quartz.x,quartz.y],"protected_fields":after,
                    "protected_fields_unchanged":changed_fields.is_empty(),
                    "changed_protected_fields":changed_fields,"appkit":observed,
                    "original_appkit_readback_unchanged":original_before["appkit"]==original_after["appkit"],
                    "original_before":original_before,"original_after":original_after,
                    "original_quartz_and_fields_unchanged":true}));
            }
            phases.push(json!({"phase":phase,"original_fields":baseline,"clones":rows}));
        }
        let mut after_all = Vec::new();
        for (index, (_, original)) in originals.iter().enumerate() {
            let after = original_snapshot(original)?;
            if !original_memory_unchanged(&before_all[index], &after) {
                return Err("original_mutated_after_all_phases".into());
            }
            after_all.push(after);
        }
        let pointers: std::collections::HashSet<_> = originals
            .iter()
            .map(|(_, event)| event.as_ptr())
            .chain(copies.iter().map(CGEvent::as_ptr))
            .collect();
        if pointers.len() != originals.len() + copies.len() {
            return Err("final_memory_identity_alias".into());
        }
        output["phases"] = json!(phases);
        output["originals_appkit_readbacks_equal_after_all_phases"] = json!(
            before_all
                .iter()
                .zip(&after_all)
                .all(|(before, after)| before["appkit"] == after["appkit"])
        );
        output["originals_before_all_phases"] = json!(before_all);
        output["originals_after_all_phases"] = json!(after_all);
        output["memory_identity"] = json!({"originals":2,"independent_copies":6,
            "all_eight_retained_event_pointers_distinct":true,
            "originals_quartz_and_protected_fields_unchanged_after_all_phases":true});
        output["comparison_complete"] = json!(true);
        Ok(output)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn fixed_geometry_is_nonzero_and_inside() {
            assert!(FRAME[0] != 0. && FRAME[1] != 0.);
            assert_eq!(local_point(), [150., 300.]);
            assert!(POINT[0] > FRAME[0] && POINT[0] < FRAME[0] + FRAME[2]);
            assert!(POINT[1] > FRAME[1] && POINT[1] < FRAME[1] + FRAME[3]);
        }
        #[test]
        fn exactly_three_independent_memory_variants() {
            assert_eq!(
                variants(),
                [
                    ("no_setter", None),
                    ("window_local", Some([150., 300.])),
                    ("screen", Some(POINT))
                ]
            );
        }
        #[test]
        fn public_event_copy_owns_independent_memory() {
            let originals = mouse_pair().unwrap();
            let original = &originals[0].1;
            let before = original_snapshot(original).unwrap();
            let copy = independent_copy(original).unwrap();
            assert_ne!(copy.as_ptr(), original.as_ptr());
            copy.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 2);
            assert_eq!(
                copy.get_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE),
                2
            );
            assert!(original_memory_unchanged(
                &before,
                &original_snapshot(original).unwrap()
            ));
        }
        #[test]
        fn all_variant_copy_identities_remain_distinct_while_live() {
            let originals = mouse_pair().unwrap();
            let mut copies = Vec::new();
            for (_, original) in &originals {
                for _ in variants() {
                    copies.push(independent_copy(original).unwrap());
                }
            }
            let pointers: std::collections::HashSet<_> = originals
                .iter()
                .map(|(_, event)| event.as_ptr())
                .chain(copies.iter().map(CGEvent::as_ptr))
                .collect();
            assert_eq!(pointers.len(), 8);
        }
        #[test]
        fn diagnostic_inspector_has_no_post_or_window_creation_path() {
            let source = include_str!("window_location_memory_probe.rs");
            let body = source
                .split("pub(super) fn inspect()")
                .nth(1)
                .unwrap()
                .split("#[cfg(test)]")
                .next()
                .unwrap();
            for forbidden in [
                "CGEventPost",
                "SLEventPost",
                "post_to_pid",
                "NSApplication",
                "NSWindow",
                "CGEventSetTimestamp",
            ] {
                assert!(!body.contains(forbidden));
            }
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().skip(1).collect::<Vec<_>>() != ["--inspect-events-only"] {
        return Err("Use only --inspect-events-only; no native operation performed.".into());
    }
    #[cfg(target_os = "macos")]
    println!("{}", probe::inspect()?);
    #[cfg(not(target_os = "macos"))]
    println!(
        "{}",
        serde_json::json!({"scope":"event_memory_only","supported_platform":false,
        "comparison_complete":false,"input_posted":false,"window_created":false})
    );
    Ok(())
}
