//! Completion ownership crosses callbacks, unlike thread-local cancellation state.
use rwi_core::{Result, fault, operation};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct Pending {
    busy: Arc<AtomicBool>,
    abandoned: AtomicBool,
    _native: operation::NativeCall,
}
impl Pending {
    pub(super) fn begin(busy: Arc<AtomicBool>) -> Result<Arc<Self>> {
        if busy.swap(true, Ordering::AcqRel) {
            return Err(fault(
                "capture_busy",
                "The previous native callback still owns this context capture slot, including across rebinding.",
            ));
        }
        match operation::native_call("macos_screen_capture_callback") {
            Ok(native) => Ok(Arc::new(Self {
                busy,
                abandoned: AtomicBool::new(false),
                _native: native,
            })),
            Err(error) => {
                busy.store(false, Ordering::Release);
                Err(error)
            }
        }
    }
    pub(super) fn abandon(&self) {
        self.abandoned.store(true, Ordering::Release);
    }
    pub(super) fn require_active(&self) -> Result<()> {
        if self.abandoned.load(Ordering::Acquire) {
            return Err(fault(
                "capture_waiter_abandoned",
                "The caller cancelled or timed out; native callbacks only finish cleanup, never schedule another capture stage.",
            ));
        }
        Ok(())
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        self.busy.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timeout_cancel_and_rebinding_cannot_discard_a_native_callback_scope() {
        let registry = operation::Registry::default();
        let op = registry.begin(Some("synthetic-capture".into())).unwrap();
        let busy = Arc::new(AtomicBool::new(false));
        let local = operation::run(&op, || Pending::begin(busy.clone()).unwrap());
        let callback = local.clone();
        local.abandon();
        assert!(callback.require_active().is_err());
        drop(local); // A timed-out waiter no longer owns the callback.
        op.cancel();
        op.finish();
        assert!(op.status().native_call_pending);
        assert!(!op.status().resources_released);
        assert_eq!(
            Pending::begin(busy.clone()).err().unwrap().code,
            "capture_busy"
        );
        drop(callback);
        assert!(op.status().terminal && op.status().resources_released);
        assert!(!busy.load(Ordering::Acquire));
    }
    #[test]
    fn cancelled_start_restores_the_capture_slot_without_entering_native_code() {
        let op = operation::Registry::default().begin(None).unwrap();
        op.cancel();
        let busy = Arc::new(AtomicBool::new(false));
        assert!(operation::run(&op, || Pending::begin(busy.clone())).is_err());
        assert!(!busy.load(Ordering::Acquire));
        assert!(!op.status().native_call_pending);
    }
}
