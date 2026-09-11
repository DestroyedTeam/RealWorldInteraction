//! Visible desktop BitBlt only. No PrintWindow, hidden capture, resizing or OS input.
use crate::desktop::api;
use rcu_core::{
    Result, fault,
    operation::{checkpoint, native_call},
};
use rcu_imaging::{Rect, bgra_to_rgb, encode_rgb};
use serde::Serialize;
use std::{ffi::c_void, mem::size_of, time::Instant};
use windows::Win32::Graphics::Gdi::*;

struct Surface {
    desktop: HDC,
    memory: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            if !self.previous.0.is_null() {
                SelectObject(self.memory, self.previous);
            }
            if !self.bitmap.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            }
            if !self.memory.0.is_null() {
                let _ = DeleteDC(self.memory);
            }
            if !self.desktop.0.is_null() {
                ReleaseDC(None, self.desktop);
            }
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Timing {
    pub capture_ms: f64,
    pub conversion_ms: f64,
    pub encoding_ms: f64,
    pub raw_bytes: usize,
    pub png_bytes: usize,
}
pub fn capture(r: Rect) -> Result<(Vec<u8>, Timing)> {
    let r = r.checked()?;
    let start = Instant::now();
    let _call = native_call("GDI_capture")?;
    let mut surface = Surface {
        desktop: unsafe { GetDC(None) },
        memory: HDC::default(),
        bitmap: HBITMAP::default(),
        previous: HGDIOBJ::default(),
    };
    if surface.desktop.0.is_null() {
        return Err(fault(
            "capture_dc_unavailable",
            "GetDC for the allocated interactive desktop failed.",
        ));
    }
    surface.memory = unsafe { CreateCompatibleDC(Some(surface.desktop)) };
    if surface.memory.0.is_null() {
        return Err(fault(
            "capture_dc_unavailable",
            "CreateCompatibleDC failed.",
        ));
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: r.width as i32,
            biHeight: -(r.height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    surface.bitmap = api("capture_bitmap_failed", unsafe {
        CreateDIBSection(
            Some(surface.desktop),
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )
    })?;
    if bits.is_null() {
        return Err(fault(
            "capture_bitmap_failed",
            "CreateDIBSection returned no raster memory.",
        ));
    }
    surface.previous = unsafe { SelectObject(surface.memory, HGDIOBJ(surface.bitmap.0)) };
    if surface.previous.0.is_null() || surface.previous.0 as isize == -1 {
        surface.previous = HGDIOBJ::default();
        return Err(fault(
            "capture_bitmap_selection_failed",
            "SelectObject failed; no image was captured.",
        ));
    }
    api("native_capture_failed", unsafe {
        BitBlt(
            surface.memory,
            0,
            0,
            r.width as i32,
            r.height as i32,
            Some(surface.desktop),
            r.x,
            r.y,
            SRCCOPY | CAPTUREBLT,
        )
    })?;
    if !unsafe { GdiFlush() }.as_bool() {
        return Err(fault(
            "capture_flush_failed",
            "GDI raster synchronization failed.",
        ));
    }
    let capture_ms = start.elapsed().as_secs_f64() * 1000.;
    drop(_call);
    checkpoint("capture_returned")?;
    let convert = Instant::now();
    let n = r.width as usize * r.height as usize * 4;
    let raw = unsafe { std::slice::from_raw_parts(bits.cast::<u8>(), n) };
    let rgb = bgra_to_rgb([r.width, r.height], r.width as usize * 4, raw)?;
    let conversion_ms = convert.elapsed().as_secs_f64() * 1000.;
    drop(surface);
    let encode = Instant::now();
    let png = encode_rgb([r.width, r.height], &rgb)?;
    let timing = Timing {
        capture_ms,
        conversion_ms,
        encoding_ms: encode.elapsed().as_secs_f64() * 1000.,
        raw_bytes: n,
        png_bytes: png.len(),
    };
    Ok((png, timing))
}
