//! Explicit opt-in red-marker comparison; ordinary pixel guards stay separate.
use super::*;

/// Explicit, local-only signature for a compact cluster of red markers. It is
/// separate from ordinary pixel references so dynamic canvas users must opt in.
#[derive(Debug, Clone)]
pub struct RedMarkerAnchor {
    image_size: [u32; 2],
    pixel: [f64; 2],
    desktop: [f64; 2],
    origin: [u32; 2],
    pub(super) markers: Vec<MarkerComponent>,
}

#[derive(Debug, Clone)]
pub(super) struct MarkerComponent {
    bounds: [u8; 4],
    center: [f64; 2],
    pixels: Vec<u16>,
    target: bool,
}

#[derive(serde::Serialize)]
pub(super) struct MarkerComparison {
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

    pub(super) fn compare(
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
