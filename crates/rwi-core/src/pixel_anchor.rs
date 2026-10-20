//! Bounded local visual grounding for a native bitmap canvas with animated
//! selection chrome. This never substitutes for native window/AX hit checks.
use std::{collections::VecDeque, io::Cursor};
mod exact_context;
mod red_marker;
pub use red_marker::RedMarkerAnchor;

const EDGE: u32 = 96;
const MIN_EDGE_CLEARANCE: u32 = 64;
const GRID: usize = 16;
const CELL: usize = 6;
const CHANNELS: usize = 3;
const MAX_PIXELS: u64 = 16_000_000;
const CHANGED_CELL_DELTA: u8 = 3;
const MAX_CHANGED_CELLS: usize = 32;
const HIT_REGION_RADIUS_CELLS: usize = 4;
const MAX_HIT_REGION_DELTA: u32 = 384;
const MAX_STABLE_CONTEXT_DELTA: u32 = 384;
const MAX_CENTRAL_DELTA: u8 = 16;
const RED_MARKER_MIN_AREA: usize = 12;
const RED_MARKER_MAX_AREA: usize = 256;
const RED_MARKER_MIN_EDGE: usize = 3;
const RED_MARKER_MAX_EDGE: usize = 20;
const RED_MARKER_MIN_DENSITY: f64 = 0.25;
const RED_MARKER_CLUSTER_LINK_PX: f64 = 22.0;
const RED_MARKER_TARGET_RADIUS_PX: f64 = 5.0;
const RED_MARKER_CENTER_DELTA_PX: f64 = 0.75;
const RED_MARKER_MIN_IOU: f64 = 0.90;
const RED_MARKER_MIN_CLUSTER: usize = 3;
const RED_MARKER_MAX_CLUSTER: usize = 16;

/// Original construction failure, not a later-frame diagnosis or input grant.
/// Only bounded stage labels are retained; no pixels or decoder error text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CreationFailure {
    ImageTooSmall,
    PixelNotFinite,
    PixelEdgeClearance,
    PngByteBudgetExceeded,
    ImagePixelBudgetExceeded,
    PngHeaderLimitExceeded,
    PngHeaderDecodeFailed,
    PngDimensionsMismatch,
    DecodedBufferSizeUnavailable,
    DecodedBufferBudgetExceeded,
    PngFrameLimitExceeded,
    PngFrameDecodeFailed,
    UnsupportedBitDepth,
    UnsupportedColorType,
    DecodedBufferIncomplete,
    DecodedLengthMismatch,
    LumaSpanBelowMinimum,
}
impl CreationFailure {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ImageTooSmall => "image_too_small",
            Self::PixelNotFinite => "pixel_not_finite",
            Self::PixelEdgeClearance => "pixel_edge_clearance",
            Self::PngByteBudgetExceeded => "png_byte_budget_exceeded",
            Self::ImagePixelBudgetExceeded => "image_pixel_budget_exceeded",
            Self::PngHeaderLimitExceeded => "png_header_limit_exceeded",
            Self::PngHeaderDecodeFailed => "png_header_decode_failed",
            Self::PngDimensionsMismatch => "png_dimensions_mismatch",
            Self::DecodedBufferSizeUnavailable => "decoded_buffer_size_unavailable",
            Self::DecodedBufferBudgetExceeded => "decoded_buffer_budget_exceeded",
            Self::PngFrameLimitExceeded => "png_frame_limit_exceeded",
            Self::PngFrameDecodeFailed => "png_frame_decode_failed",
            Self::UnsupportedBitDepth => "unsupported_bit_depth",
            Self::UnsupportedColorType => "unsupported_color_type",
            Self::DecodedBufferIncomplete => "decoded_buffer_incomplete",
            Self::DecodedLengthMismatch => "decoded_length_mismatch",
            Self::LumaSpanBelowMinimum => "luma_span_below_minimum",
        }
    }
}

