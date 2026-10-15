//! Public-Quartz gesture construction; never posts events or warps the cursor.
use super::*;
use std::sync::atomic::{AtomicU32, Ordering};

// Invocation-local event correlation only, not a durable reservation or receipt.
// Never recycle a process-local button number after exhaustion.
static NEXT_EVENT_NUMBER: AtomicU32 = AtomicU32::new(1);
fn next_event_number() -> Result<i64> {
    NEXT_EVENT_NUMBER
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
            v.checked_add(1).filter(|next| *next <= i32::MAX as u32)
        })
        .map(i64::from)
        .map_err(|_| {
            fault(
                "mouse_event_number_exhausted",
                "No mouse event number available; no input.",
            )
        })
}

/// Exactly one Private source is retained across one click/double-click/drag.
/// Each CG constructor consumes a retained clone of that same native source.
/// Owned CGEvent/CGEventSource wrappers release their CF references on all exits.
pub(super) struct MouseGesture {
    source: CGEventSource,
    button: CGMouseButton,
    event_number: i64,
    last_point: [f64; 2],
    delta_remainder: [f64; 2],
    click_pair_built: bool,
}
pub(super) struct Motion {
    point: [f64; 2],
    remainder: [f64; 2],
}

impl MouseGesture {
    pub(super) fn new(point: [f64; 2], button: CGMouseButton) -> Result<Self> {
        if !point.iter().all(|v| v.is_finite())
            || !matches!(button, CGMouseButton::Left | CGMouseButton::Right)
        {
            return Err(fault(
                "invalid_mouse_gesture",
                "A finite point and supported button are required.",
            ));
        }
        Ok(Self {
            source: source()?,
            button,
            event_number: next_event_number()?,
            last_point: point,
            delta_remainder: [0.0; 2],
            click_pair_built: false,
        })
    }
    fn event(
        &self,
        kind: CGEventType,
        point: [f64; 2],
        click_count: i64,
        pressure: f64,
        delta: [i64; 2],
    ) -> Result<CGEvent> {
        let event = CGEvent::new_mouse_event(
            self.source.clone(),
            kind,
            CGPoint::new(point[0], point[1]),
            self.button,
        )
        .map_err(|_| {
            fault(
                "input_allocation_failed",
                "No gesture mouse event allocated.",
            )
        })?;
        // Constructor defaults were probed independently: delta and eventNumber
        // are zero; up clickCount is zero. Do not infer a sequence from them.
        event.set_flags(CGEventFlags::empty());
        event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, self.button as i64);
        event.set_integer_value_field(EventField::MOUSE_EVENT_NUMBER, self.event_number);
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_count);
        event.set_double_value_field(EventField::MOUSE_EVENT_PRESSURE, pressure);
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, delta[0]);
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y, delta[1]);
        Ok(event)
    }
    fn button_types(&self) -> (CGEventType, CGEventType, CGEventType) {
        if matches!(self.button, CGMouseButton::Right) {
            (
                CGEventType::RightMouseDown,
                CGEventType::RightMouseDragged,
                CGEventType::RightMouseUp,
            )
        } else {
            (
                CGEventType::LeftMouseDown,
                CGEventType::LeftMouseDragged,
                CGEventType::LeftMouseUp,
            )
        }
    }
    /// Borrow the exact invocation-owned Private source; never replace its table.
    pub(super) fn source(&self) -> &CGEventSource {
        &self.source
    }
    pub(super) fn tracking_event(&self) -> Result<CGEvent> {
        self.event(CGEventType::MouseMoved, self.last_point, 0, 0.0, [0; 2])
    }
    pub(super) fn down_event(&self) -> Result<CGEvent> {
        self.event(self.button_types().0, self.last_point, 1, 1.0, [0; 2])
    }
    pub(super) fn click_pair(&mut self, click_count: i64) -> Result<(CGEvent, CGEvent)> {
        if !(1..=2).contains(&click_count) {
            return Err(fault(
                "invalid_mouse_click_count",
                "Only a bounded click/double-click pair is supported.",
            ));
        }
        if self.click_pair_built {
            self.event_number = next_event_number()?;
        }
        self.click_pair_built = true;
        let (down, _, up) = self.button_types();
        // Allocate the release before the guarded down/up pair, as before.
        Ok((
            self.event(down, self.last_point, click_count, 1.0, [0; 2])?,
            self.event(up, self.last_point, click_count, 0.0, [0; 2])?,
        ))
    }
    pub(super) fn motion_event(&self, point: [f64; 2]) -> Result<(CGEvent, Motion)> {
        let mut delta = [0; 2];
        let mut remainder = [0.0; 2];
        for axis in 0..2 {
            let relative = point[axis] - self.last_point[axis] + self.delta_remainder[axis];
            if !point[axis].is_finite()
                || !relative.is_finite()
                || relative.abs() > f64::from(i32::MAX)
            {
                return Err(fault(
                    "mouse_delta_out_of_range",
                    "A finite bounded gesture delta is required; no move allocated.",
                ));
            }
            // CG mouse deltas are public signed integer displacement fields.
            // This uses Quartz point displacement; receiver/DPI calibration
            // is separate evidence, never an assumed physical-pixel scale.
            // Carry subpixel remainder rather than discarding every small step;
            // the absolute event location keeps the caller's exact float point.
            delta[axis] = relative.round() as i64;
            remainder[axis] = relative - delta[axis] as f64;
        }
        let event = self.event(self.button_types().1, point, 1, 1.0, delta)?;
        Ok((event, Motion { point, remainder }))
    }
    pub(super) fn commit_motion(&mut self, motion: Motion) {
        // Commit only after the caller attempts delivery. This tracks the last
        // attempted point, not OS insertion, target receipt or goal success.
        self.last_point = motion.point;
        self.delta_remainder = motion.remainder;
    }
    pub(super) fn last_point(&self) -> [f64; 2] {
        self.last_point
    }
    pub(super) fn release_event(&self) -> Result<CGEvent> {
        // Release-only: no source replacement, cancellation gate, move or down.
        self.event(self.button_types().2, self.last_point, 1, 0.0, [0; 2])
    }
}

#[cfg(test)]
mod tests;
