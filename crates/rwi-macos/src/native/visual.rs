//! Response-only exact-window visual capture. No AX tree, input frame or lease.
use super::*;
use crate::catalog::WindowRecord;
use rwi_core::{
    VisualCapture, VisualFeedback, VisualOptions, VisualRect, lifecycle::WindowIdentity,
};
use serde_json::json;

pub(super) struct VisualWindow {
    pub(super) record: WindowRecord,
    pub(super) displays: Vec<Display>,
    pub(super) rect: Rect,
    pub(super) on_screen_inventory_match: bool,
}

/// A failed read is not AX absence or input authority. Only an exact AXWindows
/// read failure or a completed, unambiguous AX search can use fresh CG/SCK pixels.
/// CannotComplete (-25204) retains the bounded settle attempts in observe and
/// the original diagnostic; permissions, identity and action failures stay errors.
pub(super) fn ax_unavailable_for_visual(error: &Fault) -> bool {
    if error.code == "window_missing" {
        return error.diagnostic.as_ref().is_some_and(|d| {
            d["expected_window"].is_u64()
                && d["ax_descendants_checked"].is_u64()
                && d["ax_search_truncated"] == false
        });
    }
    error.code == "ax_attribute_unavailable"
        && error.diagnostic.as_ref().is_some_and(|d| {
            d["api"] == "AXUIElementCopyAttributeValue"
                && d["attribute"] == "AXWindows"
                && matches!(d["ax_error"].as_i64(), Some(-25204 | -25205 | -25212))
                && d["operation_kind"] == "read"
                && d["replay_allowed"] == false
        })
}

fn visual_only_consistency(ax_error: &Fault, attempts: u8, capture_ms: u64) -> serde_json::Value {
    json!({
        "scope":"last_observe", "visual_only":true, "window_context_stable":true,
        "pixel_semantic_atomic":false, "controls_source":"unavailable",
        "input_reference_available":false, "input_revalidation_required":true,
        "attempts":attempts, "capture_ms":capture_ms,
        "ax_coverage_after":{"complete":false,"reason":"ax_read_failed"},
        "ax_error":ax_error
    })
}

fn visual_only_scene(
    identity: Identity,
    capture: crate::capture::Capture,
    ax_error: &Fault,
) -> Scene {
    Scene {
        identity,
        assurance: "native-observed-exact-window; visual-only; AX unavailable; no input authority"
            .into(),
        binding: None,
        mutation_ready: false,
        blocked: None,
        image_size: capture.size,
        pixel_to_desktop: None,
        elements: Vec::new(),
        captured_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        atomicity: format!(
            "non-atomic exact-window ScreenCaptureKit capture after AX failure {}; pre/post native session, process birth, window identity, bounds and display checks; no input frame",
            ax_error.code
        ),
        image_normalization: "ScreenCaptureKit native pixels -> straight RGBA PNG; original-size read-only artifact; no pixel-to-desktop input mapping".into(),
        png: capture.png,
    }
}

fn integer_visual_rect(rect: Rect) -> Result<VisualRect> {
    if !rect.valid() {
        return Err(fault(
            "visual_geometry_unknown",
            "The selected window has no usable visual rectangle.",
        ));
    }
    let values = [rect.x, rect.y, rect.w, rect.h];
    if values
        .iter()
        .any(|value| (value.round() - value).abs() > 0.001)
        || rect.x < f64::from(i32::MIN)
        || rect.y < f64::from(i32::MIN)
        || rect.x > f64::from(i32::MAX)
        || rect.y > f64::from(i32::MAX)
        || rect.w > f64::from(u32::MAX)
        || rect.h > f64::from(u32::MAX)
    {
        return Err(fault(
            "visual_geometry_fractional",
            "This visual channel requires integer macOS desktop-point bounds; no rounded coordinate mapping is substituted.",
        ));
    }
    let source = VisualRect {
        x: rect.x as i32,
        y: rect.y as i32,
        width: rect.w as u32,
        height: rect.h as u32,
    };
    source.validate()?;
    Ok(source)
}

