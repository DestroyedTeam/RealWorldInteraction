//! First-frame grounding and invocation-local native continuation guards.
use super::*;
mod anchor_native;
mod continuation;
mod dispatch;
use anchor_native::AxObject;
use continuation::{Facts, Kind, Sequence};

impl NativeMac {
    pub(super) fn dispatch_input(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        intent: &Intent,
        value: Option<&str>,
    ) -> Result<Option<bool>> {
        if !self.allow_input {
            return Err(fault(
                "input_not_enabled",
                "Start an explicitly authorized candidate with --allow-input.",
            ));
        }
        if target.scope.pid != self.pid || target.scope.window != self.window_name() {
            return Err(fault(
                "target_mismatch",
                "Reference belongs to another native target.",
            ));
        }
        if matches!(intent, Intent::Drag(_)) {
            return Err(fault(
                "capability_unavailable",
                "Native drag is not implemented on this macOS backend; no input dispatched.",
            ));
        }
        let mut sequence = Sequence::new(intent);
        let counter = self.dispatches.clone();
        match intent {
            Intent::Drag(_) => unreachable!("rejected before native reads"),
            Intent::Click | Intent::DoubleClick | Intent::ContextMenu => {
                let right = matches!(intent, Intent::ContextMenu);
                let button = if right {
                    CGMouseButton::Right
                } else {
                    CGMouseButton::Left
                };
                let down = if right {
                    CGEventType::RightMouseDown
                } else {
                    CGEventType::LeftMouseDown
                };
                let up = if right {
                    CGEventType::RightMouseUp
                } else {
                    CGEventType::LeftMouseUp
                };
                for n in 1..=if matches!(intent, Intent::DoubleClick) {
                    2
                } else {
                    1
                } {
                    let press = CGEvent::new_mouse_event(
                        source()?,
                        down,
                        CGPoint::new(target.point[0], target.point[1]),
                        button,
                    )
                    .map_err(|_| {
                        fault("input_allocation_failed", "No mouse down event allocated.")
                    })?;
                    let release = CGEvent::new_mouse_event(
                        source()?,
                        up,
                        CGPoint::new(target.point[0], target.point[1]),
                        button,
                    )
                    .map_err(|_| {
                        fault(
                            "input_allocation_failed",
                            "No mouse release event allocated.",
                        )
                    })?;
                    press.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, n);
                    release.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, n);
                    sequence.pair(
                        expected,
                        target,
                        &counter,
                        (press, release),
                        |first| self.read_input_batch(first, expected, target, intent),
                        |e| e.post(CGEventTapLocation::HID),
                    )?;
                }
            }
            Intent::Text => {
                let text = value.ok_or_else(|| {
                    fault("value_required", "Text intent requires Unicode value.")
                })?;
                for chunk in utf16_chunks(text, 20) {
                    let (down, up) = key_events(0, CGEventFlags::empty(), Some(&chunk))?;
                    sequence.pair(
                        expected,
                        target,
                        &counter,
                        (down, up),
                        |first| self.read_input_batch(first, expected, target, intent),
                        |e| e.post(CGEventTapLocation::HID),
                    )?;
                }
            }
            Intent::KeyChord => {
                let (key, flags) = parse_chord(value.ok_or_else(|| {
                    fault("value_required", "Key chord requires a supported chord.")
                })?)?;
                let (down, up) = key_events(key, flags, None)?;
                sequence.pair(
                    expected,
                    target,
                    &counter,
                    (down, up),
                    |first| self.read_input_batch(first, expected, target, intent),
                    |e| e.post(CGEventTapLocation::HID),
                )?;
            }
            Intent::Scroll => {
                let distance: i32 = value
                    .ok_or_else(|| {
                        fault(
                            "value_required",
                            "Scroll value is signed vertical pixels, -2000..2000.",
                        )
                    })?
                    .parse()
                    .map_err(|_| fault("invalid_scroll", "Expected signed integer pixels."))?;
                if !(-2000..=2000).contains(&distance) || distance == 0 {
                    return Err(fault(
                        "invalid_scroll",
                        "Scroll must be nonzero and bounded to +/-2000 pixels.",
                    ));
                }
                let event =
                    CGEvent::new_scroll_event(source()?, ScrollEventUnit::PIXEL, 1, distance, 0, 0)
                        .map_err(|_| {
                            fault("input_allocation_failed", "No scroll event allocated.")
                        })?;
                unsafe {
                    ffi::CGEventSetLocation(
                        event.as_ptr().cast(),
                        CGPoint::new(target.point[0], target.point[1]),
                    );
                }
                let facts = self.read_input_batch(true, expected, target, intent)?;
                facts.validate(expected, target, Kind::Scroll)?;
                dispatch::single(&counter, || event.post(CGEventTapLocation::HID))?;
            }
        }
        // CGEventPost returns void: native dispatch is not an OS effect acknowledgement.
        Ok(None)
    }
    fn read_input_batch(
        &mut self,
        first: bool,
        expected: &Binding,
        target: &Resolved,
        intent: &Intent,
    ) -> Result<Facts<AxObject>> {
        operation::checkpoint("macos_before_input_batch")?;
        if first {
            let now = self.revalidate(target)?;
            validate_batch_scene(&self.identity, expected, target, &now)?;
        }
        self.input_anchor_facts(target, intent)
    }
}

fn validate_batch_scene(
    identity: &Identity,
    expected: &Binding,
    target: &Resolved,
    now: &Scene,
) -> Result<()> {
    if &now.identity != identity
        || !grounding_matches(expected, now, target)
        || !now.mutation_ready
        || now.blocked != Some(false)
    {
        return Err(fault(
            "stale_reference",
            "Native identity/focus/layout/element/visibility recheck failed immediately before dispatch; no input sent.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
