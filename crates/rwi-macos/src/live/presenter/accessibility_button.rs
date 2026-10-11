//! Explicit public accessibility provider for our two preview buttons only.
//! An action-trigger acknowledgement is not proof of the resulting Live state.
use super::*;
use objc2::runtime::Bool;

fn trigger_press(enabled: bool, configured: bool, send: impl FnOnce() -> bool) -> bool {
    enabled && configured && send()
}

extern "C" fn press(this: &AnyObject, _cmd: Sel) -> Bool {
    let triggered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let enabled: Bool = msg_send![this, isEnabled];
        let target: *mut AnyObject = msg_send![this, target];
        let action: Option<Sel> = msg_send![this, action];
        let configured =
            !target.is_null() && action.is_some_and(|a| a == sel!(toggle:) || a == sel!(stop:));
        trigger_press(enabled.as_bool(), configured, || {
            // Send exactly one existing target/action. No input event, retry,
            // responder-chain fallback or forced successful acknowledgement.
            let sent: Bool = msg_send![this, sendAction:action, to:target];
            sent.as_bool()
        })
    }))
    .unwrap_or(false);
    Bool::new(triggered)
}

/// Called only from the main-thread native presenter; superclass retains native
/// drawing, hit testing, labels and mouse-click behavior.
pub(super) unsafe fn new(frame: NSRect) -> std::result::Result<Retained<AnyObject>, String> {
    let button_class = if let Some(class) = AnyClass::get(c"RWILiveAccessibilityButton") {
        class
    } else {
        let mut builder = ClassBuilder::new(c"RWILiveAccessibilityButton", class(c"NSButton")?)
            .ok_or("Cannot register Live accessibility button")?;
        unsafe {
            builder.add_method(
                sel!(accessibilityPerformPress),
                press as extern "C" fn(_, _) -> _,
            );
        }
        builder.register()
    };
    let allocated: Allocated<AnyObject> = unsafe { msg_send![button_class, alloc] };
    Ok(unsafe { msg_send![allocated, initWithFrame:frame] })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_accessibility_press_only_acknowledges_one_configured_action_trigger() {
        for (enabled, configured) in [(false, true), (true, false), (false, false)] {
            assert!(!trigger_press(enabled, configured, || panic!(
                "must not send"
            )));
        }
        let mut calls = 0;
        assert!(!trigger_press(true, true, || {
            calls += 1;
            false
        }));
        assert_eq!(calls, 1);
        assert!(trigger_press(true, true, || {
            calls += 1;
            true
        }));
        assert_eq!(calls, 2);
    }
}
