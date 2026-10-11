//! One addressed MouseMoved event; no button ownership or inferred hover effect.
use super::*;
use std::sync::atomic::AtomicU64;

pub(super) fn event(point: [f64; 2]) -> Result<CGEvent> {
    if !point.iter().all(|coordinate| coordinate.is_finite()) {
        return Err(fault(
            "invalid_pointer_point",
            "A finite grounded pointer point is required; no move allocated.",
        ));
    }
    let event = CGEvent::new_mouse_event(
        source()?,
        CGEventType::MouseMoved,
        CGPoint::new(point[0], point[1]),
        CGMouseButton::Left,
    )
    .map_err(|_| {
        fault(
            "input_allocation_failed",
            "No pointer move event allocated.",
        )
    })?;
    event.set_flags(CGEventFlags::empty());
    event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 0);
    event.set_double_value_field(EventField::MOUSE_EVENT_PRESSURE, 0.0);
    // Invocation-local history is unknown. A single grounded point cannot prove
    // a preceding attempted/received location or a physical cursor displacement.
    // Zero deltas encode that absence, not an assertion that no movement occurred.
    event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, 0);
    event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y, 0);
    // kCGMouseEventNumber correlates mouse button down/up; it is not a move ID.
    // Do not set/read it or synthesize a press/up to manufacture tracking state.
    Ok(event)
}

/// Guard through the same exact native readback used by other pointer intents.
/// A blocking native read may observe cancellation or lease expiry; recheck on
/// entering the counted native scope. CGEventPostToPid returns void, not receipt.
pub(super) fn guarded_single(
    counter: &AtomicU64,
    guard: impl FnOnce() -> Result<()>,
    post: impl FnOnce(),
) -> Result<()> {
    operation::checkpoint("macos_move_pointer_guard")?;
    guard()?;
    let _call = operation::native_call("macos_move_pointer_event")?;
    dispatch::count(counter);
    post();
    Ok(())
}

#[cfg(test)]
mod tests;
