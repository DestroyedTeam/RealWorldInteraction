//! One bounded, guarded, device-paced background left-button drag.
use super::*;
use rwi_core::input_trajectory::{self, TrajectorySink};
use rwi_core::trajectory::DragPath;
use std::sync::Arc;
use std::time::Instant;

/// Synchronous AX guards cannot sustain a 16 ms grid in real applications.
/// This is a development cadence, not a frame-rate or latency guarantee.
pub(crate) const NATIVE_SAMPLE_INTERVAL_MS: u64 = 64;

struct Sink<'a> {
    native: &'a mut NativeMac,
    delivery: crate::background::Delivery,
    expected: &'a Binding,
    target: &'a Resolved,
    counters: Arc<std::sync::atomic::AtomicU64>,
    started: Instant,
    last_point: [f64; 2],
}

impl TrajectorySink for Sink<'_> {
    fn sample_interval_ms(&self) -> u64 {
        NATIVE_SAMPLE_INTERVAL_MS
    }
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
    fn sleep_ms(&mut self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
    fn validate_point(&mut self, point: [f64; 2]) -> Result<()> {
        if !point.iter().all(|v| v.is_finite()) || !self.delivery.frame.contains(point) {
            return Err(fault(
                "drag_point_outside_window",
                "Every drag sample must stay inside the pinned window frame; no input.",
            ));
        }
        Ok(())
    }
    fn guard_point(&mut self, point: [f64; 2], held: bool) -> Result<()> {
        operation::checkpoint(if held {
            "macos_drag_guard_held"
        } else {
            "macos_drag_guard_idle"
        })?;
        if !held {
            // First input of this invocation: full grounding like any first pair.
            let facts = self.native.drag_anchor_facts(self.target)?;
            facts.validate(self.expected, self.target, Kind::Pointer)?;
            return Ok(());
        }
        // Held guards stay cheap and bounded: identity, process, exact window
        // and point containment only; no AX tree reads inside the schedule.
        let identity = console_identity()?;
        if identity != self.native.identity
            || process_epoch(self.native.pid)? != self.native.process
        {
            return Err(fault(
                "session_or_process_changed",
                "Native context or process changed mid-drag; stop and release.",
            ));
        }
        let window = windows()?
            .into_iter()
            .find(|w| w.id == self.native.window && w.pid == self.native.pid)
            .ok_or_else(|| {
                fault(
                    "window_binding_changed",
                    "The pinned window disappeared mid-drag; stop and release.",
                )
            })?;
        if !window.rect.same(self.delivery.frame) || !self.delivery.frame.contains(point) {
            return Err(fault(
                "drag_binding_changed",
                "Window bounds or drag point changed mid-drag; stop and release.",
            ));
        }
        Ok(())
    }
    fn press_left(&mut self, point: [f64; 2]) -> Result<()> {
        let event = CGEvent::new_mouse_event(
            source()?,
            CGEventType::LeftMouseDown,
            CGPoint::new(point[0], point[1]),
            CGMouseButton::Left,
        )
        .map_err(|_| fault("input_allocation_failed", "No mouse down event allocated."))?;
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 1);
        dispatch::count(&self.counters);
        crate::background::deliver_pointer(&event, &self.delivery, point);
        self.last_point = point;
        Ok(())
    }
    fn move_held(&mut self, point: [f64; 2]) -> Result<()> {
        let event = CGEvent::new_mouse_event(
            source()?,
            CGEventType::LeftMouseDragged,
            CGPoint::new(point[0], point[1]),
            CGMouseButton::Left,
        )
        .map_err(|_| fault("input_allocation_failed", "No drag event allocated."))?;
        dispatch::count(&self.counters);
        crate::background::deliver_pointer(&event, &self.delivery, point);
        self.last_point = point;
        Ok(())
    }
    fn release_left(&mut self) -> Result<()> {
        let event = CGEvent::new_mouse_event(
            source()?,
            CGEventType::LeftMouseUp,
            CGPoint::new(self.last_point[0], self.last_point[1]),
            CGMouseButton::Left,
        )
        .map_err(|_| {
            fault(
                "input_allocation_failed",
                "No mouse release event allocated.",
            )
        })?;
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 1);
        dispatch::count(&self.counters);
        crate::background::deliver_pointer(&event, &self.delivery, self.last_point);
        Ok(())
    }
}

impl NativeMac {
    pub(super) fn dispatch_drag(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        path: &DragPath,
    ) -> Result<()> {
        let delivery = crate::background::Delivery {
            pid: self.pid,
            window: self.window,
            frame: self.window_frame()?,
        };
        if !crate::background::pointer_delivery_available() {
            return Err(fault(
                "background_delivery_unavailable",
                "Window-local background event addressing is unavailable on this macOS build; no input dispatched.",
            ));
        }
        let counters = self.dispatches.clone();
        let mut sink = Sink {
            native: self,
            delivery,
            expected,
            target,
            counters,
            started: Instant::now(),
            last_point: target.point,
        };
        let stats = input_trajectory::execute(path, target.point, &mut sink)?;
        let _ = stats;
        Ok(())
    }
}
