//! GDI grayscale glyph coverage only, on an owned offscreen DIB. No screen read.
use crate::pointer_feedback::{material::Label, scale_for_dpi};
use std::mem::size_of;
use windows::{
    Win32::{Foundation::*, Graphics::Gdi::*},
    core::w,
};

struct Canvas {
    dc: HDC,
    bitmap: HBITMAP,
    font: HFONT,
    old_bitmap: HGDIOBJ,
    old_font: HGDIOBJ,
}
impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            if !self.old_font.0.is_null() {
                SelectObject(self.dc, self.old_font);
            }
            if !self.old_bitmap.0.is_null() {
                SelectObject(self.dc, self.old_bitmap);
            }
            if !self.font.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(self.font.0));
            }
            if !self.bitmap.0.is_null() {
                let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            }
            if !self.dc.0.is_null() {
                let _ = DeleteDC(self.dc);
            }
        }
    }
}
pub(super) fn coverage(dpi: u32) -> windows::core::Result<Label> {
    let scale = scale_for_dpi(dpi);
    let width = (44. * scale).ceil() as i32;
    let height = (15. * scale).ceil() as i32;
    let mut c = Canvas {
        dc: unsafe { CreateCompatibleDC(None) },
        bitmap: HBITMAP::default(),
        font: HFONT::default(),
        old_bitmap: HGDIOBJ::default(),
        old_font: HGDIOBJ::default(),
    };
    if c.dc.0.is_null() {
        return Err(windows::core::Error::from_thread());
    }
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    c.bitmap = unsafe { CreateDIBSection(Some(c.dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) }?;
    if bits.is_null() {
        return Err(windows::core::Error::from_thread());
    }
    c.old_bitmap = unsafe { SelectObject(c.dc, HGDIOBJ(c.bitmap.0)) };
    if c.old_bitmap.0.is_null() || c.old_bitmap.0 as isize == -1 {
        c.old_bitmap = HGDIOBJ::default();
        return Err(windows::core::Error::from_thread());
    }
    c.font = unsafe {
        CreateFontW(
            -(7. * scale).round() as i32,
            0,
            0,
            0,
            750,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            ANTIALIASED_QUALITY,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        )
    };
    if c.font.0.is_null() {
        return Err(windows::core::Error::from_thread());
    }
    c.old_font = unsafe { SelectObject(c.dc, HGDIOBJ(c.font.0)) };
    if c.old_font.0.is_null() || c.old_font.0 as isize == -1 {
        c.old_font = HGDIOBJ::default();
        return Err(windows::core::Error::from_thread());
    }
    let len = width as usize * height as usize * 4;
    let pixels = unsafe { std::slice::from_raw_parts_mut(bits.cast::<u8>(), len) };
    pixels.fill(0);
    unsafe {
        SetBkMode(c.dc, TRANSPARENT);
        SetTextColor(c.dc, COLORREF(0x00ffffff));
        SetTextCharacterExtra(c.dc, scale.round() as i32);
        TextOutW(
            c.dc,
            (3. * scale).round() as i32,
            (3. * scale).round() as i32,
            &[82, 67, 85],
        )
        .ok()?;
        GdiFlush().ok()?;
    }
    let coverage = pixels
        .chunks_exact(4)
        .map(|p| ((u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2])) / 3) as u8)
        .collect();
    Ok(Label {
        width: width as usize,
        height: height as usize,
        coverage,
        padding: 3.,
    })
}
