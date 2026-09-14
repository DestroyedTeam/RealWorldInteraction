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
            dispatches,
        })
    }
    pub(super) fn window_name(&self) -> String {
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
    pub(super) fn metadata_scene(&self, meta: Metadata, size: [u32; 2], png: Vec<u8>) -> Scene {
        let mut binding = meta.binding;
        if !png.is_empty() {
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
        let blocked = modal;
        let (layout, mapping_ready) = display_layout(window.rect)?;
        let system = unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide())? };
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(system.0, 0.05);
        }
        let elements = ax_tree(&focused, &system, window.rect, blocked)?;
        let focused_element = application
            .attr("AXFocusedUIElement")
            .ok()
            .and_then(|e| element_description(&e).ok());
        let native_epoch = digest(
            format!(
                "{}|{:?}|{}|{:?}|{:?}",
                self.process,
                window.rect,
                title,
                modal,
                focused_element.as_ref().map(|v| &v.element.id)
            )
            .as_bytes(),
        );
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
    /// when the private attribute is unavailable. Ambiguity always fails closed.
    pub(super) fn resolve_window_element(&self, application: &Cf, expected: Rect) -> Result<Cf> {
        let value = application.attr("AXWindows")?;
        let windows = unsafe { ffi::array(value.0, 64)? };
        let mut exact = Vec::new();
        let mut geometric = Vec::new();
        for window in windows {
            if ffi::window_number_for_ax_element(&window) == Some(self.window) {
                exact.push(window);
            } else if ax_rect(&window).is_ok_and(|r| r.same(expected)) {
                geometric.push(window);
            }
        }
        match (exact.len(), geometric.len()) {
            (1, _) => Ok(exact.pop().expect("one exact window")),
            (0, 1) => Ok(geometric.pop().expect("one geometric window")),
            (0, 0) => Err(fault(
                "window_missing",
                "Pinned CGWindowID/PID has no matching AX window; do not substitute another window.",
            )),
            _ => Err(fault(
                "window_binding_ambiguous",
                "Multiple AX windows match the pinned identity or bounds; rebind explicitly.",
            )),
        }
    }
}
