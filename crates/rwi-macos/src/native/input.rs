//! First-frame grounding and invocation-local native continuation guards.
use super::*;
mod anchor_native;
pub(crate) mod drag;
mod drag_between;
pub(crate) use drag::NATIVE_SAMPLE_INTERVAL_MS;
mod continuation;
mod dispatch;
mod mouse_gesture;
mod request;
use request::native_action_request;
mod pointer_surface;
use pointer_surface::native_pointer_requires_axpress;
mod move_pointer;
use anchor_native::AxObject;
use continuation::{Facts, Kind, Sequence};
use mouse_gesture::MouseGesture;
mod foreground_move;

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
        let requested_native_action =
            native_action_request(intent, target.native_action.as_deref(), value)?;
        let click_options = match intent {
            Intent::Click => rwi_core::ClickDeliveryOptions::from_click_value(value)?,
            _ => None,
        };
        let experimental_click = click_options.is_some();
        let move_options = if matches!(intent, Intent::MovePointer) {
            rwi_core::MovePointerOptions::parse(value)?
        } else {
            rwi_core::MovePointerOptions::BackgroundWindow {}
        };
        let foreground_pointer = move_options.requires_foreground();
        let cooperative_pointer = matches!(
            move_options,
            rwi_core::MovePointerOptions::BackgroundCooperativeTrackingExperimental { .. }
        );
        // The global combined-session timer cannot prove a human or receiving
        // window. Scope a possible conflict to native foreground/focused identity.
        // Only an explicit native semantic action can use exact sibling isolation.
        // Synthetic delivery may prepare a shared app receiver: retain PID yield.
        if let Some(seconds) = crate::activity::user_active(1.2) {
            let (foreground_pid, focused_window) = self.foreground_input_window();
            if crate::activity::same_input_resource(
                self.pid,
                self.window,
                foreground_pid,
                focused_window,
                target.native_action.is_some(),
            ) {
                let mut error = fault(
                    "user_active",
                    &format!(
                        "Recent session input ({seconds:.2}s) may conflict with the selected foreground resource; RWI yields without queuing input."
                    ),
                );
                error.diagnostic = Some(Box::new(serde_json::json!({
                    "scope":"foreground_native_window_when_known",
                    "selected_window":self.window_name(), "focused_window":focused_window,
                    "window_identity_known":focused_window.is_some(),
                    "native_window_isolated":target.native_action.is_some(),
                    "combined_session_recent_seconds":seconds,
                    "hid_system_recent_seconds":crate::activity::seconds_since_hid_input(),
                    "human_attested":false,"event_source_class":"unknown",
                    "native_input_dispatched":false,"replay_allowed":false
                })));
                return Err(error);
            }
        }
        if target.scope.pid != self.pid || target.scope.window != self.window_name() {
            return Err(fault(
                "target_mismatch",
                "Reference belongs to another native target.",
            ));
        }
        require_surface_action(self.semantic_only, intent, target.native_action.as_deref())?;
        // Explicit actions retain their marked reference and fresh advertisement checks.
        // Do not make their authority depend on ordinary-pointer surface classification.
        if let Some(action) = requested_native_action {
            return self.dispatch_native_action(expected, target, action);
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
                | Intent::MovePointer
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
        if experimental_click && native_press {
            return Err(fault(
                "experimental_click_pointer_required",
                "This target requires an advertised native action; experimental pointer delivery cannot substitute AXPress or fall back.",
            ));
        }
        if pointer && !native_press {
            validate_capture_fringe(delivery.frame, target.point, intent)?;
        }
        let background_pointer = pointer && !foreground_pointer && !cooperative_pointer;
        if background_pointer && !crate::background::pointer_delivery_available() {
            return Err(fault(
                "background_delivery_unavailable",
                "Window-local background event addressing is unavailable on this macOS build; no input dispatched.",
            ));
        }
        // Validate keyboard targets and payloads before the AppKit activation
        // event. Otherwise a rejected field has already incremented the effect
        // counter and is correctly (but unnecessarily) reported as uncertain.
        // The sequence still revalidates immediately before every down/up pair.
        if matches!(intent, Intent::Text | Intent::KeyChord) {
            validate_keyboard_payload(intent, value)?;
            self.read_input_batch(true, expected, target, intent)?
                .validate(expected, target, Kind::of(intent))?;
        }
        // The local animated-pixel exception is valid only on a native bitmap
        // canvas. Confirm the exact AX hit before even a background AppKit
        // activation, then repeat the check at dispatch.
        if target.pixel_anchor_used {
            self.input_anchor_facts(target, Kind::of(intent))?
                .validate(expected, target, Kind::of(intent))?;
        }
        // Pointer input is exactly the requested addressed sequence. Do not
        // manufacture an AppKit application-activated notification first: it
        // changes receiving active/key/order state before even the first down.
        // This does not promise that an inactive client consumes pointer input.
        // Keyboard compatibility retains its separately validated preparation.
        let receiver_prepared = !experimental_click
            && !matches!(intent, Intent::MovePointer)
            && needs_background_activation(
                intent,
                native_press,
                false,
                self.foreground_pid(),
                self.pid,
            )
            // AXMain can stay true after the application loses activation.
            // It is not evidence that AppKit will deliver the first mouse-down.
            && crate::background::activate_background_window(self.pid, self.window);
        if receiver_prepared {
            dispatch::count(&counter);
            std::thread::sleep(std::time::Duration::from_millis(30));
            // Do not prime with an unrelated mouse click. A titlebar can
            // contain toolbar/accessory controls (or the whole content view),
            // so no hard-coded point is a neutral activation target. The only
            // pointer sequence below is the caller's grounded intent; an app
            // ignoring its first mouse still requires independent readback.
        }
        // Preparation is an application effect but never holds a button/key.
        // Keep it in the effect counter; prove separately whether the caller's
        // requested event sequence was reached before a returned failure.
        let requested_input_before = counter.load(std::sync::atomic::Ordering::Acquire);
        let result = (|| {
            match intent {
                Intent::DragBetween(_) => {
                    return Err(fault(
                        "paired_dispatch_required",
                        "Paired input must pass both endpoint guards, never ordinary act.",
                    ));
                }
                Intent::MovePointer => {
                    if let rwi_core::MovePointerOptions::BackgroundCooperativeTrackingExperimental { phase, owner_endpoint, registration_id } = &move_options {
                        let title=expected.mutation_context.as_ref().map(|c|c.window_title.as_str())
                            .ok_or_else(||fault("cooperative_context_unknown","Exact native window title/epoch context is required."))?;
                        let frame=[delivery.frame.x,delivery.frame.y,delivery.frame.w,delivery.frame.h];
                        let client=crate::cooperative_tracking::client::Client::prepare(owner_endpoint,registration_id,
                            crate::cooperative_tracking::client::ExpectedNativeTarget {
                                pid:self.pid,birth:&self.process,window:self.window,frame,title,
                                native_epoch:&expected.native_epoch,
                            })?;
                        client.matches_context(self.window,frame,title,&expected.native_epoch)?;
                        let local=crate::background::window_local(target.point,delivery.frame);
                        let base=[local[0],delivery.frame.h-local[1]];
                        client.execute(*phase,base,|| {
                            // AFTER final socket connection/peer/request construction,
                            // immediately BEFORE its first mutation byte is written.
                            self.read_input_batch(true,expected,target,intent)?
                                .validate(expected,target,Kind::Pointer)?;
                            let call=operation::native_call("macos_cooperative_tracking_owner_once")?;
                            dispatch::count(&counter);
                            Ok(call)
                        })?;
                    } else {
                    let event = move_pointer::event_for(target.point, &delivery, &move_options)?;
                    let skylight_move = if matches!(
                        move_options,
                        rwi_core::MovePointerOptions::BackgroundSkylightExperimental {}
                    ) {
                        Some(crate::background::skylight::prepare_pointer(
                            &event,
                            &delivery,
                            target.point,
                        )?)
                    } else {
                        None
                    };
                    move_pointer::guarded_single(
                        &counter,
                        || {
                            self.read_input_batch(true, expected, target, intent)?
                                .validate(expected, target, Kind::Pointer)?;
                            if let rwi_core::MovePointerOptions::ForegroundSystem {
                                expected_foreground: foreground,
                            } = &move_options
                            {
                                foreground_move::guard(self, foreground, target.point)?;
                            }
                            Ok(())
                        },
                        || {
                            if foreground_pointer {
                                foreground_move::post(&event)
                            } else if let Some(prepared) = skylight_move {
                                prepared.post()
                            } else {
                                crate::background::deliver_pointer(&event, &delivery, target.point)
                            }
                        },
                    )?;
                    }
                }
                Intent::Drag(path) => {
                    self.dispatch_drag(expected, target, path)?;
                }
                Intent::Click | Intent::DoubleClick | Intent::ContextMenu => {
                    if native_press {
                        return self.dispatch_native_action(expected, target, "AXPress");
                    }
                    let right = matches!(intent, Intent::ContextMenu);
                    let button = if right {
                        CGMouseButton::Right
                    } else {
                        CGMouseButton::Left
                    };
                    let mut gesture = MouseGesture::new(target.point, button)?;
                    for n in 1..=if matches!(intent, Intent::DoubleClick) {
                        2
                    } else {
                        1
                    } {
                        let (press, release) = gesture.click_pair(n)?;
                        if let Some(options) = &click_options {
                            let pair = crate::background::skylight::prepare_selected_click_pair(
                                &press,
                                &release,
                                gesture.source(),
                                &delivery,
                                target.point,
                                options,
                            )?;
                            // Same native/semantic/context/lease checks and counted
                            // pair custody. Up is cleanup of this exact down; no
                            // checkpoint, activation or fallback between them.
                            sequence.pair(
                                expected,
                                target,
                                &counter,
                                pair,
                                |first| self.read_input_batch(first, expected, target, intent),
                                |prepared| prepared.post(),
                            )?;
                        } else {
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
                }
                Intent::Text => {
                    let text = value.ok_or_else(|| {
                        fault("value_required", "Text intent requires Unicode value.")
                    })?;
                    for chunk in utf16_scalar_chunks(text) {
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
                    let event = CGEvent::new_scroll_event(
                        source()?,
                        ScrollEventUnit::PIXEL,
                        1,
                        distance,
                        0,
                        0,
                    )
                    .map_err(|_| fault("input_allocation_failed", "No scroll event allocated."))?;
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
        })();
        result.map_err(|error| {
            dispatch::preparation_only_failure(
                error,
                receiver_prepared,
                requested_input_before,
                counter.load(std::sync::atomic::Ordering::Acquire),
            )
        })
    }
    fn native_press_required(&self, target: &Resolved) -> Result<bool> {
        if target.scope.element.is_none() {
            // Existing bitmap-coordinate path keeps its independent native guards.
            return Ok(false);
        }
        let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 0.25);
        }
        // Resolve this exact pinned window, including actual sheets, without using
        // AXMainWindow as a proxy for its kind or changing focus/activation.
        let window = self.resolve_window_element(&application, self.window_record()?.rect)?;
        let role = window.attr("AXRole").ok().and_then(|v| v.string().ok());
        let subrole = window.attr("AXSubrole").ok().and_then(|v| v.string().ok());
        let modal = window.attr("AXModal").ok().and_then(|v| v.boolean());
        native_pointer_requires_axpress(role.as_deref(), subrole.as_deref(), modal)
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
        let actual_pid = facts.hit.native().pid()?;
        let owner_epoch = super::delegation::validate_hosted_descendant(self.pid, actual_pid)?;
        if process_epoch(actual_pid)? != owner_epoch {
            return Err(fault(
                "delegate_process_changed",
                "Native action provider changed before dispatch.",
            ));
        }
        let _call = operation::native_call("macos_native_action")?;
        // Action callbacks may redraw or enter modal processing. Give this exact
        // object a bounded action budget, independent of short metadata probes.
        // This configures one request; it never retries a dispatched action.
        let timeout_code =
            unsafe { ffi::AXUIElementSetMessagingTimeout(facts.hit.native().0, 0.5) };
        if timeout_code != 0 {
            return Err(fault(
                "native_action_timeout_configuration_failed",
                &format!(
                    "Cannot configure exact AX action timeout: AXError {timeout_code}; no action dispatched."
                ),
            ));
        }
        dispatch::count(&self.dispatches);
        let code = unsafe {
            ffi::AXUIElementPerformAction(facts.hit.native().0, action.as_concrete_TypeRef().cast())
        };
        if code != 0 {
            return Err(native_action_failure(action_name, code));
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
        let mut scoped = target.clone();
        if first {
            let now = self.revalidate(target)?;
            validate_batch_scene(&self.identity, expected, target, &now)?;
            scoped.pixel_anchor_used |= (scoped.pixel_anchor.is_some()
                || scoped.red_marker_anchor.is_some())
                && now.binding.as_ref().map(|binding| &binding.content) != Some(&expected.content);
        }
        self.input_anchor_facts_in_sequence(
            &scoped,
            Kind::of(intent),
            !first && matches!(intent, Intent::Text),
        )
    }
}
fn require_surface_action(
    semantic_only: bool,
    intent: &Intent,
    native_action: Option<&str>,
) -> Result<()> {
    if semantic_only && (!matches!(intent, Intent::Click) || native_action.is_none()) {
        return Err(fault(
            "semantic_action_required",
            "A logical AX dialog accepts only an explicit advertised native action; no pixel, keyboard or synthetic-window substitution.",
        ));
    }
    Ok(())
}

// The capture fingerprint excludes the compositor's bottom one-point fringe.
// Keep synthetic clicks/scrolls and the entire drag path out of that region;
// exact advertised AX actions retain their separate native-element grounding.
fn validate_capture_fringe(frame: Rect, point: [f64; 2], intent: &Intent) -> Result<()> {
    let allowed = |p: [f64; 2]| frame.contains(p) && p[1] < frame.y + frame.h - 1.0;
    let valid = allowed(point)
        && match intent {
            Intent::Drag(path) => path
                .waypoints
                .iter()
                .all(|w| allowed([point[0] + w.offset[0], point[1] + w.offset[1]])),
            _ => true,
        };
    if valid {
        Ok(())
    } else {
        Err(fault(
            "capture_fringe_not_grounded",
            "Synthetic pointer input must stay inside the window and above its excluded one-point compositor border; no input sent.",
        ))
    }
}

fn validate_batch_scene(
    identity: &Identity,
    expected: &Binding,
    target: &Resolved,
    now: &Scene,
) -> Result<()> {
    // Native identity is outside Binding. Do not compare pixel anchors in a
    // replaced native session merely to build a diagnostic. Semantic grounding
    // retains its prior diagnostic value while the identity guard still rejects.
    let pixel_identity_changed = &now.identity != identity && target.scope.element.is_none();
    let (grounded, local_pixel_diagnostic, red_marker_diagnostic) = if pixel_identity_changed {
        (false, None, None)
    } else {
        rwi_core::evaluate_grounding(expected, now, target)
    };
    if &now.identity != identity || !grounded || !now.mutation_ready || now.blocked != Some(false) {
        let mut error = fault(
            "stale_reference",
            "The first requested input batch failed its final native recheck; inspect fresh state, never replay.",
        );
        let mut diagnostic = match &now.binding {
            Some(binding) => serde_json::json!({
                "scope_changed": expected.scope != binding.scope,
                "focus_changed": expected.focus_window != binding.focus_window,
                "layout_changed": expected.layout != binding.layout,
                "native_epoch_changed": expected.native_epoch != binding.native_epoch,
                "mutation_context_changed": expected.mutation_context != binding.mutation_context,
                "content_changed": expected.content != binding.content
            }),
            None => serde_json::json!({"binding_missing": true}),
        };
        diagnostic["stage"] = serde_json::json!("macos_first_input_batch");
        diagnostic["identity_changed"] = serde_json::json!(&now.identity != identity);
        diagnostic["grounding_matches"] = serde_json::json!(grounded);
        diagnostic["reference_kind"] = serde_json::json!(if target.scope.element.is_some() {
            "element"
        } else {
            "pixel"
        });
        diagnostic["local_pixel_anchor_available"] =
            serde_json::json!(target.pixel_anchor.is_some());
        diagnostic["mutation_ready"] = serde_json::json!(now.mutation_ready);
        diagnostic["blocked"] = serde_json::json!(now.blocked);
        if !grounded && target.scope.element.is_none() {
            // Reuse the comparator result for this exact fresh scene. A missing
            // comparison is unknown/skipped, never a local-pixel match or ticket.
            let anchor_comparison_skipped = local_pixel_diagnostic.is_none()
                && red_marker_diagnostic.is_none()
                && (target.pixel_anchor.is_some() || target.red_marker_anchor.is_some());
            if let Some(local) = local_pixel_diagnostic {
                diagnostic["local_pixel_anchor"] = local;
            }
            if let Some(marker) = red_marker_diagnostic {
                diagnostic["red_marker_cluster_anchor"] = marker;
            }
            if pixel_identity_changed {
                diagnostic["pixel_comparison_skipped"] = serde_json::json!("identity_changed");
            } else if anchor_comparison_skipped {
                diagnostic["pixel_comparison_skipped"] =
                    serde_json::json!("native_binding_changed");
            }
        }
        error.diagnostic = Some(Box::new(diagnostic));
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

// A nonzero AX status is not proof that the application made no change.
fn native_action_failure(action: &str, status: i32) -> rwi_core::Fault {
    let mut error = fault(
        "native_action_unconfirmed",
        &format!(
            "{action} returned AXError {status}; effect unknown. Inspect the original operation and fresh state; never replay or send fallback input."
        ),
    );
    error.diagnostic = Some(Box::new(serde_json::json!({
        "api": "AXUIElementPerformAction", "action": action,
        "native_status": status, "effect": "unknown", "replay_allowed": false,
        // This path invokes one AX action, not a key/button down/up sequence.
        // Its uncertain application effect must not demand global HID idle.
        "held_input_resources": false
    })));
    error
}

fn needs_background_activation(
    intent: &Intent,
    native_press: bool,
    explicit_action: bool,
    foreground_pid: Option<u32>,
    target_pid: u32,
) -> bool {
    matches!(intent, Intent::Text | Intent::KeyChord)
        && !native_press
        && !explicit_action
        && foreground_pid != Some(target_pid)
}

fn validate_keyboard_payload(intent: &Intent, value: Option<&str>) -> Result<()> {
    match intent {
        Intent::Text => {
            value.ok_or_else(|| fault("value_required", "Text intent requires Unicode value."))?;
        }
        Intent::KeyChord => {
            parse_chord(value.ok_or_else(|| {
                fault("value_required", "Key chord requires a supported chord.")
            })?)?;
        }
        _ => {}
    }
    Ok(())
}
