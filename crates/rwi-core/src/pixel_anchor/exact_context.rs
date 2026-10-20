//! A blank center is not an anchor. Require nearby texture and equality of every
//! normalized pixel in a centered, bounded context. Never search another location,
//! accept a whole uniform window, trim changed pixels, or skip native hit checks.
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExactContext {
    origin: [u32; 2],
    edge: u32,
    digest: [u8; 32],
}
impl ExactContext {
    pub(super) fn from_decoded(
        bytes: &[u8],
        size: [u32; 2],
        channels: usize,
        pixel: [f64; 2],
    ) -> Option<Self> {
        if !matches!(channels, 1..=4) || pixel.iter().any(|p| !p.is_finite() || *p < 0.) {
            return None;
        }
        let center = [pixel[0].floor() as u32, pixel[1].floor() as u32];
        // Fixed finite search, still containing the actual requested point.
        for edge in [192u32, 384] {
            let half = edge / 2;
            if center
                .iter()
                .zip(size)
                .any(|(&p, n)| p < half || p.checked_add(half).is_none_or(|r| r > n))
            {
                continue;
            }
            let origin = [center[0] - half, center[1] - half];
            let mut hash = Sha256::new();
            let mut histogram = [0u32; 256];
            let mut row = Vec::with_capacity(edge as usize * 3);
            for y in origin[1]..origin[1] + edge {
                row.clear();
                for x in origin[0]..origin[0] + edge {
                    let i = (y as usize * size[0] as usize + x as usize).checked_mul(channels)?;
                    let px = bytes.get(i..i.checked_add(channels)?)?;
                    let (rgb, alpha) = match channels {
                        4 => ([px[0], px[1], px[2]], px[3]),
                        3 => ([px[0], px[1], px[2]], 255),
                        2 => ([px[0]; 3], px[1]),
                        _ => ([px[0]; 3], 255),
                    };
                    let normalized = rgb.map(|c| {
                        ((u32::from(c) * u32::from(alpha) + 255 * (255 - u32::from(alpha)) + 127)
                            / 255) as u8
                    });
                    row.extend_from_slice(&normalized);
                    let luma = normalized.iter().map(|&c| u16::from(c)).sum::<u16>() / 3;
                    histogram[luma as usize] += 1;
                }
                hash.update(&row);
            }
            let low = histogram.iter().position(|&n| n != 0)?;
            let high = histogram.iter().rposition(|&n| n != 0)?;
            // Contrast in both populated bands, not a lone noisy pixel.
            if high - low >= 32
                && histogram[..=high - 32].iter().sum::<u32>() >= 36
                && histogram[low + 32..].iter().sum::<u32>() >= 36
            {
                return Some(Self {
                    origin,
                    edge,
                    digest: hash.finalize().into(),
                });
            }
        }
        None
    }
    pub(super) fn diagnostic(&self) -> serde_json::Value {
        serde_json::json!({"origin":self.origin,"edge":self.edge,"max_edge":384,
            "normalized_pixel_equality_required":true,"input_authority":false})
    }
}
