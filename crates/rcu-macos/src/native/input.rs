//! Native input execution; split from observation without changing behavior.
use super::*;

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
        let now = self.revalidate(target)?;
        if !grounding_matches(expected, &now, target)
            || !now.mutation_ready
            || now.blocked != Some(false)
        {
            return Err(fault(
                "stale_reference",
                "Native recheck failed immediately before dispatch; no input sent.",
            ));
        }
        self.focus_and_hit(
            target,
            matches!(intent, Intent::Text),
            matches!(intent, Intent::Text | Intent::KeyChord),
        )?;
        match intent {
            Intent::Drag(_) => {
                return Err(fault(
                    "capability_unavailable",
                    "Native drag is not implemented on this macOS backend; no input dispatched.",
                ));
            }
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
                    self.focus_and_hit(target, false, false)?;
                    let press = CGEvent::new_mouse_event(
                        source()?,
                        down,
                        CGPoint::new(target.point[0], target.point[1]),
                        button,
                    )
                    .map_err(|_| fault("input_allocation_failed", "No event allocated."))?;
                    let release = CGEvent::new_mouse_event(
                        source()?,
                        up,
                        CGPoint::new(target.point[0], target.point[1]),
                        button,
                    )
                    .map_err(|_| fault("input_allocation_failed", "No event allocated."))?;
                    press.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, n);
                    release.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, n);
                    press.post(CGEventTapLocation::HID);
                    release.post(CGEventTapLocation::HID);
                }
            }
            Intent::Text => {
                let text = value.ok_or_else(|| {
                    fault("value_required", "Text intent requires Unicode value.")
                })?;
                for chunk in utf16_chunks(text, 20) {
                    self.focus_and_hit(target, true, true)?;
                    key_event(0, CGEventFlags::empty(), Some(&chunk))?;
                }
            }
            Intent::KeyChord => {
                let (key, flags) = parse_chord(value.ok_or_else(|| {
                    fault("value_required", "Key chord requires a supported chord.")
                })?)?;
                key_event(key, flags, None)?;
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
                event.post(CGEventTapLocation::HID);
            }
        }
        // CGEventPost returns void: native dispatch is not an OS effect acknowledgement.
        Ok(None)
    }
}
