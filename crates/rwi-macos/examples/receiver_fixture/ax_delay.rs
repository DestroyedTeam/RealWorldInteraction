//! Own receiving-fixture AX provider latency. Never an agent input bridge.
use objc2::{
    msg_send,
    runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel},
    sel,
};
use std::{
    sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    time::{Duration, Instant},
};
static DELAY_MS: AtomicU32 = AtomicU32::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static DELAYED: AtomicU64 = AtomicU64::new(0);
static MAX_US: AtomicU64 = AtomicU64::new(0);
static FOCUS_MS: AtomicU32 = AtomicU32::new(0);
static FOCUS_CALLS: AtomicU64 = AtomicU64::new(0);
static FOCUS_DELAYED: AtomicU64 = AtomicU64::new(0);
static FOCUS_MAX_US: AtomicU64 = AtomicU64::new(0);
static INACTIVE_SEEN: AtomicBool = AtomicBool::new(false);
static FOCUS_CLAIMED: AtomicBool = AtomicBool::new(false);

pub(super) fn configure(ms: Option<u32>, focused_once_ms: Option<u32>) {
    DELAY_MS.store(ms.unwrap_or(0), Ordering::Relaxed);
    FOCUS_MS.store(focused_once_ms.unwrap_or(0), Ordering::Relaxed);
}
/// # Safety
/// Only register on this fixture's unregistered NSApplication subclass.
pub(super) unsafe fn register(application: &mut ClassBuilder) {
    unsafe {
        application.add_method(
            sel!(accessibilityWindows),
            accessibility_windows as extern "C" fn(_, _) -> _,
        );
        application.add_method(
            sel!(accessibilityFocusedWindow),
            accessibility_focused_window as extern "C" fn(_, _) -> _,
        );
    }
}
pub(super) fn snapshot() -> serde_json::Value {
    serde_json::json!({
        "requested_delay_ms":DELAY_MS.load(Ordering::Relaxed),
        "getter_calls":CALLS.load(Ordering::Relaxed),
        "delayed_getter_calls":DELAYED.load(Ordering::Relaxed),
        "maximum_getter_us":MAX_US.load(Ordering::Relaxed),
        "AX_error_attested":false,
        "input_sent":false,
        "scope":"own_NSApplication_accessibilityWindows; other attributes use superclass",
        "focused_window_once":{
            "requested_delay_ms":FOCUS_MS.load(Ordering::Relaxed),
            "getter_calls":FOCUS_CALLS.load(Ordering::Relaxed),
            "delayed_getter_calls":FOCUS_DELAYED.load(Ordering::Relaxed),
            "maximum_getter_us":FOCUS_MAX_US.load(Ordering::Relaxed),
            "inactive_getter_seen":INACTIVE_SEEN.load(Ordering::Acquire),
            "once_claimed":FOCUS_CLAIMED.load(Ordering::Acquire),
            "AX_error_attested":false,"input_sent":false,
            "scope":"own public focused-window getter latency; unchanged superclass value"
        },
    })
}

fn claim_focused_delay(ms: u32, active: bool, inactive: &AtomicBool, claimed: &AtomicBool) -> bool {
    if ms == 0 {
        return false;
    }
    if !active {
        inactive.store(true, Ordering::Release);
        return false;
    }
    inactive.load(Ordering::Acquire)
        && claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
}

extern "C" fn accessibility_focused_window(this: &AnyObject, _cmd: Sel) -> *mut AnyObject {
    let started = Instant::now();
    FOCUS_CALLS.fetch_add(1, Ordering::Relaxed);
    let delay = FOCUS_MS.load(Ordering::Relaxed);
    // Default-off fixtures do not even query isActive for this test policy.
    if delay != 0 {
        let active: Bool = unsafe { msg_send![this, isActive] };
        if claim_focused_delay(delay, active.as_bool(), &INACTIVE_SEEN, &FOCUS_CLAIMED) {
            FOCUS_DELAYED.fetch_add(1, Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(u64::from(delay)));
        }
    }
    let parent = AnyClass::get(c"NSApplication").expect("own AppKit superclass exists");
    // Public nullable id getter, +0 convention. No fabricated value or AX error.
    let value: *mut AnyObject =
        unsafe { msg_send![super(this, parent), accessibilityFocusedWindow] };
    FOCUS_MAX_US.fetch_max(
        started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
        Ordering::Relaxed,
    );
    value
}

#[cfg(test)]
mod focused_delay_tests {
    use super::*;
    #[test]
    fn f119_foreground_provider_default_never_arms_or_claims() {
        let inactive = AtomicBool::new(false);
        let claimed = AtomicBool::new(false);
        assert!(!claim_focused_delay(0, false, &inactive, &claimed));
        assert!(!claim_focused_delay(0, true, &inactive, &claimed));
        assert!(!inactive.load(Ordering::Acquire));
        assert!(!claimed.load(Ordering::Acquire));
    }
    #[test]
    fn f119_foreground_provider_active_before_inactive_does_not_consume() {
        let inactive = AtomicBool::new(false);
        let claimed = AtomicBool::new(false);
        assert!(!claim_focused_delay(60, true, &inactive, &claimed));
        assert!(!claimed.load(Ordering::Acquire));
    }
    #[test]
    fn f119_foreground_provider_inactive_arms_without_delay() {
        let inactive = AtomicBool::new(false);
        let claimed = AtomicBool::new(false);
        assert!(!claim_focused_delay(60, false, &inactive, &claimed));
        assert!(inactive.load(Ordering::Acquire));
        assert!(!claimed.load(Ordering::Acquire));
    }
    #[test]
    fn f119_foreground_provider_reactivation_consumes_only_once() {
        let inactive = AtomicBool::new(true);
        let claimed = AtomicBool::new(false);
        assert!(claim_focused_delay(60, true, &inactive, &claimed));
        assert!(!claim_focused_delay(60, true, &inactive, &claimed));
        assert!(!claim_focused_delay(60, false, &inactive, &claimed));
        assert!(!claim_focused_delay(60, true, &inactive, &claimed));
    }
    #[test]
    fn f119_foreground_provider_concurrent_claims_have_one_winner() {
        let inactive = AtomicBool::new(true);
        let claimed = AtomicBool::new(false);
        let winners = AtomicU32::new(0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    if claim_focused_delay(60, true, &inactive, &claimed) {
                        winners.fetch_add(1, Ordering::Relaxed);
                    }
                });
            }
        });
        assert_eq!(winners.load(Ordering::Relaxed), 1);
    }
}
pub(super) extern "C" fn accessibility_windows(this: &AnyObject, _cmd: Sel) -> *mut AnyObject {
    let started = Instant::now();
    CALLS.fetch_add(1, Ordering::Relaxed);
    let delay = DELAY_MS.load(Ordering::Relaxed);
    if delay != 0 {
        DELAYED.fetch_add(1, Ordering::Relaxed);
        std::thread::sleep(Duration::from_millis(u64::from(delay)));
    }
    // Public nullable NSArray* getter, +0 return convention. No replacement
    // list/fake AXError, no swizzle/foreign object/input/global hook.
    let parent = AnyClass::get(c"NSApplication").expect("own AppKit superclass exists");
    let value: *mut AnyObject = unsafe { msg_send![super(this, parent), accessibilityWindows] };
    MAX_US.fetch_max(
        started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
        Ordering::Relaxed,
    );
    value
}
