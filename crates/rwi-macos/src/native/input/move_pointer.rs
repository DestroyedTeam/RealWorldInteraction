//! One addressed MouseMoved event; no button ownership or inferred hover effect.
use super::*;
use std::sync::atomic::AtomicU64;

pub(super) fn event(point: [f64; 2]) -> Result<CGEvent> {
    validate_point(point)?;
    event_with_source(point, source()?)
}

fn validate_point(point: [f64; 2]) -> Result<()> {
    if !point.iter().all(|coordinate| coordinate.is_finite()) {
        return Err(fault(
            "invalid_pointer_point",
            "A finite grounded pointer point is required; no move allocated.",
        ));
    }
    Ok(())
}

fn event_with_source(point: [f64; 2], source: CGEventSource) -> Result<CGEvent> {
    let event = CGEvent::new_mouse_event(
        source,
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

/// Select construction only, never transport, focus or receiving policy.
pub(super) fn event_for(
    point: [f64; 2],
    delivery: &crate::background::Delivery,
    options: &rwi_core::MovePointerOptions,
) -> Result<CGEvent> {
    match options {
        rwi_core::MovePointerOptions::BackgroundAppKitExperimental { delta: Some(delta) } => {
            rwi_core::MovePointerOptions::validate_explicit_delta(*delta)?;
            let event = appkit_event(point, delivery)?;
            crate::background::appkit_move::apply_explicit_delta(&event, *delta)?;
            Ok(event)
        }
        rwi_core::MovePointerOptions::BackgroundAppKitExperimental { delta: None } => {
            appkit_event(point, delivery)
        }
        rwi_core::MovePointerOptions::BackgroundAppKitTrackingExperimental {
            phase,
            tracking_number,
        } => {
            validate_point(point)?;
            if !crate::background::appkit_tracking::available() {
                return Err(fault(
                    "capability_unavailable",
                    "Registered legacy tracking Native experiment is unavailable; no fallback.",
                ));
            }
            let source = source()?;
            let original = event_with_source(point, source.clone())?;
            crate::background::appkit_tracking::construct(
                &original,
                &source,
                delivery,
                point,
                *phase,
                tracking_number,
            )
        }
        rwi_core::MovePointerOptions::BackgroundCooperativeTrackingExperimental { .. } => {
            Err(fault(
                "cooperative_owner_dispatch_required",
                "Cooperative tracking cannot fall back to any CGEvent carrier or foreign tag post.",
            ))
        }
        rwi_core::MovePointerOptions::BackgroundWindow {}
        | rwi_core::MovePointerOptions::BackgroundSkylightExperimental {}
        | rwi_core::MovePointerOptions::ForegroundSystem { .. } => event(point),
    }
}

/// Construction only. One invocation-private source is shared by the original
/// Quartz template and the independent factory-derived copy, never a HID table.
pub(super) fn appkit_event(
    point: [f64; 2],
    delivery: &crate::background::Delivery,
) -> Result<CGEvent> {
    validate_point(point)?;
    let source = source()?;
    let original = event_with_source(point, source.clone())?;
    crate::background::appkit_move::construct(&original, &source, delivery, point)
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
