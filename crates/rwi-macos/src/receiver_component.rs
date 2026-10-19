//! Experimental, explicitly cooperating AppKit receiver. Not foreign-app injection.
//!
//! An owning app calls this from its NSApplication.sendEvent override *before*
//! superclass dispatch. Only its registered canvas receives original mouse events;
//! everything else retains AppKit dispatch. No activation, window policy override,
//! synthesized event, global hook, NSWindow.sendEvent call, or fallback is installed.
//! CGEvent source PID/birth is a routing filter, NOT authenticated IPC or a security
//! boundary. Agent authorization, leases and durable no-replay remain independent.
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Stats {
    pub downs: u64,
    pub drags: u64,
    pub ups: u64,
    pub rejected: u64,
    pub reentrant_rejected: u64,
    pub sequence_active: bool,
    pub source_authenticated: bool,
}

fn route_matches(
    expected_window: isize,
    actual_window: isize,
    pointer_matches: bool,
    expected_pid: u32,
    observed_pid: Option<i64>,
) -> bool {
    expected_window > 0
        && expected_window == actual_window
        && pointer_matches
        && expected_pid > 0
        && expected_pid <= i32::MAX as u32
        && observed_pid == Some(i64::from(expected_pid))
}

#[derive(Clone, Copy)]
struct Sample {
    kind: usize,
    number: isize,
    click_count: isize,
    flags: usize,
    subtype: i16,
    time: f64,
    in_view: bool,
    ready: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Decision {
    Pass,
    Reject,
    Down,
    Drag,
    Up,
}
#[derive(Clone, Copy, Default)]
struct Policy {
    active: Option<(isize, f64)>,
    last_terminal: Option<(isize, f64)>,
}
impl Policy {
    fn decide(&mut self, s: Sample) -> Decision {
        if !matches!(s.kind, 1 | 2 | 6) || (self.active.is_none() && !s.in_view) {
            return Decision::Pass;
        }
        if !s.ready
            || s.number <= 0
            || s.flags != 0
            || s.subtype != 0
            || s.click_count != 1
            || !s.time.is_finite()
            || s.time <= 0.
        {
            return Decision::Reject;
        }
        match (s.kind, self.active) {
            (1, None)
                if self
                    .last_terminal
                    .is_none_or(|(n, t)| n != s.number && s.time > t) =>
            {
                self.active = Some((s.number, s.time));
                Decision::Down
            }
            (2 | 6, Some((n, t))) if n == s.number && s.time >= t && s.time - t <= 15. => {
                if s.kind == 2 {
                    self.active = None;
                    self.last_terminal = Some((n, s.time));
                    Decision::Up
                } else {
                    self.active = Some((n, s.time));
                    Decision::Drag
                }
            }
            _ => Decision::Reject,
        }
    }
}

#[cfg(target_os = "macos")]
mod native {
    use super::{Decision, Policy, Sample, Stats};
    use objc2::{
        Encoding, RefEncode, msg_send,
        rc::Retained,
        runtime::{AnyClass, AnyObject},
    };
    use objc2_foundation::{NSPoint, NSRect};
    use std::{cell::Cell, ffi::c_void};

    // CGEventRef is a pointer to this *named opaque struct*, not void*. Keep
    // objc2's runtime signature checking enabled; never release this borrowed ref.
    #[repr(C)]
    struct AppKitCGEvent {
        _opaque: [u8; 0],
    }
    unsafe impl RefEncode for AppKitCGEvent {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CGEvent", &[]));
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventGetIntegerValueField(event: *const c_void, field: u32) -> i64;
    }
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {
        static NSApp: *mut AnyObject;
    }

