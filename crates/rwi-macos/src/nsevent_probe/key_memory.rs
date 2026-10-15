//! Detached key construction/serialization diagnostics, compiled into tests only.
use super::*;
use objc2_foundation::NSString;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventKeyboardGetUnicodeString(
        event: *const c_void,
        max_length: std::os::raw::c_ulong,
        actual_length: *mut std::os::raw::c_ulong,
        units: *mut u16,
    );
}

// Detached key diagnostics only. Never resolve a window, create NSApplication,
// post an event, activate input context, or infer a third-party receiver effect.
fn key_string_units(
    event: &AnyObject,
    ignoring_modifiers: bool,
) -> Result<Option<Vec<u16>>, &'static str> {
    let string: *const AnyObject = unsafe {
        if ignoring_modifiers {
            msg_send![event, charactersIgnoringModifiers]
        } else {
            msg_send![event, characters]
        }
    };
    if string.is_null() {
        return Ok(None);
    }
    // The retained key event keeps its immutable NSString alive. Read exact
    // UTF16 units, not a lossy UTF8 conversion of a possibly malformed payload.
    let length: usize = unsafe { msg_send![&*string, length] };
    if length > 64 {
        return Err("key_probe_string_budget");
    }
    let mut units = Vec::with_capacity(length);
    for index in 0..length {
        // This public NSString getter returns one UTF16 unit. The immutable
        // bounded string avoids both lossy conversion and an optional NSRange ABI.
        let unit: u16 = unsafe { msg_send![&*string, characterAtIndex:index] };
        units.push(unit);
    }
    Ok(Some(units))
}

fn appkit_key_snapshot(event: &AnyObject) -> Result<serde_json::Value, &'static str> {
    let kind: usize = unsafe { msg_send![event, type] };
    if !matches!(kind, 10 | 11) {
        return Err("key_probe_non_key_getters_refused");
    }
    let (window, key, flags, timestamp, repeat): (isize, u16, usize, f64, objc2::runtime::Bool) = unsafe {
        (
            msg_send![event, windowNumber],
            msg_send![event, keyCode],
            msg_send![event, modifierFlags],
            msg_send![event, timestamp],
            msg_send![event, isARepeat],
        )
    };
    Ok(serde_json::json!({"event_type":kind,"window_number":window,
        "key_code":key,"flags":flags,"timestamp_seconds":timestamp,"repeat":repeat.as_bool(),
        "characters_utf16":key_string_units(event,false)?,
        "characters_ignoring_modifiers_utf16":key_string_units(event,true)?}))
}

fn quartz_key_snapshot(event: &CGEvent) -> Result<serde_json::Value, &'static str> {
    let mut units = [0u16; 64];
    let mut actual = 0;
    unsafe {
        CGEventKeyboardGetUnicodeString(event.as_ptr().cast(), 64, &mut actual, units.as_mut_ptr());
    }
    if actual > 64 {
        return Err("key_probe_quartz_string_budget");
    }
    Ok(
        serde_json::json!({"event_type":unsafe{CGEventGetType(event.as_ptr().cast())},
        "unicode_utf16":&units[..actual as usize],
        "key_code":event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE),
        "flags":event.get_flags().bits(),
        "timestamp_nanoseconds":unsafe{CGEventGetTimestamp(event.as_ptr().cast())},
        "source_state_id":event.get_integer_value_field(EventField::EVENT_SOURCE_STATE_ID)}),
    )
}

/// Compare the caller's actual Quartz constructor with the public NSEvent key
/// factory and a genuinely reconstructed serialized event. No delivery occurs.
pub(crate) fn probe_key_constructor(
    current: &CGEvent,
    text: &str,
) -> Result<serde_json::Value, &'static str> {
    let kind = unsafe { CGEventGetType(current.as_ptr().cast()) };
    if !matches!(kind, 10 | 11) || text.chars().count() != 1 {
        return Err("invalid_key_constructor_fixture");
    }
    let expected: Vec<u16> = text.encode_utf16().collect();
    let current_quartz = quartz_key_snapshot(current)?;
    if current_quartz["unicode_utf16"] != serde_json::json!(expected) {
        return Err("key_probe_current_payload_differs");
    }
    autoreleasepool(|_| {
        let class = event_class()?;
        if class.class_method(sel!(keyEventWithType:location:modifierFlags:timestamp:windowNumber:context:characters:charactersIgnoringModifiers:isARepeat:keyCode:)).is_none() {
            return Err("key_factory_selector_unavailable");
        }
        let original_ns = bridge_to_appkit(current)?;
        let current_appkit = appkit_key_snapshot(&original_ns)?;
        let characters = NSString::from_str(text);
        let timestamp =
            unsafe { CGEventGetTimestamp(current.as_ptr().cast()) } as f64 / 1_000_000_000.;
        // Exact public NSEvent.h ABI; legal key kind, nil deprecated context,
        // local fixture window0 and explicit identical full scalar strings.
        let raw: *mut AnyObject = unsafe {
            msg_send![class, keyEventWithType:kind as usize,
                location:NSPoint::new(0.,0.), modifierFlags:0usize, timestamp:timestamp,
                windowNumber:0isize, context:std::ptr::null_mut::<AnyObject>(),
                characters:&*characters, charactersIgnoringModifiers:&*characters,
                isARepeat:objc2::runtime::Bool::NO, keyCode:0u16]
        };
        let event = unsafe { Retained::retain(raw) }.ok_or("key_factory_returned_nil")?;
        let constructed = appkit_key_snapshot(&event)?;
        let quartz = bridge_to_quartz(&event)?;
        let factory_quartz = quartz_key_snapshot(&quartz)?;
        let data =
            unsafe { ffi::Cf::owned(CGEventCreateData(std::ptr::null(), quartz.as_ptr().cast())) }
                .map_err(|_| "key_serialization_data_unavailable")?;
        let rebuilt = unsafe { CGEventCreateFromData(std::ptr::null(), data.0) };
        if rebuilt.is_null() {
            return Err("key_deserialization_returned_null");
        }
        let rebuilt = unsafe { CGEvent::from_ptr(rebuilt.cast_mut().cast()) };
        if rebuilt.as_ptr() == quartz.as_ptr() {
            return Err("key_deserialization_reused_memory");
        }
        let serialized_quartz = quartz_key_snapshot(&rebuilt)?;
        let restored = bridge_to_appkit(&rebuilt)?;
        let restored_appkit = appkit_key_snapshot(&restored)?;
        Ok(
            serde_json::json!({"probe":"key_constructor_and_serialization_memory_only",
            "expected_utf16":expected,"current":{"quartz":current_quartz,"appkit":current_appkit},
            "public_nsevent":{"constructed":constructed,"quartz":factory_quartz,
                "serialized_quartz":serialized_quartz,"serialized_appkit":restored_appkit},
            "factory_selected_metadata_survived_serialization":constructed==restored_appkit,
            "independent_deserialized_memory":true,"window_created":false,
            "post_calls":0,"activation_calls":0,"warp_calls":0,"capture_calls":0,
            "receiver_effect_verified":false,"scope":"same-process event memory; cross-process and inactive IME unknown"}),
        )
    })
}
