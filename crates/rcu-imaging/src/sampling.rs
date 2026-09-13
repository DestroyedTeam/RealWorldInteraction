//! Exact RGB8 box-area downsampling. Every overlapping source pixel contributes;
//! quantization/strong reduction can still make very low-contrast detail invisible.
use super::*;

/// Source pixels are unit squares. Axis boundaries use integer units of the
/// output dimension, so fractional edge coverage needs no floating-point math.
/// Downscaling visits each source pixel at most twice per axis (four total).
/// Only the target RGB buffer is allocated; accumulators are constant-size.
pub(super) fn area_rgb(
    size: [u32; 2],
    rgb: &[u8],
    crop: Rect,
    output: [u32; 2],
) -> Result<Vec<u8>> {
    let full = Rect {
        x: 0,
        y: 0,
        width: size[0],
        height: size[1],
    }
    .checked()?;
    crop.checked()?;
    if rgb.len() != size[0] as usize * size[1] as usize * 3 {
        return Err(fault(
            "encoding_source_layout",
            "Expected exactly the bounded contiguous RGB source.",
        ));
    }
    if !full.contains(crop) {
        return Err(fault(
            "invalid_crop",
            "Area sampling never clips outside the original image.",
        ));
    }
    if output[0] == 0 || output[1] == 0 || output[0] > crop.width || output[1] > crop.height {
        return Err(fault(
            "invalid_sample_size",
            "Area sampling only preserves or reduces both source dimensions.",
        ));
    }
    let mut sampled = Vec::with_capacity(output[0] as usize * output[1] as usize * 3);
    if output == [crop.width, crop.height] {
        for y in 0..crop.height as usize {
            checkpoint("area_rgb_native_row")?;
            let start = ((crop.y as usize + y) * size[0] as usize + crop.x as usize) * 3;
            sampled.extend_from_slice(&rgb[start..start + crop.width as usize * 3]);
        }
        return Ok(sampled);
    }
    let [ow, oh] = output.map(u64::from);
    let (cw, ch) = (u64::from(crop.width), u64::from(crop.height));
    // Sum(wx) == crop.width, Sum(wy) == crop.height for every output pixel.
    // Thus denominator <= MAX_PIXELS and a channel sum <= 255 * MAX_PIXELS.
    let denominator = cw * ch;
    let mut visits = 0u32;
    for y in 0..oh {
        checkpoint("area_rgb_sample_row")?;
        let (top, bottom) = (y * ch, (y + 1) * ch);
        for x in 0..ow {
            let (left, right) = (x * cw, (x + 1) * cw);
            let mut sum = [0u64; 3];
            for sy in top / oh..=(bottom - 1) / oh {
                let wy = bottom.min((sy + 1) * oh) - top.max(sy * oh);
                let row =
                    ((crop.y as usize + sy as usize) * size[0] as usize + crop.x as usize) * 3;
                for sx in left / ow..=(right - 1) / ow {
                    // Also bound cancellation latency for very long/thin rasters.
                    visits += 1;
                    if visits & 4095 == 0 {
                        checkpoint("area_rgb_sample_chunk")?;
                    }
                    let wx = right.min((sx + 1) * ow) - left.max(sx * ow);
                    let weight = wx * wy;
                    let i = row + sx as usize * 3;
                    for c in 0..3 {
                        sum[c] += u64::from(rgb[i + c]) * weight;
                    }
                }
            }
            // Round half upward in normalized RGB8, not linear-light RGB.
            sampled.extend(sum.map(|v| ((v + denominator / 2) / denominator) as u8));
        }
    }
    checkpoint("area_rgb_sample_returned")?;
    Ok(sampled)
}

#[cfg(test)]
mod tests;
