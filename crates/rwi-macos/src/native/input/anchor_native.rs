//! AX continuation objects stay on the synchronous input worker's stack.
use super::*;
use continuation::{Facts, Kind};

/// Grounding stays in the pinned application; otherwise walk the exact window.
/// Never set a timeout on AXUIElementCreateSystemWide: that changes the global
/// timeout for every AX object in this agent, including later action callbacks.
fn point_element(window: &Cf, pid: u32, point: [f64; 2]) -> Result<(Cf, bool)> {
    let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(pid as i32))? };
    unsafe {
        ffi::AXUIElementSetMessagingTimeout(application.0, 0.05);
    }
    if let Ok(hit) = hit_test(&application, point)
        && hit.pid().ok() == Some(pid)
    {
        return Ok((hit, true));
    }
    Ok((
        element_at_point(window, point)?.unwrap_or_else(|| window.retained()),
        false,
    ))
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
struct WindowCanvasProof {
    application_hit: bool,
    exact_window_object: bool,
    same_pid: bool,
    exact_window_number: bool,
    children_count: Option<usize>,
}

fn local_pixel_canvas(role: Option<&str>, proof: WindowCanvasProof) -> bool {
    // Preserve the existing scrolling-canvas path. A window-level canvas is a
    // separate, positively proven case, never a fallback for failed hit testing.
    role == Some("AXScrollArea")
        || (role == Some("AXWindow")
            && proof.application_hit
            && proof.exact_window_object
            && proof.same_pid
            && proof.exact_window_number
            && proof.children_count == Some(0))
}
pub(super) struct AxObject(Cf);
impl AxObject {
    /// The exact native element, for actions that must not go through
    /// synthetic event routing (e.g. an accessibility press).
    pub(super) fn native(&self) -> &Cf {
        &self.0
    }
}
impl PartialEq for AxObject {
    fn eq(&self, other: &Self) -> bool {
        unsafe { ffi::CFEqual(self.0.0, other.0.0) != 0 }
    }
}

