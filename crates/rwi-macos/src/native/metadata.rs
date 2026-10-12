//! Native AX metadata, focus and hit-testing. No implicit application activation.
use super::*;
mod exact_relay;
mod placement;
mod settled_hit;
mod windows;
use placement::panel_placement_position;

impl NativeMac {
    pub fn new(pid: u32, window: u32, allow_input: bool) -> Result<Self> {
        Self::with_shared_resources(
            pid,
            window,
            allow_input,
            Capturer::default(),
            std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        )
    }
    pub(crate) fn with_shared_resources(
        pid: u32,
        window: u32,
        allow_input: bool,
        capturer: Capturer,
        dispatches: std::sync::Arc<std::sync::atomic::AtomicU64>,
    ) -> Result<Self> {
        if pid == 0 || pid > i32::MAX as u32 || window == 0 {
            return Err(fault(
                "invalid_target",
                "Pin a positive PID and CGWindowID.",
            ));
        }
        Ok(Self {
            pid,
            window,
            identity: console_identity()?,
            process: process_epoch(pid)?,
            allow_input,
            semantic_only: false,
            capturer,
            hit_diagnostics: std::sync::Mutex::new(Vec::new()),
            dispatches,
            objects: std::sync::Mutex::new(objects::Objects::default()),
            last_observation: std::sync::Mutex::new(serde_json::Value::Null),
        })
    }
    pub(crate) fn window_name(&self) -> String {
        format!("cgwindow:{}", self.window)
    }
    pub(crate) fn require_discovered_process(&self, process: &str) -> Result<()> {
        if self.process != process {
            return Err(fault(
                "window_epoch_changed",
                "The exact process birth changed while constructing the selected native backend.",
            ));
        }
        Ok(())
    }
    pub(super) fn foreground_pid(&self) -> Option<u32> {
        let system = unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide()).ok()? };
        system.attr("AXFocusedApplication").ok()?.pid().ok()
    }
    /// Read native focused-window identity only for a recent-activity conflict.
    /// No AXMain guess, activation, or fallback to a similarly titled window.
    pub(super) fn foreground_input_window(&self) -> (Option<u32>, Option<u32>) {
        let Ok(system) = (unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide()) }) else {
            return (None, None);
        };
        let Ok(application) = system.attr("AXFocusedApplication") else {
            return (None, None);
        };
        let pid = application.pid().ok();
        if pid != Some(self.pid) {
            return (pid, None);
        }
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 0.05);
        }
        let window = application
            .attr("AXFocusedWindow")
            .ok()
            .and_then(|window| ffi::window_number_for_ax_element(&window));
        (pid, window)
    }
    pub(super) fn window_frame(&self) -> Result<Rect> {
        Ok(self.window_record()?.rect)
    }
    pub(super) fn metadata_scene(
        &self,
        meta: Metadata,
        size: [u32; 2],
        png: Vec<u8>,
        content_sha: Option<&str>,
    ) -> Scene {
        let mut binding = meta.binding;
        if let Some(content_sha) = content_sha {
            binding.content = content_sha.to_owned();
        } else if !png.is_empty() {
            binding.content = digest(&png);
        }
        Scene {identity:meta.identity,assurance:"native-attested".into(),binding:Some(binding),
            mutation_ready:self.allow_input&&meta.mapping_ready&&meta.blocked==Some(false)&&unsafe{ffi::CGPreflightPostEventAccess()},
            blocked:meta.blocked,image_size:size,pixel_to_desktop:if meta.mapping_ready{meta.rect.map(size)}else{None},
            elements:meta.elements.into_iter().map(|e|e.element).collect(),
            captured_unix_ms:SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            atomicity:"pre/post native validation; non-atomic; live acceptance pending".into(),
            image_normalization:"CGImage -> device RGB bitmap -> straight RGBA PNG; native dimensions; no resizing; color/alpha normalization, not source-byte identity; full PNG retained, input fingerprint excludes title chrome and bottom one-point compositor border; synthetic pointer paths cannot enter bottom border".into(),png}
    }
    pub(super) fn metadata(&self) -> Result<Metadata> {
        self.metadata_for(None)
    }
    /// Revalidation reads the exact retained target, not unrelated siblings.
    /// Full observations still walk the bounded tree and apply capture guards.
    pub(super) fn metadata_for(&self, target: Option<&Resolved>) -> Result<Metadata> {
        self.metadata_read(target, true)
    }
    /// Snapshot pixels/controls are explicitly non-atomic. The pre-capture
    /// guard needs the native window/context/modal state, not a discarded tree.
    pub(super) fn capture_guard_metadata(&self) -> Result<Metadata> {
        self.metadata_read(None, false)
    }
    fn metadata_read(&self, target: Option<&Resolved>, include_controls: bool) -> Result<Metadata> {
        let _call = operation::native_call("macos_ax_metadata")?;
        let identity = console_identity()?;
        if identity != self.identity || process_epoch(self.pid)? != self.process {
            return Err(fault(
                "session_or_process_changed",
                "Pinned console session/process instance changed. Rebind explicitly.",
            ));
        }
        if unsafe { ffi::AXIsProcessTrusted() } == 0 {
            return Err(fault(
                "accessibility_permission_missing",
                "Grant permission to the native binary through an authorized setup, not this observation call.",
            ));
        }
        let window = self.window_record()?;
        // Background observation: resolve the exact AX window from the target
        // application itself. No focused-application or focused-window
        // requirement; the pinned window does not need to be frontmost.
        let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 0.05);
        }
        let mut window_recovery = None;
        let focused =
            self.resolve_window_element_traced(&application, window.rect, &mut window_recovery)?;
        let ax_rect = ax_rect(&focused)?;
        if !window.rect.same(ax_rect) {
            return Err(fault(
                "window_binding_ambiguous",
                &format!(
                    "Resolved AX window rect (x={:.1},y={:.1},w={:.1},h={:.1}) does not match the pinned CGWindowID/PID/bounds (x={:.1},y={:.1},w={:.1},h={:.1}).",
                    ax_rect.x,
                    ax_rect.y,
                    ax_rect.w,
                    ax_rect.h,
                    window.rect.x,
                    window.rect.y,
                    window.rect.w,
                    window.rect.h
                ),
            ));
        }
        let title = focused
            .attr("AXTitle")
            .ok()
            .and_then(|v| v.string().ok())
            .or_else(|| window.title.clone())
            .unwrap_or_default();
        let modal = focused.attr("AXModal").ok().and_then(|v| v.boolean());
        let blocked = self.modal_state(&application, &focused);
        let (layout, mapping_ready) = display_layout(window.rect)?;
        // This tree belongs to the pinned application. A system-wide hit test
        // would turn another app covering it into a false foreground requirement.
        // Application-scoped AX hit testing still respects that app's window
        // z-order; equality with each exact tree node preserves window identity.
        let mut diagnostics = Vec::new();
        let (elements, mut coverage) = read_control_metadata(include_controls, || {
            Ok(if let Some(target) = target {
                if target.scope.pid != self.pid || target.scope.window != self.window_name() {
                    return Err(fault(
                        "target_mismatch",
                        "Semantic revalidation belongs to another window or process.",
                    ));
                }
                let element = objects::exact(
                    &focused,
                    self.pid,
                    target.scope.element.as_deref().ok_or_else(|| {
                        fault("semantic_target_missing", "Exact semantic object required.")
                    })?,
                    &self.objects,
                )?;
                (
                    vec![if target.native_action.is_none() {
                        settled_hit::observed(
                            self,
                            &element,
                            &focused,
                            &application,
                            window.rect,
                            blocked,
                            &mut diagnostics,
                        )?
                    } else {
                        observed_element(
                            &element,
                            &focused,
                            &application,
                            window.rect,
                            blocked,
                            &mut diagnostics,
                            &self.objects,
                        )?
                    }],
                    serde_json::json!({"scope":"exact_target_revalidation","complete":false,"input_authority":false}),
                )
            } else {
                ax_tree(
                    &focused,
                    &application,
                    window.rect,
                    blocked,
                    &mut diagnostics,
                    &self.objects,
                )?
            })
        })?;
        if let Some(note) = window_recovery {
            coverage["window_resolution_recovery"] = note;
        }
        if include_controls && let Ok(mut saved) = self.hit_diagnostics.lock() {
            *saved = diagnostics;
        }
        // The application's internal focused element is not window identity: it
        // changes whenever the user or activation chrome moves focus, including
        // while the user works in other applications. Keyboard/text paths verify
        // the exact focused target separately before dispatch.
        let native_epoch = digest(
            format!(
                "{}|{:?}|{}|{:?}|semantic:{}",
                self.process, window.rect, title, modal, self.semantic_only
            )
            .as_bytes(),
        );
        let binding = Binding {
            // Already-read native title, not an invented document-state oracle.
            // Epoch already binds this same title; no new AX reads or weaker gate.
            mutation_context: Some(MutationContext {
                window_title: title.clone(),
                document_state: None,
            }),
            scope: Scope {
                window: self.window_name(),
                pid: self.pid,
                element: None,
            },
            focus_window: self.window_name(),
            layout,
            content: native_epoch.clone(),
            native_epoch,
        };
        if console_identity()? != identity || process_epoch(self.pid)? != self.process {
            return Err(fault(
                "session_or_process_changed",
                "The native context/process changed during AX readback; discard this observation.",
            ));
        }
        Ok(Metadata {
            identity,
            rect: window.rect,
            binding,
            elements,
            title,
            modal,
            blocked,
            mapping_ready,
            coverage,
        })
    }
}

