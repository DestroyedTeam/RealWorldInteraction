//! AX continuation objects stay on the synchronous input worker's stack.
use super::*;
use continuation::{Facts, Kind};
#[path = "anchor_native/system_window_hit.rs"]
mod system_window_hit;

fn application_hit_geometry(bounds: Rect, point: [f64; 2]) -> bool {
    bounds.valid() && bounds.contains(point)
}

fn window_local_offdisplay_point(displays: &[Display], window: Rect, point: [f64; 2]) -> bool {
    window_mapping_ready(displays, window)
        && application_hit_geometry(window, point)
        && !displays.iter().any(|display| display.rect.contains(point))
}

/// Physical system-cursor delivery cannot use background tree selection to
/// authorize another window or an application provider's incompatible hit.
fn foreground_hit_matches(
    owner_matches: bool,
    same_window: bool,
    bounds: Rect,
    point: [f64; 2],
) -> bool {
    owner_matches && same_window && application_hit_geometry(bounds, point)
}

pub(super) fn require_foreground_pointer_hit(native: &NativeMac, point: [f64; 2]) -> Result<()> {
    let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(native.pid as i32))? };
    unsafe {
        ffi::AXUIElementSetMessagingTimeout(application.0, 0.05);
    }
    let root = native.resolve_window_element(&application, native.window_record()?.rect)?;
    let hit = hit_test(&application, point)?;
    let bounds = ax_rect(&hit)?;
    let same_window = unsafe { ffi::CFEqual(hit.0, root.0) != 0 }
        || hit
            .attr("AXWindow")
            .ok()
            .is_some_and(|owner| unsafe { ffi::CFEqual(owner.0, root.0) != 0 });
    if !foreground_hit_matches(
        hit.pid().ok() == Some(native.pid),
        same_window,
        bounds,
        point,
    ) {
        let mut error = fault(
            "foreground_hit_unproven",
            "Physical input requires a positive exact-window native hit at the actual point; no window-local or background substitution.",
        );
        error.diagnostic = Some(Box::new(serde_json::json!({
            "point":point,
            "hit_bounds":[bounds.x,bounds.y,bounds.w,bounds.h],
            "same_native_window":same_window,
            "current_batch_native_input_dispatched":false,
            "replay_allowed":false,
        })));
        return Err(error);
    }
    require_nonsecure_input(&element_description(&hit)?)
}

/// Grounding stays in the pinned application; otherwise walk the exact window.
/// Never set a timeout on AXUIElementCreateSystemWide: that changes the global
/// timeout for every AX object in this agent, including later action callbacks.
fn point_element(window: &Cf, pid: u32, point: [f64; 2]) -> Result<(Cf, bool)> {
    // Application hit testing consumes screen coordinates. A point in the
    // offdisplay part of a positively mapped partial window has no physical
    // screen receiver. Resolve its metadata from the exact window's native
    // children instead; do not clamp or borrow an application/menu-bar object.
    if window_local_offdisplay_point(&displays()?, ax_rect(window)?, point) {
        let hit = element_at_point(window, point)?.ok_or_else(|| {
            fault(
                "window_local_hit_unproven",
                "No native descendant contains this window-local point; no input dispatched.",
            )
        })?;
        if hit.pid().ok() != Some(pid) || !application_hit_geometry(ax_rect(&hit)?, point) {
            return Err(fault(
                "window_local_hit_unproven",
                "The exact-window descendant owner or geometry is unproven; no input dispatched.",
            ));
        }
        return Ok((hit, false));
    }
    let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(pid as i32))? };
    unsafe {
        ffi::AXUIElementSetMessagingTimeout(application.0, 0.05);
    }
    if let Ok(hit) = hit_test(&application, point)
        && hit.pid().ok() == Some(pid)
    {
        let bounds = ax_rect(&hit)?;
        let same_window = unsafe { ffi::CFEqual(hit.0, window.0) != 0 }
            || hit
                .attr("AXWindow")
                .ok()
                .is_some_and(|owner| unsafe { ffi::CFEqual(owner.0, window.0) != 0 });
        if same_window && application_hit_geometry(bounds, point) {
            return Ok((hit, true));
        }
        // Background events address the already selected native window, not
        // the application hit's frontmost panel. Independently resolve a real
        // descendant of this exact root; never retain or authorize the foreign
        // hit. Foreground-system delivery has its own positive physical-hit
        // guard and cannot inherit this window-addressed admission.
        if ax_rect(window)?.contains(point)
            && let Some(candidate) = element_at_point(window, point)?
            && candidate.pid().ok() == Some(pid)
            && application_hit_geometry(ax_rect(&candidate)?, point)
        {
            // A tree-selected window must not inherit the positive application
            // hit proof needed by the childless-window canvas path.
            return Ok((candidate, false));
        }
        let mut error = fault(
            "input_boundary_changed",
            "Application hit is not a valid exact-window hit and no native window descendant contains the actual point. No input dispatched.",
        );
        error.diagnostic = Some(Box::new(serde_json::json!({
            "changed":["hit_rect"],
            "point":point,
            "hit_bounds":[bounds.x,bounds.y,bounds.w,bounds.h],
            "same_native_window":same_window,
            "current_batch_native_input_dispatched":false,
            "prior_batches_may_have_dispatched":true,
            "replay_allowed":false,
        })));
        return Err(error);
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
    #[serde(skip_serializing_if = "Option::is_none")]
    system_window_hit: Option<bool>,
}

