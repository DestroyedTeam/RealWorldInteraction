//! One device-local presenter. It never sends input, activates an app or reads UIA.
use super::{
    Anchor, CaptureGate, HIGHLIGHT_MS, Phase, SIZE,
    material::{Label, Material, Style},
    motion::{Frame, Motion},
    normalize_dpi, raster_size, scale_for_dpi,
};
mod label;
mod preferences;
use crate::{WindowRecord, desktop};
use preferences::Preferences;
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
        UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
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
    dpi: u32,
    phase: Phase,
    expires: Instant,
}

#[derive(Default)]
struct Model {
    event: Option<Event>,
    motion: Motion,
    preferences: Option<Preferences>,
    frame: Option<Frame>,
    raster_ms: f64,
    max_raster_ms: f64,
    native_label: bool,
    anchor: Option<Anchor>,
    phase: Option<Phase>,
    shutdown: bool,
    service_started: bool,
    gate: CaptureGate,
    started: bool,
    stopped: bool,
    updates: u64,
    render_submissions: u64,
    capture_suspensions: u64,
    error: Option<String>,
}
struct Shared {
    model: Mutex<Model>,
    changed: Condvar,
    origin: Instant,
}
impl Default for Shared {
    fn default() -> Self {
        Self {
            model: Mutex::new(Model::default()),
            changed: Condvar::new(),
            origin: Instant::now(),
        }
    }
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

/// One original dispatch. Errors clear its active highlight, not the idle marker.
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
        if model.stopped || model.shutdown || !point.iter().all(|v| v.is_finite()) {
            return;
        }
        let dpi = normalize_dpi(window.window_dpi.unwrap_or(96));
        model.motion.sample(
            point,
            dpi,
            pressed,
            shared.origin.elapsed().as_secs_f64() * 1000.,
        );
        // A single coalescing slot, not an unbounded replay queue.
        model.event = Some(Event {
            gesture: self.id,
            hwnd: hwnd.0 as usize,
            pid: window.pid,
            point,
            dpi,
            phase: if pressed {
                Phase::Pressed
            } else {
                Phase::Pointer
            },
            expires: Instant::now() + Duration::from_millis(HIGHLIGHT_MS),
        });
        model.gate.changed();
        model.updates += 1;
        shared.changed.notify_all();
    }
    pub(crate) fn click(&self, window: &WindowRecord, point: [f64; 2]) {
        self.show(window, point, false);
        if let Some(Ok(shared)) = PRESENTER.get() {
            let mut m = shared.lock();
            if m.event.as_ref().is_some_and(|e| e.gesture == self.id) {
                m.motion
                    .click(shared.origin.elapsed().as_secs_f64() * 1000.);
                m.gate.changed();
                shared.changed.notify_all();
            }
        }
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
                model.motion.cancel();
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
        "implemented":true,"mode":"device_native_a1_diamond","design_scope":"user-approved development build; not formal UIOS acceptance",
        "raster_logical_size":SIZE,"dpi_cap":super::MAX_DPI,"position_filter":false,"continuous_expression":true,"settle_ms":super::motion::SETTLE_MS,
        "started":false,"visible":false,"input_injection":false,
        "lifetime":"agent_service","idle_marker":true,"active_highlight_ms":HIGHLIGHT_MS,"keyboard_feedback":false,
        "position_retention":"agent_lifetime; restart seeds the current OS cursor without moving it",
        "grounding_capture":"presenter hidden with acknowledgement during GDI copy; feedback is not included in grounding PNG",
        "visual_latency_attested":false
    });
    match PRESENTER.get() {
        Some(Ok(shared)) => {
            let m = shared.lock();
            result["started"] = m.started.into();
            result["initialized_from_service"] = m.service_started.into();
            result["phase"] = serde_json::json!(m.phase);
            result["expression"] = serde_json::json!(m.frame);
            result["preferences"] = serde_json::json!(m.preferences);
            result["last_raster_ms"] = serde_json::json!(m.raster_ms);
            result["max_raster_ms"] = serde_json::json!(m.max_raster_ms);
            result["label_font"] = if m.native_label {
                "Segoe UI native GDI"
            } else {
                "portable fallback or not yet initialized"
            }
            .into();
            result["anchor"] = serde_json::json!(m.anchor);
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

pub(crate) fn start() -> Result<()> {
    let shared = presenter().ok_or_else(|| {
        fault(
            "pointer_feedback_start_failed",
            "The device presenter thread could not be started.",
        )
    })?;
    let mut model = shared.lock();
    model.service_started = true;
    model.gate.changed();
    shared.changed.notify_all();
    Ok(())
}

pub(crate) fn stop() {
    if let Some(Ok(shared)) = PRESENTER.get() {
        let mut model = shared.lock();
        model.shutdown = true;
        model.event = None;
        model.motion.cancel();
        model.gate.changed();
        shared.changed.notify_all();
    }
}

fn cursor_anchor() -> Option<Anchor> {
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor) }.ok()?;
    let dpi = unsafe { GetDpiForWindow(GetForegroundWindow()) };
    clamp_anchor(Anchor {
        point: [cursor.x, cursor.y],
        dpi: normalize_dpi(dpi),
    })
}