/// Positive, exact-parent evidence only; never an application-wide PID gate.
/// AppKit document sheets are AXChildren of their owner even when AXModal=false
/// and AXFocusedWindow remains on the owner. A separately selected sheet has
/// buttons as children and therefore does not block itself. A remote provider's
/// sheet can also block this parent; its PID never confers input authority here.
fn known_direct_sheet_child(window: &Cf) -> bool {
    let Ok(value) = window.attr("AXChildren") else {
        return false;
    };
    let Ok(children) = (unsafe { ffi::array(value.0, 64) }) else {
        return false;
    };
    children.iter().any(|child| {
        if unsafe { ffi::CFGetTypeID(child.0) != ffi::AXUIElementGetTypeID() } {
            return false;
        }
        child
            .attr("AXRole")
            .ok()
            .and_then(|v| v.string().ok())
            .as_deref()
            == Some("AXSheet")
            && child.attr("AXParent").ok().is_some_and(|parent| unsafe {
                ffi::CFEqual(parent.0, window.0) != 0 && ffi::CFEqual(child.0, window.0) == 0
            })
            && child.attr("AXHidden").ok().and_then(|v| v.boolean()) != Some(true)
    })
}
fn with_known_attached_sheet(base: Option<bool>, attached: bool) -> Option<bool> {
    if attached { Some(true) } else { base }
}

