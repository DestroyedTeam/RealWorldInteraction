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
        let window = &all[index];
        let system = unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide()) }?;
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(system.0, 0.05);
        }
        let application = system.attr("AXFocusedApplication")?;
        if application.pid()? != self.pid {
            return Err(fault(
                "focus_mismatch",
                "Pinned application is not focused. Do not silently focus another app.",
            ));
        }
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 0.05);
        }
        let focused = application.attr("AXFocusedWindow")?;
        let ax_rect = ax_rect(&focused)?;
        if !window.rect.same(ax_rect)
            || all
                .iter()
                .filter(|w| w.pid == self.pid && w.rect.same(ax_rect))
                .count()
                != 1
        {
            return Err(fault(
                "window_binding_ambiguous",
                "Focused AX window does not uniquely match the pinned CGWindowID/PID/bounds.",
            ));
        }
        let title = focused.attr("AXTitle")?.string()?;
        let modal = focused.attr("AXModal").ok().and_then(|v| v.boolean());
        let covered = all[..index]
            .iter()
            .any(|w| w.alpha > 0.01 && w.rect.overlaps(window.rect));
        let blocked = if covered || modal == Some(true) {
            Some(true)
        } else {
            modal.map(|_| false)
        };
        let (layout, mapping_ready) = display_layout(window.rect)?;
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
}