fn clamp_anchor(anchor: Anchor) -> Option<Anchor> {
    let point = POINT {
        x: anchor.point[0],
        y: anchor.point[1],
    };
    let monitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if monitor.0.is_null()
        || !unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool()
        || info.rcMonitor.right <= info.rcMonitor.left
        || info.rcMonitor.bottom <= info.rcMonitor.top
    {
        return None;
    }
    Some(Anchor {
        point: [
            anchor.point[0].clamp(info.rcMonitor.left, info.rcMonitor.right - 1),
            anchor.point[1].clamp(info.rcMonitor.top, info.rcMonitor.bottom - 1),
        ],
        ..anchor
    })
}

fn event_anchor(event: Event) -> Option<(Anchor, Phase)> {
    if Instant::now() >= event.expires {
        return None;
    }
    let mut cursor = POINT::default();
    if unsafe { GetCursorPos(&mut cursor) }.is_err()
        || (f64::from(cursor.x) - event.point[0]).hypot(f64::from(cursor.y) - event.point[1]) > 1.5
    {
        return None;
    }
    let hwnd = HWND(event.hwnd as *mut c_void);
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    let phase = if pid == event.pid && unsafe { GetForegroundWindow() } == hwnd {
        event.phase
    } else {
        Phase::Idle
    };
    clamp_anchor(Anchor {
        point: [cursor.x, cursor.y],
        dpi: event.dpi,
    })
    .map(|a| (a, phase))
}

fn run(shared: &Shared) -> std::result::Result<(), String> {
    let _dpi = desktop::DpiScope::enter().map_err(|e| format!("{}: {}", e.code, e.next_step))?;
    let mut window = Window::new().map_err(|e| e.to_string())?;
    let mut prefs = Preferences::read();
    let mut preferences_at = Instant::now();
    let mut last_input = 0;
    shared.lock().started = true;
    loop {
        let mut message = MSG::default();
        let mut prefs_dirty = false;
        while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
            if message.message == WM_QUIT {
                return Ok(());
            }
            prefs_dirty |= matches!(
                message.message,
                WM_SETTINGCHANGE | WM_SYSCOLORCHANGE | WM_THEMECHANGED
            );
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if prefs_dirty || preferences_at.elapsed() >= Duration::from_secs(1) {
            prefs = Preferences::read();
            preferences_at = Instant::now();
        }
        let (revision, event, anchor, suspended, motion, input) = {
            let m = shared.lock();
            if m.shutdown {
                return Ok(());
            }
            (
                m.gate.revision,
                m.event.clone(),
                m.anchor,
                m.gate.suspended != 0,
                m.motion.clone(),
                m.updates,
            )
        };
        let now = shared.origin.elapsed().as_secs_f64() * 1000.;
        let active = event.and_then(event_anchor);
        // Only a new acknowledged sample may update the retained agent location.
        // Idle/physical user movement cannot continually drag that anchor around.
        let received = if input != last_input {
            active.map(|p| p.0)
        } else {
            None
        };
        let anchor = received
            .or(anchor)
            .or_else(cursor_anchor)
            .and_then(clamp_anchor)
            .map(|a| window.refresh_dpi(a));
        if received.is_some() {
            last_input = input;
        }
        let mut frame = motion.frame(now, prefs.animate);
        if active.is_none_or(|(_, p)| p == Phase::Idle) && frame.phase != Phase::Idle {
            frame = Frame::idle();
        }
        let display = if suspended { None } else { anchor };
        let (visible, submitted) = if let Some(anchor) = display {
            let submitted = window
                .present(anchor, frame, prefs.style())
                .map_err(|e| e.to_string())?;
            (unsafe { IsWindowVisible(window.hwnd) }.as_bool(), submitted)
        } else {
            window.hide().map_err(|e| e.to_string())?;
            (false, false)
        };
        let mut model = shared.lock();
        if anchor.is_some() && model.updates == input {
            model.anchor = anchor;
        }
        model.phase = display.map(|_| frame.phase);
        model.frame = Some(frame);
        model.preferences = Some(prefs);
        model.raster_ms = window.raster_ms;
        model.max_raster_ms = model.max_raster_ms.max(window.raster_ms);
        model.native_label = window.native_label;
        model.gate.acknowledge(revision, visible);
        if submitted {
            model.render_submissions += 1;
        }
        shared.changed.notify_all();
        if model.gate.revision != revision {
            continue;
        }
        let delay = if frame.animating && !suspended {
            16
        } else {
            100
        };
        drop(
            shared
                .changed
                .wait_timeout(model, Duration::from_millis(delay))
                .unwrap_or_else(|e| e.into_inner()),
        );
    }
}

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

struct Window {
    hwnd: HWND,
    surface: Option<Surface>,
    last: Option<(Anchor, Frame, Style, [i32; 2])>,
    material: Option<(u32, Style, Material)>,
    raster_ms: f64,
    native_label: bool,
}
impl Window {
    fn refresh_dpi(&self, anchor: Anchor) -> Anchor {
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
            last: None,
            material: None,
            raster_ms: 0.,
            native_label: false,
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
    fn present(
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
        let _ = self.hide();
        unsafe {
            let _ = DestroyWindow(self.hwnd);
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
