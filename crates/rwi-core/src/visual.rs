//! Independent read-only visual captures. Never retained as input grounding.
use crate::{Engine, Identity, Result, digest, fault, lifecycle::WindowIdentity, operation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_VISUAL_SOURCE_PIXELS: u64 = 16_000_000;
pub const MAX_VISUAL_PNG_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_VISUAL_BASE64_BYTES: usize = MAX_VISUAL_PNG_BYTES.div_ceil(3) * 4;
pub const MAX_VISUAL_METADATA_BYTES: usize = 32 * 1024;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VisualFeedbackMode {
    #[default]
    Exclude,
    /// Do not actively exclude; a marker's presence in the image is not promised.
    Include,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct VisualOptions {
    pub max_edge: u32,
    pub feedback: VisualFeedbackMode,
}
impl Default for VisualOptions {
    fn default() -> Self {
        Self {
            max_edge: 1024,
            feedback: VisualFeedbackMode::Exclude,
        }
    }
}
impl VisualOptions {
    pub fn validate(self) -> Result<()> {
        if !(64..=2048).contains(&self.max_edge) {
            return Err(fault(
                "invalid_visual_limit",
                "max_edge must be 64..2048; the default is 1024. Small sources are never enlarged.",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VisualRect {
    /// Backend desktop coordinates: physical pixels on Windows, logical points
    /// on macOS. This response-only mapping never authorizes input.
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
impl VisualRect {
    pub fn validate(self) -> Result<()> {
        if self.width == 0
            || self.height == 0
            || u64::from(self.width) * u64::from(self.height) > MAX_VISUAL_SOURCE_PIXELS
            || i64::from(self.x) + i64::from(self.width) > i64::from(i32::MAX)
            || i64::from(self.y) + i64::from(self.height) > i64::from(i32::MAX)
        {
            return Err(fault(
                "visual_source_budget",
                "Use a valid backend desktop rectangle with at most 16 million source pixels/points; no silent clipping.",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VisualFeedback {
    pub requested: VisualFeedbackMode,
    pub suspension_ticket: Option<u64>,
    pub exclusion: String,
    pub before_copy: Value,
    pub after_copy: Value,
    pub after_release: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct VisualCapture {
    pub identity: Identity,
    pub window: WindowIdentity,
    pub source_rect: VisualRect,
    /// Exact capturable native pixel extent before the max_edge reduction.
    /// This can exceed source_rect on Retina displays, where that rectangle
    /// is measured in desktop points rather than physical pixels.
    /// An older peer may omit this field when decoding its legacy response;
    /// validation of a newly produced capture still rejects that unknown size.
    #[serde(default)]
    pub source_pixel_size: [u32; 2],
    pub image_size: [u32; 2],
    /// Descriptive pixel-center geometry only; not a resolve_view mapping or authority.
    pub visual_to_desktop: [f64; 6],
    pub captured_unix_ms: u64,
    pub assurance: String,
    pub atomicity: String,
    pub normalization: String,
    pub feedback: VisualFeedback,
    pub details: Value,
    #[serde(skip)]
    pub png: Vec<u8>,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct VisualObservation {
    pub visual_id: String,
    pub target: String,
    pub capture: VisualCapture,
    pub png_sha256: String,
    pub png_bytes: usize,
    pub base64_bytes: usize,
    pub frame_created: bool,
    pub input_authority_granted: bool,
    pub goal_verified: bool,
    pub retention: String,
}
fn validate_capture(v: &VisualCapture, options: VisualOptions) -> Result<()> {
    v.source_rect.validate()?;
    let source = v.source_pixel_size;
    if source.contains(&0) || u64::from(source[0]) * u64::from(source[1]) > MAX_VISUAL_SOURCE_PIXELS
    {
        return Err(fault(
            "visual_source_budget",
            "The native capturable source must contain at most 16 million pixels.",
        ));
    }
    let [w, h] = v.image_size;
    let edge = source[0].max(source[1]);
    let expected_size = if edge <= options.max_edge {
        source
    } else {
        source
            .map(|v| ((u64::from(v) * u64::from(options.max_edge)) / u64::from(edge)).max(1) as u32)
    };
    if w == 0 || h == 0 || w.max(h) > options.max_edge || v.image_size != expected_size {
        return Err(fault(
            "visual_output_budget",
            "The visual must fit max_edge without upscaling the source.",
        ));
    }
    let sx = f64::from(v.source_rect.width) / f64::from(w);
    let sy = f64::from(v.source_rect.height) / f64::from(h);
    let expected = [
        sx,
        0.,
        0.,
        sy,
        f64::from(v.source_rect.x) + (sx - 1.) / 2.,
        f64::from(v.source_rect.y) + (sy - 1.) / 2.,
    ];
    if v.visual_to_desktop != expected {
        return Err(fault(
            "visual_mapping_mismatch",
            "Visual pixel-center geometry must match its exact source rectangle and dimensions.",
        ));
    }
    if v.feedback.requested != options.feedback
        || v.window.window.is_empty()
        || v.window.pid == 0
        || v.window.process_created == 0
        || v.window.native_epoch.is_empty()
    {
        return Err(fault(
            "invalid_visual_capture",
            "Native window identity and requested feedback policy must remain explicit.",
        ));
    }
    if v.png.len() > MAX_VISUAL_PNG_BYTES || v.png.len().div_ceil(3) * 4 > MAX_VISUAL_BASE64_BYTES {
        return Err(fault(
            "visual_transport_budget",
            "Visual PNG exceeds 4 MiB or its separate base64 budget; request a smaller max_edge.",
        ));
    }
    // Check declared format and dimensions without decoding a second raster.
    if v.png.len() < 45
        || &v.png[..8] != b"\x89PNG\r\n\x1a\n"
        || &v.png[v.png.len() - 12..] != b"\0\0\0\0IEND\xaeB`\x82"
        || v.png[8..12] != 13u32.to_be_bytes()
        || &v.png[12..16] != b"IHDR"
        || v.png[16..20] != w.to_be_bytes()
        || v.png[20..24] != h.to_be_bytes()
    {
        return Err(fault(
            "invalid_visual_png",
            "Expected an encoded PNG whose IHDR matches the bounded visual dimensions.",
        ));
    }
    if serde_json::to_vec(v)
        .map_err(|_| {
            fault(
                "invalid_visual_capture",
                "Visual metadata serialization failed.",
            )
        })?
        .len()
        > MAX_VISUAL_METADATA_BYTES
    {
        return Err(fault(
            "visual_metadata_budget",
            "Visual metadata exceeds the independent 32 KiB bound.",
        ));
    }
    Ok(())
}
impl Engine {
    pub fn capture_visual(
        &mut self,
        target: &str,
        options: VisualOptions,
    ) -> Result<VisualObservation> {
        options.validate()?;
        operation::checkpoint("capture_visual_begin")?;
        let cleanup = operation::CleanupAudit::begin();
        let expected = self.identity(target)?;
        let capture = self.backend(target)?.capture_visual(options)?;
        operation::checkpoint("capture_visual_returned")?;
        if capture.identity != expected || self.backend(target)?.identity() != expected {
            return Err(fault(
                "session_mismatch",
                "Visual capture crossed its registered native identity; open the intended context.",
            ));
        }
        validate_capture(&capture, options)?;
        if cleanup.uncertain() {
            return Err(fault(
                "native_resource_release_uncertain",
                "Visual capture cleanup is unproven; inspect the execution. No visual/input authority is returned.",
            ));
        }
        operation::checkpoint("capture_visual_validated")?;
        let result = VisualObservation {
            visual_id: format!("visual:{}", crate::id()),
            target: target.into(),
            png_sha256: digest(&capture.png),
            png_bytes: capture.png.len(),
            base64_bytes: capture.png.len().div_ceil(3) * 4,
            capture,
            frame_created: false,
            input_authority_granted: false,
            goal_verified: false,
            retention: "response_only; not a frame, artifact, view or input reference".into(),
        };
        if serde_json::to_vec(&result)
            .map_err(|_| {
                fault(
                    "invalid_visual_capture",
                    "Visual result serialization failed.",
                )
            })?
            .len()
            > MAX_VISUAL_METADATA_BYTES
        {
            return Err(fault(
                "visual_metadata_budget",
                "Complete visual result metadata, including target and envelope, exceeds 32 KiB.",
            ));
        }
        Ok(result)
    }
}
#[cfg(test)]
#[path = "visual_tests.rs"]
mod tests;
