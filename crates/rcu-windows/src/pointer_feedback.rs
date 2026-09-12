//! Service-owned feedback, separate from input authority and grounding pixels.
mod material;
mod motion;
#[cfg(windows)]
mod native;
#[cfg(windows)]
pub(crate) use native::{Gesture, start, status, stop, suspend_capture};

pub(super) const SIZE: usize = 160;
pub(super) const CENTER: usize = SIZE / 2;
pub(super) const MAX_DPI: u32 = 288;
pub(super) const HIGHLIGHT_MS: u64 = 800; // Activity validity, not idle lifetime.
pub(super) fn normalize_dpi(dpi: u32) -> u32 {
    if dpi == 0 { 96 } else { dpi.clamp(96, MAX_DPI) }
}
pub(super) fn scale_for_dpi(dpi: u32) -> f64 {
    f64::from(normalize_dpi(dpi)) / 96.
}
pub(super) fn raster_size(dpi: u32) -> usize {
    2 * (CENTER as f64 * scale_for_dpi(dpi)).ceil() as usize
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Phase {
    Idle,
    Pointer,
    Pressed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub(super) struct Anchor {
    pub point: [i32; 2],
    pub dpi: u32,
}
impl Anchor {
    pub fn with_dpi(self, dpi: u32) -> Self {
        Self {
            dpi: normalize_dpi(dpi),
            ..self
        }
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
}