    fn kind_of(object: &AnyObject, name: &std::ffi::CStr) -> bool {
        let Some(expected) = AnyClass::get(name) else {
            return false;
        };
        let mut actual = Some(object.class());
        while let Some(class) = actual {
            if std::ptr::eq(class, expected) {
                return true;
            }
            actual = class.superclass();
        }
        false
    }
    fn contains(r: NSRect, p: NSPoint) -> bool {
        [
            r.origin.x,
            r.origin.y,
            r.size.width,
            r.size.height,
            p.x,
            p.y,
        ]
        .iter()
        .all(|x| x.is_finite())
            && r.size.width > 0.
            && r.size.height > 0.
            && p.x >= r.origin.x
            && p.y >= r.origin.y
            && p.x < r.origin.x + r.size.width
            && p.y < r.origin.y + r.size.height
    }
    struct Busy<'a>(&'a Cell<bool>);
    impl Drop for Busy<'_> {
        fn drop(&mut self) {
            self.0.set(false);
        }
    }

    /// Own main-thread objects only; retained without any foreign pointer adoption.
    /// The caller explicitly chooses this app-side receiving policy for its view.
    pub struct Registration {
        view: Retained<AnyObject>,
        window: Retained<AnyObject>,
        window_number: isize,
        source_pid: u32,
        source_birth: String,
        policy: Cell<Policy>,
        stats: Cell<Stats>,
        busy: Cell<bool>,
    }
    impl Registration {
        pub fn register(
            view: &AnyObject,
            window: &AnyObject,
            source_pid: u32,
        ) -> Result<Self, String> {
            if unsafe { libc::pthread_main_np() } != 1
                || source_pid == 0
                || source_pid > i32::MAX as u32
                || !kind_of(view, c"NSView")
                || !kind_of(window, c"NSWindow")
            {
                return Err("receiver_component_invalid_own_registration".into());
            }
            let birth = crate::native::process_epoch(source_pid).map_err(|e| e.to_string())?;
            unsafe {
                let attached: *const AnyObject = msg_send![view, window];
                let visible: bool = msg_send![window, isVisible];
                let number: isize = msg_send![window, windowNumber];
                if !std::ptr::eq(attached, window) || !visible || number <= 0 {
                    return Err("receiver_component_detached_or_invisible".into());
                }
                Ok(Self {
                    view: Retained::retain(view as *const _ as *mut _).ok_or("missing view")?,
                    window: Retained::retain(window as *const _ as *mut _)
                        .ok_or("missing window")?,
                    window_number: number,
                    source_pid,
                    source_birth: birth,
                    policy: Cell::new(Policy::default()),
                    stats: Cell::new(Stats::default()),
                    busy: Cell::new(false),
                })
            }
        }
        pub fn stats(&self) -> Stats {
            self.stats.get()
        }

        /// Returns true only when the opt-in component consumed this event. Do not
        /// pass it to superclass as well, retry, or turn rejection into activation.
        pub fn dispatch(&self, event: &AnyObject) -> bool {
            if unsafe { libc::pthread_main_np() } != 1 || !kind_of(event, c"NSEvent") {
                return false;
            }
            unsafe {
                let kind: usize = msg_send![event, type];
                if !matches!(kind, 1 | 2 | 6) {
                    return false;
                }
                let number: isize = msg_send![event, windowNumber];
                let window: *const AnyObject = msg_send![event, window];
                let cg: *const AppKitCGEvent = msg_send![event, CGEvent];
                // Public kCGEventSourceUnixProcessID. A filter, not peer authentication.
                let exact_window =
                    number == self.window_number && std::ptr::eq(window, &*self.window);
                let observed_pid = if exact_window && !cg.is_null() {
                    Some(CGEventGetIntegerValueField(cg.cast(), 41))
                } else {
                    None
                };
                if !super::route_matches(
                    self.window_number,
                    number,
                    exact_window,
                    self.source_pid,
                    observed_pid,
                ) {
                    return false;
                }
                if self.busy.replace(true) {
                    let mut stats = self.stats.get();
                    stats.reentrant_rejected += 1;
                    self.stats.set(stats);
                    return true;
                }
                let _guard = Busy(&self.busy);
                let point: NSPoint = msg_send![event, locationInWindow];
                let local: NSPoint = msg_send![&*self.view, convertPoint:point, fromView:std::ptr::null_mut::<AnyObject>()];
                let bounds: NSRect = msg_send![&*self.view, bounds];
                let visible_rect: NSRect = msg_send![&*self.view, visibleRect];
                let attached: *const AnyObject = msg_send![&*self.view, window];
                let visible: bool = msg_send![&*self.window, isVisible];
                let mini: bool = msg_send![&*self.window, isMiniaturized];
                let hidden: bool = msg_send![&*self.view, isHiddenOrHasHiddenAncestor];
                let sheet: *const AnyObject = msg_send![&*self.window, attachedSheet];
                let app = NSApp;
                let modal: *const AnyObject = if app.is_null() {
                    window
                } else {
                    msg_send![app, modalWindow]
                };
                let ready = visible
                    && !mini
                    && !hidden
                    && sheet.is_null()
                    && modal.is_null()
                    && std::ptr::eq(attached, &*self.window)
                    && crate::native::process_epoch(self.source_pid)
                        .is_ok_and(|b| b == self.source_birth);
                let sample = Sample {
                    kind,
                    number: msg_send![event, eventNumber],
                    click_count: msg_send![event, clickCount],
                    flags: msg_send![event, modifierFlags],
                    subtype: msg_send![event, subtype],
                    time: msg_send![event, timestamp],
                    in_view: contains(bounds, local) && contains(visible_rect, local),
                    ready,
                };
                let mut policy = self.policy.get();
                let decision = policy.decide(sample);
                self.policy.set(policy);
                let mut stats = self.stats.get();
                stats.sequence_active = policy.active.is_some();
                match decision {
                    Decision::Pass => return false,
                    Decision::Reject => stats.rejected += 1,
                    Decision::Down => stats.downs += 1,
                    Decision::Drag => stats.drags += 1,
                    Decision::Up => stats.ups += 1,
                }
                self.stats.set(stats);
                // Actual original NSEvent to the own responder, never oracle state
                // setters or NSWindow.sendEvent (which would promote a key window).
                match decision {
                    Decision::Down => {
                        let _: () = msg_send![&*self.view,mouseDown:event];
                    }
                    Decision::Drag => {
                        let _: () = msg_send![&*self.view,mouseDragged:event];
                    }
                    Decision::Up => {
                        let _: () = msg_send![&*self.view,mouseUp:event];
                    }
                    _ => {}
                }
                true
            }
        }
    }
    #[cfg(test)]
    pub(super) fn check_borrowed_bridge() {
        use core_graphics::{
            event::{CGEvent, CGEventType, CGMouseButton},
            event_source::{CGEventSource, CGEventSourceStateID},
            geometry::CGPoint,
        };
        use foreign_types::ForeignType;
        objc2::rc::autoreleasepool(|_| unsafe {
            let source = CGEventSource::new(CGEventSourceStateID::Private).unwrap();
            let cg = CGEvent::new_mouse_event(
                source,
                CGEventType::LeftMouseDown,
                CGPoint::new(12., 34.),
                CGMouseButton::Left,
            )
            .unwrap();
            let class = AnyClass::get(c"NSEvent").unwrap();
            let event: *mut AnyObject =
                msg_send![class,eventWithCGEvent:cg.as_ptr().cast::<AppKitCGEvent>()];
            assert!(!event.is_null());
            let borrowed: *const AppKitCGEvent = msg_send![event, CGEvent];
            assert!(!borrowed.is_null());
            // Native getters operate while the NSEvent/CGEvent owners are live.
            assert!(CGEventGetIntegerValueField(borrowed.cast(), 41) > 0);
        });
    }
}
#[cfg(target_os = "macos")]
pub use native::Registration;

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    #[test]
    fn borrowed_cgevent_uses_named_opaque_abi() {
        super::native::check_borrowed_bridge();
    }
    fn sample(kind: usize) -> Sample {
        Sample {
            kind,
            number: 17,
            click_count: 1,
            flags: 0,
            subtype: 0,
            time: 1.,
            in_view: true,
            ready: true,
        }
    }
    #[test]
    fn ordinary_pair_is_delivered_once() {
        let mut p = Policy::default();
        assert_eq!(p.decide(sample(1)), Decision::Down);
        assert_eq!(p.decide(sample(2)), Decision::Up);
        assert_eq!(p.decide(sample(2)), Decision::Reject);
    }
    #[test]
    fn new_number_requires_new_time() {
        let mut p = Policy::default();
        p.decide(sample(1));
        p.decide(sample(2));
        let mut s = sample(1);
        s.number = 18;
        assert_eq!(p.decide(s), Decision::Reject);
        s.time = 2.;
        assert_eq!(p.decide(s), Decision::Down);
    }
    #[test]
    fn duplicate_down_cannot_replace_sequence() {
        let mut p = Policy::default();
        p.decide(sample(1));
        assert_eq!(p.decide(sample(1)), Decision::Reject);
        assert_eq!(p.active.unwrap().0, 17);
    }
    #[test]
    fn foreign_press_cannot_release_sequence() {
        let mut p = Policy::default();
        p.decide(sample(1));
        let mut s = sample(2);
        s.number = 18;
        assert_eq!(p.decide(s), Decision::Reject);
        assert_eq!(p.decide(sample(2)), Decision::Up);
    }
    #[test]
    fn orphan_drag_rejected() {
        assert_eq!(Policy::default().decide(sample(6)), Decision::Reject);
    }
    #[test]
    fn outside_down_keeps_normal_dispatch() {
        let mut s = sample(1);
        s.in_view = false;
        assert_eq!(Policy::default().decide(s), Decision::Pass);
    }
    #[test]
    fn original_drag_can_leave_own_bounds() {
        let mut p = Policy::default();
        p.decide(sample(1));
        let mut s = sample(6);
        s.in_view = false;
        assert_eq!(p.decide(s), Decision::Drag);
        s.kind = 2;
        assert_eq!(p.decide(s), Decision::Up);
    }
    #[test]
    fn modifiers_and_touch_not_silently_reinterpreted() {
        for (flags, subtype) in [(1, 0), (0, 3)] {
            let mut s = sample(1);
            s.flags = flags;
            s.subtype = subtype;
            assert_eq!(Policy::default().decide(s), Decision::Reject);
        }
    }
    #[test]
    fn invalid_and_stale_times_rejected() {
        for time in [f64::NAN, f64::INFINITY, 0., -1.] {
            let mut s = sample(1);
            s.time = time;
            assert_eq!(Policy::default().decide(s), Decision::Reject);
        }
        let mut p = Policy::default();
        p.decide(sample(1));
        let mut s = sample(2);
        s.time = 17.;
        assert_eq!(p.decide(s), Decision::Reject);
    }
    #[test]
    fn unavailable_owner_is_not_fallback() {
        let mut s = sample(1);
        s.ready = false;
        assert_eq!(Policy::default().decide(s), Decision::Reject);
    }
    #[test]
    fn other_events_keep_appkit_dispatch() {
        assert_eq!(Policy::default().decide(sample(10)), Decision::Pass);
    }
    #[test]
    fn routing_stats_do_not_attest_security() {
        assert!(!Stats::default().source_authenticated);
    }
    #[test]
    fn f399_exact_origin_routes_but_does_not_authenticate() {
        assert!(route_matches(42, 42, true, 17, Some(17)));
        assert!(!Stats::default().source_authenticated);
    }
    #[test]
    fn f399_foreign_pid_keeps_normal_dispatch() {
        assert!(!route_matches(42, 42, true, 17, Some(18)));
    }
    #[test]
    fn f399_numeric_window_alone_is_not_owned_identity() {
        assert!(!route_matches(42, 42, false, 17, Some(17)));
        assert!(!route_matches(42, 43, true, 17, Some(17)));
    }
    #[test]
    fn f399_missing_or_invalid_origin_is_not_zero() {
        for (pid, observed) in [
            (17, None),
            (0, Some(0)),
            (u32::MAX, Some(i64::from(u32::MAX))),
        ] {
            assert!(!route_matches(42, 42, true, pid, observed));
        }
        assert!(!route_matches(0, 0, true, 17, Some(17)));
    }
}
