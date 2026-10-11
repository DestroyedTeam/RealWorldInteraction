//! Own receiving-fixture AX provider latency. Never an agent input bridge.
use objc2::{
    msg_send,
    runtime::{AnyClass, AnyObject, Sel},
};
use std::{
    sync::atomic::{AtomicU32, AtomicU64, Ordering},
    time::{Duration, Instant},
};
static DELAY_MS: AtomicU32 = AtomicU32::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static DELAYED: AtomicU64 = AtomicU64::new(0);
static MAX_US: AtomicU64 = AtomicU64::new(0);

pub(super) fn configure(ms: Option<u32>) {
    DELAY_MS.store(ms.unwrap_or(0), Ordering::Relaxed);
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
    })
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
