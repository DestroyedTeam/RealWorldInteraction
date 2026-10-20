//! Service-owned feedback, separate from input authority and grounding pixels.
#[allow(unused_imports)]
pub(super) use rwi_pointer::{
    Anchor, CENTER, HIGHLIGHT_MS, MAX_DPI, Phase, SIZE, normalize_dpi, raster_size, scale_for_dpi,
};
#[allow(unused_imports)]
pub(super) use rwi_pointer::{material, motion};
#[cfg(windows)]
mod native;
#[cfg(windows)]
pub(crate) use native::{Gesture, capture_status, start, status, stop, suspend_capture};

/// Own exactly one release action, including early errors and unwind paths.
pub(super) struct ReleaseOnce<T> {
    value: Option<T>,
    release: fn(T),
}
impl<T> ReleaseOnce<T> {
    pub fn new(value: T, release: fn(T)) -> Self {
        Self {
            value: Some(value),
            release,
        }
    }
    pub fn get(&self) -> Option<&T> {
        self.value.as_ref()
    }
}
impl<T> Drop for ReleaseOnce<T> {
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            (self.release)(value);
        }
    }
}

/// An old renderer snapshot cannot acknowledge a newer capture suspension.
#[derive(Default)]
pub(super) struct CaptureGate {
    pub revision: u64,
    pub applied_revision: u64,
    pub suspended: usize,
    pub visible: bool,
}
impl CaptureGate {
    pub fn changed(&mut self) {
        self.revision += 1;
    }
    pub fn suspend(&mut self) -> u64 {
        self.suspended += 1;
        self.changed();
        self.revision
    }
    pub fn resume(&mut self) {
        self.suspended = self.suspended.saturating_sub(1);
        self.changed();
    }
    pub fn acknowledge(&mut self, revision: u64, visible: bool) {
        self.applied_revision = revision;
        self.visible = visible;
    }
    pub fn capture_ready(&self, ticket: u64) -> bool {
        self.suspended > 0 && self.applied_revision >= ticket && !self.visible
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fractional_dpi_changes_expression_not_native_hotspot() {
        const {
            assert!(HIGHLIGHT_MS <= 1000);
        }
        assert_eq!(
            [0, 96, 120, 144, 192, 768].map(raster_size),
            [160, 160, 200, 240, 320, 480]
        );
        for dpi in [96, 120, 144, 192, 288] {
            let point = [-202, 987];
            let anchor = Anchor { point, dpi: 96 }.with_dpi(dpi);
            assert_eq!(anchor.point, point);
            assert_eq!(anchor.dpi, dpi);
            let half = raster_size(dpi) as i32 / 2;
            let destination = [point[0] - half, point[1] - half];
            assert_eq!([destination[0] + half, destination[1] + half], point);
        }
    }
    #[test]
    fn in_flight_old_render_cannot_acknowledge_a_capture() {
        let mut gate = CaptureGate::default();
        gate.changed();
        let old = gate.revision;
        let ticket = gate.suspend();
        gate.acknowledge(old, true);
        assert!(!gate.capture_ready(ticket));
        gate.acknowledge(old, false);
        assert!(!gate.capture_ready(ticket));
        gate.acknowledge(ticket, false);
        assert!(gate.capture_ready(ticket));
    }
    #[test]
    fn nested_capture_and_new_input_stay_hidden_until_all_guards_leave() {
        let mut gate = CaptureGate::default();
        let a = gate.suspend();
        let b = gate.suspend();
        gate.changed();
        gate.acknowledge(gate.revision, false);
        assert!(gate.capture_ready(a) && gate.capture_ready(b));
        gate.resume();
        assert_eq!(gate.suspended, 1);
        gate.resume();
        assert!(!gate.capture_ready(b));
    }
    #[test]
    fn capture_ticket_precedes_worker_start_and_releases_only_its_own_nested_hold() {
        use std::sync::{Arc, Mutex};
        fn release(g: Arc<Mutex<CaptureGate>>) {
            g.lock().unwrap().resume();
        }
        let gate = Arc::new(Mutex::new(CaptureGate::default()));
        let first = gate.lock().unwrap().suspend();
        let outer = ReleaseOnce::new(gate.clone(), release);
        assert!(outer.get().is_some());
        let outcome = std::panic::catch_unwind(|| {
            let ticket = gate.lock().unwrap().suspend();
            let _inner = ReleaseOnce::new(gate.clone(), release);
            let mut model = gate.lock().unwrap();
            assert_eq!(model.suspended, 2); // A newly starting renderer must hide.
            model.acknowledge(first, false);
            assert!(!model.capture_ready(ticket));
            model.acknowledge(ticket, false);
            assert!(model.capture_ready(ticket));
            drop(model);
            panic!("synthetic cancelled/error capture unwind");
        });
        assert!(outcome.is_err());
        assert_eq!(gate.lock().unwrap().suspended, 1);
        drop(outer);
        assert_eq!(gate.lock().unwrap().suspended, 0);
    }
}
