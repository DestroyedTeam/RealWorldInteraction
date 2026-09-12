//! One device-local presenter. It never sends input, activates an app or reads UIA.
use super::{CaptureGate, PULSE_MS, Phase, SIZE, raster, scale_for_dpi};
use crate::{WindowRecord, desktop};
use rcu_core::{Result, fault};
use std::{
    ffi::c_void,
    mem::size_of,
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{Dwm::DwmFlush, Gdi::*},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::*,
    },
    core::w,
};

static PRESENTER: OnceLock<std::result::Result<Arc<Shared>, String>> = OnceLock::new();
static GESTURE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
struct Event {
    gesture: u64,
    hwnd: usize,
    pid: u32,
    point: [f64; 2],
    scale: u32,
    phase: Phase,
    expires: Instant,
}

#[derive(Default)]
struct Model {
    event: Option<Event>,
    gate: CaptureGate,
    started: bool,
    stopped: bool,
    updates: u64,
    render_submissions: u64,
    capture_suspensions: u64,
    error: Option<String>,
}
#[derive(Default)]
struct Shared {
    model: Mutex<Model>,
    changed: Condvar,
}
impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, Model> {
        self.model.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn presenter() -> Option<&'static Arc<Shared>> {
    PRESENTER
        .get_or_init(|| {
            let shared = Arc::new(Shared::default());
            let worker = shared.clone();
            std::thread::Builder::new()
                .name("rcu-pointer-feedback".into())
                .spawn(move || {
                    let outcome =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&worker)));
                    let mut model = worker.lock();
                    model.error = match outcome {
                        Ok(Ok(())) => None,
                        Ok(Err(error)) => Some(error),
                        Err(_) => Some("pointer presenter panicked".into()),
                    };
                    // run's thread-owned Window/Surface guards were destroyed before this.
                    model.gate.visible = false;
                    model.stopped = true;
                    model.gate.applied_revision = model.gate.revision;
                    worker.changed.notify_all();
                })
                .map_err(|e| e.to_string())?;
            Ok(shared)
        })
        .as_ref()
        .ok()
}

/// One original input dispatch. An error/unwind hides only this gesture's hint.
pub(crate) struct Gesture {
    id: u64,
    finished: bool,
}
impl Gesture {
    pub(crate) fn new() -> Self {
        Self {
            id: GESTURE_ID.fetch_add(1, Ordering::Relaxed),
            finished: false,
        }
    }
    pub(crate) fn show(&self, window: &WindowRecord, point: [f64; 2], pressed: bool) {
        let Ok(hwnd) = desktop::hwnd(&window.window) else {
            return;
        };
        let Some(shared) = presenter() else { return };
        let mut model = shared.lock();
        if model.stopped || !point.iter().all(|v| v.is_finite()) {
            return;
        }
        // A single coalescing slot, not an unbounded replay queue.
        model.event = Some(Event {
            gesture: self.id,
            hwnd: hwnd.0 as usize,
            pid: window.pid,
            point,
            scale: scale_for_dpi(window.window_dpi.unwrap_or(96)),
            phase: if pressed {
                Phase::Pressed
            } else {
                Phase::Pointer
            },
            expires: Instant::now() + Duration::from_millis(PULSE_MS),
        });
        model.gate.changed();
        model.updates += 1;
        shared.changed.notify_all();
    }
    pub(crate) fn finish(&mut self) {
        self.finished = true;
    }
}
impl Drop for Gesture {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        if let Some(Ok(shared)) = PRESENTER.get() {
            let mut model = shared.lock();
            if model.event.as_ref().is_some_and(|e| e.gesture == self.id) {
                model.event = None;
                model.gate.changed();
                shared.changed.notify_all();
            }
        }
    }
}

/// Grounding frames exclude this presenter. Otherwise an expiring halo would
/// change the content hash even though the target application had not changed.
pub(crate) struct CaptureGuard(Option<Arc<Shared>>);
pub(crate) fn suspend_capture() -> Result<CaptureGuard> {
    let Some(Ok(shared)) = PRESENTER.get() else {
        return Ok(CaptureGuard(None));
    };
    let mut model = shared.lock();
    if model.stopped {
        return Ok(CaptureGuard(None));
    }
    model.capture_suspensions += 1;
    let ticket = model.gate.suspend();
    let guard = CaptureGuard(Some(shared.clone()));
    shared.changed.notify_all();
    let deadline = Instant::now() + Duration::from_millis(250);
    while !model.stopped && !model.gate.capture_ready(ticket) {
        let now = Instant::now();
        if now >= deadline {
            drop(model);
            drop(guard);
            return Err(fault(
                "pointer_feedback_capture_busy",
                "Feedback did not acknowledge hidden state; no potentially contaminated grounding frame returned.",
            ));
        }
        model = shared
            .changed
            .wait_timeout(model, deadline - now)
            .unwrap_or_else(|e| e.into_inner())
            .0;
    }
    drop(model);
    Ok(guard)
}
impl Drop for CaptureGuard {
    fn drop(&mut self) {
        if let Some(shared) = &self.0 {
            let mut model = shared.lock();
            model.gate.resume();
            shared.changed.notify_all();
        }
    }
}

