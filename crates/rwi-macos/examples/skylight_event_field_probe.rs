//! Explicit event-memory calibration. Never posts input, opens a window,
//! obtains authentication material, changes focus, or moves the system cursor.
//! Private signatures are pinned to the primary-source snapshot recorded by R62;
//! a resolved symbol is not a compatibility or application-effect guarantee.
#[cfg(target_os = "macos")]
fn inspect() -> Result<(), Box<dyn std::error::Error>> {
    use core_graphics::{
        event::{CGEvent, CGEventType, CGMouseButton},
        event_source::{CGEventSource, CGEventSourceStateID},
        geometry::CGPoint,
    };
    use foreign_types::ForeignType;
    use std::ffi::c_void;
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventGetTimestamp(event: *mut c_void) -> u64;
        fn CGEventSetTimestamp(event: *mut c_void, timestamp: u64);
    }
    struct Library(*mut c_void);
    impl Drop for Library {
        fn drop(&mut self) {
            unsafe {
                libc::dlclose(self.0);
            }
        }
    }
    let raw = unsafe {
        libc::dlopen(
            c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight".as_ptr(),
            libc::RTLD_LOCAL | libc::RTLD_NOW,
        )
    };
    if raw.is_null() {
        println!(
            "{}",
            serde_json::json!({"scope":"event_memory_only", "library_available":false,"input_posted":false,"window_created":false,"cursor_changed":false})
        );
        return Ok(());
    }
    let library = Library(raw);
    let setter = unsafe { libc::dlsym(library.0, c"SLEventSetIntegerValueField".as_ptr()) };
    let post_available = !unsafe { libc::dlsym(library.0, c"SLEventPostToPid".as_ptr()) }.is_null();
    let record_available =
        !unsafe { libc::dlsym(library.0, c"SLPSPostEventRecordTo".as_ptr()) }.is_null();
    let mut result = serde_json::json!({"scope":"event_memory_only", "library_available":true,"integer_setter_available":!setter.is_null(),"post_symbol_available_not_called":post_available,"record_symbol_available_not_called":record_available,"input_posted":false,"window_created":false,"cursor_changed":false,"authentication_requested":false});
    if !setter.is_null() {
        // SAFETY: exact pointer signature from the pinned primary implementation;
        // the only call modifies a locally created and retained CGEvent. No PID,
        // native window, authentication handle, event queue or post entry is used.
        let set: unsafe extern "C" fn(*mut c_void, u32, i64) =
            unsafe { std::mem::transmute(setter) };
        let source = CGEventSource::new(CGEventSourceStateID::Private)
            .map_err(|_| "event source allocation failed")?;
        let event = CGEvent::new_mouse_event(
            source,
            CGEventType::MouseMoved,
            CGPoint::new(10., 20.),
            CGMouseButton::Left,
        )
        .map_err(|_| "event allocation failed")?;
        let ptr = event.as_ptr().cast();
        unsafe {
            CGEventSetTimestamp(ptr, 123_456_789);
        }
        let before = unsafe { CGEventGetTimestamp(ptr) };
        unsafe {
            set(ptr, 58, 987_654_321);
        }
        let after = unsafe { CGEventGetTimestamp(ptr) };
        result["field58"] = serde_json::json!({"written":987_654_321,"timestamp_before":before,"timestamp_after":after,"timestamp_changed":before!=after,"namespace":"SLEventSetIntegerValueField -> public CGEventGetTimestamp"});
    }
    println!("{result}");
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().skip(1).collect::<Vec<_>>() != ["--inspect-events-only"] {
        println!("Use --inspect-events-only; no native operation performed.");
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    inspect()?;
    #[cfg(not(target_os = "macos"))]
    println!("Unsupported platform; no native operation performed.");
    Ok(())
}
