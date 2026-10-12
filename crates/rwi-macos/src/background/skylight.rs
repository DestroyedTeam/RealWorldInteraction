//! Opt-in MouseMoved-only SkyLight experiment. Missing SPI is a refusal, never
//! a public PID/HID retry. Availability and a void post do not attest app effect.
//! Signature reference: trycua/cua 0a10f6964b727cd8ae45cd118a1d392815cdb8b7,
//! libs/cua-driver/rust/crates/platform-macos/src/input/skylight.rs.
use super::{Delivery, stamp, window_local};
use crate::ffi;
use core_graphics::{
    event::{CGEvent, CGEventType},
    geometry::CGPoint,
};
use foreign_types::ForeignType;
use rwi_core::{Result, fault};
use std::{ffi::c_void, sync::OnceLock};

type PostToPid = unsafe extern "C" fn(libc::pid_t, *mut c_void);
struct Driver {
    post: PostToPid,
    // A process-lifetime OnceLock retains this dlopen reference for the function
    // pointer's lifetime. Failed resolutions close immediately. No per-input leak.
    _library: usize,
}
fn driver() -> Option<&'static Driver> {
    static DRIVER: OnceLock<Option<Driver>> = OnceLock::new();
    DRIVER
        .get_or_init(|| {
            let library = unsafe {
                libc::dlopen(
                    c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight".as_ptr(),
                    libc::RTLD_LOCAL | libc::RTLD_NOW,
                )
            };
            if library.is_null() {
                return None;
            }
            let pointer = unsafe { libc::dlsym(library, c"SLEventPostToPid".as_ptr()) };
            if pointer.is_null() {
                unsafe {
                    libc::dlclose(library);
                }
                return None;
            }
            // SAFETY: the exact C signature is pinned above. The successful library
            // reference remains retained; this code never obtains an auth envelope.
            let post = unsafe { std::mem::transmute::<*mut c_void, PostToPid>(pointer) };
            Some(Driver {
                post,
                _library: library as usize,
            })
        })
        .as_ref()
}
pub(crate) fn available() -> bool {
    driver().is_some() && ffi::window_location_spi_available()
}
pub(crate) struct PreparedMove {
    driver: &'static Driver,
    event: CGEvent,
    pid: libc::pid_t,
}
/// Prepare only local event memory. Native authority and the shared lease must
/// still be checked immediately before the one counted post in guarded_single.
pub(crate) fn prepare_pointer(
    event: &CGEvent,
    delivery: &Delivery,
    point: [f64; 2],
) -> Result<PreparedMove> {
    if event.get_type() as u32 != CGEventType::MouseMoved as u32
        || delivery.pid == 0
        || delivery.pid > i32::MAX as u32
        || delivery.window == 0
        || !delivery.frame.valid()
        || !point.iter().all(|v| v.is_finite())
        || !delivery.frame.contains(point)
    {
        return Err(fault(
            "invalid_pointer_delivery",
            "One exact finite window-addressed MouseMoved is required.",
        ));
    }
    let driver = driver().ok_or_else(|| {
        fault(
            "background_delivery_unavailable",
            "Experimental SLEventPostToPid is unavailable; no fallback or input.",
        )
    })?;
    let event = event.clone();
    stamp(&event, delivery, true);
    let local = window_local(point, delivery.frame);
    if !ffi::set_window_location(event.as_ptr().cast(), CGPoint::new(local[0], local[1])) {
        return Err(fault(
            "background_delivery_unavailable",
            "Exact window event addressing is unavailable; no input.",
        ));
    }
    Ok(PreparedMove {
        driver,
        event,
        pid: delivery.pid as libc::pid_t,
    })
}
impl PreparedMove {
    /// Called only by the counted, freshly validated native scope. Consuming the
    /// prepared value prevents reusing it as a retry. The private post returns void.
    pub(crate) fn post(self) {
        unsafe {
            (self.driver.post)(self.pid, self.event.as_ptr().cast());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::{
        event::CGMouseButton,
        event_source::{CGEventSource, CGEventSourceStateID},
    };
    #[test]
    fn experimental_route_rejects_non_move_and_invalid_identity_before_post() {
        let source = CGEventSource::new(CGEventSourceStateID::Private).unwrap();
        let event = CGEvent::new_mouse_event(
            source,
            CGEventType::LeftMouseDown,
            CGPoint::new(2., 3.),
            CGMouseButton::Left,
        )
        .unwrap();
        let delivery = Delivery {
            pid: 1,
            window: 1,
            frame: crate::geometry::Rect {
                x: 0.,
                y: 0.,
                w: 10.,
                h: 10.,
            },
        };
        assert_eq!(
            prepare_pointer(&event, &delivery, [2., 3.])
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
        let source = CGEventSource::new(CGEventSourceStateID::Private).unwrap();
        let event = CGEvent::new_mouse_event(
            source,
            CGEventType::MouseMoved,
            CGPoint::new(2., 3.),
            CGMouseButton::Left,
        )
        .unwrap();
        for bad in [
            Delivery { pid: 0, ..delivery },
            Delivery {
                pid: u32::MAX,
                ..delivery
            },
            Delivery {
                window: 0,
                ..delivery
            },
        ] {
            assert_eq!(
                prepare_pointer(&event, &bad, [2., 3.]).err().unwrap().code,
                "invalid_pointer_delivery"
            );
        }
        for point in [[f64::NAN, 2.], [10., 3.], [-1., 3.]] {
            assert_eq!(
                prepare_pointer(&event, &delivery, point)
                    .err()
                    .unwrap()
                    .code,
                "invalid_pointer_delivery"
            );
        }
    }
    #[test]
    fn experimental_preparation_changes_only_local_event_without_posting() {
        use core_graphics::event::{CGEventFlags, EventField};
        #[link(name = "CoreGraphics", kind = "framework")]
        unsafe extern "C" {
            fn CGEventGetTimestamp(event: *const c_void) -> u64;
        }
        let source = CGEventSource::new(CGEventSourceStateID::Private).unwrap();
        let event = CGEvent::new_mouse_event(
            source,
            CGEventType::MouseMoved,
            CGPoint::new(12., 23.),
            CGMouseButton::Left,
        )
        .unwrap();
        let before = unsafe { CGEventGetTimestamp(event.as_ptr().cast()) };
        let delivery = Delivery {
            pid: 1,
            window: 1,
            frame: crate::geometry::Rect {
                x: 10.,
                y: 20.,
                w: 10.,
                h: 10.,
            },
        };
        if !available() {
            assert_eq!(
                prepare_pointer(&event, &delivery, [12., 23.])
                    .err()
                    .unwrap()
                    .code,
                "background_delivery_unavailable"
            );
            return;
        }
        let prepared = prepare_pointer(&event, &delivery, [12., 23.]).unwrap();
        assert_eq!(
            prepared.event.get_type() as u32,
            CGEventType::MouseMoved as u32
        );
        assert_eq!(prepared.event.get_flags(), CGEventFlags::empty());
        assert_eq!(
            prepared
                .event
                .get_integer_value_field(EventField::EVENT_TARGET_UNIX_PROCESS_ID),
            1
        );
        assert_eq!(
            unsafe { CGEventGetTimestamp(prepared.event.as_ptr().cast()) },
            before
        );
        // Dropping prepared memory does not call PreparedMove::post.
        drop(prepared);
    }
}