pub(crate) fn status() -> serde_json::Value {
    let mut result = serde_json::json!({
        "implemented":true,"mode":"device_native_layered_pointer_with_rcu_badge",
        "started":false,"visible":false,"input_injection":false,
        "pulse_expiry_ms":PULSE_MS,"keyboard_feedback":false,
        "grounding_capture":"presenter hidden with acknowledgement during GDI copy; feedback is not included in grounding PNG",
        "visual_latency_attested":false
    });
    match PRESENTER.get() {
        Some(Ok(shared)) => {
            let m = shared.lock();
            result["started"] = m.started.into();
            result["visible"] = m.gate.visible.into();
            result["stopped"] = m.stopped.into();
            result["updates_requested"] = m.updates.into();
            result["render_submissions"] = m.render_submissions.into();
            result["capture_suspensions"] = m.capture_suspensions.into();
            result["error"] = serde_json::json!(m.error);
        }
        Some(Err(e)) => result["error"] = e.clone().into(),
        None => {}
    }
    result
}

fn run(shared: &Shared) -> std::result::Result<(), String> {
    let _dpi = desktop::DpiScope::enter().map_err(|e| format!("{}: {}", e.code, e.next_step))?;
    let mut window = Window::new().map_err(|e| e.to_string())?;
    shared.lock().started = true;
    loop {
        let mut message = MSG::default();
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            if message.message == WM_QUIT {
                return Ok(());
            }
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        let (revision, event) = {
            let m = shared.lock();
            (
                m.gate.revision,
                (m.gate.suspended == 0)
                    .then(|| m.event.clone())
                    .flatten()
                    .filter(|e| Instant::now() < e.expires),
            )
        };
        let display = event.and_then(|event| {
            let hwnd = HWND(event.hwnd as *mut c_void);
            let mut pid = 0;
            let mut cursor = POINT::default();
            unsafe {
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
            }
            if pid != event.pid
                || unsafe { GetForegroundWindow() } != hwnd
                || unsafe { GetCursorPos(&mut cursor) }.is_err()
                || (f64::from(cursor.x) - event.point[0])
                    .hypot(f64::from(cursor.y) - event.point[1])
                    > f64::from(event.scale * 12)
            {
                return None;
            }
            Some((event, cursor))
        });
        let visible = if let Some((event, cursor)) = display {
            window
                .present(cursor, event.scale, event.phase)
                .map_err(|e| e.to_string())?;
            unsafe { IsWindowVisible(window.hwnd) }.as_bool()
        } else {
            window.hide().map_err(|e| e.to_string())?;
            false
        };
        let mut model = shared.lock();
        model.gate.acknowledge(revision, visible);
        if visible {
            model.render_submissions += 1;
        }
        shared.changed.notify_all();
        if model.gate.revision != revision {
            continue;
        }
        let delay = if visible { 16 } else { 100 };
        drop(
            shared
                .changed
                .wait_timeout(model, Duration::from_millis(delay))
                .unwrap_or_else(|e| e.into_inner()),
        );
    }
}

struct Window {
    hwnd: HWND,
    surface: Option<Surface>,
}
impl Window {
    fn new() -> windows::core::Result<Self> {
        let instance = HINSTANCE(unsafe { GetModuleHandleW(None) }?.0);
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: w!("RCU.PointerFeedback.v1"),
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
                w!("RCU.PointerFeedback.v1"),
                w!("RCU Pointer Feedback"),
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
        })
    }
    fn hide(&self) -> windows::core::Result<()> {
        unsafe {
            if IsWindowVisible(self.hwnd).as_bool() {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
                DwmFlush()?;
            }
        }
        Ok(())
    }
    fn present(&mut self, point: POINT, scale: u32, phase: Phase) -> windows::core::Result<()> {
        if self.surface.as_ref().is_none_or(|s| s.scale != scale) {
            self.surface = Some(Surface::new(scale)?);
        }
        let surface = self.surface.as_mut().expect("surface initialized");
        if surface.phase != Some(phase) {
            let pixels = raster(scale, phase);
            unsafe {
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), surface.bits, pixels.len());
            }
            surface.phase = Some(phase);
        }
        let destination = POINT {
            x: point.x.saturating_sub(36 * scale as i32),
            y: point.y.saturating_sub(36 * scale as i32),
        };
        let size = windows::Win32::Foundation::SIZE {
            cx: (SIZE as u32 * scale) as i32,
            cy: (SIZE as u32 * scale) as i32,
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
        Ok(())
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        let _ = self.hide();
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}
unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
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
    scale: u32,
    phase: Option<Phase>,
}
impl Surface {
    fn new(scale: u32) -> windows::core::Result<Self> {
        let mut s = Self {
            dc: unsafe { CreateCompatibleDC(None) },
            bitmap: HBITMAP::default(),
            previous: HGDIOBJ::default(),
            bits: std::ptr::null_mut(),
            scale,
            phase: None,
        };
        if s.dc.0.is_null() {
            return Err(windows::core::Error::from_thread());
        }
        let size = (SIZE as u32 * scale) as i32;
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
                SelectObject(self.dc, self.previous);
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
