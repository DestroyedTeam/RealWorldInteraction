//! One device-local presenter. It never sends input, activates an app or reads UIA.
use super::{
    Anchor, CaptureGate, HIGHLIGHT_MS, Phase, SIZE,
    motion::{Frame, Motion},
    normalize_dpi,
};
mod label;
mod window;
use window::Window;
mod preferences;
use crate::{WindowRecord, desktop};
use preferences::Preferences;
use rwi_core::{Result, fault};
use std::{
    ffi::c_void,
    mem::size_of,
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    UI::{HiDpi::GetDpiForWindow, WindowsAndMessaging::*},
};

static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();
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
    worker_requested: bool,
    cleanup_uncertain: bool,
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

fn shared() -> &'static Arc<Shared> {
    SHARED.get_or_init(|| Arc::new(Shared::default()))
}

fn presenter() -> Option<&'static Arc<Shared>> {
    PRESENTER
        .get_or_init(|| {
            let shared = shared().clone();
            shared.lock().worker_requested = true;
            let worker = shared.clone();
            std::thread::Builder::new()
                .name("rwi-pointer-feedback".into())
                .spawn(move || {
                    let cleanup = rwi_core::operation::CleanupAudit::begin();
                    let outcome =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&worker)));
                    let mut model = worker.lock();
                    model.cleanup_uncertain = cleanup.uncertain();
                    model.error = match outcome {
                        Ok(Ok(())) => None,
                        Ok(Err(error)) => Some(error),
                        Err(_) => Some("pointer presenter panicked".into()),
                    };
                    // run's thread-owned Window/Surface guards were destroyed before this.
                    if !model.cleanup_uncertain {
                        model.gate.visible = false;
                    }
                    model.stopped = true;
                    model.gate.applied_revision = model.gate.revision;
                    worker.changed.notify_all();
                })
                .map_err(|e| {
                    let mut model = shared.lock();
                    model.error = Some(e.to_string());
                    model.stopped = true;
                    model.gate.visible = false;
                    shared.changed.notify_all();
                    e.to_string()
                })?;
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

mod capture;
pub(crate) use capture::{snapshot as capture_status, suspend_capture};

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
    result["capture_state"] = capture_status();
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