fn modal_blocked(
    window_modal: Option<bool>,
    focused_here: bool,
    focused_modal: Option<bool>,
    modal_hit: Option<bool>,
) -> Option<bool> {
    // A modal surface does not block itself; it does block a different window.
    // A different ordinary/nonmodal focus is not a foreground prerequisite.
    if focused_here {
        Some(false)
    } else if focused_modal == Some(true) {
        Some(true)
    } else if window_modal == Some(true) {
        modal_hit.map(|hit| !hit)
    } else {
        window_modal
    }
}

fn popover_subrole(element: &Cf) -> bool {
    ["AXRole", "AXSubrole"].into_iter().any(|key| {
        element
            .attr(key)
            .ok()
            .and_then(|value| value.string().ok())
            .as_deref()
            == Some("AXPopover")
    })
}

fn hit_descends_from(hit: &Cf, ancestor: &Cf, pid: u32) -> Option<bool> {
    if hit.pid().ok() != Some(pid) {
        return Some(false);
    }
    let mut node = hit.retained();
    for _ in 0..8 {
        if unsafe { ffi::CFEqual(node.0, ancestor.0) } != 0 {
            return Some(true);
        }
        node = node.attr("AXParent").ok()?;
    }
    Some(false)
}

#[cfg(test)]
#[test]
fn focused_modal_dialog_is_actionable_but_parent_is_blocked() {
    assert_eq!(
        modal_blocked(Some(true), true, Some(true), None),
        Some(false)
    );
    assert_eq!(modal_blocked(None, true, None, None), Some(false));
    assert_eq!(
        modal_blocked(Some(false), false, Some(true), None),
        Some(true)
    );
    assert_eq!(
        modal_blocked(Some(true), false, Some(true), Some(true)),
        Some(true)
    );
    assert_eq!(
        modal_blocked(Some(true), false, Some(false), Some(true)),
        Some(false)
    );
    assert_eq!(
        modal_blocked(Some(true), false, Some(false), Some(false)),
        Some(true)
    );
    assert_eq!(modal_blocked(Some(true), false, Some(false), None), None);
    assert_eq!(
        modal_blocked(Some(false), false, Some(false), None),
        Some(false)
    );
    assert_eq!(modal_blocked(Some(false), false, None, None), Some(false));
    assert_eq!(modal_blocked(None, false, None, None), None);
    assert_eq!(
        modal_blocked(Some(true), false, None, Some(true)),
        Some(false)
    );
}