fn process_birth(process: &str, pid: u32) -> Result<u64> {
    let mut parts = process.split(':');
    let parsed = parts.next().and_then(|value| value.parse::<u32>().ok());
    let seconds = parts.next().and_then(|value| value.parse::<u64>().ok());
    let micros = parts.next().and_then(|value| value.parse::<u64>().ok());
    if parsed != Some(pid)
        || parts.next().is_some()
        || micros.is_none_or(|value| value >= 1_000_000)
    {
        return Err(fault(
            "process_identity_unknown",
            "The pinned native process birth could not be represented for visual metadata.",
        ));
    }
    seconds
        .zip(micros)
        .and_then(|(seconds, micros)| seconds.checked_mul(1_000_000)?.checked_add(micros))
        .filter(|value| *value != 0)
        .ok_or_else(|| {
            fault(
                "process_identity_unknown",
                "The native process birth is invalid.",
            )
        })
}

// A response-only record is separate from input eligibility.
fn record_for_visual(
    identity: &Identity,
    window: &Window,
    process: String,
    displays: &[Display],
) -> Result<WindowRecord> {
    // Exact-window SCK capture owns its point/pixel scale. A containing display
    // is descriptive only here; absence must not invent one or grant input.
    // Native session, process, on-screen identity and pre/post topology guards
    // remain at the caller. Normal metadata/input still require valid mapping.
    WindowRecord::from_native(
        identity,
        window,
        process,
        containing_display(displays, window.rect),
    )
}

mod additional;
mod selection;

impl NativeMac {
    #[cfg(test)]
    pub(crate) fn fixture_for_live() -> Self {
        Self {
            pid: 1,
            window: 1,
            identity: Identity {
                host: "fixture".into(),
                guest: None,
                session: "fixture".into(),
                seat: "fixture".into(),
                desktop: "fixture".into(),
            },
            process: "1:1:1".into(),
            allow_input: false,
            semantic_only: false,
            capturer: Capturer::default(),
            hit_diagnostics: std::sync::Mutex::new(vec![]),
            dispatches: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            objects: std::sync::Mutex::new(super::objects::Objects::default()),
            last_observation: std::sync::Mutex::new(serde_json::Value::Null),
            text_effect: None,
        }
    }
    /// Independent capture slot for a bounded local viewer; no input capability.
    pub(crate) fn visual_copy(&self) -> Result<Self> {
        if self.semantic_only {
            return Err(fault(
                "semantic_surface_has_no_pixels",
                "This logical AX dialog has no asserted visible surface. Keep visual selection separate.",
            ));
        }
        let copy = Self::with_shared_resources(
            self.pid,
            self.window,
            false,
            Capturer::default(),
            self.dispatches.clone(),
        )?;
        if copy.identity != self.identity || copy.process != self.process {
            return Err(fault(
                "session_or_process_changed",
                "Live source identity changed before subscription.",
            ));
        }
        Ok(copy)
    }

    /// Give AX-incompatible windows a read-only original-pixel snapshot. The
    /// first and second CG inventory samples plus SCK's exact window/PID/rect
    /// check are independent of the failed AX lookup. No Binding, transform or
    /// controls are created, so neither this frame nor its image tiles can
    /// become an input reference.
    pub(super) fn observe_visual_only(&self, ax_error: &Fault, attempts: u8) -> Result<Scene> {
        operation::checkpoint("macos_visual_only_begin")?;
        let before = self.visual_window()?;
        let phase = Instant::now();
        let capture = self.capturer.capture(self.pid, self.window, before.rect)?;
        let capture_ms = phase.elapsed().as_millis() as u64;
        operation::checkpoint("macos_visual_only_post_capture")?;
        let after = self.visual_window()?;
        if before.record != after.record
            || before.displays != after.displays
            || before.on_screen_inventory_match != after.on_screen_inventory_match
        {
            return Err(fault(
                "visual_window_changed",
                "Window identity, geometry or display topology changed during visual-only capture; discard this image.",
            ));
        }
        if let Ok(mut saved) = self.last_observation.lock() {
            *saved = visual_only_consistency(ax_error, attempts, capture_ms);
            saved["capture_phases"] = capture.timings.details();
        }
        Ok(visual_only_scene(self.identity.clone(), capture, ax_error))
    }
    pub(super) fn visual_window(&self) -> Result<VisualWindow> {
        if console_identity()? != self.identity || process_epoch(self.pid)? != self.process {
            return Err(fault(
                "session_or_process_changed",
                "Visual capture requires the original macOS user/session and process birth.",
            ));
        }
        let displays = displays()?;
        let (window, on_screen_inventory_match) = selection::read(self.pid, self.window)?;
        let record = record_for_visual(&self.identity, &window, self.process.clone(), &displays)?;
        Ok(VisualWindow {
            record,
            displays,
            rect: window.rect,
            on_screen_inventory_match,
        })
    }

