//! Independent semantic UI oracle; not native pointer/tracking acceptance.
use serde::Serialize;
use std::collections::VecDeque;

pub const MAX_CALLBACKS: usize = 128;
pub const CONTROL_ID: &str = "rwi-semantic-alternate-ui";
pub const SHOW: &str = "AXShowAlternateUI";
pub const RESTORE: &str = "AXShowDefaultUI";

#[derive(Clone, Debug, Serialize)]
pub struct Callback {
    pub entry: &'static str,
    pub action: &'static str,
    pub unix_ms: Option<u64>,
    pub own_window_number: Option<isize>,
    pub app_active: Option<bool>,
    pub window_key: Option<bool>,
    pub alternate_visible_before: Option<bool>,
    pub alternate_visible_after: Option<bool>,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Oracle {
    pub enabled: bool,
    pub callback_calls: u64,
    pub show_calls: u64,
    pub restore_calls: u64,
    pub dropped_callbacks: u64,
    pub callbacks: VecDeque<Callback>,
    /// Independent native view isHidden samples, not action return values.
    pub source_alternate_visible: Option<bool>,
    pub source_default_visible: Option<bool>,
    /// Not exposed by the current model tools; unknown is never false.
    pub externally_observed_ax_alternate_ui_visible: Option<bool>,
    pub diagnostics: Diagnostics,
}
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct Diagnostics {
    pub action_selector_calls: u64,
    pub show_selector_calls: u64,
    pub restore_selector_calls: u64,
    pub action_names_calls: u64,
    pub native_visibility_getter_calls: u64,
    pub native_value_getter_calls: u64,
    pub legacy_action_entry_calls: u64,
    pub invalid_control_calls: u64,
    pub missing_label_calls: u64,
    pub oracle_borrow_skips: u64,
    pub unknown_action_calls: u64,
}
impl Oracle {
    pub fn receive(&mut self, event: Callback) -> bool {
        if !self.enabled || !matches!(event.action, SHOW | RESTORE) {
            return false;
        }
        self.callback_calls = self.callback_calls.saturating_add(1);
        self.show_calls = self
            .show_calls
            .saturating_add(u64::from(event.action == SHOW));
        self.restore_calls = self
            .restore_calls
            .saturating_add(u64::from(event.action == RESTORE));
        if self.callbacks.len() == MAX_CALLBACKS {
            self.callbacks.pop_front();
            self.dropped_callbacks = self.dropped_callbacks.saturating_add(1);
        }
        self.callbacks.push_back(event);
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn event(action: &'static str) -> Callback {
        Callback {
            entry: "pure_model_not_native",
            action,
            unix_ms: None,
            own_window_number: None,
            app_active: None,
            window_key: None,
            alternate_visible_before: None,
            alternate_visible_after: None,
        }
    }
    #[test]
    fn default_is_unknown_and_records_no_native_or_ui_effect() {
        let mut o = Oracle::default();
        assert!(!o.receive(event(SHOW)));
        assert_eq!(o.callback_calls, 0);
        assert_eq!(o.source_alternate_visible, None);
        assert_eq!(o.externally_observed_ax_alternate_ui_visible, None);
    }
    #[test]
    fn pure_accounting_does_not_normalize_visibility_or_environment() {
        let mut o = Oracle {
            enabled: true,
            ..Oracle::default()
        };
        assert!(o.receive(event(SHOW)));
        assert!(o.receive(event(RESTORE)));
        assert_eq!((o.callback_calls, o.show_calls, o.restore_calls), (2, 1, 1));
        assert!(!o.receive(event("AXPress")));
        assert!(o.callbacks.iter().all(|r| r.window_key.is_none()));
        assert_eq!(o.source_alternate_visible, None);
    }
    #[test]
    fn bounded_history_retains_counts_and_reports_evictions() {
        let mut o = Oracle {
            enabled: true,
            ..Oracle::default()
        };
        for _ in 0..MAX_CALLBACKS + 3 {
            assert!(o.receive(event(SHOW)));
        }
        assert_eq!(o.callbacks.len(), MAX_CALLBACKS);
        assert_eq!(o.dropped_callbacks, 3);
        assert_eq!(o.callback_calls, (MAX_CALLBACKS + 3) as u64);
    }
}

#[cfg(target_os = "macos")]
pub mod native {
    use super::*;
    use objc2::{
        msg_send,
        rc::{Allocated, Retained},
        runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel},
        sel,
    };
    use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
    use std::{
        cell::{Cell, RefCell},
        ffi::CStr,
    };
    const ALT_TAG: isize = 73101;
    const DEFAULT_TAG: isize = 73102;
    thread_local! {
        static CONTROL: Cell<usize> = const { Cell::new(0) };
        static ORACLE: RefCell<Oracle> = RefCell::new(Oracle::default());
        static DIAG: Cell<Diagnostics> = Cell::new(Diagnostics::default());
    }
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {
        fn NSAccessibilityActionDescription(action: *const AnyObject) -> *mut AnyObject;
    }
    fn class(name: &'static CStr) -> Result<&'static AnyClass, String> {
        AnyClass::get(name).ok_or_else(|| format!("semantic fixture class unavailable: {name:?}"))
    }
    fn diagnostic(f: impl FnOnce(&mut Diagnostics)) {
        DIAG.with(|d| {
            let mut n = d.get();
            f(&mut n);
            d.set(n)
        });
    }
    fn exact(this: &AnyObject) -> bool {
        let okay = unsafe { libc::pthread_main_np() } == 1
            && CONTROL.with(|c| c.get() == this as *const AnyObject as usize);
        if !okay {
            diagnostic(|d| d.invalid_control_calls += 1);
        }
        okay
    }
    unsafe fn label(this: &AnyObject, tag: isize) -> Option<&AnyObject> {
        let p: *mut AnyObject = unsafe { msg_send![this,viewWithTag:tag] };
        let value = unsafe { p.as_ref() };
        if value.is_none() {
            diagnostic(|d| d.missing_label_calls += 1);
        }
        value
    }
    fn visible(this: &AnyObject) -> Option<bool> {
        if !exact(this) {
            return None;
        }
        unsafe {
            label(this, ALT_TAG).map(|v| {
                let hidden: bool = msg_send![v, isHidden];
                !hidden
            })
        }
    }
    fn transition(this: &AnyObject, show: bool, entry: &'static str) -> Bool {
        // Count actual action selector entry even if owned UI/effect cannot be recorded.
        diagnostic(|d| {
            d.action_selector_calls += 1;
            d.show_selector_calls += u64::from(show);
            d.restore_selector_calls += u64::from(!show);
        });
        if !exact(this) {
            return Bool::NO;
        }
        let before = visible(this);
        let after = unsafe {
            let (Some(alt), Some(default)) = (label(this, ALT_TAG), label(this, DEFAULT_TAG))
            else {
                return Bool::NO;
            };
            let _: () = msg_send![alt,setHidden:!show];
            let _: () = msg_send![default,setHidden:show];
            let _: () = msg_send![this,setNeedsDisplay:true];
            visible(this)
        };
        let (window_number, key, active) = unsafe {
            let window: *mut AnyObject = msg_send![this, window];
            let own = window.as_ref();
            let app: *mut AnyObject =
                msg_send![class(c"NSApplication").unwrap(), sharedApplication];
            (
                own.map(|w| msg_send![w, windowNumber]),
                own.map(|w| msg_send![w, isKeyWindow]),
                app.as_ref().map(|a| msg_send![a, isActive]),
            )
        };
        let unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|t| u64::try_from(t.as_millis()).ok());
        let event = Callback {
            entry,
            action: if show { SHOW } else { RESTORE },
            unix_ms,
            own_window_number: window_number,
            app_active: active,
            window_key: key,
            alternate_visible_before: before,
            alternate_visible_after: after,
        };
        ORACLE.with(|slot| match slot.try_borrow_mut() {
            Ok(mut oracle) => {
                oracle.receive(event);
            }
            Err(_) => diagnostic(|d| d.oracle_borrow_skips += 1),
        });
        // Trigger result only; an independent sample/AX/native image verifies effect.
        Bool::YES
    }
    extern "C" fn show(this: &AnyObject, _: Sel) -> Bool {
        transition(this, true, "modern_accessibilityPerformShowAlternateUI")
    }
    extern "C" fn restore(this: &AnyObject, _: Sel) -> Bool {
        transition(this, false, "modern_accessibilityPerformShowDefaultUI")
    }
    extern "C" fn alternate_visible(this: &AnyObject, _: Sel) -> Bool {
        diagnostic(|d| d.native_visibility_getter_calls += 1);
        Bool::new(visible(this).unwrap_or(false))
    }
    extern "C" fn value(this: &AnyObject, _: Sel) -> *mut AnyObject {
        diagnostic(|d| d.native_value_getter_calls += 1);
        let Some(v) = visible(this) else {
            return std::ptr::null_mut();
        };
        let text = NSString::from_str(if v { "alternate" } else { "default" });
        unsafe { msg_send![class(c"NSString").unwrap(),stringWithString:&*text] }
    }
    extern "C" fn action_names(this: &AnyObject, _: Sel) -> *mut AnyObject {
        diagnostic(|d| d.action_names_calls += 1);
        let superclass = class(c"NSView").unwrap();
        unsafe {
            let mut array: *mut AnyObject =
                msg_send![super(this, superclass), accessibilityActionNames];
            if array.is_null() {
                array = msg_send![class(c"NSArray").unwrap(), array];
            }
            for action in [SHOW, RESTORE] {
                let action = NSString::from_str(action);
                let has: bool = msg_send![array,containsObject:&*action];
                if !has {
                    array = msg_send![array,arrayByAddingObject:&*action];
                }
            }
            array
        }
    }
    extern "C" fn action_description(
        this: &AnyObject,
        _: Sel,
        action: &NSString,
    ) -> *mut AnyObject {
        if matches!(action.to_string().as_str(), SHOW | RESTORE) {
            unsafe { NSAccessibilityActionDescription((action as *const NSString).cast()) }
        } else {
            unsafe {
                msg_send![super(this,class(c"NSView").unwrap()),accessibilityActionDescription:action]
            }
        }
    }
    extern "C" fn perform(this: &AnyObject, _: Sel, action: &NSString) {
        diagnostic(|d| d.legacy_action_entry_calls += 1);
        match action.to_string().as_str() {
            SHOW => {
                transition(this, true, "legacy_accessibilityPerformAction");
            }
            RESTORE => {
                transition(this, false, "legacy_accessibilityPerformAction");
            }
            _ => {
                diagnostic(|d| d.unknown_action_calls += 1);
                unsafe {
                    let _: () = msg_send![super(this,class(c"NSView").unwrap()),accessibilityPerformAction:action];
                }
            }
        }
    }
    pub struct Objects {
        pub control: Retained<AnyObject>,
        alternate: Retained<AnyObject>,
        default: Retained<AnyObject>,
    }
    impl Objects {
        pub fn new() -> Result<Self, String> {
            if unsafe { libc::pthread_main_np() } != 1 {
                return Err("semantic control requires own main thread".into());
            }
            let mut builder =
                ClassBuilder::new(c"RWISemanticAlternateUIFixtureControl", class(c"NSView")?)
                    .ok_or("semantic control class already registered")?;
            unsafe {
                builder.add_method(
                    sel!(accessibilityPerformShowAlternateUI),
                    show as extern "C" fn(_, _) -> _,
                );
                builder.add_method(
                    sel!(accessibilityPerformShowDefaultUI),
                    restore as extern "C" fn(_, _) -> _,
                );
                builder.add_method(
                    sel!(isAccessibilityAlternateUIVisible),
                    alternate_visible as extern "C" fn(_, _) -> _,
                );
                builder.add_method(sel!(accessibilityValue), value as extern "C" fn(_, _) -> _);
                // Public legacy discovery/dispatch compatibility: retain inherited actions.
                builder.add_method(
                    sel!(accessibilityActionNames),
                    action_names as extern "C" fn(_, _) -> _,
                );
                builder.add_method(
                    sel!(accessibilityActionDescription:),
                    action_description as extern "C" fn(_, _, _) -> _,
                );
                builder.add_method(
                    sel!(accessibilityPerformAction:),
                    perform as extern "C" fn(_, _, _),
                );
            }
            let own_class = builder.register();
            unsafe {
                let allocated: Allocated<AnyObject> = msg_send![own_class, alloc];
                let control: Retained<AnyObject> = msg_send![allocated,initWithFrame:NSRect::new(NSPoint::new(24.,532.),NSSize::new(612.,40.))];
                let _: () = msg_send![&*control,setAccessibilityElement:true];
                let role = NSString::from_str("AXGroup");
                let identifier = NSString::from_str(CONTROL_ID);
                let title = NSString::from_str("RWI semantic alternate UI (not pointer tracking)");
                let _: () = msg_send![&*control,setAccessibilityRole:&*role];
                let _: () = msg_send![&*control,setAccessibilityIdentifier:&*identifier];
                let _: () = msg_send![&*control,setAccessibilityLabel:&*title];
                let make_label = |tag: isize,
                                  title: &str,
                                  hidden: bool|
                 -> Result<Retained<AnyObject>, String> {
                    let allocated: Allocated<AnyObject> = msg_send![class(c"NSTextField")?, alloc];
                    let label: Retained<AnyObject> = msg_send![allocated,initWithFrame:NSRect::new(NSPoint::new(8.,4.),NSSize::new(596.,30.))];
                    let text = NSString::from_str(title);
                    let _: () = msg_send![&*label,setStringValue:&*text];
                    let _: () = msg_send![&*label,setEditable:false];
                    let _: () = msg_send![&*label,setSelectable:false];
                    let _: () = msg_send![&*label,setBezeled:false];
                    let _: () = msg_send![&*label,setDrawsBackground:true];
                    let _: () = msg_send![&*label,setTag:tag];
                    let _: () = msg_send![&*label,setHidden:hidden];
                    let _: () = msg_send![&*control,addSubview:&*label];
                    Ok(label)
                };
                let default = make_label(
                    DEFAULT_TAG,
                    "SEMANTIC DEFAULT UI — separate from native hover",
                    false,
                )?;
                let alternate = make_label(
                    ALT_TAG,
                    "SEMANTIC ALTERNATE UI — actual native AX action",
                    true,
                )?;
                CONTROL.with(|p| p.set(&*control as *const AnyObject as usize));
                // One own object per process. Preserve all startup selector diagnostics;
                // never reset counters to manufacture a zero baseline.
                ORACLE.with(|o| o.borrow_mut().enabled = true);
                Ok(Self {
                    control,
                    alternate,
                    default,
                })
            }
        }
        pub fn snapshot(&self) -> Option<Oracle> {
            let mut value = ORACLE.with(|o| o.try_borrow().ok().map(|o| o.clone()))?;
            // Sample native UI directly; do not call the accessibility getters or a callback.
            unsafe {
                value.source_alternate_visible = Some(!msg_send![&*self.alternate, isHidden]);
                value.source_default_visible = Some(!msg_send![&*self.default, isHidden]);
            }
            value.diagnostics = DIAG.with(Cell::get);
            Some(value)
        }
    }
    impl Drop for Objects {
        fn drop(&mut self) {
            CONTROL.with(|p| {
                if p.get() == &*self.control as *const AnyObject as usize {
                    p.set(0)
                }
            });
        }
    }
}

#[cfg(test)]
#[test]
fn semantic_ui_is_explicit_independent_and_never_a_pointer_calibration() {
    let base = [
        "--open-window",
        "--run-id",
        "semantic-model",
        "--state-dir",
        "/tmp/rwi-semantic-model-never-created",
        "--display-id",
        "4",
    ];
    let parse = |extra: &[&str]| {
        crate::model::Options::parse(
            base.into_iter()
                .chain(extra.iter().copied())
                .map(str::to_owned),
        )
    };
    assert!(!parse(&[]).unwrap().unwrap().semantic_alternate_ui);
    assert!(
        parse(&["--semantic-alternate-ui", "--check-only"])
            .unwrap()
            .unwrap()
            .semantic_alternate_ui
    );
    assert!(parse(&["--semantic-alternate-ui", "--semantic-alternate-ui"]).is_err());
    assert!(parse(&["--semantic-alternate-ui", "--hover-first-responder"]).is_err());
    let s = crate::model::State::new("semantic-no-native".into());
    assert!(s.semantic_alternate_ui.is_none());
    assert_eq!(
        (
            s.hover.moves,
            s.hover.enters,
            s.hover.exits,
            s.hover.tooltip_requests
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(s.sequence, 0);
    assert!(!s.responder.calibration_requested);
}
