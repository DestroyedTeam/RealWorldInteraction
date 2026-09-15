//! First-frame grounding and invocation-local native continuation guards.
use super::*;
mod anchor_native;
pub(crate) mod drag;
pub(crate) use drag::NATIVE_SAMPLE_INTERVAL_MS;
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
        // Yield only for a real conflict: a person actively using *this* app.
        // Background delivery to a non-frontmost window never races the person's
        // typing in another app, which is exactly what background operation is.
        if (self.foreground_pid() == Some(self.pid) || self.window_is_main())
            && let Some(seconds) = crate::activity::user_active(1.2)
        {
            return Err(fault(
                "user_active",
                &format!(
                    "A person is using this window (clicked or typed {seconds:.2}s ago); RCU yields and never queues input."
                ),
            ));
        }
        if target.scope.pid != self.pid || target.scope.window != self.window_name() {
            return Err(fault(
                "target_mismatch",
                "Reference belongs to another native target.",
            ));
        }
        let mut sequence = Sequence::new(intent);
        let counter = self.dispatches.clone();
        let delivery = crate::background::Delivery {
            pid: self.pid,
            window: self.window,
            frame: self.window_frame()?,
        };
        let pointer = matches!(
            intent,
            Intent::Click
                | Intent::DoubleClick
                | Intent::ContextMenu
                | Intent::Scroll
                | Intent::Drag(_)
        );
        // Sheet/panel windows that the application does not list in AXWindows
        // cannot be reached by background synthetic events (the click
        // dispatches but never activates); they accept the native
        // accessibility press action instead, with the same grounded element
        // validation and no synthetic primer.
        let native_press = matches!(intent, Intent::Click) && self.native_press_required(target)?;
        // A click value that names an advertised native action performs that
        // action instead of a synthetic click (the element's own action list
        // is the whitelist, so no schema change is needed).
        let explicit_action = match intent {
            Intent::Click if target.scope.element.is_some() => value,
            _ => None,
        };
        if pointer && !crate::background::pointer_delivery_available() {
            return Err(fault(
                "background_delivery_unavailable",
                "Window-local background event addressing is unavailable on this macOS build; no input dispatched.",
            ));
        }
        if !native_press
            && self.foreground_pid() != Some(self.pid)
            && !self.window_is_main()
            && crate::background::activate_background_window(self.pid, self.window)
        {
            std::thread::sleep(std::time::Duration::from_millis(30));
            if pointer {
                let alternate = self.dispatches.load(std::sync::atomic::Ordering::Acquire) % 2 == 1;
                crate::background::primer_click(&delivery, alternate);
            }
        }
        match intent {
            Intent::Drag(path) => {
                self.dispatch_drag(expected, target, path)?;
            }
            Intent::Click | Intent::DoubleClick | Intent::ContextMenu => {
                if let Some(action) = explicit_action {
                    return self.dispatch_native_action(expected, target, action);
                }
                if native_press {
                    return self.dispatch_native_action(expected, target, "AXPress");
                }
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
                        |e| crate::background::deliver_pointer(e, &delivery, target.point),
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
                        |e| crate::background::deliver_key(e, &delivery),
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
                    |e| crate::background::deliver_key(e, &delivery),
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
                dispatch::single(&counter, || {
                    crate::background::deliver_pointer(&event, &delivery, target.point)
                })?;
            }
        }
        // CGEventPost returns void: native dispatch is not an OS effect acknowledgement.
        Ok(None)
    }
    fn native_press_required(&self, target: &Resolved) -> Result<bool> {
        if target.scope.element.is_none() {
            return Ok(false);
        }
        let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 0.25);
        }
        let value = application.attr("AXWindows")?;
        let windows = unsafe { ffi::array(value.0, 64)? };
        if !windows
            .iter()
            .any(|w| ffi::window_number_for_ax_element(w) == Some(self.window))
        {
            return Ok(true);
        }
        // A window that is present in AXWindows can still be a sheet/panel that
        // synthetic background clicks do not reach (observed live: an NSOpenPanel
        // accepted the click with dispatch=returned and nothing happened). Any
        // window that is not the application's main window therefore takes the
        // native accessibility action instead.
        let main = application
            .attr("AXMainWindow")
            .ok()
            .and_then(|element| ffi::window_number_for_ax_element(&element));
        Ok(main.is_some_and(|number| number != self.window))
    }
    fn dispatch_native_action(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        action_name: &str,
    ) -> Result<Option<bool>> {
        let facts = self.drag_anchor_facts(target)?;
        facts.validate(expected, target, Kind::Pointer)?;
        if !crate::native::action_names(facts.hit.native())
            .iter()
            .any(|name| name == action_name)
        {
            return Err(fault(
                "unsupported_action",
                "The exact element does not advertise this native action; nothing was dispatched.",
            ));
        }
        use core_foundation::base::TCFType;
        let action = core_foundation::string::CFString::new(action_name);
        let _call = operation::native_call("macos_native_action")?;
        dispatch::count(&self.dispatches);
        let code = unsafe {
            ffi::AXUIElementPerformAction(facts.hit.native().0, action.as_concrete_TypeRef().cast())
        };
        if code != 0 {
            let (code_name, message) = if action_name == "AXPress" {
                (
                    "native_press_rejected",
                    "The native accessibility press action was refused by the application; no synthetic input was sent.",
                )
            } else {
                (
                    "native_action_rejected",
                    "The native accessibility action was refused by the application; no synthetic input was sent.",
                )
            };
            return Err(fault(code_name, message));
        }
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
        self.input_anchor_facts(target, Kind::of(intent))
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
