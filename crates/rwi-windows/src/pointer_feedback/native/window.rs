//! Thread-owned layered window, raster surface and placement. No input dispatch.
use super::label;
use crate::pointer_feedback::{
    Anchor, SIZE,
    material::{Label, Material, Style},
    motion::Frame,
    raster_size, scale_for_dpi,
};
use std::{mem::size_of, time::Instant};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{Dwm::DwmFlush, Gdi::*},
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
    },
    core::w,
};

fn label_offset(anchor: Anchor) -> [i32; 2] {
    let monitor = unsafe {
        MonitorFromPoint(
            POINT {
                x: anchor.point[0],
                y: anchor.point[1],
            },
            MONITOR_DEFAULTTONEAREST,
        )
    };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        return [14, 14];
    }
    let scale = scale_for_dpi(anchor.dpi);
    let r = info.rcWork;
    let x = if f64::from(r.right) - f64::from(anchor.point[0]) < 50. * scale {
        -40
    } else if f64::from(anchor.point[0]) - f64::from(r.left) < 8. * scale {
        18
    } else {
        14
    };
    let y = if f64::from(r.bottom) - f64::from(anchor.point[1]) < 45. * scale {
        -33
    } else {
        14
    };
    [x, y]
}

pub(super) struct Window {
    pub(super) hwnd: HWND,
    surface: Option<Surface>,
    last: Option<(Anchor, Frame, Style, [i32; 2])>,
    material: Option<(u32, Style, Material)>,
    pub(super) raster_ms: f64,
    pub(super) native_label: bool,
}
impl Window {
    pub(super) fn refresh_dpi(&self, anchor: Anchor) -> Anchor {
        if self.last.is_some() {
            let point_monitor = unsafe {
                MonitorFromPoint(
                    POINT {
                        x: anchor.point[0],
                        y: anchor.point[1],
                    },
                    MONITOR_DEFAULTTONEAREST,
                )
            };
            let window_monitor = unsafe { MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST) };
            let dpi = unsafe { GetDpiForWindow(self.hwnd) };
            if point_monitor == window_monitor && dpi != 0 {
                return anchor.with_dpi(dpi);
            }
        }
        anchor
    }
    pub(super) fn new() -> windows::core::Result<Self> {
        let instance = HINSTANCE(unsafe { GetModuleHandleW(None) }?.0);
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: w!("RWI.PointerFeedback.v1"),
            ..Default::default()
        };
        if unsafe { RegisterClassW(&class) } == 0 {
            return Err(windows::core::Error::from_thread());
        }
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_NOACTIVATE
                    | WS_EX_TOOLWINDOW
                    | WS_EX_TOPMOST,
                w!("RWI.PointerFeedback.v1"),
                w!("RWI Pointer Feedback"),
                WS_POPUP | WS_DISABLED,
                0,
                0,
                SIZE as i32,
                SIZE as i32,
                None,
                None,
                Some(instance),
                None,
            )
        }?;
        Ok(Self {
            hwnd,
            surface: None,
            last: None,
            material: None,
            raster_ms: 0.,
            native_label: false,
        })
    }
    pub(super) fn hide(&self) -> windows::core::Result<()> {
        unsafe {
            if IsWindowVisible(self.hwnd).as_bool() {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
                DwmFlush()?;
            }
        }
        Ok(())
    }
    pub(super) fn present(
        &mut self,
        anchor: Anchor,
        frame: Frame,
        style: Style,
    ) -> windows::core::Result<bool> {
        let offset = label_offset(anchor);
        let key = (anchor, frame, style, offset);
        if self.last == Some(key) && unsafe { IsWindowVisible(self.hwnd) }.as_bool() {
            return Ok(false);
        }
        let point = POINT {
            x: anchor.point[0],
            y: anchor.point[1],
        };
        let dpi = anchor.dpi;
        if self.surface.as_ref().is_none_or(|s| s.dpi != dpi) {
            self.surface = Some(Surface::new(dpi)?);
        }
        if self
            .material
            .as_ref()
            .is_none_or(|(d, s, _)| *d != dpi || *s != style)
        {
            let label = match label::coverage(dpi) {
                Ok(l) => {
                    self.native_label = true;
                    l
                }
                Err(_) => {
                    self.native_label = false;
                    Label::fallback(dpi)
                }
            };
            self.material = Some((dpi, style, Material::new(dpi, style, label)));
        }
        let surface = self.surface.as_mut().expect("surface initialized");
        if surface.frame != Some((frame, style, offset)) {
            let began = Instant::now();
            let pixels = self
                .material
                .as_ref()
                .unwrap()
                .2
                .raster(frame, offset.map(f64::from));
            self.raster_ms = began.elapsed().as_secs_f64() * 1000.;
            unsafe {
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), surface.bits, pixels.len());
            }
            surface.frame = Some((frame, style, offset));
        }
        let half = (raster_size(dpi) / 2) as i32;
        let destination = POINT {
            x: point.x.saturating_sub(half),
            y: point.y.saturating_sub(half),
        };
        let size = windows::Win32::Foundation::SIZE {
            cx: raster_size(dpi) as i32,
            cy: raster_size(dpi) as i32,
        };
        let source = POINT::default();
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        unsafe {
            UpdateLayeredWindow(
                self.hwnd,
                None,
                Some(&destination),
                Some(&size),
                Some(surface.dc),
                Some(&source),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )?;
            SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            )?;
        }
        self.last = Some(key);
        Ok(true)
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        if self.hide().is_err() {
            rwi_core::operation::resource_release_uncertain();
        }
        unsafe {
            if DestroyWindow(self.hwnd).is_err() {
                rwi_core::operation::resource_release_uncertain();
            }
        }
    }
}
unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        // Keep the true hotspot; do not adopt the OS's suggested shifted RECT.
        // The service loop re-reads HWND DPI and rebuilds only raster resources.
        WM_DPICHANGED => LRESULT(0),
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        _ => unsafe { DefWindowProcW(hwnd, msg, w, l) },
    }
}

struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    bits: *mut u8,
    dpi: u32,
    frame: Option<(Frame, Style, [i32; 2])>,
}
impl Surface {
    fn new(dpi: u32) -> windows::core::Result<Self> {
        let mut s = Self {
            dc: unsafe { CreateCompatibleDC(None) },
            bitmap: HBITMAP::default(),
            previous: HGDIOBJ::default(),
            bits: std::ptr::null_mut(),
            dpi,
            frame: None,
        };
        if s.dc.0.is_null() {
            return Err(windows::core::Error::from_thread());
        }
        let size = raster_size(dpi) as i32;
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: size,
                biHeight: -size,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = std::ptr::null_mut();
        s.bitmap =
            unsafe { CreateDIBSection(Some(s.dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) }?;
        if bits.is_null() {
            return Err(windows::core::Error::from_thread());
        }
        s.bits = bits.cast();
        s.previous = unsafe { SelectObject(s.dc, HGDIOBJ(s.bitmap.0)) };
        if s.previous.0.is_null() || s.previous.0 as isize == -1 {
            s.previous = HGDIOBJ::default();
            return Err(windows::core::Error::from_thread());
        }
        Ok(s)
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            if !self.previous.0.is_null() {
                let restored = SelectObject(self.dc, self.previous);
                if restored.0.is_null() || restored.0 as isize == -1 {
                    rwi_core::operation::resource_release_uncertain();
                }
            }
            if !self.bitmap.0.is_null() && !DeleteObject(HGDIOBJ(self.bitmap.0)).as_bool() {
                rwi_core::operation::resource_release_uncertain();
            }
            if !self.dc.0.is_null() && !DeleteDC(self.dc).as_bool() {
                rwi_core::operation::resource_release_uncertain();
            }
        }
    }
}
