//! Background delivery: postToPid input carrying exact window addressing.
//! No system cursor movement or explicit system-frontmost request. The target
//! may change its active/key/order state when consuming an event. Keyboard
//! compatibility may additionally send an AppKit activation notification;
//! pointer sequences never do. State preservation needs independent readback.
//! Window-addressing field numbers follow
//! the MIT-licensed kwwk-computer-use-core / trycua/cua reference implementations.
use crate::ffi;
use crate::geometry::Rect;
use core_graphics::{
    event::{CGEvent, CGEventFlags, EventField},
    geometry::CGPoint,
};
use foreign_types::ForeignType;
use objc2::{
    msg_send,
    runtime::{AnyClass, AnyObject},
};
use objc2_foundation::NSPoint;
pub(crate) mod appkit_move;
pub(crate) mod appkit_tracking;
pub(crate) mod key_window_record;
pub(crate) mod skylight;

/// AppKit-defined event, subtype 1 = NSApplicationActivatedEventType. Posted
/// only to the target process so the application treats itself as active
/// without the window server changing the user's frontmost application.
fn application_activated_event(window: isize) -> Option<CGEvent> {
    let class = AnyClass::get(c"NSEvent")?;
    let event: *mut AnyObject = unsafe {
        msg_send![class, otherEventWithType: 13usize, location: NSPoint { x: 0.0, y: 0.0 },
            modifierFlags: 0usize, timestamp: 0.0f64, windowNumber: window,
            context: std::ptr::null_mut::<AnyObject>(), subtype: 1i16, data1: 0isize, data2: 0isize]
    };
    if event.is_null() {
        return None;
    }
    let cg: *mut AnyObject = unsafe { msg_send![event, CGEvent] };
    if cg.is_null() {
        return None;
    }
    Some(unsafe { CGEvent::from_ptr(ffi::CFRetain(cg.cast()).cast_mut().cast()) })
}

