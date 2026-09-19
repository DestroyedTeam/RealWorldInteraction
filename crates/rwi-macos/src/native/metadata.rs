//! Native AX metadata, focus and hit-testing. No implicit application activation.
use super::*;

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
            capturer,
            hit_diagnostics: std::sync::Mutex::new(Vec::new()),
            dispatches,
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
    pub(super) fn window_is_main(&self) -> bool {
        (|| -> Result<bool> {
            let application =
                unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
            unsafe {
                ffi::AXUIElementSetMessagingTimeout(application.0, 0.05);
            }
            let window = self.resolve_window_element(&application, self.window_frame()?)?;
            Ok(window.attr("AXMain").ok().and_then(|v| v.boolean()) == Some(true))
        })()
        .unwrap_or(false)
    }
    pub(super) fn foreground_pid(&self) -> Option<u32> {
        let system = unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide()).ok()? };
        system.attr("AXFocusedApplication").ok()?.pid().ok()
    }
    pub(super) fn window_frame(&self) -> Result<Rect> {
        windows()?
            .into_iter()
            .find(|w| w.id == self.window && w.pid == self.pid)
            .map(|w| w.rect)
            .ok_or_else(|| {
                fault(
                    "window_missing",
                    "Pinned on-screen window is absent; rebind explicitly.",
                )
            })
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
            image_normalization:"CGImage -> device RGB bitmap -> straight RGBA PNG; native dimensions; no resizing; color/alpha normalization, not source-byte identity".into(),png}
    }
    pub(super) fn metadata(&self) -> Result<Metadata> {
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
        let all = windows()?;
        let index = all
            .iter()
            .position(|w| w.id == self.window && w.pid == self.pid)
            .ok_or_else(|| {
                fault(
                    "window_missing",
                    "Exact pinned on-screen window/PID is absent.",
                )
            })?;
        let window = &all[index].clone();
        // Background observation: resolve the exact AX window from the target
        // application itself. No focused-application or focused-window
        // requirement; the pinned window does not need to be frontmost.
        let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 0.05);
        }
        let focused = self.resolve_window_element(&application, window.rect)?;
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
        let elements = ax_tree(
            &focused,
            &application,
            window.rect,
            blocked,
            &mut diagnostics,
        )?;
        if let Ok(mut saved) = self.hit_diagnostics.lock() {
            *saved = diagnostics;
        }
        // The application's internal focused element is not window identity: it
        // changes whenever the user or activation chrome moves focus, including
        // while the user works in other applications. Keyboard/text paths verify
        // the exact focused target separately before dispatch.
        let native_epoch =
            digest(format!("{}|{:?}|{}|{:?}", self.process, window.rect, title, modal).as_bytes());
        let binding = Binding {
            mutation_context: None,
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
        })
    }
    /// Exact private-SPI window match first; a strict geometry fallback only
    /// when the private attribute is unavailable. Modal sheets and dialogs are
    /// child elements of a top-level AX window rather than entries in
    /// `AXWindows`, and some sandboxed panels appear only as the application's
    /// focused/main window, so both are covered with the same strict bounds
    /// rule. Ambiguity always fails closed.
    /// Panel/sheet windows often do not expose `AXModal` at all. Unknown
    /// modality is acceptable only for the application's focused window, which
    /// is by definition the current interaction surface (the dialog itself,
    /// not something it blocks); every other unknown state stays fail-closed.
    pub(super) fn modal_state(&self, application: &Cf, window_element: &Cf) -> Option<bool> {
        let focused_here = application
            .attr("AXFocusedWindow")
            .ok()
            .and_then(|element| ffi::window_number_for_ax_element(&element))
            .is_some_and(|number| number == self.window);
        match window_element
            .attr("AXModal")
            .ok()
            .and_then(|value| value.boolean())
        {
            // AppKit reports AXModal as "this window is modal", so the focused
            // interaction surface must not be blocked by its own modality
            // (observed live: the colour panel refused input to itself).
            Some(true) if focused_here => Some(false),
            Some(value) => Some(value),
            None => focused_here.then_some(false),
        }
    }
    /// Native placement: set the window's accessibility position so its
    /// top-left lands on the authorized display. No activation, no synthetic
    /// input; the exact window element is re-resolved first and failures stay
    /// fail-closed.
    /// Native subrole of the resolved pinned window (AXStandardWindow, AXSheet,
    /// AXDialog, AXPopover, ...): dialog awareness for the caller without
    /// classifying by title. Read-only; no activation and no input.
    pub(crate) fn window_subrole(&self) -> Result<Option<String>> {
        let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 0.25);
        }
        let window = self.resolve_window_element(&application, self.window_frame()?)?;
        Ok(window
            .attr("AXSubrole")
            .ok()
            .and_then(|value| value.string().ok()))
    }
    pub(crate) fn place_window_on(&self, display: &crate::native::Display) -> Result<()> {
        let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 1.0);
        }
        let window = self.resolve_window_element(&application, self.window_frame()?)?;
        let window_rect = ax_rect(&window)?;
        if window_rect.w > display.rect.w || window_rect.h > display.rect.h {
            return Err(fault(
                "placement_impossible",
                "The window is larger than the authorized display; it was not moved.",
            ));
        }
        let (x, y) = crate::native::placement_position(window_rect, display.rect);
        let point = core_graphics::geometry::CGPoint::new(x, y);
        let value = unsafe {
            ffi::AXValueCreate(
                1,
                (&point as *const core_graphics::geometry::CGPoint).cast(),
            )
        };
        let value = unsafe { Cf::owned(value)? };
        use core_foundation::base::TCFType;
        let attribute = core_foundation::string::CFString::new("AXPosition");
        let code = unsafe {
            ffi::AXUIElementSetAttributeValue(
                window.0,
                attribute.as_concrete_TypeRef().cast(),
                value.0,
            )
        };
        if code != 0 {
            return Err(fault(
                "placement_rejected",
                "The application refused the native window position change; the window stays where it is.",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
        Ok(())
    }
    pub(super) fn resolve_window_element(&self, application: &Cf, expected: Rect) -> Result<Cf> {
        let value = application.attr("AXWindows")?;
        let mut windows = unsafe { ffi::array(value.0, 64)? };
        let mut exact = Vec::new();
        let mut geometric = Vec::new();
        for (index, window) in windows.iter().enumerate() {
            if ffi::window_number_for_ax_element(window) == Some(self.window) {
                exact.push(index);
            } else if ax_rect(window).is_ok_and(|r| r.same(expected)) {
                geometric.push(index);
            }
        }
        match (exact.len(), geometric.len()) {
            (1, _) => return Ok(windows.swap_remove(exact[0])),
            (0, 1) => return Ok(windows.swap_remove(geometric[0])),
            (0, 0) => (),
            _ => {
                return Err(fault(
                    "window_binding_ambiguous",
                    "Multiple AX windows match the pinned identity or bounds; rebind explicitly.",
                ));
            }
        }
        // Sheet/panel windows are often absent from AXWindows entirely. Before
        // any relay on slower reads, look at the native child sheets first with
        // the same strict bounds rule.
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 0.25);
        }
        let mut child_exact = Vec::new();
        let mut child_geometric = Vec::new();
        for window in &windows {
            let Ok(children_value) = window.attr("AXChildren") else {
                continue;
            };
            let Ok(children) = (unsafe { ffi::array(children_value.0, 32) }) else {
                continue;
            };
            for child in children {
                if ffi::window_number_for_ax_element(&child) == Some(self.window) {
                    child_exact.push(child);
                } else if dialog_subrole(&child) && ax_rect(&child).is_ok_and(|r| r.same(expected))
                {
                    child_geometric.push(child);
                }
            }
        }
        match (child_exact.len(), child_geometric.len()) {
            (1, _) => return Ok(child_exact.remove(0)),
            (0, 1) => return Ok(child_geometric.remove(0)),
            (0, 0) => (),
            _ => {
                return Err(fault(
                    "window_binding_ambiguous",
                    "Multiple AX child sheets/dialogs match the pinned identity or bounds; rebind explicitly.",
                ));
            }
        }
        // Some apps (e.g. the sandboxed open panel) expose the sheet only as
        // the application's focused or main window.
        let mut relay = Vec::new();
        for key in ["AXFocusedWindow", "AXMainWindow"] {
            let Ok(element) = application.attr(key) else {
                continue;
            };
            if ffi::window_number_for_ax_element(&element) == Some(self.window) {
                return Ok(element);
            }
            if dialog_subrole(&element) && ax_rect(&element).is_ok_and(|r| r.same(expected)) {
                relay.push(element);
            }
        }
        match relay.len() {
            1 => Ok(relay.remove(0)),
            0 => Err(fault(
                "window_missing",
                "Pinned CGWindowID/PID has no matching AX window; do not substitute another window.",
            )),
            _ => Err(fault(
                "window_binding_ambiguous",
                "Multiple AX focused/main windows match the pinned identity or bounds; rebind explicitly.",
            )),
        }
    }
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
fn dialog_subrole(element: &Cf) -> bool {
    element
        .attr("AXSubrole")
        .ok()
        .and_then(|value| value.string().ok())
        .is_some_and(|subrole| {
            matches!(
                subrole.as_str(),
                "AXSheet" | "AXDialog" | "AXSystemDialog" | "AXPopover"
            )
        })
}