fn local_pixel_canvas(role: Option<&str>, proof: WindowCanvasProof) -> bool {
    // Preserve the existing scrolling-canvas path. A window-level canvas is a
    // separate, positively proven case, never a fallback for failed hit testing.
    role == Some("AXScrollArea")
        || (role == Some("AXWindow")
            && proof.exact_window_object
            && proof.same_pid
            && proof.exact_window_number
            && ((proof.application_hit && proof.children_count == Some(0))
                || proof.system_window_hit == Some(true)))
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
        let (mut hit, application_hit) = if target.native_action.is_some() {
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
        // Preserve an already referenced button when its passive text label is
        // the actual native hit. Never search by attributes or apply this to a
        // pixel target, text/key receiver, another button or another window.
        if target.native_action.is_none()
            && matches!(kind, Kind::Pointer | Kind::DoubleClick)
            && let Some(id) = target.scope.element.as_deref()
        {
            let original =
                super::super::objects::exact(&window_element, self.pid, id, &self.objects)?;
            if unsafe { ffi::CFEqual(original.0, hit.0) == 0 }
                && label_hit::equivalent(
                    &original,
                    &hit,
                    &window_element,
                    target.point,
                    Some("AXButton"),
                    Duration::from_millis(40),
                )?
            {
                if let Ok(mut rows) = self.hit_diagnostics.lock()
                    && rows.len() < 8
                {
                    rows.push(serde_json::json!({
                        "stage":"macos_original_button_label_hit","original_element":id,
                        "hit_pid":hit.pid().ok(),"hit_role":hit.attr("AXRole").ok().and_then(|role|role.string().ok()),
                        "hit_window":hit.attr("AXWindow").ok().as_ref().and_then(ffi::window_number_for_ax_element),
                        "label_equivalent":true,"original_retained":true,"native_input_dispatched":false
                    }));
                }
                hit = original;
            }
        }
        let hit_description =
            super::super::objects::describe(&hit, &window_element, &self.objects)?;
        if target.pixel_anchor_used {
            let role = hit_description.element.role.as_deref();
            let proof = if role == Some("AXWindow") {
                let children_count = window_element
                    .attr("AXChildren")
                    .ok()
                    .and_then(|value| unsafe { ffi::array(value.0, 64).ok() })
                    .map(|children| children.len());
                WindowCanvasProof {
                    application_hit,
                    // A real, independently owned system-wide hit is a different
                    // proof, not geometry fallback or an empty-AX inference.
                    system_window_hit: if application_hit && children_count == Some(0) {
                        None // Preserve the existing positive fast path, no extra RPC/read.
                    } else {
                        system_window_hit::read(
                            &window_element,
                            self.pid,
                            self.window,
                            target.point,
                        )
                    },
                    exact_window_object: unsafe { ffi::CFEqual(hit.0, window_element.0) } != 0,
                    same_pid: hit.pid().ok() == Some(self.pid)
                        && window_element.pid().ok() == Some(self.pid),
                    exact_window_number: ffi::window_number_for_ax_element(&hit)
                        == Some(self.window)
                        && ffi::window_number_for_ax_element(&window_element) == Some(self.window),
                    // Unsupported/malformed/timeout is unknown, never an empty tree.
                    children_count,
                }
            } else {
                WindowCanvasProof::default()
            };
            if !local_pixel_canvas(role, proof) {
                let mut error = fault(
                    "local_pixel_surface_changed",
                    "Local pixel grounding needs AXScrollArea, the existing positive childless application hit, or an independently positive exact system-window hit; missing AX children or geometry alone is not proof. No input dispatched.",
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
            system_window_hit: None,
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

#[cfg(test)]
#[test]
fn offdisplay_application_hit_cannot_bypass_its_own_point_geometry() {
    let source = include_str!("anchor_native.rs");
    let production = source.split("#[cfg(test)]").next().unwrap();
    assert!(
        production.contains("application_hit_geometry") && production.contains("ax_rect(&hit)"),
        "same-PID alone must not accept an application hit whose rectangle misses the actual point"
    );
}

#[cfg(test)]
#[test]
fn application_hit_geometry_keeps_actual_point_and_rejects_invalid_bounds() {
    let bounds = Rect {
        x: 3225.,
        y: 504.,
        w: 660.,
        h: 552.,
    };
    assert!(application_hit_geometry(bounds, [3745., 844.]));
    assert!(!application_hit_geometry(bounds, [4000., 844.]));
    assert!(!application_hit_geometry(bounds, [f64::NAN, 844.]));
    assert!(!application_hit_geometry(
        Rect { w: 0., ..bounds },
        [3745., 844.]
    ));
    assert!(!application_hit_geometry(
        Rect {
            x: f64::INFINITY,
            ..bounds
        },
        [3745., 844.]
    ));
    let source = include_str!("anchor_native.rs");
    let production = source.split("#[cfg(test)]").next().unwrap();
    assert!(production.contains(".attr(\"AXWindow\")"));
    assert!(production.contains("ffi::CFEqual(owner.0, window.0)"));
    assert!(production.contains("element_at_point(window, point)?"));
    assert!(production.contains("return Ok((candidate, false))"));
}

#[cfg(test)]
#[test]
fn partial_window_offdisplay_point_is_not_a_system_cursor_coordinate() {
    let display = Display {
        id: 4,
        uuid: None,
        built_in: false,
        rect: Rect {
            x: 1800.,
            y: 0.,
            w: 1920.,
            h: 1080.,
        },
        pixels: [1920, 1080],
        rotation: 0.,
    };
    let window = Rect {
        x: 3225.,
        y: 504.,
        w: 660.,
        h: 552.,
    };
    let screens = std::slice::from_ref(&display);
    assert!(window_local_offdisplay_point(
        screens,
        window,
        [3745., 844.]
    ));
    assert!(!window_local_offdisplay_point(
        screens,
        window,
        [3600., 844.]
    ));
    assert!(!window_local_offdisplay_point(
        screens,
        window,
        [4000., 844.]
    ));
    assert!(!window_local_offdisplay_point(
        screens,
        window,
        [f64::NAN, 844.]
    ));
    assert!(!window_local_offdisplay_point(&[], window, [3745., 844.]));
    assert!(!window_local_offdisplay_point(
        screens,
        Rect { x: 4000., ..window },
        [4100., 844.]
    ));
    assert!(!window_local_offdisplay_point(
        &[Display {
            rotation: 90.,
            ..display
        }],
        window,
        [3745., 844.]
    ));
}

#[cfg(test)]
#[test]
fn addressed_window_metadata_never_substitutes_for_a_foreground_physical_hit() {
    let bounds = Rect {
        x: 3400.,
        y: 750.,
        w: 300.,
        h: 200.,
    };
    assert!(foreground_hit_matches(true, true, bounds, [3600., 829.]));
    for (owner, window, point) in [
        (false, true, [3600., 829.]),
        (true, false, [3600., 829.]),
        (true, true, [3701., 829.]),
        (true, true, [f64::NAN, 829.]),
    ] {
        assert!(!foreground_hit_matches(owner, window, bounds, point));
    }
    let source = include_str!("anchor_native.rs");
    let point_element = source
        .split("fn point_element(")
        .nth(1)
        .unwrap()
        .split("#[derive")
        .next()
        .unwrap();
    assert!(point_element.contains("if same_window && application_hit_geometry(bounds, point)"));
    assert!(point_element.contains("element_at_point(window, point)?"));
    assert!(point_element.contains("return Ok((candidate, false))"));
    assert!(
        include_str!("foreground_move.rs")
            .contains("anchor_native::require_foreground_pointer_hit(native, point)?")
    );
}