/// Sheets and dialogs are native child windows of their parent AX window.
/// Advisory per-process window inventory for popup delivery. Covers the same
/// AX surface as binding (AXWindows plus child sheets/dialogs and the
/// focused/main relays) and reports unknown purposes verbatim.
pub(super) fn popup_inventory(pid: u32) -> Vec<rwi_core::Popup> {
    let mut out: Vec<rwi_core::Popup> = Vec::new();
    let push = |element: &Cf, out: &mut Vec<rwi_core::Popup>| {
        if out.len() >= 16 {
            return;
        }
        let subrole = element
            .attr("AXSubrole")
            .ok()
            .and_then(|value| value.string().ok());
        let role = element
            .attr("AXRole")
            .ok()
            .and_then(|value| value.string().ok());
        let title = element
            .attr("AXTitle")
            .ok()
            .and_then(|value| value.string().ok())
            .filter(|title| !title.is_empty());
        let window =
            ffi::window_number_for_ax_element(element).map(|number| format!("cgwindow:{number}"));
        if out
            .iter()
            .any(|existing| existing.window.is_some() && existing.window == window)
        {
            return;
        }
        let kind = rwi_core::popup_kind(subrole.as_deref()).to_string();
        out.push(rwi_core::Popup {
            window,
            title,
            role,
            subrole,
            kind,
            bounds: ax_rect(element).ok().map(|r| [r.x, r.y, r.w, r.h]),
            display: None,
        });
    };
    let Ok(application) = (unsafe { Cf::owned(ffi::AXUIElementCreateApplication(pid as i32)) })
    else {
        return out;
    };
    unsafe {
        ffi::AXUIElementSetMessagingTimeout(application.0, 0.25);
    }
    if let Ok(value) = application.attr("AXWindows")
        && let Ok(windows) = unsafe { ffi::array(value.0, 64) }
    {
        for window in &windows {
            push(window, &mut out);
            if let Ok(children_value) = window.attr("AXChildren")
                && let Ok(children) = unsafe { ffi::array(children_value.0, 32) }
            {
                for child in &children {
                    if dialog_subrole(child) {
                        push(child, &mut out);
                    }
                }
            }
        }
    }
    for key in ["AXFocusedWindow", "AXMainWindow"] {
        if let Ok(element) = application.attr(key) {
            push(&element, &mut out);
        }
    }
    out
}
pub(super) fn dialog_subrole(element: &Cf) -> bool {
    // AXSheet may be exposed as the role itself with no AXSubrole (observed
    // Paint S unsaved-document sheet). Keep both forms, but never classify a
    // generic AXWindow with unknown role/subrole as a dialog.
    let role = element
        .attr("AXRole")
        .ok()
        .and_then(|value| value.string().ok());
    let subrole = element
        .attr("AXSubrole")
        .ok()
        .and_then(|value| value.string().ok());
    dialog_semantics(role.as_deref(), subrole.as_deref())
}
fn dialog_semantics(role: Option<&str>, subrole: Option<&str>) -> bool {
    [role, subrole].into_iter().flatten().any(|value| {
        matches!(
            value,
            "AXSheet" | "AXDialog" | "AXSystemDialog" | "AXPopover"
        )
    })
}
#[cfg(test)]
#[test]
fn ax_sheet_role_without_subrole_is_a_groundable_dialog() {
    assert!(dialog_semantics(Some("AXSheet"), None));
    assert!(dialog_semantics(Some("AXWindow"), Some("AXDialog")));
    assert!(!dialog_semantics(Some("AXWindow"), None));
    assert!(!dialog_semantics(None, None));
}

/// Window guards are not control observations and cannot manufacture authority.
fn read_control_metadata<T>(
    include: bool,
    read: impl FnOnce() -> Result<(Vec<T>, serde_json::Value)>,
) -> Result<(Vec<T>, serde_json::Value)> {
    if include {
        read()
    } else {
        Ok((
            Vec::new(),
            serde_json::json!({"scope":"window_context_only","complete":false,
            "controls_read":false,"input_authority":false,"absence_proven":false}),
        ))
    }
}

#[test]
fn capture_guard_skips_only_the_control_walk_not_exact_target_revalidation() {
    let mut reads = 0;
    let (elements, coverage) = read_control_metadata::<u8>(false, || {
        reads += 1;
        Ok((vec![1], serde_json::Value::Null))
    })
    .unwrap();
    assert_eq!(reads, 0);
    assert!(elements.is_empty());
    assert_eq!(coverage["controls_read"], false);
    assert_eq!(coverage["complete"], false);
    assert_eq!(coverage["input_authority"], false);
    let (elements, _) = read_control_metadata(true, || {
        reads += 1;
        Ok((vec![1], serde_json::Value::Null))
    })
    .unwrap();
    assert_eq!(reads, 1);
    assert_eq!(elements, vec![1]);
    assert!(
        read_control_metadata::<u8>(true, || Err(fault("target_replaced", "exact read refused")))
            .is_err()
    );
}

#[cfg(test)]
mod attached_sheet_policy_contract {
    use super::*;
    #[test]
    fn direct_document_sheet_blocks_only_its_parent_surface() {
        assert_eq!(with_known_attached_sheet(Some(false), true), Some(true));
        assert_eq!(with_known_attached_sheet(Some(false), false), Some(false));
        assert_eq!(with_known_attached_sheet(None, false), None);
        assert_eq!(with_known_attached_sheet(None, true), Some(true));
        assert_eq!(with_known_attached_sheet(Some(true), false), Some(true));
    }
}
