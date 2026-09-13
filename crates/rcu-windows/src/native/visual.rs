//! Independent visual channel: native facts and raw RGB, never scene/UIA/input state.
use super::*;
use rcu_core::{VisualCapture, VisualOptions, VisualRect, operation::CleanupAudit};

pub(super) fn capability() -> Value {
    json!({"implemented":true,"selected_window_required":true,"max_edge":{"minimum":64,"maximum":2048,"default":1024},"feedback":{"default":"exclude","include":"do not actively exclude; marker presence is not guaranteed"},"source_pixels":rcu_core::MAX_VISUAL_SOURCE_PIXELS,"png_bytes":rcu_core::MAX_VISUAL_PNG_BYTES,"base64_bytes":rcu_core::MAX_VISUAL_BASE64_BYTES,"input_authority":false,"frame_created":false,"uia_walk":false,"retention":"response_only"})
}
impl NativeWindows {
    fn visual_window(&self) -> Result<WindowRecord> {
        // Observation authority is not the expiring selection's input grant.
        if desktop::identity()? != self.identity {
            return Err(fault(
                "session_mismatch",
                "Visual capture must stay in its allocated host/session/seat/desktop.",
            ));
        }
        let selected = self.selected()?;
        let actual = desktop::record(hwnd(&selected.window)?, 0)?;
        if selected.key().is_none()
            || selected.key() != actual.key()
            || selected.native_epoch.is_none()
            || selected.native_epoch != actual.native_epoch
        {
            return Err(fault(
                "window_epoch_changed",
                "Visual capture requires the same selected HWND/PID/process birth/native epoch; no replacement by title.",
            ));
        }
        Ok(actual)
    }
    pub(super) fn visual_capture(&self, options: VisualOptions) -> Result<VisualCapture> {
        options.validate()?;
        checkpoint("windows_visual_begin")?;
        let cleanup = CleanupAudit::begin();
        let dpi = DpiScope::enter()?;
        let before = self.visual_window()?;
        let displays = desktop::displays()?;
        let rectangle = crate::visual_policy::rectangle(&before)?;
        if !displays.iter().any(|d| d.physical_rect.contains(rectangle)) {
            return Err(fault(
                "capture_spanning_or_offscreen_unavailable",
                "The selected visible rectangle must fit one physical monitor; no clipping or display changes.",
            ));
        }
        let raw = capture::capture_rgb(rectangle, options.feedback)?;
        let captured_unix_ms = input::now_ms();
        checkpoint("windows_visual_post_copy")?;
        let after = self.visual_window()?;
        let after_displays = desktop::displays()?;
        crate::visual_policy::unchanged(&before, &after)?;
        if serde_json::to_value(&displays).ok() != serde_json::to_value(&after_displays).ok() {
            return Err(fault(
                "visual_display_changed",
                "Display geometry/mode changed during the visible-desktop copy; discard this visual.",
            ));
        }
        // Copy/exclusion and native geometry checks finish before resizing/encoding.
        drop(dpi);
        if cleanup.uncertain() {
            return Err(fault(
                "native_resource_release_uncertain",
                "Native capture/DPI resources did not report successful release.",
            ));
        }
        let resize = Instant::now();
        let (png, image_size) =
            rcu_imaging::encode_visual_rgb(raw.raster.size, &raw.raster.rgb, options.max_edge)?;
        checkpoint("windows_visual_encoded")?;
        let sx = f64::from(rectangle.width) / f64::from(image_size[0]);
        let sy = f64::from(rectangle.height) / f64::from(image_size[1]);
        Ok(VisualCapture {
            identity: self.identity.clone(),
            window: rcu_core::lifecycle::WindowIdentity { window: before.window.clone(), pid: before.pid, process_created: before.process_created.expect("validated native tuple"), native_epoch: before.native_epoch.clone().expect("validated native epoch") },
            source_rect: VisualRect { x: rectangle.x, y: rectangle.y, width: rectangle.width, height: rectangle.height },
            image_size, visual_to_desktop: [sx,0.,0.,sy,f64::from(rectangle.x)+(sx-1.)/2.,f64::from(rectangle.y)+(sy-1.)/2.],
            captured_unix_ms, assurance: "native-attested-visible-desktop".into(),
            atomicity: "non-atomic visible-desktop rectangle; pre/post selected identity, visibility, geometry, display and DPI checks; other occlusion remains visible".into(),
            normalization: "GDI top-down BGRA -> raw RGB -> optional area-weighted RGB8 box downsample with half-up rounding -> one SDR 8-bit RGB PNG encode; no upscaling".into(),
            feedback: raw.feedback,
            details: json!({"window_dpi":before.window_dpi,"window_dpi_awareness":before.window_dpi_awareness,"bounds_source":if before.dwm_visible_frame.is_some(){"DWM_EXTENDED_FRAME_BOUNDS"}else{"GetWindowRect under PMv2"},"foreground_before":before.foreground,"foreground_after":after.foreground,"displays":displays,"capture_timing":raw.timing,"resize_and_encode_ms":resize.elapsed().as_secs_f64()*1000.,"output_png_bytes":png.len(),"capture_backend":"GetDC/CreateDIBSection/BitBlt SRCCOPY|CAPTUREBLT/GdiFlush","hdr_protected_content_cursor_fidelity":"not attested","native_window_epoch_assurance":"HWND/PID/process birth/class; no persistent HWND generation tracking","uia_reads":0,"input_grant_updated":false,"selection_updated":false}),
            png,
        })
    }
}