    pub(crate) fn visual_capture(&self, options: VisualOptions) -> Result<VisualCapture> {
        if self.semantic_only {
            return Err(fault(
                "semantic_surface_has_no_pixels",
                "Select an independently discovered visible window for pixels; no AX-to-CG surrogate is inferred.",
            ));
        }
        options.validate()?;
        operation::checkpoint("macos_visual_begin")?;
        let before = self.visual_window()?;
        let source_rect = integer_visual_rect(before.rect)?;
        let before_feedback = crate::overlay::status_json();
        let capture = self.capturer.capture_visual(
            self.pid,
            self.window,
            before.rect,
            options.max_edge,
            !before.on_screen_inventory_match,
        )?;
        let captured_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        operation::checkpoint("macos_visual_post_capture")?;
        let after = self.visual_window()?;
        if before.record != after.record
            || before.displays != after.displays
            || before.on_screen_inventory_match != after.on_screen_inventory_match
        {
            return Err(fault(
                "visual_window_changed",
                "Window identity, geometry or display topology changed during capture; discard this image.",
            ));
        }
        let image_size = capture.size;
        if image_size
            .iter()
            .any(|size| *size == 0 || *size > options.max_edge)
        {
            return Err(fault(
                "visual_output_budget",
                "ScreenCaptureKit output exceeds the requested bounded visual size.",
            ));
        }
        let after_feedback = crate::overlay::status_json();
        let sx = f64::from(source_rect.width) / f64::from(image_size[0]);
        let sy = f64::from(source_rect.height) / f64::from(image_size[1]);
        Ok(VisualCapture {
            identity: self.identity.clone(),
            window: WindowIdentity {
                window: before.record.window,
                pid: self.pid,
                process_created: process_birth(&self.process, self.pid)?,
                native_epoch: before.record.native_epoch,
            },
            source_rect,
            source_pixel_size: capture.native_size,
            image_size,
            visual_to_desktop: [
                sx,
                0.0,
                0.0,
                sy,
                f64::from(source_rect.x) + (sx - 1.0) / 2.0,
                f64::from(source_rect.y) + (sy - 1.0) / 2.0,
            ],
            captured_unix_ms,
            assurance: "native-observed-exact-window; no continuous window-birth attestation".into(),
            atomicity: "non-atomic ScreenCaptureKit exact-window capture; pre/post PID, process birth, bounds, title and display topology checks; separate panels are not included".into(),
            normalization: "macOS desktop-point source -> ScreenCaptureKit native pointPixelScale, bounded to max_edge -> straight RGBA PNG; response-only visual, not an input frame".into(),
            feedback: VisualFeedback {
                requested: options.feedback,
                suspension_ticket: None,
                exclusion: "The exact-window filter does not composite the separate RWI Agent feedback window; include does not promise a marker.".into(),
                before_copy: before_feedback,
                after_copy: after_feedback.clone(),
                after_release: after_feedback,
            },
            details: json!({
                "capture_backend":"ScreenCaptureKit.SCScreenshotManager",
                "capture_phases":capture.timings.brief(),
                "source_space":"macos_desktop_points",
                "title": before.record.title,
                "display": before.record.display,
                "capture_scope": "exact_window_local",
                "containing_display_required": false,
                "on_screen_inventory_match": before.on_screen_inventory_match,
                "native_record_scope": "exact ID/PID; offscreen record does not classify hidden/minimized",
                "ax_walk": false,
                "input_authority": false,
                "frame_created": false,
                "selection_updated": false,
                "output_png_bytes": capture.png.len(),
            }),
            png: capture.png,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ax_failure(attribute: &str, code: i32) -> Fault {
        let mut error = fault("ax_attribute_unavailable", "offline AX failure");
        error.diagnostic = Some(Box::new(json!({
            "api":"AXUIElementCopyAttributeValue", "attribute":attribute,
            "ax_error":code, "operation_kind":"read", "replay_allowed":false
        })));
        error
    }

    #[test]
    fn visual_only_fallback_requires_exact_read_failure_not_identity_or_action_loss() {
        assert!(ax_unavailable_for_visual(&ax_failure("AXWindows", -25205)));
        assert!(ax_unavailable_for_visual(&ax_failure("AXWindows", -25212)));
        assert!(ax_unavailable_for_visual(&ax_failure("AXWindows", -25204)));
        assert!(!ax_unavailable_for_visual(&ax_failure(
            "AXChildren",
            -25205
        )));
        let mut missing = fault("window_missing", "no matching AX window");
        missing.diagnostic = Some(Box::new(json!({
            "expected_window":42, "ax_descendants_checked":3,
            "ax_search_truncated":false
        })));
        assert!(ax_unavailable_for_visual(&missing));
        missing.diagnostic.as_mut().unwrap()["ax_search_truncated"] = json!(true);
        assert!(!ax_unavailable_for_visual(&missing));
        assert!(!ax_unavailable_for_visual(&fault(
            "window_missing",
            "native window gone"
        )));
        for code in [
            "session_or_process_changed",
            "window_epoch_changed",
            "window_binding_ambiguous",
            "visual_window_changed",
            "screen_capture_denied",
            "accessibility_permission_missing",
        ] {
            assert!(!ax_unavailable_for_visual(&fault(
                code,
                "must remain an error"
            )));
        }
    }

    #[test]
    fn visual_only_scene_has_no_input_binding_mapping_or_controls() {
        let identity = Identity {
            host: "offline-host".into(),
            guest: None,
            session: "offline-session".into(),
            seat: "offline-seat".into(),
            desktop: "offline-desktop".into(),
        };
        let png = vec![1, 2, 3];
        let scene = visual_only_scene(
            identity.clone(),
            crate::capture::Capture {
                png: png.clone(),
                size: [2, 2],
                native_size: [2, 2],
                content_sha: String::new(),
                timings: Default::default(),
            },
            &ax_failure("AXWindows", -25205),
        );
        assert_eq!(scene.identity, identity);
        assert_eq!(scene.png, png);
        assert_eq!(scene.image_size, [2, 2]);
        assert!(scene.binding.is_none());
        assert!(scene.pixel_to_desktop.is_none());
        assert!(!scene.mutation_ready);
        assert!(scene.blocked.is_none());
        assert!(scene.elements.is_empty());
        assert!(scene.assurance.contains("visual-only"));
    }

    #[test]
    fn integer_desktop_rect_and_visual_size_are_exact_and_bounded() {
        let rect = integer_visual_rect(Rect {
            x: -1200.0,
            y: 24.0,
            w: 1434.0,
            h: 660.0,
        })
        .unwrap();
        assert_eq!(
            rect,
            VisualRect {
                x: -1200,
                y: 24,
                width: 1434,
                height: 660
            }
        );
        assert_eq!(
            integer_visual_rect(Rect {
                x: 0.5,
                y: 0.0,
                w: 100.0,
                h: 100.0
            })
            .unwrap_err()
            .code,
            "visual_geometry_fractional"
        );
    }

    #[test]
    fn process_birth_rejects_wrong_pid_and_fractional_microsecond_overflow() {
        assert_eq!(
            process_birth("42:1790246422:436437", 42).unwrap(),
            1790246422436437
        );
        assert!(process_birth("43:1790246422:436437", 42).is_err());
        assert!(process_birth("42:1790246422:1000000", 42).is_err());
        assert!(process_birth("42:0:0", 42).is_err());
    }
    #[test]
    fn visual_only_keeps_observation_available_for_exact_read_timeout_without_input_authority() {
        let timeout = ax_failure("AXWindows", -25204);
        assert!(ax_unavailable_for_visual(&timeout));
        let scene = visual_only_scene(
            NativeMac::fixture_for_live().identity,
            crate::capture::Capture {
                png: vec![1, 2, 3],
                size: [2, 2],
                native_size: [2, 2],
                content_sha: String::new(),
                timings: Default::default(),
            },
            &timeout,
        );
        assert!(scene.binding.is_none());
        assert!(scene.pixel_to_desktop.is_none());
        assert!(!scene.mutation_ready);
        assert!(scene.elements.is_empty());
        let consistency = visual_only_consistency(&timeout, 3, 42);
        assert_eq!(consistency["ax_error"], json!(timeout));
        assert_eq!(consistency["ax_error"]["diagnostic"]["ax_error"], -25204);
        assert_eq!(consistency["ax_coverage_after"]["complete"], false);
        assert_eq!(consistency["input_reference_available"], false);
        assert_eq!(consistency["attempts"], 3);
    }

    #[test]
    fn read_timeout_visual_policy_rejects_unknown_attribute_action_and_replay() {
        assert!(!ax_unavailable_for_visual(&ax_failure(
            "AXChildren",
            -25204
        )));
        assert!(!ax_unavailable_for_visual(&ax_failure("AXWindows", -25211)));
        for (key, value) in [
            ("api", json!("AXUIElementPerformAction")),
            ("attribute", json!("AXFocusedWindow")),
            ("operation_kind", json!("action")),
            ("replay_allowed", json!(true)),
        ] {
            let mut timeout = ax_failure("AXWindows", -25204);
            timeout.diagnostic.as_mut().unwrap()[key] = value;
            assert!(!ax_unavailable_for_visual(&timeout), "{key}");
        }
        assert!(!ax_unavailable_for_visual(&fault(
            "ax_attribute_unavailable",
            "no diagnostic"
        )));
    }
}

#[cfg(test)]
mod window_local_visual_contract {
    use super::*;
    fn identity() -> Identity {
        Identity {
            host: "offline".into(),
            guest: None,
            session: "own".into(),
            seat: "seat".into(),
            desktop: "desktop".into(),
        }
    }
    fn display() -> Display {
        Display {
            id: 4,
            uuid: Some("own-virtual".into()),
            built_in: false,
            rect: Rect {
                x: 1800.,
                y: 0.,
                w: 1920.,
                h: 1080.,
            },
            pixels: [1920, 1080],
            rotation: 0.,
        }
    }
    fn window(x: f64) -> Window {
        Window {
            id: 128,
            pid: 42,
            rect: Rect {
                x,
                y: 24.,
                w: 660.,
                h: 552.,
            },
            alpha: 1.,
            title: Some("own".into()),
        }
    }
    #[test]
    fn window_local_visual_record_does_not_require_single_display_input_mapping() {
        let screens = [display()];
        let own = window(3600.);
        assert!(containing_display(&screens, own.rect).is_none());
        let record = record_for_visual(&identity(), &own, "42:100:1".into(), &screens)
            .expect("exact-window visual metadata must not require full-screen containment");
        assert!(record.display.is_none());
        assert_eq!(record.window, "cgwindow:128");
        assert_eq!(record.pid, 42);
    }
    #[test]
    fn window_local_visual_record_retains_single_display_and_invalid_geometry_checks() {
        let screens = [display()];
        let own = window(1824.);
        let record = record_for_visual(&identity(), &own, "42:100:1".into(), &screens).unwrap();
        assert_eq!(record.display.unwrap().id, 4);
        let bad = Window {
            rect: Rect {
                w: f64::NAN,
                ..own.rect
            },
            ..own
        };
        assert!(record_for_visual(&identity(), &bad, "42:100:1".into(), &screens).is_err());
    }
}