pub(crate) fn activate_background_window(pid: u32, window: u32) -> bool {
    let Some(event) = application_activated_event(window as isize) else {
        return false;
    };
    event.set_integer_value_field(EventField::EVENT_TARGET_UNIX_PROCESS_ID, i64::from(pid));
    event.set_integer_value_field(TARGET_WINDOW_NUMBER_FIELD, i64::from(window));
    event.post_to_pid(pid as libc::pid_t);
    true
}
pub(crate) const TARGET_WINDOW_NUMBER_FIELD: u32 = 51;
#[derive(Clone, Copy)]
pub(crate) struct Delivery {
    pub pid: u32,
    pub window: u32,
    pub frame: Rect,
}
pub(crate) fn pointer_delivery_available() -> bool {
    ffi::window_location_spi_available()
}
pub(crate) fn window_local(desktop_point: [f64; 2], frame: Rect) -> [f64; 2] {
    [desktop_point[0] - frame.x, desktop_point[1] - frame.y]
}
pub(crate) fn deliver_pointer(event: &CGEvent, delivery: &Delivery, desktop_point: [f64; 2]) {
    stamp(event, delivery, true);
    let local = window_local(desktop_point, delivery.frame);
    let _ = ffi::set_window_location(event.as_ptr().cast(), CGPoint::new(local[0], local[1]));
    event.post_to_pid(delivery.pid as libc::pid_t);
}
pub(crate) fn deliver_key(event: &CGEvent, delivery: &Delivery) {
    stamp(event, delivery, false);
    event.post_to_pid(delivery.pid as libc::pid_t);
}
fn stamp(event: &CGEvent, delivery: &Delivery, pointer: bool) {
    event.set_integer_value_field(
        EventField::EVENT_TARGET_UNIX_PROCESS_ID,
        i64::from(delivery.pid),
    );
    event.set_integer_value_field(TARGET_WINDOW_NUMBER_FIELD, i64::from(delivery.window));
    // Field 58 aliases CGEventGetTimestamp on this native runtime. It is not a
    // routing flag: writing 1 corrupted caller timestamps. Keep the event clock
    // intact; exact PID/window fields and window-local coordinates carry routing.
    if pointer {
        // Pointer intents currently have no modifier argument. Never inherit
        // a user's Alt/Command/Shift held in another application.
        event.set_flags(CGEventFlags::empty());
        event.set_integer_value_field(
            EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER,
            i64::from(delivery.window),
        );
        event.set_integer_value_field(
            EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER_THAT_CAN_HANDLE_THIS_EVENT,
            i64::from(delivery.window),
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::event::CGEventType;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    fn source() -> CGEventSource {
        CGEventSource::new(CGEventSourceStateID::Private).unwrap()
    }
    fn event() -> CGEvent {
        CGEvent::new_keyboard_event(source(), 0, true).unwrap()
    }
    fn mouse_event() -> CGEvent {
        CGEvent::new_mouse_event(
            source(),
            CGEventType::LeftMouseDown,
            CGPoint::new(100.0, 100.0),
            core_graphics::event::CGMouseButton::Left,
        )
        .unwrap()
    }
    #[test]
    fn window_local_point_is_top_down_window_relative() {
        let frame = Rect {
            x: 584.0,
            y: 176.0,
            w: 880.0,
            h: 448.0,
        };
        assert_eq!(window_local([584.0, 176.0], frame), [0.0, 0.0]);
        assert_eq!(window_local([1024.0, 400.0], frame), [440.0, 224.0]);
    }
    #[test]
    fn stamped_events_carry_pid_window_and_routing_fields() {
        let delivery = Delivery {
            pid: 514,
            window: 10656,
            frame: Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            },
        };
        let event = mouse_event();
        event.set_flags(CGEventFlags::CGEventFlagAlternate | CGEventFlags::CGEventFlagCommand);
        stamp(&event, &delivery, true);
        assert_eq!(event.get_flags(), CGEventFlags::empty());
        let read = |field: u32| event.get_integer_value_field(field);
        assert_eq!(read(TARGET_WINDOW_NUMBER_FIELD), 10656);
        assert_eq!(read(EventField::EVENT_TARGET_UNIX_PROCESS_ID), 514);
        assert_eq!(
            read(EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER),
            10656
        );
        assert_eq!(
            read(EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER_THAT_CAN_HANDLE_THIS_EVENT),
            10656
        );
    }
    #[test]
    fn key_events_stamp_pid_and_window_without_pointer_fields() {
        let delivery = Delivery {
            pid: 514,
            window: 10656,
            frame: Rect {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            },
        };
        let event = event();
        event.set_flags(CGEventFlags::CGEventFlagCommand);
        let original_timestamp = event.get_integer_value_field(58);
        stamp(&event, &delivery, false);
        assert_eq!(event.get_flags(), CGEventFlags::CGEventFlagCommand);
        assert_eq!(
            event.get_integer_value_field(EventField::EVENT_TARGET_UNIX_PROCESS_ID),
            514
        );
        assert_eq!(
            event.get_integer_value_field(TARGET_WINDOW_NUMBER_FIELD),
            10656
        );
        assert_eq!(event.get_integer_value_field(58), original_timestamp);
    }
    #[test]
    fn background_stamp_preserves_sender_event_timestamp() {
        #[link(name = "CoreGraphics", kind = "framework")]
        unsafe extern "C" {
            fn CGEventGetTimestamp(event: ffi::Ref) -> u64;
            fn CGEventSetTimestamp(event: ffi::Ref, timestamp: u64);
        }
        let e = mouse_event();
        unsafe {
            CGEventSetTimestamp(e.as_ptr().cast(), 123_456_789);
        }
        let before = unsafe { CGEventGetTimestamp(e.as_ptr().cast()) };
        assert_eq!(before, 123_456_789);
        let delivery = Delivery {
            pid: 514,
            window: 10656,
            frame: Rect {
                x: 0.,
                y: 0.,
                w: 100.,
                h: 100.,
            },
        };
        stamp(&e, &delivery, true);
        assert_eq!(unsafe { CGEventGetTimestamp(e.as_ptr().cast()) }, before);
    }
}
