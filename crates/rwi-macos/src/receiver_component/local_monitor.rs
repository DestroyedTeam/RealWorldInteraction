//! Explicit own-app local monitor, never a global hook or foreign injection.
use super::{Registration, Stats};
use block2::RcBlock;
use objc2::{
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
};
use std::rc::Rc;

/// Main-thread-only because the registered receiver and AppKit monitor are owned
/// by this process. Remove explicitly before closing its owning window.
pub struct LocalMonitor {
    token: Option<Retained<AnyObject>>,
    class: &'static AnyClass,
    registration: Rc<Registration>,
}
impl Registration {
    /// Opt-in application-side hook. No NSApplication subclass, activation,
    /// re-created event or fallback is installed. Source fields stay filters.
    pub fn install_local_monitor(self: Rc<Self>) -> Result<LocalMonitor, String> {
        if unsafe { libc::pthread_main_np() } != 1 {
            return Err("receiver_local_monitor_main_thread_required".into());
        }
        let class = AnyClass::get(c"NSEvent").ok_or("receiver_local_monitor_class_missing")?;
        let receiver = Rc::clone(&self);
        let handler = RcBlock::new(move |event: *mut AnyObject| -> *mut AnyObject {
            let Some(event_ref) = (unsafe { event.as_ref() }) else {
                return event;
            };
            // A panic cannot cross the block ABI or fall through to AppKit after
            // an own responder may already have handled the original event.
            let consumed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                receiver.dispatch(event_ref)
            }))
            .unwrap_or(true);
            if consumed {
                std::ptr::null_mut()
            } else {
                event
            }
        });
        let mask: usize = (1 << 1) | (1 << 2) | (1 << 6);
        let raw: *mut AnyObject = unsafe {
            msg_send![class, addLocalMonitorForEventsMatchingMask:mask, handler:&*handler]
        };
        let token =
            unsafe { Retained::retain(raw) }.ok_or("receiver_local_monitor_install_failed")?;
        Ok(LocalMonitor {
            token: Some(token),
            class,
            registration: self,
        })
    }
}
impl LocalMonitor {
    pub fn is_installed(&self) -> bool {
        self.token.is_some()
    }
    pub fn stats(&self) -> Stats {
        self.registration.stats()
    }
    /// Exactly one remove call. Safe Rust cannot move this Rc-owning token to
    /// another thread; the check also protects callers using unsafe embedding.
    pub fn stop(&mut self) -> Result<bool, String> {
        if unsafe { libc::pthread_main_np() } != 1 {
            return Err("receiver_local_monitor_main_thread_required".into());
        }
        let Some(token) = self.token.take() else {
            return Ok(false);
        };
        unsafe {
            let _: () = msg_send![self.class, removeMonitor:&*token];
        }
        Ok(true)
    }
}
impl Drop for LocalMonitor {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
#[cfg(test)]
#[test]
fn f404_monitor_original_pointer_block_has_paired_object_argument_and_return() {
    // ABI/type exercise only; no AppKit monitor or window is installed here.
    let handler = RcBlock::new(|event: *mut AnyObject| -> *mut AnyObject { event });
    assert!(handler.call((std::ptr::null_mut(),)).is_null());
}
