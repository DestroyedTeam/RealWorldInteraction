//! AX continuation objects stay on the synchronous input worker's stack.
use super::*;
use continuation::{Facts, Kind};

/// Grounding for one action point. A system-wide hit that lands in the pinned
/// process is authoritative; otherwise the pinned window's own AX subtree is
/// walked geometrically. Cross-window coverage never blocks background delivery.
fn point_element(window: &Cf, pid: u32, point: [f64; 2]) -> Result<Cf> {
    let system = unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide())? };
    unsafe {
        ffi::AXUIElementSetMessagingTimeout(system.0, 0.05);
    }
    if let Ok(hit) = hit_test(&system, point)
        && hit.pid().ok() == Some(pid)
    {
        return Ok(hit);
    }
    Ok(element_at_point(window, point)?.unwrap_or_else(|| window.retained()))
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
        let all = windows()?;
        let window = all
            .iter()
            .find(|w| w.id == self.window && w.pid == self.pid)
            .ok_or_else(|| {
                fault(
                    "window_binding_changed",
                    "The pinned CGWindowID/PID is absent from the on-screen window list.",
                )
            })?;
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
        let hit = if target.native_action.is_some() {
            exact_action_element(
                &window_element,
                self.pid,
                target.scope.element.as_deref().ok_or_else(|| {
                    fault("native_action_target_missing", "Exact element required.")
                })?,
            )?
        } else {
            point_element(&window_element, self.pid, target.point)?
        };
        let hit_description = element_description(&hit)?;
        require_nonsecure_input(&hit_description)?;
        if matches!(kind, Kind::Text | Kind::Keyboard) {
            require_keyboard_target(&hit_description, target.scope.element.as_deref())?;
            if matches!(kind, Kind::Text) {
                require_text_target(&hit_description, target.scope.element.as_deref())?;
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

// Semantic actions resolve the actual native object in the exact bound subtree,
// never a point hit or a global name search. Ambiguity and walk limits fail closed.
fn exact_action_element(window: &Cf, pid: u32, id: &str) -> Result<Cf> {
    let mut pending = VecDeque::from([(window.retained(), 0usize)]);
    let mut selected: Option<Cf> = None;
    let start = Instant::now();
    let mut count = 0;
    while let Some((node, depth)) = pending.pop_front() {
        operation::checkpoint("macos_semantic_lookup")?;
        if count >= 128 || start.elapsed() > Duration::from_millis(3000) {
            return Err(fault(
                "semantic_walk_incomplete",
                "Cannot prove unique native target within the bounded subtree.",
            ));
        }
        count += 1;
        if node.attr("AXHidden").ok().and_then(|v| v.boolean()) == Some(true) {
            continue;
        }
        if let Ok(description) = element_description(&node)
            && description.element.id == id
        {
            let actual_pid = node.pid()?;
            if actual_pid != pid {
                return Err(fault(
                    "semantic_target_process_mismatch",
                    &format!(
                        "Observed native object PID {actual_pid} differs from bound PID {pid}; inspect delegated ownership before input."
                    ),
                ));
            }
            if let Some(previous) = &selected {
                if unsafe { ffi::CFEqual(previous.0, node.0) } == 0 {
                    return Err(fault(
                        "semantic_target_ambiguous",
                        "Distinct native objects share the observed identity; nothing dispatched.",
                    ));
                }
            } else {
                selected = Some(node.retained());
            }
        }
        let children = node.children().unwrap_or_default();
        if depth >= 16 && !children.is_empty() {
            return Err(fault(
                "semantic_walk_incomplete",
                "Native subtree exceeds depth budget.",
            ));
        }
        for child in children {
            pending.push_back((child, depth + 1));
        }
        if pending.len() > 256 {
            return Err(fault(
                "semantic_walk_incomplete",
                "Native subtree exceeds queue budget.",
            ));
        }
    }
    selected.ok_or_else(|| {
        fault(
            "semantic_target_missing",
            "Exact observed native object no longer exists in this window.",
        )
    })
}