#[cfg(test)]
thread_local! {
    static FEATURE_DECODE_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_feature_decode_count() {
    FEATURE_DECODE_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn feature_decode_count() -> usize {
    FEATURE_DECODE_COUNT.with(std::cell::Cell::get)
}

#[derive(serde::Serialize)]
struct Comparison {
    matched: bool,
    reason: &'static str,
    changed_cells: Option<usize>,
    hit_changed_cells: Option<usize>,
    context_changed_cells: Option<usize>,
    context_trimmed_changed_cells: Option<usize>,
    stable_context_changed_cells: Option<usize>,
    total_delta: Option<u32>,
    hit_region_delta: Option<u32>,
    stable_context_delta: Option<u32>,
    central_max_delta: Option<u8>,
    failed_checks: Vec<&'static str>,
}
impl Comparison {
    fn unavailable(reason: &'static str) -> Self {
        Self {
            matched: false,
            reason,
            changed_cells: None,
            hit_changed_cells: None,
            context_changed_cells: None,
            context_trimmed_changed_cells: None,
            stable_context_changed_cells: None,
            total_delta: None,
            hit_region_delta: None,
            stable_context_delta: None,
            central_max_delta: None,
            failed_checks: vec![reason],
        }
    }
}

#[derive(Debug, Clone)]
pub struct PixelAnchor {
    image_size: [u32; 2],
    pixel: [f64; 2],
    desktop: [f64; 2],
    origin: [u32; 2],
    features: Vec<u8>,
    context: Option<exact_context::ExactContext>,
}

impl PixelAnchor {
    // Internal Option compatibility oracle; production retains the diagnosed
    // Result so an original construction failure is not silently discarded.
    #[cfg(test)]
    pub(crate) fn from_png(
        png: &[u8],
        image_size: [u32; 2],
        pixel: [f64; 2],
        desktop: [f64; 2],
    ) -> Option<Self> {
        Self::from_png_diagnosed(png, image_size, pixel, desktop).ok()
    }

    pub(crate) fn from_png_diagnosed(
        png: &[u8],
        image_size: [u32; 2],
        pixel: [f64; 2],
        desktop: [f64; 2],
    ) -> Result<Self, CreationFailure> {
        if image_size[0] < EDGE || image_size[1] < EDGE {
            return Err(CreationFailure::ImageTooSmall);
        }
        if pixel.iter().any(|value| !value.is_finite()) {
            return Err(CreationFailure::PixelNotFinite);
        }
        if pixel[0] < f64::from(MIN_EDGE_CLEARANCE)
            || pixel[1] < f64::from(MIN_EDGE_CLEARANCE)
            || pixel[0] >= f64::from(image_size[0] - MIN_EDGE_CLEARANCE)
            || pixel[1] >= f64::from(image_size[1] - MIN_EDGE_CLEARANCE)
        {
            return Err(CreationFailure::PixelEdgeClearance);
        }
        let origin = [
            pixel[0].floor() as u32 - EDGE / 2,
            pixel[1].floor() as u32 - EDGE / 2,
        ];
        let (features, context) =
            features_with_context_diagnosed(png, image_size, origin, Some(pixel))?;
        // A blank center requires a bounded, textured surrounding context that
        // will match *exactly*. A wholly uniform surface keeps full-frame equality.
        let (low, high) = features
            .chunks_exact(CHANNELS)
            .fold((255u8, 0u8), |(low, high), rgb| {
                let luma = ((u16::from(rgb[0]) + u16::from(rgb[1]) + u16::from(rgb[2])) / 3) as u8;
                (low.min(luma), high.max(luma))
            });
        if high.saturating_sub(low) < 32 && context.is_none() {
            return Err(CreationFailure::LumaSpanBelowMinimum);
        }
        Ok(Self {
            image_size,
            pixel,
            desktop,
            origin,
            features,
            context,
        })
    }

    pub(crate) fn matches(
        &self,
        png: &[u8],
        image_size: [u32; 2],
        desktop: [f64; 2],
        transform: Option<[f64; 6]>,
    ) -> bool {
        self.compare(png, image_size, desktop, transform).matched
    }
    /// Error-path readback of the same comparator; aggregate metrics only, no
    /// pixels, document text, new frame or input authority. No threshold tuning.
    pub fn diagnostic(
        &self,
        png: &[u8],
        image_size: [u32; 2],
        desktop: [f64; 2],
        transform: Option<[f64; 6]>,
    ) -> serde_json::Value {
        let mut value = serde_json::to_value(self.compare(png, image_size, desktop, transform))
            .expect("finite comparison");
        value["limits"] = serde_json::json!({"patch_edge":EDGE,"grid":GRID,"changed_cell_delta":CHANGED_CELL_DELTA,"max_changed_cells":MAX_CHANGED_CELLS,"ignored_high_delta_context_cells":MAX_CHANGED_CELLS,"hit_region_radius_cells":HIT_REGION_RADIUS_CELLS,"max_hit_region_delta":MAX_HIT_REGION_DELTA,"max_stable_context_delta":MAX_STABLE_CONTEXT_DELTA,"max_central_delta":MAX_CENTRAL_DELTA});
        value["input_authority"] = serde_json::json!(false);
        if let Some(context) = &self.context {
            value["anchor_kind"] = serde_json::json!("exact_textured_context");
            value["context"] = context.diagnostic();
        }
        value
    }
    fn compare(
        &self,
        png: &[u8],
        image_size: [u32; 2],
        desktop: [f64; 2],
        transform: Option<[f64; 6]>,
    ) -> Comparison {
        if image_size != self.image_size {
            return Comparison::unavailable("image_dimensions_changed");
        }
        if desktop != self.desktop {
            return Comparison::unavailable("desktop_point_changed");
        }
        let Some([a, b, c, d, e, f]) = transform else {
            return Comparison::unavailable("transform_missing");
        };
        let mapped = [
            a * self.pixel[0] + c * self.pixel[1] + e,
            b * self.pixel[0] + d * self.pixel[1] + f,
        ];
        if !mapped
            .iter()
            .zip(desktop)
            .all(|(x, y)| (x - y).abs() <= 0.25)
        {
            return Comparison::unavailable("transform_changed");
        }
        if let Some(expected_context) = &self.context {
            let Ok((_, current_context)) =
                features_with_context_diagnosed(png, image_size, self.origin, Some(self.pixel))
            else {
                return Comparison::unavailable("patch_decode_unavailable");
            };
            let matched = current_context.as_ref() == Some(expected_context);
            let mut result = Comparison::unavailable("context_patch_changed");
            if matched {
                result.matched = true;
                result.reason = "matched";
                result.failed_checks.clear();
            }
            return result;
        }
        let Some(now) = features(png, image_size, self.origin) else {
            return Comparison::unavailable("patch_decode_unavailable");
        };
        let mut changed_cells = 0usize;
        let mut hit_changed_cells = 0usize;
        let mut context_changed_cells = 0usize;
        let mut total_delta = 0u32;
        let mut hit_region_delta = 0u32;
        let mut central_max_delta = 0u8;
        let mut context_deltas = Vec::with_capacity(GRID * GRID);
        for (index, (before, after)) in self
            .features
            .chunks_exact(CHANNELS)
            .zip(now.chunks_exact(CHANNELS))
            .enumerate()
        {
            let delta = before
                .iter()
                .zip(after)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap_or(0);
            let row = index / GRID;
            let column = index % GRID;
            let center = GRID / 2;
            let in_hit_region =
                (center - HIT_REGION_RADIUS_CELLS..center + HIT_REGION_RADIUS_CELLS).contains(&row)
                    && (center - HIT_REGION_RADIUS_CELLS..center + HIT_REGION_RADIUS_CELLS)
                        .contains(&column);
            if in_hit_region {
                hit_region_delta += u32::from(delta);
                hit_changed_cells += usize::from(delta > CHANGED_CELL_DELTA);
            } else {
                context_deltas.push(delta);
                context_changed_cells += usize::from(delta > CHANGED_CELL_DELTA);
            }
            changed_cells += usize::from(delta > CHANGED_CELL_DELTA);
            total_delta += u32::from(delta);
            // The four cells directly under the click may animate, but must
            // not change enough to indicate a replaced small target.
            if (7..=8).contains(&(index / GRID)) && (7..=8).contains(&(index % GRID)) {
                central_max_delta = central_max_delta.max(delta);
            }
        }
        // Never trim changes in the 48x48 hit region around the click point:
        // small subject movement must remain detectable even when the exact
        // center is unchanged. Outside that region, a bounded number of high-
        // change cells may be local animation; broad drift still fails.
        context_deltas.sort_unstable();
        let (stable_context, trimmed_context) =
            context_deltas.split_at(context_deltas.len().saturating_sub(MAX_CHANGED_CELLS));
        let stable_context_delta = stable_context
            .iter()
            .map(|delta| u32::from(*delta))
            .sum::<u32>();
        // Diagnostics only: reuse this comparison's deltas, without decoding a
        // second patch, changing the budgets, or treating trimmed cells as safe.
        let count_changed = |cells: &[u8]| {
            cells
                .iter()
                .filter(|delta| **delta > CHANGED_CELL_DELTA)
                .count()
        };
        let stable_context_changed_cells = count_changed(stable_context);
        let context_trimmed_changed_cells = count_changed(trimmed_context);
        let mut failed_checks = Vec::new();
        for (failed, check) in [
            (
                central_max_delta > MAX_CENTRAL_DELTA,
                "central_patch_changed",
            ),
            (
                changed_cells > MAX_CHANGED_CELLS,
                "changed_cell_budget_exceeded",
            ),
            (
                hit_region_delta > MAX_HIT_REGION_DELTA,
                "hit_region_delta_budget_exceeded",
            ),
            (
                stable_context_delta > MAX_STABLE_CONTEXT_DELTA,
                "stable_context_delta_budget_exceeded",
            ),
        ] {
            if failed {
                failed_checks.push(check);
            }
        }
        let reason = if central_max_delta > MAX_CENTRAL_DELTA {
            "central_patch_changed"
        } else if changed_cells > MAX_CHANGED_CELLS {
            "changed_cell_budget_exceeded"
        } else if hit_region_delta > MAX_HIT_REGION_DELTA {
            "hit_region_delta_budget_exceeded"
        } else if stable_context_delta > MAX_STABLE_CONTEXT_DELTA {
            "stable_context_delta_budget_exceeded"
        } else {
            "matched"
        };
        Comparison {
            matched: reason == "matched",
            reason,
            changed_cells: Some(changed_cells),
            hit_changed_cells: Some(hit_changed_cells),
            context_changed_cells: Some(context_changed_cells),
            context_trimmed_changed_cells: Some(context_trimmed_changed_cells),
            stable_context_changed_cells: Some(stable_context_changed_cells),
            total_delta: Some(total_delta),
            hit_region_delta: Some(hit_region_delta),
            stable_context_delta: Some(stable_context_delta),
            central_max_delta: Some(central_max_delta),
            failed_checks,
        }
    }
}

fn features(png: &[u8], expected: [u32; 2], origin: [u32; 2]) -> Option<Vec<u8>> {
    features_diagnosed(png, expected, origin).ok()
}

fn features_diagnosed(
    png: &[u8],
    expected: [u32; 2],
    origin: [u32; 2],
) -> Result<Vec<u8>, CreationFailure> {
    features_with_context_diagnosed(png, expected, origin, None).map(|(features, _)| features)
}

fn features_with_context_diagnosed(
    png: &[u8],
    expected: [u32; 2],
    origin: [u32; 2],
    pixel: Option<[f64; 2]>,
) -> Result<(Vec<u8>, Option<exact_context::ExactContext>), CreationFailure> {
    #[cfg(test)]
    FEATURE_DECODE_COUNT.with(|count| count.set(count.get() + 1));
    if png.len() > crate::MAX_FRAME_BYTES {
        return Err(CreationFailure::PngByteBudgetExceeded);
    }
    if u64::from(expected[0]) * u64::from(expected[1]) > MAX_PIXELS {
        return Err(CreationFailure::ImagePixelBudgetExceeded);
    }
    let mut decoder = png::Decoder::new(Cursor::new(png));
    decoder.set_limits(png::Limits {
        bytes: (MAX_PIXELS * 4) as usize,
    });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| match error {
        png::DecodingError::LimitsExceeded => CreationFailure::PngHeaderLimitExceeded,
        _ => CreationFailure::PngHeaderDecodeFailed,
    })?;
    if [reader.info().width, reader.info().height] != expected {
        return Err(CreationFailure::PngDimensionsMismatch);
    }
    let n = reader
        .output_buffer_size()
        .ok_or(CreationFailure::DecodedBufferSizeUnavailable)?;
    if n > (MAX_PIXELS * 4) as usize {
        return Err(CreationFailure::DecodedBufferBudgetExceeded);
    }
    let mut decoded = vec![0; n];
    let info = reader
        .next_frame(&mut decoded)
        .map_err(|error| match error {
            png::DecodingError::LimitsExceeded => CreationFailure::PngFrameLimitExceeded,
            _ => CreationFailure::PngFrameDecodeFailed,
        })?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err(CreationFailure::UnsupportedBitDepth);
    }
    let channels = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        _ => return Err(CreationFailure::UnsupportedColorType),
    };
    let bytes = decoded
        .get(..info.buffer_size())
        .ok_or(CreationFailure::DecodedBufferIncomplete)?;
    if bytes.len() != expected[0] as usize * expected[1] as usize * channels {
        return Err(CreationFailure::DecodedLengthMismatch);
    }
    let mut out = Vec::with_capacity(GRID * GRID * CHANNELS);
    let stride = expected[0] as usize * channels;
    for gy in 0..GRID {
        for gx in 0..GRID {
            let mut sums = [0u32; CHANNELS];
            for dy in 0..CELL {
                for dx in 0..CELL {
                    let y = origin[1] as usize + gy * CELL + dy;
                    let x = origin[0] as usize + gx * CELL + dx;
                    let offset = y * stride + x * channels;
                    let px = &bytes[offset..offset + channels];
                    let (rgb, alpha) = match channels {
                        4 => ([px[0], px[1], px[2]], px[3]),
                        3 => ([px[0], px[1], px[2]], 255),
                        2 => ([px[0]; 3], px[1]),
                        _ => ([px[0]; 3], 255),
                    };
                    for channel in 0..CHANNELS {
                        sums[channel] += (u32::from(rgb[channel]) * u32::from(alpha)
                            + 255 * (255 - u32::from(alpha))
                            + 127)
                            / 255;
                    }
                }
            }
            for sum in sums {
                out.push(((sum + 18) / 36) as u8);
            }
        }
    }
    let (low, high) = out
        .chunks_exact(CHANNELS)
        .fold((255u8, 0u8), |(low, high), rgb| {
            let luma = ((u16::from(rgb[0]) + u16::from(rgb[1]) + u16::from(rgb[2])) / 3) as u8;
            (low.min(luma), high.max(luma))
        });
    let context = pixel
        .filter(|_| high.saturating_sub(low) < 32)
        .and_then(|pixel| {
            exact_context::ExactContext::from_decoded(bytes, expected, channels, pixel)
        });
    Ok((out, context))
}

#[cfg(test)]
mod tests;
