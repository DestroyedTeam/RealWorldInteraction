//! Bounded native-size PNG artifacts and explicitly mapped visual views.
use rcu_core::{Result, fault, operation::checkpoint};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Write};
mod visual;
pub use visual::encode_visual_rgb;

pub const MAX_PIXELS: usize = 16_000_000;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
impl Rect {
    pub fn checked(self) -> Result<Self> {
        if self.width == 0
            || self.height == 0
            || self.width as u64 * self.height as u64 > MAX_PIXELS as u64
            || i64::from(self.x) + i64::from(self.width) > i64::from(i32::MAX)
            || i64::from(self.y) + i64::from(self.height) > i64::from(i32::MAX)
        {
            return Err(fault(
                "image_dimension_limit",
                "Use positive dimensions with at most 16 million pixels and a valid physical desktop rectangle.",
            ));
        }
        Ok(self)
    }
    pub fn contains(&self, other: Self) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && i64::from(other.x) + i64::from(other.width)
                <= i64::from(self.x) + i64::from(self.width)
            && i64::from(other.y) + i64::from(other.height)
                <= i64::from(self.y) + i64::from(self.height)
    }
}
struct BoundedOutput {
    bytes: Vec<u8>,
    limit_exceeded: bool,
    limit: usize,
}
impl Write for BoundedOutput {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(b.len()) > self.limit {
            self.limit_exceeded = true;
            return Err(std::io::Error::other("PNG artifact size limit"));
        }
        self.bytes.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub fn encode_rgb(size: [u32; 2], rgb: &[u8]) -> Result<Vec<u8>> {
    encode_rgb_limited(size, rgb, rcu_core::MAX_FRAME_BYTES)
}
fn encode_rgb_limited(size: [u32; 2], rgb: &[u8], limit: usize) -> Result<Vec<u8>> {
    Rect {
        x: 0,
        y: 0,
        width: size[0],
        height: size[1],
    }
    .checked()?;
    if rgb.len() != size[0] as usize * size[1] as usize * 3 {
        return Err(fault(
            "encoding_source_layout",
            "RGB rows must be contiguous with exactly three bytes per pixel.",
        ));
    }
    checkpoint("png_encode_begin")?;
    let mut out = BoundedOutput {
        bytes: Vec::new(),
        limit_exceeded: false,
        limit,
    };
    let encoded: Result<()> = (|| {
        let mut enc = png::Encoder::new(&mut out, size[0], size[1]);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().map_err(|_| {
            fault(
                "png_encoding_failed",
                "Inspect the encoder and original raster.",
            )
        })?;
        let mut stream = writer
            .stream_writer()
            .map_err(|_| fault("png_encoding_failed", "Inspect the encoder."))?;
        for row in rgb.chunks_exact(size[0] as usize * 3) {
            checkpoint("png_encode_row")?;
            stream.write_all(row).map_err(|_| {
                fault(
                    "png_encoding_failed",
                    "PNG encoding failed; no transport attempted.",
                )
            })?;
        }
        stream.finish().map_err(|_| {
            fault(
                "png_encoding_failed",
                "PNG could not be finalized within the output budget.",
            )
        })?;
        Ok(())
    })();
    if out.limit_exceeded {
        return Err(fault(
            "png_size_limit",
            "Encoded PNG exceeded its byte budget; capture and transport are distinct stages. Request a smaller visual rather than altering desktop resolution.",
        ));
    }
    encoded?;
    checkpoint("png_encode_returned")?;
    Ok(out.bytes)
}
pub fn bgra_to_rgb(size: [u32; 2], stride: usize, bytes: &[u8]) -> Result<Vec<u8>> {
    Rect {
        x: 0,
        y: 0,
        width: size[0],
        height: size[1],
    }
    .checked()?;
    if stride < size[0] as usize * 4
        || bytes.len()
            != stride
                .checked_mul(size[1] as usize)
                .ok_or_else(|| fault("capture_layout", "Stride overflow."))?
    {
        return Err(fault(
            "capture_layout",
            "Expected bounded top-down BGRA rows; alpha is unused in GDI BI_RGB.",
        ));
    }
    let mut rgb = Vec::with_capacity(size[0] as usize * size[1] as usize * 3);
    for row in bytes.chunks_exact(stride) {
        checkpoint("bgra_convert_row")?;
        for p in row[..size[0] as usize * 4].chunks_exact(4) {
            rgb.extend_from_slice(&[p[2], p[1], p[0]]);
        }
    }
    Ok(rgb)
}
pub struct Raster {
    pub size: [u32; 2],
    pub rgb: Vec<u8>,
}
pub fn decode_original(png_bytes: &[u8]) -> Result<Raster> {
    if png_bytes.len() > rcu_core::MAX_FRAME_BYTES {
        return Err(fault(
            "image_size_limit",
            "Original PNG exceeds the artifact budget.",
        ));
    }
    let mut decoder = png::Decoder::new(Cursor::new(png_bytes));
    decoder.set_limits(png::Limits {
        bytes: MAX_PIXELS * 4,
    });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|_| {
        fault(
            "invalid_png",
            "The original artifact is not a supported bounded PNG.",
        )
    })?;
    let size = [reader.info().width, reader.info().height];
    Rect {
        x: 0,
        y: 0,
        width: size[0],
        height: size[1],
    }
    .checked()?;
    let n = reader
        .output_buffer_size()
        .filter(|n| *n <= MAX_PIXELS * 4)
        .ok_or_else(|| {
            fault(
                "image_dimension_limit",
                "Decoded PNG exceeds the raster budget.",
            )
        })?;
    let mut bytes = vec![0; n];
    checkpoint("png_decode_begin")?;
    let info = reader.next_frame(&mut bytes).map_err(|_| {
        fault(
            "invalid_png",
            "Decode failed; keep the original for diagnosis.",
        )
    })?;
    checkpoint("png_decode_returned")?;
    let bytes = &bytes[..info.buffer_size()];
    let mut rgb = Vec::with_capacity(size[0] as usize * size[1] as usize * 3);
    match info.color_type {
        png::ColorType::Rgb => rgb.extend_from_slice(bytes),
        png::ColorType::Rgba => {
            for p in bytes.chunks_exact(4) {
                // Explicit white compositing only for the VIEW; never alters original bytes.
                for &v in &p[..3] {
                    rgb.push(
                        ((u16::from(v) * u16::from(p[3]) + 255 * (255 - u16::from(p[3])) + 127)
                            / 255) as u8,
                    );
                }
            }
        }
        png::ColorType::Grayscale => {
            for &v in bytes {
                rgb.extend_from_slice(&[v; 3]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for p in bytes.chunks_exact(2) {
                let v = ((u16::from(p[0]) * u16::from(p[1]) + 255 * (255 - u16::from(p[1])) + 127)
                    / 255) as u8;
                rgb.extend_from_slice(&[v; 3]);
            }
        }
        _ => return Err(fault("unsupported_png_layout", "Palette expansion failed.")),
    }
    Ok(Raster { size, rgb })
}
#[derive(Debug, Clone, Serialize)]
pub struct ViewMapping {
    pub kind: String,
    pub original_size: [u32; 2],
    pub crop: Rect,
    pub view_size: [u32; 2],
    pub view_to_original: [f64; 6],
    pub original_to_desktop: Option<[f64; 6]>,
    pub normalization: String,
}
impl ViewMapping {
    pub fn original_point(&self, p: [f64; 2]) -> Result<[f64; 2]> {
        if !p.iter().all(|v| v.is_finite())
            || p[0] < 0.0
            || p[1] < 0.0
            || p[0] >= f64::from(self.view_size[0])
            || p[1] >= f64::from(self.view_size[1])
        {
            return Err(fault(
                "outside_view",
                "Use coordinates in this exact image view.",
            ));
        }
        let m = self.view_to_original;
        let q = [
            m[0] * p[0] + m[2] * p[1] + m[4],
            m[1] * p[0] + m[3] * p[1] + m[5],
        ];
        if q[0] < 0.0
            || q[1] < 0.0
            || q[0] >= f64::from(self.original_size[0])
            || q[1] >= f64::from(self.original_size[1])
        {
            return Err(fault(
                "outside_frame",
                "Mapped point exceeds the original frame.",
            ));
        }
        Ok(q)
    }
}
pub fn view(
    bytes: &[u8],
    crop: Option<Rect>,
    max_edge: u32,
    desktop: Option<[f64; 6]>,
) -> Result<(Vec<u8>, ViewMapping)> {
    if !(64..=2048).contains(&max_edge) {
        return Err(fault(
            "invalid_view_limit",
            "max_edge must be 64..2048; use native-resolution tiles for detail.",
        ));
    }
    let raster = decode_original(bytes)?;
    let full = Rect {
        x: 0,
        y: 0,
        width: raster.size[0],
        height: raster.size[1],
    };
    let crop = crop.unwrap_or(full).checked()?;
    if !full.contains(crop) {
        return Err(fault(
            "invalid_crop",
            "Crop must lie completely within the original image; no silent clipping.",
        ));
    }
    let scale = f64::from(max_edge) / f64::from(crop.width.max(crop.height));
    let size = if scale >= 1.0 {
        [crop.width, crop.height]
    } else {
        [
            (f64::from(crop.width) * scale).floor().max(1.0) as u32,
            (f64::from(crop.height) * scale).floor().max(1.0) as u32,
        ]
    };
    let sx = f64::from(crop.width) / f64::from(size[0]);
    let sy = f64::from(crop.height) / f64::from(size[1]);
    let mut rgb = Vec::with_capacity(size[0] as usize * size[1] as usize * 3);
    for y in 0..size[1] {
        checkpoint("view_sample_row")?;
        for x in 0..size[0] {
            let ox = crop.x as usize + ((f64::from(x) + 0.5) * sx).floor() as usize;
            let oy = crop.y as usize + ((f64::from(y) + 0.5) * sy).floor() as usize;
            let i = (oy * raster.size[0] as usize + ox) * 3;
            rgb.extend_from_slice(&raster.rgb[i..i + 3]);
        }
    }
    let png = encode_rgb(size, &rgb)?;
    if png.len() > rcu_core::INLINE_BYTES {
        return Err(fault(
            "view_transport_size_limit",
            "Use a smaller preview or tile; original remains unchanged.",
        ));
    }
    Ok((png,ViewMapping{kind:if sx==1.0&&sy==1.0 {"native_resolution_tile"}else{"nearest_neighbor_preview"}.into(),original_size:raster.size,crop,view_size:size,view_to_original:[sx,0.0,0.0,sy,f64::from(crop.x)+(sx-1.0)/2.0,f64::from(crop.y)+(sy-1.0)/2.0],original_to_desktop:desktop,normalization:"view only: 8-bit RGB, RGBA composited on white; pixel-center affine; original PNG untouched".into()}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn top_down_bgra_alpha_and_padding() {
        assert_eq!(
            bgra_to_rgb(
                [1, 2],
                8,
                &[1, 2, 3, 0, 9, 9, 9, 9, 4, 5, 6, 255, 8, 8, 8, 8]
            )
            .unwrap(),
            [3, 2, 1, 6, 5, 4]
        );
        assert!(bgra_to_rgb([1, 2], 3, &[0; 6]).is_err());
    }
    #[test]
    fn malformed_and_overflowing_dimensions_rejected() {
        assert!(
            Rect {
                x: i32::MAX,
                y: 0,
                width: 2,
                height: 1
            }
            .checked()
            .is_err()
        );
        assert!(decode_original(b"not png").is_err());
        assert!(encode_rgb([100_000, 100_000], &[]).is_err());
    }
    #[test]
    fn full_3600x2186_original_and_1200x600_roi_regression() {
        let mut seed = 0x3c927ab1u32;
        let mut rgb = vec![0; 3600 * 2186 * 3];
        for p in &mut rgb {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *p = seed as u8;
        }
        let original = encode_rgb([3600, 2186], &rgb).unwrap();
        let hash = rcu_core::digest(&original);
        assert!(original.len() > rcu_core::INLINE_BYTES);
        assert!(original.len() < rcu_core::MAX_FRAME_BYTES);
        let (tile, m) = view(
            &original,
            Some(Rect {
                x: 1700,
                y: 800,
                width: 1200,
                height: 600,
            }),
            1200,
            Some([1., 0., 0., 1., -1800., -300.]),
        )
        .unwrap();
        let decoded = decode_original(&tile).unwrap();
        assert_eq!(decoded.size, [1200, 600]);
        assert_eq!(
            &decoded.rgb[..3],
            &rgb[(800 * 3600 + 1700) * 3..(800 * 3600 + 1700) * 3 + 3]
        );
        assert_eq!(m.original_point([1199., 599.]).unwrap(), [2899., 1399.]);
        assert_eq!(m.view_to_original, [1., 0., 0., 1., 1700., 800.]);
        let (preview, p) = view(&original, None, 1200, None).unwrap();
        assert!(preview.len() < rcu_core::INLINE_BYTES);
        assert_eq!(p.view_size, [1200, 728]);
        assert_eq!(p.original_point([0., 0.]).unwrap()[0], 1.);
        assert_eq!(rcu_core::digest(&original), hash);
        assert!(p.original_point([f64::NAN, 0.]).is_err());
        assert!(
            view(
                &original,
                Some(Rect {
                    x: 3599,
                    y: 0,
                    width: 2,
                    height: 1
                }),
                1200,
                None
            )
            .is_err()
        );
    }
}
