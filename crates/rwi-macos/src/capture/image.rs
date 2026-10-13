//! Exact existing bitmap conversion/PNG logic moved without behavior changes.
use super::{Capture, validate_pixels};
use crate::ffi;
use core_graphics::{
    color_space::CGColorSpace,
    context::CGContext,
    geometry::{CGPoint, CGRect, CGSize},
    image::{CGImage, CGImageAlphaInfo, CGImageByteOrderInfo},
};
use foreign_types::ForeignType;
use rwi_core::{MAX_FRAME_BYTES, Result, fault};
use std::{ffi::c_void, io::Write};
pub(super) unsafe fn encode_native_image(
    ptr: *const c_void,
    w: usize,
    h: usize,
    logical_height: f64,
) -> Result<Capture> {
    // Borrowed callback image is retained exactly once, then released by CGImage.
    let image = unsafe { CGImage::from_ptr(ffi::CFRetain(ptr).cast_mut().cast()) };
    if image.width() != w || image.height() != h {
        return Err(fault(
            "capture_geometry_changed",
            "Native image size differs from the attested filter configuration.",
        ));
    }
    validate_pixels(w, h)?;
    let color = CGColorSpace::create_device_rgb();
    let mut context = CGContext::create_bitmap_context(
        None,
        w,
        h,
        8,
        w * 4,
        &color,
        CGImageAlphaInfo::CGImageAlphaPremultipliedLast as u32
            | CGImageByteOrderInfo::CGImageByteOrder32Big as u32,
    );
    context.draw_image(
        CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(w as f64, h as f64)),
        &image,
    );
    let pixels = context.data();
    for rgba in pixels.chunks_exact_mut(4) {
        let a = rgba[3] as u32;
        if a != 0 && a != 255 {
            for channel in &mut rgba[..3] {
                *channel = ((*channel as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    let content_sha = content_digest(pixels, w, h, logical_height);
    Ok(Capture {
        png: encode_rgba(w, h, pixels)?,
        size: [w as u32, h as u32],
        native_size: [w as u32, h as u32],
        content_sha,
    })
}

pub(super) fn content_digest(pixels: &[u8], w: usize, h: usize, logical_height: f64) -> String {
    let band = (h / 16).clamp(16, 96).min(h / 4);
    let bottom = (h as f64 / logical_height).ceil() as usize;
    // Tiny captures retain all pixels rather than accepting an empty digest.
    let range = if band.saturating_add(bottom) < h {
        band * w * 4..(h - bottom) * w * 4
    } else {
        0..pixels.len()
    };
    rwi_core::digest(&pixels[range])
}
struct Limited(Vec<u8>);
impl Write for Limited {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(buf.len()) > MAX_FRAME_BYTES {
            return Err(std::io::Error::other("PNG budget"));
        }
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(super) fn encode_rgba(w: usize, h: usize, pixels: &[u8]) -> Result<Vec<u8>> {
    validate_pixels(w, h)?;
    if pixels.len() != w * h * 4 {
        return Err(fault("invalid_pixels", "RGBA buffer size mismatch."));
    }
    let mut out = Limited(Vec::new());
    {
        let mut encoder = png::Encoder::new(&mut out, w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|_| fault("png_encode_failed", "PNG header failed."))?;
        writer.write_image_data(pixels).map_err(|_|fault("screenshot_size_limit","PNG exceeded bounded output or failed encoding; no degraded image was substituted."))?;
        writer
            .finish()
            .map_err(|_| fault("png_encode_failed", "PNG did not finish."))?;
    }
    Ok(out.0)
}
