//! Bounded local visual grounding for a native bitmap canvas with animated
//! selection chrome. This never substitutes for native window/AX hit checks.
use std::{collections::VecDeque, io::Cursor};

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
}

/// Explicit, local-only signature for a compact cluster of red markers. It is
/// separate from ordinary pixel references so dynamic canvas users must opt in.
#[derive(Debug, Clone)]
pub struct RedMarkerAnchor {
    image_size: [u32; 2],
    pixel: [f64; 2],
    desktop: [f64; 2],
    origin: [u32; 2],
    markers: Vec<MarkerComponent>,
}

#[derive(Debug, Clone)]
struct MarkerComponent {
    bounds: [u8; 4],
    center: [f64; 2],
    pixels: Vec<u16>,
    target: bool,
}

#[derive(serde::Serialize)]
struct MarkerComparison {
    matched: bool,
    reason: &'static str,
    baseline_markers: usize,
    current_candidates: usize,
    matched_markers: usize,
    target_marker_matched: bool,
    maximum_center_delta_px: Option<f64>,
    minimum_shape_iou: Option<f64>,
}

impl RedMarkerAnchor {
    pub(crate) fn from_png(
        png: &[u8],
        image_size: [u32; 2],
        pixel: [f64; 2],
        desktop: [f64; 2],
    ) -> Option<Self> {
        if image_size[0] < EDGE
            || image_size[1] < EDGE
            || pixel.iter().any(|value| !value.is_finite())
            || pixel[0] < f64::from(MIN_EDGE_CLEARANCE)
            || pixel[1] < f64::from(MIN_EDGE_CLEARANCE)
            || pixel[0] >= f64::from(image_size[0] - MIN_EDGE_CLEARANCE)
            || pixel[1] >= f64::from(image_size[1] - MIN_EDGE_CLEARANCE)
        {
            return None;
        }
        let origin = [
            pixel[0].floor() as u32 - EDGE / 2,
            pixel[1].floor() as u32 - EDGE / 2,
        ];
        let mut candidates = red_marker_components(png, image_size, origin)?;
        let mut targets = candidates
            .iter()
            .enumerate()
            .filter_map(|(index, marker)| {
                let delta = distance(
                    marker.center,
                    [
                        pixel[0] - f64::from(origin[0]),
                        pixel[1] - f64::from(origin[1]),
                    ],
                );
                (delta <= RED_MARKER_TARGET_RADIUS_PX).then_some((index, delta))
            })
            .collect::<Vec<_>>();
        targets.sort_by(|a, b| a.1.total_cmp(&b.1));
        let (target_index, target_distance) = *targets.first()?;
        if targets
            .get(1)
            .is_some_and(|(_, next)| (*next - target_distance).abs() < 1.0)
        {
            return None;
        }

        // Retain the connected group around the intended dot, not unrelated
        // red text elsewhere in the 96px patch. Extra animation elsewhere is
        // ignored; every baseline member of this cluster must remain present.
        let mut selected = vec![false; candidates.len()];
        selected[target_index] = true;
        loop {
            let mut changed = false;
            for candidate in 0..candidates.len() {
                if selected[candidate] {
                    continue;
                }
                if (0..candidates.len()).any(|known| {
                    selected[known]
                        && distance(candidates[candidate].center, candidates[known].center)
                            <= RED_MARKER_CLUSTER_LINK_PX
                }) {
                    selected[candidate] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        if selected.iter().filter(|selected| **selected).count() < RED_MARKER_MIN_CLUSTER {
            return None;
        }
        if selected.iter().filter(|selected| **selected).count() > RED_MARKER_MAX_CLUSTER {
            return None;
        }
        let mut markers = candidates
            .drain(..)
            .zip(selected)
            .filter_map(|(marker, selected)| {
                if !selected {
                    return None;
                }
                Some(marker)
            })
            .collect::<Vec<_>>();
        markers.sort_by(|a, b| {
            a.center[1]
                .total_cmp(&b.center[1])
                .then(a.center[0].total_cmp(&b.center[0]))
        });
        let target = markers.iter_mut().min_by(|a, b| {
            distance(
                a.center,
                [
                    pixel[0] - f64::from(origin[0]),
                    pixel[1] - f64::from(origin[1]),
                ],
            )
            .total_cmp(&distance(
                b.center,
                [
                    pixel[0] - f64::from(origin[0]),
                    pixel[1] - f64::from(origin[1]),
                ],
            ))
        })?;
        target.target = true;
        Some(Self {
            image_size,
            pixel,
            desktop,
            origin,
            markers,
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

    pub(crate) fn diagnostic(
        &self,
        png: &[u8],
        image_size: [u32; 2],
        desktop: [f64; 2],
        transform: Option<[f64; 6]>,
    ) -> serde_json::Value {
        let mut value = serde_json::to_value(self.compare(png, image_size, desktop, transform))
            .expect("finite marker comparison");
        value["limits"] = serde_json::json!({
            "patch_edge":EDGE,
            "minimum_cluster":RED_MARKER_MIN_CLUSTER,
            "maximum_cluster":RED_MARKER_MAX_CLUSTER,
            "target_radius_px":RED_MARKER_TARGET_RADIUS_PX,
            "maximum_center_delta_px":RED_MARKER_CENTER_DELTA_PX,
            "minimum_shape_iou":RED_MARKER_MIN_IOU,
        });
        value["input_authority"] = serde_json::json!(false);
        value
    }

    fn compare(
        &self,
        png: &[u8],
        image_size: [u32; 2],
        desktop: [f64; 2],
        transform: Option<[f64; 6]>,
    ) -> MarkerComparison {
        let unavailable = |reason| MarkerComparison {
            matched: false,
            reason,
            baseline_markers: self.markers.len(),
            current_candidates: 0,
            matched_markers: 0,
            target_marker_matched: false,
            maximum_center_delta_px: None,
            minimum_shape_iou: None,
        };
        if image_size != self.image_size {
            return unavailable("image_dimensions_changed");
        }
        if desktop != self.desktop {
            return unavailable("desktop_point_changed");
        }
        let Some([a, b, c, d, e, f]) = transform else {
            return unavailable("transform_missing");
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
            return unavailable("transform_changed");
        }
        let Some(current) = red_marker_components(png, image_size, self.origin) else {
            return unavailable("patch_decode_unavailable");
        };
        let mut used = vec![false; current.len()];
        let mut matched_markers = 0usize;
        let mut target_marker_matched = false;
        let mut maximum_center_delta_px = 0.0f64;
        let mut minimum_shape_iou = 1.0f64;
        for marker in &self.markers {
            let candidate = current
                .iter()
                .enumerate()
                .filter(|(index, _)| !used[*index])
                .map(|(index, candidate)| {
                    let center_delta = distance(marker.center, candidate.center);
                    let iou = shape_iou(&marker.pixels, &candidate.pixels);
                    (index, candidate, center_delta, iou)
                })
                .filter(|(_, candidate, center_delta, _)| {
                    *center_delta <= RED_MARKER_CENTER_DELTA_PX
                        && marker.bounds[2..4] == candidate.bounds[2..4]
                })
                .max_by(|a, b| a.3.total_cmp(&b.3));
            let Some((index, _, center_delta, iou)) = candidate else {
                return MarkerComparison {
                    matched: false,
                    reason: if marker.target {
                        "target_marker_missing_or_moved"
                    } else {
                        "cluster_marker_missing_or_moved"
                    },
                    baseline_markers: self.markers.len(),
                    current_candidates: current.len(),
                    matched_markers,
                    target_marker_matched,
                    maximum_center_delta_px: Some(maximum_center_delta_px),
                    minimum_shape_iou: Some(minimum_shape_iou),
                };
            };
            if iou < RED_MARKER_MIN_IOU {
                return MarkerComparison {
                    matched: false,
                    reason: if marker.target {
                        "target_marker_covered_or_changed"
                    } else {
                        "cluster_marker_covered_or_changed"
                    },
                    baseline_markers: self.markers.len(),
                    current_candidates: current.len(),
                    matched_markers,
                    target_marker_matched,
                    maximum_center_delta_px: Some(maximum_center_delta_px.max(center_delta)),
                    minimum_shape_iou: Some(minimum_shape_iou.min(iou)),
                };
            }
            used[index] = true;
            matched_markers += 1;
            target_marker_matched |= marker.target;
            maximum_center_delta_px = maximum_center_delta_px.max(center_delta);
            minimum_shape_iou = minimum_shape_iou.min(iou);
        }
        MarkerComparison {
            matched: target_marker_matched,
            reason: if target_marker_matched {
                "matched"
            } else {
                "target_marker_unverified"
            },
            baseline_markers: self.markers.len(),
            current_candidates: current.len(),
            matched_markers,
            target_marker_matched,
            maximum_center_delta_px: Some(maximum_center_delta_px),
            minimum_shape_iou: Some(minimum_shape_iou),
        }
    }
}

fn distance(a: [f64; 2], b: [f64; 2]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

fn shape_iou(a: &[u16], b: &[u16]) -> f64 {
    let mut intersection = 0usize;
    let (small, large) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    let mut left = 0usize;
    let mut right = 0usize;
    while left < small.len() && right < large.len() {
        match small[left].cmp(&large[right]) {
            std::cmp::Ordering::Less => left += 1,
            std::cmp::Ordering::Greater => right += 1,
            std::cmp::Ordering::Equal => {
                intersection += 1;
                left += 1;
                right += 1;
            }
        }
    }
    let union = a.len() + b.len() - intersection;
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

fn red_marker_components(
    png: &[u8],
    image_size: [u32; 2],
    origin: [u32; 2],
) -> Option<Vec<MarkerComponent>> {
    if png.len() > crate::MAX_FRAME_BYTES
        || u64::from(image_size[0]) * u64::from(image_size[1]) > MAX_PIXELS
    {
        return None;
    }
    let mut decoder = png::Decoder::new(Cursor::new(png));
    decoder.set_limits(png::Limits {
        bytes: (MAX_PIXELS * 4) as usize,
    });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    if [reader.info().width, reader.info().height] != image_size {
        return None;
    }
    let n = reader
        .output_buffer_size()
        .filter(|n| *n <= (MAX_PIXELS * 4) as usize)?;
    let mut decoded = vec![0; n];
    let info = reader.next_frame(&mut decoded).ok()?;
    if info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let channels = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        _ => return None,
    };
    let bytes = decoded.get(..info.buffer_size())?;
    if bytes.len() != image_size[0] as usize * image_size[1] as usize * channels
        || origin[0] + EDGE > image_size[0]
        || origin[1] + EDGE > image_size[1]
    {
        return None;
    }
    let stride = image_size[0] as usize * channels;
    let mut red = vec![false; EDGE as usize * EDGE as usize];
    for y in 0..EDGE as usize {
        for x in 0..EDGE as usize {
            let row = origin[1] as usize + y;
            let column = origin[0] as usize + x;
            let offset = row * stride + column * channels;
            let px = &bytes[offset..offset + channels];
            let (rgb, alpha) = match channels {
                4 => ([px[0], px[1], px[2]], px[3]),
                3 => ([px[0], px[1], px[2]], 255),
                2 => ([px[0]; 3], px[1]),
                _ => ([px[0]; 3], 255),
            };
            let rgb = rgb.map(|value| {
                ((u32::from(value) * u32::from(alpha) + 255 * (255 - u32::from(alpha)) + 127) / 255)
                    as u8
            });
            red[y * EDGE as usize + x] = rgb[0] >= 150
                && rgb[0].saturating_sub(rgb[1]) >= 70
                && rgb[0].saturating_sub(rgb[2]) >= 70
                && rgb[1] <= 150
                && rgb[2] <= 150;
        }
    }
    let width = EDGE as usize;
    let mut seen = vec![false; red.len()];
    let mut components = Vec::new();
    for start in 0..red.len() {
        if !red[start] || seen[start] {
            continue;
        }
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        let mut pixels = Vec::new();
        let (mut left, mut top, mut right, mut bottom) = (width as u8, width as u8, 0u8, 0u8);
        let (mut sum_x, mut sum_y) = (0u32, 0u32);
        while let Some(index) = queue.pop_front() {
            let x = index % width;
            let y = index / width;
            pixels.push(index as u16);
            let (x, y) = (x as u8, y as u8);
            left = left.min(x);
            right = right.max(x);
            top = top.min(y);
            bottom = bottom.max(y);
            sum_x += u32::from(x);
            sum_y += u32::from(y);
            let y0 = y.saturating_sub(1) as usize;
            let y1 = (usize::from(y) + 1).min(width - 1);
            let x0 = x.saturating_sub(1) as usize;
            let x1 = (usize::from(x) + 1).min(width - 1);
            for ny in y0..=y1 {
                for nx in x0..=x1 {
                    let next = ny * width + nx;
                    if red[next] && !seen[next] {
                        seen[next] = true;
                        queue.push_back(next);
                    }
                }
            }
        }
        let box_width = usize::from(right - left) + 1;
        let box_height = usize::from(bottom - top) + 1;
        let area = pixels.len();
        let aspect = box_width as f64 / box_height as f64;
        let density = area as f64 / (box_width * box_height) as f64;
        if !(RED_MARKER_MIN_AREA..=RED_MARKER_MAX_AREA).contains(&area)
            || !(RED_MARKER_MIN_EDGE..=RED_MARKER_MAX_EDGE).contains(&box_width)
            || !(RED_MARKER_MIN_EDGE..=RED_MARKER_MAX_EDGE).contains(&box_height)
            || !(0.45..=2.2).contains(&aspect)
            || density < RED_MARKER_MIN_DENSITY
        {
            continue;
        }
        pixels.sort_unstable();
        components.push(MarkerComponent {
            bounds: [left, top, box_width as u8, box_height as u8],
            center: [sum_x as f64 / area as f64, sum_y as f64 / area as f64],
            pixels,
            target: false,
        });
    }
    Some(components)
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
        let features = features_diagnosed(png, image_size, origin)?;
        // A blank/uniform patch cannot prove that scrolling or an object move
        // preserved the intended pixel. Keep the original full-frame rule.
        let (low, high) = features
            .chunks_exact(CHANNELS)
            .fold((255u8, 0u8), |(low, high), rgb| {
                let luma = ((u16::from(rgb[0]) + u16::from(rgb[1]) + u16::from(rgb[2])) / 3) as u8;
                (low.min(luma), high.max(luma))
            });
        if high.saturating_sub(low) < 32 {
            return Err(CreationFailure::LumaSpanBelowMinimum);
        }
        Ok(Self {
            image_size,
            pixel,
            desktop,
            origin,
            features,
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
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_creation_failure(
        encoded: &[u8],
        size: [u32; 2],
        pixel: [f64; 2],
        expected: CreationFailure,
        feature_calls: usize,
    ) {
        reset_feature_decode_count();
        assert_eq!(
            PixelAnchor::from_png_diagnosed(encoded, size, pixel, [80., 80.]).err(),
            Some(expected)
        );
        assert_eq!(feature_decode_count(), feature_calls);
        reset_feature_decode_count();
        assert!(PixelAnchor::from_png(encoded, size, pixel, [80., 80.]).is_none());
        assert_eq!(feature_decode_count(), feature_calls);
    }

    #[test]
    fn creation_preconditions_preserve_option_admission_without_decode() {
        let (rgb, size) = fixture();
        let encoded = png(&rgb, size);
        assert_creation_failure(
            &encoded,
            [95, 160],
            [80., 80.],
            CreationFailure::ImageTooSmall,
            0,
        );
        assert_creation_failure(
            &encoded,
            size,
            [f64::NAN, 80.],
            CreationFailure::PixelNotFinite,
            0,
        );
        for pixel in [[63.99, 80.], [80., 63.99], [96., 80.], [80., 96.]] {
            assert_creation_failure(
                &encoded,
                size,
                pixel,
                CreationFailure::PixelEdgeClearance,
                0,
            );
        }
        assert_eq!(CreationFailure::ImageTooSmall.as_str(), "image_too_small");
        assert_eq!(CreationFailure::PixelNotFinite.as_str(), "pixel_not_finite");
        assert_eq!(
            CreationFailure::PixelEdgeClearance.as_str(),
            "pixel_edge_clearance"
        );
    }

    #[test]
    fn creation_png_failures_preserve_option_and_record_the_actual_decode_stage() {
        let (rgb, size) = fixture();
        let encoded = png(&rgb, size);
        assert_creation_failure(
            b"not-a-png",
            size,
            [80., 80.],
            CreationFailure::PngHeaderDecodeFailed,
            1,
        );
        assert_creation_failure(
            &encoded,
            [161, 160],
            [80., 80.],
            CreationFailure::PngDimensionsMismatch,
            1,
        );
        let idat = encoded
            .windows(4)
            .position(|bytes| bytes == b"IDAT")
            .unwrap();
        assert_creation_failure(
            &encoded[..idat + 5],
            size,
            [80., 80.],
            CreationFailure::PngFrameDecodeFailed,
            1,
        );
        assert_eq!(
            CreationFailure::PngHeaderDecodeFailed.as_str(),
            "png_header_decode_failed"
        );
        assert_eq!(
            CreationFailure::PngDimensionsMismatch.as_str(),
            "png_dimensions_mismatch"
        );
        assert_eq!(
            CreationFailure::PngFrameDecodeFailed.as_str(),
            "png_frame_decode_failed"
        );
    }

    #[test]
    fn creation_input_budgets_preserve_option_without_extra_feature_calls() {
        let oversized = vec![0; crate::MAX_FRAME_BYTES + 1];
        assert_creation_failure(
            &oversized,
            [160, 160],
            [80., 80.],
            CreationFailure::PngByteBudgetExceeded,
            1,
        );
        assert_creation_failure(
            b"not-decoded-when-pixel-budget-is-exceeded",
            [4001, 4000],
            [80., 80.],
            CreationFailure::ImagePixelBudgetExceeded,
            1,
        );
        assert_eq!(
            CreationFailure::PngByteBudgetExceeded.as_str(),
            "png_byte_budget_exceeded"
        );
        assert_eq!(
            CreationFailure::ImagePixelBudgetExceeded.as_str(),
            "image_pixel_budget_exceeded"
        );
    }

    #[test]
    fn creation_luma_reason_does_not_infer_a_visually_blank_patch() {
        let size = [160, 160];
        let uniform = png(&vec![90; 160 * 160 * 3], size);
        assert_creation_failure(
            &uniform,
            size,
            [80., 80.],
            CreationFailure::LumaSpanBelowMinimum,
            1,
        );
        // Different RGB colors may still have the same existing average luma.
        // The reason describes that actual feature gate, not visual uniformity.
        let mut colored = vec![0; 160 * 160 * 3];
        for y in 0..160 {
            for x in 0..160 {
                let color = if (x / 6 + y / 6) % 2 == 0 {
                    [255, 0, 0]
                } else {
                    [0, 255, 0]
                };
                let at = (y * 160 + x) * 3;
                colored[at..at + 3].copy_from_slice(&color);
            }
        }
        assert_creation_failure(
            &png(&colored, size),
            size,
            [80., 80.],
            CreationFailure::LumaSpanBelowMinimum,
            1,
        );
        assert_eq!(
            CreationFailure::LumaSpanBelowMinimum.as_str(),
            "luma_span_below_minimum"
        );
    }

    #[test]
    fn diagnosed_creation_success_preserves_features_and_decodes_once() {
        let (rgb, size) = fixture();
        let encoded = png(&rgb, size);
        reset_feature_decode_count();
        let diagnosed =
            PixelAnchor::from_png_diagnosed(&encoded, size, [80., 80.], [80., 80.]).unwrap();
        assert_eq!(feature_decode_count(), 1);
        reset_feature_decode_count();
        let compatible = PixelAnchor::from_png(&encoded, size, [80., 80.], [80., 80.]).unwrap();
        assert_eq!(feature_decode_count(), 1);
        assert_eq!(diagnosed.image_size, compatible.image_size);
        assert_eq!(diagnosed.pixel, compatible.pixel);
        assert_eq!(diagnosed.desktop, compatible.desktop);
        assert_eq!(diagnosed.origin, compatible.origin);
        assert_eq!(diagnosed.features, compatible.features);
        // The old constructor did not validate desktop coordinates. Resolution
        // and later native mapping checks own that gate; do not add one here.
        for desktop in [[f64::NAN, 80.], [80., f64::INFINITY]] {
            reset_feature_decode_count();
            assert!(PixelAnchor::from_png_diagnosed(&encoded, size, [80., 80.], desktop).is_ok());
            assert_eq!(feature_decode_count(), 1);
            reset_feature_decode_count();
            assert!(PixelAnchor::from_png(&encoded, size, [80., 80.], desktop).is_some());
            assert_eq!(feature_decode_count(), 1);
        }
        println!(
            "anchor_creation_success: result=some; compatibility=some; feature_extractions=1; input_authority=false"
        );
    }

    #[test]
    fn feature_decode_counter_is_thread_local_and_resettable() {
        reset_feature_decode_count();
        let (rgb, size) = fixture();
        assert!(features(&png(&rgb, size), size, [32, 32]).is_some());
        assert_eq!(feature_decode_count(), 1);
        std::thread::spawn(|| {
            reset_feature_decode_count();
            assert_eq!(feature_decode_count(), 0);
            let (rgb, size) = fixture();
            assert!(features(&png(&rgb, size), size, [32, 32]).is_some());
            assert_eq!(feature_decode_count(), 1);
        })
        .join()
        .unwrap();
        assert_eq!(feature_decode_count(), 1);
        reset_feature_decode_count();
        assert_eq!(feature_decode_count(), 0);
    }
    fn png(rgb: &[u8], size: [u32; 2]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, size[0], size[1]);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(rgb).unwrap();
        }
        out
    }
    fn fixture() -> (Vec<u8>, [u32; 2]) {
        let size = [160, 160];
        let mut rgb = vec![255; 160 * 160 * 3];
        for y in 61..99 {
            for x in 61..99 {
                let i = (y * 160 + x) * 3;
                rgb[i..i + 3].copy_from_slice(&[20, 50, 90]);
            }
        }
        (rgb, size)
    }
    fn red_map(phase: u8) -> (Vec<u8>, [u32; 2]) {
        let size = [160, 160];
        let background = if phase == 1 {
            [104, 132, 52]
        } else {
            [72, 118, 60]
        };
        let mut rgb = vec![0; 160 * 160 * 3];
        for pixel in rgb.chunks_exact_mut(3) {
            pixel.copy_from_slice(&background);
        }
        for y in 32usize..128 {
            for x in 32usize..128 {
                if (x / 12 + y / 12) % 2 == 0 {
                    let i = (y * 160 + x) * 3;
                    rgb[i..i + 3].copy_from_slice(&[220, 220, 220]);
                }
            }
        }
        let mut markers = vec![
            (80i32, 80i32),
            (64, 80),
            (96, 80),
            (80, 64),
            (80, 96),
            (69, 69),
            (91, 69),
            (69, 91),
            (91, 91),
        ];
        if phase == 2 {
            markers[0] = (84, 80);
        } else if phase == 3 {
            markers.remove(0);
        }
        for (cx, cy) in markers {
            for dy in -3..=3 {
                for dx in -3..=3 {
                    if dx * dx + dy * dy > 9 {
                        continue;
                    }
                    let x = (cx + dx) as usize;
                    let y = (cy + dy) as usize;
                    let i = (y * 160 + x) * 3;
                    rgb[i..i + 3].copy_from_slice(&[250, 18, 20]);
                }
            }
        }
        if phase == 4 {
            // A non-red translucent/damage overlay occludes only the target dot.
            for y in 76..85 {
                for x in 76..85 {
                    let i = (y * 160 + x) * 3;
                    rgb[i..i + 3].copy_from_slice(&[132, 145, 150]);
                }
            }
        }
        if phase == 1 {
            // Moving terrain/health text and a separate red damage glyph should
            // not invalidate the saved map-marker cluster.
            for y in 40..56 {
                for x in 100..132 {
                    let i = (y * 160 + x) * 3;
                    rgb[i..i + 3].copy_from_slice(&[232, 238, 235]);
                }
            }
            for y in 40..45 {
                for x in 38..45 {
                    let i = (y * 160 + x) * 3;
                    rgb[i..i + 3].copy_from_slice(&[250, 18, 20]);
                }
            }
        }
        (png(&rgb, size), size)
    }
    #[test]
    fn animated_small_chrome_is_tolerated_but_local_move_is_not() {
        let (base, size) = fixture();
        let original = png(&base, size);
        let anchor = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
        let mut animation = base.clone();
        for x in 65..81 {
            let i = (55 * 160 + x) * 3;
            animation[i..i + 3].copy_from_slice(&[0, 0, 0]);
        }
        assert!(anchor.matches(
            &png(&animation, size),
            size,
            [80., 80.],
            Some([1., 0., 0., 1., 0., 0.])
        ));
        let mut moved = vec![255; base.len()];
        for y in 61..99 {
            for x in 63..101 {
                let i = (y * 160 + x) * 3;
                moved[i..i + 3].copy_from_slice(&[20, 50, 90]);
            }
        }
        assert!(!anchor.matches(
            &png(&moved, size),
            size,
            [80., 80.],
            Some([1., 0., 0., 1., 0., 0.])
        ));
        assert!(!anchor.matches(&original, size, [81., 80.], Some([1., 0., 0., 1., 0., 0.])));
        assert!(!anchor.matches(&original, size, [80., 80.], None));
    }
    #[test]
    fn bounded_animated_cells_do_not_outvote_a_stable_hit_patch() {
        let (base, size) = fixture();
        let original = png(&base, size);
        let anchor = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
        let mut animated = base;
        let origin = [32usize, 32usize];
        for cell in 0..21usize {
            let gx = cell % GRID;
            let gy = cell / GRID;
            let value = if cell == 0 { 214 } else { 223 };
            for dy in 0..CELL {
                for dx in 0..CELL {
                    let x = origin[0] + gx * CELL + dx;
                    let y = origin[1] + gy * CELL + dy;
                    let i = (y * size[0] as usize + x) * CHANNELS;
                    animated[i..i + CHANNELS].fill(value);
                }
            }
        }
        let report = anchor.diagnostic(
            &png(&animated, size),
            size,
            [80., 80.],
            Some([1., 0., 0., 1., 0., 0.]),
        );
        // This reproduces the reported 21 changed cells and raw total delta 681.
        assert_eq!(report["changed_cells"], 21);
        assert_eq!(report["total_delta"], 681);
        assert_eq!(report["central_max_delta"], 0);
        assert_eq!(report["stable_context_delta"], 0);
        assert_eq!(report["matched"], true);
    }

    #[test]
    fn broad_low_level_patch_drift_still_fails_stable_context_budget() {
        let (base, size) = fixture();
        let original = png(&base, size);
        let anchor = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
        let mut drift = base;
        for y in 32..128usize {
            for x in 32..128usize {
                let i = (y * size[0] as usize + x) * CHANNELS;
                for channel in 0..CHANNELS {
                    drift[i + channel] = drift[i + channel].saturating_sub(3);
                }
            }
        }
        let report = anchor.diagnostic(
            &png(&drift, size),
            size,
            [80., 80.],
            Some([1., 0., 0., 1., 0., 0.]),
        );
        assert_eq!(report["changed_cells"], 0);
        assert_eq!(report["total_delta"], 768);
        assert_eq!(report["hit_region_delta"], 192);
        assert_eq!(report["stable_context_delta"], 480);
        assert_eq!(report["reason"], "stable_context_delta_budget_exceeded");
        assert_eq!(report["matched"], false);
    }

    #[test]
    fn opt_in_red_marker_cluster_ignores_unrelated_animation_but_not_marker_changes() {
        let (original, size) = red_map(0);
        let anchor = RedMarkerAnchor::from_png(&original, size, [80., 80.], [80., 80.])
            .expect("target is centered on a compact red cluster");
        assert_eq!(anchor.markers.len(), 9);

        let (animated, _) = red_map(1);
        assert!(anchor.matches(&animated, size, [80., 80.], Some([1., 0., 0., 1., 0., 0.])));
        let strict = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
        assert!(!strict.matches(&animated, size, [80., 80.], Some([1., 0., 0., 1., 0., 0.])));

        for (phase, reason) in [
            (2, "target_marker_missing_or_moved"),
            (3, "target_marker_missing_or_moved"),
            (4, "target_marker_missing_or_moved"),
        ] {
            let (changed, _) = red_map(phase);
            let report =
                anchor.diagnostic(&changed, size, [80., 80.], Some([1., 0., 0., 1., 0., 0.]));
            assert_eq!(report["matched"], false);
            assert_eq!(report["reason"], reason);
            assert_eq!(report["target_marker_matched"], false);
            assert_eq!(report["input_authority"], false);
        }
    }

    #[test]
    fn red_marker_cluster_requires_a_unique_target_and_neighboring_markers() {
        let size = [160, 160];
        let mut rgb = vec![80; 160 * 160 * 3];
        for (cx, cy) in [(80, 80), (64, 80)] {
            for dy in -3..=3 {
                for dx in -3..=3 {
                    if dx * dx + dy * dy <= 9 {
                        let i = ((cy + dy) as usize * 160 + (cx + dx) as usize) * 3;
                        rgb[i..i + 3].copy_from_slice(&[250, 18, 20]);
                    }
                }
            }
        }
        let two_markers = png(&rgb, size);
        assert!(RedMarkerAnchor::from_png(&two_markers, size, [80., 80.], [80., 80.]).is_none());
        assert!(RedMarkerAnchor::from_png(&two_markers, size, [48., 48.], [48., 48.]).is_none());
    }
    #[test]
    fn blank_or_invalid_source_does_not_weaken_full_content_guard() {
        let size = [160, 160];
        assert!(
            PixelAnchor::from_png(
                &png(&vec![255; 160 * 160 * 3], size),
                size,
                [80., 80.],
                [80., 80.]
            )
            .is_none()
        );
        assert!(PixelAnchor::from_png(b"not-png", size, [80., 80.], [80., 80.]).is_none());
        let (textured, size) = fixture();
        assert!(
            PixelAnchor::from_png(&png(&textured, size), size, [51., 51.], [51., 51.]).is_none()
        );
    }
    #[test]
    fn diagnostic_reports_central_and_outer_change_without_changing_admission() {
        let (base, size) = fixture();
        let original = png(&base, size);
        let anchor = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
        let transform = Some([1., 0., 0., 1., 0., 0.]);
        let mut outer = base.clone();
        // Static central button, many moving pixels in the surrounding 96px
        // patch: this must remain refused rather than inventing target identity.
        for y in 32..58 {
            for x in 32..128 {
                let i = (y * 160 + x) * 3;
                outer[i..i + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
        let report = anchor.diagnostic(&png(&outer, size), size, [80., 80.], transform);
        assert_eq!(report["reason"], "changed_cell_budget_exceeded");
        assert_eq!(report["central_max_delta"], 0);
        assert_eq!(report["matched"], false);
        assert_eq!(report["input_authority"], false);
        let mut center = base;
        for y in 74..86 {
            for x in 74..86 {
                let i = (y * 160 + x) * 3;
                center[i..i + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
        let report = anchor.diagnostic(&png(&center, size), size, [80., 80.], transform);
        assert_eq!(report["reason"], "central_patch_changed");
        assert!(report["central_max_delta"].as_u64().unwrap() > 16);
        assert!(!anchor.matches(&png(&center, size), size, [80., 80.], transform));
        assert_eq!(
            anchor.diagnostic(&original, size, [80., 80.], transform)["reason"],
            "matched"
        );
    }
    #[test]
    fn diagnostics_distinguish_mapping_dimensions_and_decode_failure() {
        let (base, size) = fixture();
        let original = png(&base, size);
        let anchor = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
        for (image, dimensions, point, transform, reason) in [
            (
                &original[..],
                [161, 160],
                [80., 80.],
                Some([1., 0., 0., 1., 0., 0.]),
                "image_dimensions_changed",
            ),
            (
                &original[..],
                size,
                [81., 80.],
                Some([1., 0., 0., 1., 0., 0.]),
                "desktop_point_changed",
            ),
            (&original[..], size, [80., 80.], None, "transform_missing"),
            (
                &original[..],
                size,
                [80., 80.],
                Some([1., 0., 0., 1., 1., 0.]),
                "transform_changed",
            ),
            (
                &b"bad-png"[..],
                size,
                [80., 80.],
                Some([1., 0., 0., 1., 0., 0.]),
                "patch_decode_unavailable",
            ),
        ] {
            let report = anchor.diagnostic(image, dimensions, point, transform);
            assert_eq!(report["reason"], reason);
            assert_eq!(
                report["matched"],
                anchor.matches(image, dimensions, point, transform)
            );
            assert!(report["changed_cells"].is_null());
        }
    }

    // Synthetic source-pixel cells, not a capture or reconstruction of the
    // CreatureCurios before/after PNG. These tests exercise the same comparator.
    fn dynamic_anchor_diagnostic_fixture(cell_deltas: &[(usize, u8)]) -> serde_json::Value {
        let (base, size) = fixture();
        let original = png(&base, size);
        let anchor = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
        let mut current = base;
        for &(cell, delta) in cell_deltas {
            for dy in 0..CELL {
                for dx in 0..CELL {
                    let x = 32 + (cell % GRID) * CELL + dx;
                    let y = 32 + (cell / GRID) * CELL + dy;
                    let i = (y * size[0] as usize + x) * CHANNELS;
                    for channel in 0..CHANNELS {
                        current[i + channel] = current[i + channel].checked_sub(delta).unwrap();
                    }
                }
            }
        }
        anchor.diagnostic(
            &png(&current, size),
            size,
            [80., 80.],
            Some([1., 0., 0., 1., 0., 0.]),
        )
    }

    #[test]
    fn dynamic_anchor_diagnostic_reports_spatial_split_for_synthetic_46_cell_rejection() {
        let mut deltas = Vec::new();
        let hit = (0..GRID * GRID).filter(|cell| {
            let (row, column) = (cell / GRID, cell % GRID);
            (4..12).contains(&row)
                && (4..12).contains(&column)
                && !((7..=8).contains(&row) && (7..=8).contains(&column))
        });
        deltas.extend(hit.take(24).map(|cell| (cell, 13)));
        let context = (0..GRID * GRID)
            .filter(|cell| !((4..12).contains(&(cell / GRID)) && (4..12).contains(&(cell % GRID))))
            .collect::<Vec<_>>();
        for (index, &cell) in context.iter().take(22).enumerate() {
            deltas.push((cell, if index == 0 { 145 } else { 142 }));
        }
        deltas.extend(context.iter().skip(22).take(36).map(|&cell| (cell, 3)));
        let report = dynamic_anchor_diagnostic_fixture(&deltas);
        assert_eq!(report["changed_cells"], 46);
        assert_eq!(report["central_max_delta"], 0);
        assert_eq!(report["hit_region_delta"], 312);
        assert_eq!(report["stable_context_delta"], 78);
        assert_eq!(report["total_delta"], 3547);
        assert_eq!(report["reason"], "changed_cell_budget_exceeded");
        assert_eq!(report["matched"], false);
        assert_eq!(report["hit_changed_cells"], 24);
        assert_eq!(report["context_changed_cells"], 22);
        assert_eq!(report["context_trimmed_changed_cells"], 22);
        assert_eq!(report["stable_context_changed_cells"], 0);
        assert_eq!(
            report["failed_checks"],
            serde_json::json!(["changed_cell_budget_exceeded"])
        );
        assert_eq!(report["input_authority"], false);
        println!("synthetic_46_cell_report={report}");
    }

    #[test]
    fn dynamic_anchor_diagnostic_lists_all_failed_budgets_without_changing_reason_priority() {
        let report = dynamic_anchor_diagnostic_fixture(
            &(0..GRID * GRID).map(|cell| (cell, 20)).collect::<Vec<_>>(),
        );
        assert_eq!(report["reason"], "central_patch_changed");
        assert_eq!(report["matched"], false);
        assert_eq!(report["changed_cells"], 256);
        assert_eq!(report["hit_changed_cells"], 64);
        assert_eq!(report["context_changed_cells"], 192);
        assert_eq!(report["context_trimmed_changed_cells"], 32);
        assert_eq!(report["stable_context_changed_cells"], 160);
        assert_eq!(
            report["failed_checks"],
            serde_json::json!([
                "central_patch_changed",
                "changed_cell_budget_exceeded",
                "hit_region_delta_budget_exceeded",
                "stable_context_delta_budget_exceeded",
            ])
        );
    }

    #[test]
    fn dynamic_anchor_diagnostic_distinguishes_stable_context_drift_from_changed_cell_budget() {
        let report = dynamic_anchor_diagnostic_fixture(
            &(0..GRID * GRID).map(|cell| (cell, 3)).collect::<Vec<_>>(),
        );
        assert_eq!(report["reason"], "stable_context_delta_budget_exceeded");
        assert_eq!(report["changed_cells"], 0);
        assert_eq!(report["hit_changed_cells"], 0);
        assert_eq!(report["context_changed_cells"], 0);
        assert_eq!(report["context_trimmed_changed_cells"], 0);
        assert_eq!(report["stable_context_changed_cells"], 0);
        assert_eq!(
            report["failed_checks"],
            serde_json::json!(["stable_context_delta_budget_exceeded"])
        );
        assert_eq!(report["matched"], false);
    }

    #[test]
    fn dynamic_anchor_diagnostic_stable_patch_has_no_failures_or_changed_cells() {
        let report = dynamic_anchor_diagnostic_fixture(&[]);
        assert_eq!(report["reason"], "matched");
        assert_eq!(report["matched"], true);
        assert_eq!(report["hit_changed_cells"], 0);
        assert_eq!(report["context_changed_cells"], 0);
        assert_eq!(report["context_trimmed_changed_cells"], 0);
        assert_eq!(report["stable_context_changed_cells"], 0);
        assert_eq!(report["failed_checks"], serde_json::json!([]));
    }

    #[test]
    fn dynamic_anchor_diagnostic_unavailable_patch_does_not_invent_spatial_metrics() {
        let (base, size) = fixture();
        let original = png(&base, size);
        let anchor = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
        let report = anchor.diagnostic(
            &original,
            [161, 160],
            [80., 80.],
            Some([1., 0., 0., 1., 0., 0.]),
        );
        assert_eq!(report["matched"], false);
        assert_eq!(report["reason"], "image_dimensions_changed");
        for field in [
            "hit_changed_cells",
            "context_changed_cells",
            "context_trimmed_changed_cells",
            "stable_context_changed_cells",
        ] {
            assert!(report.as_object().unwrap().contains_key(field));
            assert!(report[field].is_null());
        }
        assert_eq!(
            report["failed_checks"],
            serde_json::json!(["image_dimensions_changed"])
        );
    }
}