impl NativeMac {
    pub(super) fn input_anchor_facts(
        &self,
        target: &Resolved,
        kind: Kind,
    ) -> Result<Facts<AxObject>> {
        self.input_anchor_facts_in_sequence(target, kind, false)
    }
    pub(super) fn input_anchor_facts_in_sequence(
        &self,
        target: &Resolved,
        kind: Kind,
        continuing_text: bool,
    ) -> Result<Facts<AxObject>> {
        let _call = operation::native_call("macos_input_anchor_read")?;
        let permitted =
            || unsafe { ffi::AXIsProcessTrusted() != 0 && ffi::CGPreflightPostEventAccess() };
        if !permitted() {
            return Err(fault(
                "input_permission_missing",
                "Native AX or posting permission changed; no prompt or input.",
            ));
        }
        let identity = console_identity()?;
        let process = process_epoch(self.pid)?;
        if identity != self.identity || process != self.process {
            return Err(fault(
                "session_or_process_changed",
                "No more input in a replaced native context or process.",
            ));
        }
        let window = self.window_record()?;
        let rect = window.rect;
        // Background operation: resolve the exact AX window from the target
        // application; no focused-application or focused-window requirement.
        let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 0.05);
        }
        let window_element = self.resolve_window_element(&application, rect)?;
        if !ax_rect(&window_element)?.same(rect) {
            return Err(fault(
                "window_binding_changed",
                "The exact AX window no longer matches the pinned native bounds.",
            ));
        }
        let (hit, application_hit) = if target.native_action.is_some() {
            (
                super::super::objects::exact(
                    &window_element,
                    self.pid,
                    target.scope.element.as_deref().ok_or_else(|| {
                        fault("native_action_target_missing", "Exact element required.")
                    })?,
                    &self.objects,
                )?,
                false,
            )
        } else {
            point_element(&window_element, self.pid, target.point)?
        };
        let hit_description =
            super::super::objects::describe(&hit, &window_element, &self.objects)?;
        if target.pixel_anchor_used {
            let role = hit_description.element.role.as_deref();
            let proof = if role == Some("AXWindow") {
                WindowCanvasProof {
                    application_hit,
                    exact_window_object: unsafe { ffi::CFEqual(hit.0, window_element.0) } != 0,
                    same_pid: hit.pid().ok() == Some(self.pid)
                        && window_element.pid().ok() == Some(self.pid),
                    exact_window_number: ffi::window_number_for_ax_element(&hit)
                        == Some(self.window)
                        && ffi::window_number_for_ax_element(&window_element) == Some(self.window),
                    // Unsupported/malformed/timeout is unknown, never an empty tree.
                    children_count: window_element
                        .attr("AXChildren")
                        .ok()
                        .and_then(|value| unsafe { ffi::array(value.0, 64).ok() })
                        .map(|children| children.len()),
                }
            } else {
                WindowCanvasProof::default()
            };
            if !local_pixel_canvas(role, proof) {
                let mut error = fault(
                    "local_pixel_surface_changed",
                    "Local pixel grounding requires AXScrollArea or a positively hit exact AXWindow with an explicitly empty child array; this native surface is unproven. No input dispatched.",
                );
                error.diagnostic = Some(Box::new(serde_json::json!({
                    "reason":"local_pixel_canvas_unproven",
                    "actual_role":role,
                    "window_canvas_proof":proof,
                    "native_input_dispatched":false,
                    "replay_allowed":false,
                })));
                return Err(error);
            }
        }
        require_nonsecure_input(&hit_description)?;
        if matches!(kind, Kind::Text | Kind::Keyboard) {
            // Later text batches are grounded by Sequence's retained CFEqual
            // object and focus, not the geometry-derived public element ID.
            let requested = if continuing_text && matches!(kind, Kind::Text) {
                Some(hit_description.element.id.as_str())
            } else {
                target.scope.element.as_deref()
            };
            require_keyboard_target(&hit_description, requested)?;
            if matches!(kind, Kind::Text) {
                require_text_target(&hit_description, requested)?;
            }
        }
        let focused = if kind.needs_focus() {
            let element = application.attr("AXFocusedUIElement")?;
            if element.pid()? != self.pid {
                return Err(fault(
                    "focus_changed",
                    "The focused native element belongs to another process.",
                ));
            }
            Some(AxObject(element))
        } else {
            None
        };
        let (layout, mapping_ready) = display_layout(rect)?;
        let facts = Facts {
            identity,
            process,
            scope: target.scope.clone(),
            window_rect: rect,
            hit_rect: ax_rect(&hit)?,
            hit_id: hit_description.element.id,
            focused,
            point: target.point,
            layout,
            mapping_ready,
            permissions: true,
            modal: self.modal_state(&application, &window_element),
            // Cross-window coverage does not block background delivery; input is
            // addressed to the pinned window/PID directly.
            occluded: false,
            enabled: hit.attr("AXEnabled").ok().and_then(|v| v.boolean()),
            writable: if matches!(kind, Kind::Text) {
                Some(hit.attribute_settable("AXValue")?)
            } else {
                None
            },
            class: hit_description.text_class,
            window: AxObject(window_element),
            hit: AxObject(hit),
        };
        if console_identity()? != facts.identity
            || process_epoch(self.pid)? != facts.process
            || !permitted()
        {
            return Err(fault(
                "input_boundary_changed",
                "Native context/process or TCC changed during continuation readback.",
            ));
        }
        Ok(facts)
    }
    pub(super) fn drag_anchor_facts(&self, target: &Resolved) -> Result<Facts<AxObject>> {
        self.input_anchor_facts(target, Kind::Pointer)
    }
}

#[cfg(test)]
mod canvas_tests {
    use super::*;

    fn window_proof() -> WindowCanvasProof {
        WindowCanvasProof {
            application_hit: true,
            exact_window_object: true,
            same_pid: true,
            exact_window_number: true,
            children_count: Some(0),
        }
    }

    #[test]
    fn empty_exact_window_canvas_supports_local_pixels_without_scroll_area() {
        assert!(local_pixel_canvas(Some("AXWindow"), window_proof()));
        assert!(local_pixel_canvas(
            Some("AXScrollArea"),
            WindowCanvasProof::default()
        ));
        for role in [None, Some("AXGroup"), Some("AXButton"), Some("AXTextField")] {
            assert!(!local_pixel_canvas(role, window_proof()));
        }
    }

    #[test]
    fn empty_snapshot_or_geometry_fallback_cannot_prove_a_window_canvas() {
        for proof in [
            WindowCanvasProof {
                application_hit: false,
                ..window_proof()
            },
            WindowCanvasProof {
                exact_window_object: false,
                ..window_proof()
            },
            WindowCanvasProof {
                same_pid: false,
                ..window_proof()
            },
            WindowCanvasProof {
                exact_window_number: false,
                ..window_proof()
            },
            WindowCanvasProof {
                children_count: None,
                ..window_proof()
            },
            WindowCanvasProof {
                children_count: Some(1),
                ..window_proof()
            },
        ] {
            assert!(!local_pixel_canvas(Some("AXWindow"), proof), "{proof:?}");
        }
    }
}

#[cfg(test)]
#[path = "anchor_native/canvas_flow_tests.rs"]
mod canvas_flow_tests;
