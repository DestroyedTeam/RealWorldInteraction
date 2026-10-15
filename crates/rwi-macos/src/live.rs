//! Context-owned preview workers. Only the AppKit presenter touches windows.
mod image;
mod presenter;
mod stop_drain;
mod view_gate;
mod worker;
use crate::{
    native::{Display, NativeMac, WindowInventory, displays, window_inventory},
    session,
};
pub(crate) use presenter::Presenter;
use rwi_core::{LiveAction, LiveOptions, Result, VisualCapture, VisualOptions, fault};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Condvar, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use view_gate::ViewGate;
use worker::run;
#[cfg(test)]
use worker::wait_for_next_frame;

const CAPACITY: usize = 4;
const SOURCE_RECOVERY_ATTEMPTS: u8 = 8;
const SOURCE_RECOVERY_DELAY: Duration = Duration::from_secs(1);
static INSTALLED: AtomicBool = AtomicBool::new(false);
static NEXT: AtomicU64 = AtomicU64::new(1);
static SESSIONS: OnceLock<Mutex<BTreeMap<u64, Weak<Shared>>>> = OnceLock::new();
fn sessions() -> &'static Mutex<BTreeMap<u64, Weak<Shared>>> {
    SESSIONS.get_or_init(Mutex::default)
}
pub(crate) fn install() {
    INSTALLED.store(true, Ordering::Release);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Running,
    Recovering,
    Paused,
    Hidden,
    Stopped,
}
struct State {
    source: Arc<NativeMac>,
    additional_sources: Vec<Arc<NativeMac>>,
    generation: u64,
    mode: Mode,
    options: LiveOptions,
    display: Display,
    deadline: Instant,
    latest: Option<Arc<VisualCapture>>,
    export_latest: Option<image::CachedFrame>,
    metadata: Value,
    captured: u64,
    presented: u64,
    capture_pending: bool,
    native_operation: Option<rwi_core::operation::Operation>,
    worker_done: bool,
    visible: bool,
    image_present: bool,
    window_id: Option<u32>,
    last_capture_ms: Option<u128>,
    last_error: Option<String>,
    last_error_detail: Option<String>,
    source_retry_count: u8,
    source_retry_at: Option<Instant>,
    presenter_failure: Option<Value>,
    stop_reason: Option<String>,
    host_view: Option<ViewGate>,
}
impl State {
    fn reset_source_recovery(&mut self) {
        self.source_retry_count = 0;
        self.source_retry_at = None;
        self.last_error_detail = None;
    }
    // Exact read-only source only: no input, focus, unhide, selection or TTL extension.
    fn capture_failed(&mut self, error: rwi_core::Fault) {
        if !matches!(self.mode, Mode::Running | Mode::Recovering) {
            return;
        }
        let temporary = matches!(
            error.code.as_str(),
            "window_missing" | "visual_window_changed" | "capture_binding_changed"
        );
        self.last_error = Some(error.code);
        self.last_error_detail = Some(error.next_step);
        self.clear();
        if temporary {
            self.source_retry_count = self
                .source_retry_count
                .saturating_add(1)
                .min(SOURCE_RECOVERY_ATTEMPTS);
            if self.source_retry_count < SOURCE_RECOVERY_ATTEMPTS {
                self.mode = Mode::Recovering;
                self.source_retry_at = Some(Instant::now() + SOURCE_RECOVERY_DELAY);
                return;
            }
        }
        self.mode = Mode::Paused;
        self.source_retry_at = None;
    }
    fn source_retry_ready(&self, now: Instant) -> bool {
        self.mode != Mode::Recovering || self.source_retry_at.is_some_and(|at| now >= at)
    }
    // Called only after the worker's existing exact native pre/post checks.
    fn capture_succeeded(&mut self, frame: VisualCapture) {
        debug_assert!(matches!(self.mode, Mode::Running | Mode::Recovering));
        self.mode = Mode::Running;
        self.reset_source_recovery();
        self.metadata = json!({"window":frame.window,"title":frame.details["title"],"display":frame.details["display"],"captured_unix_ms":frame.captured_unix_ms,"image_size":frame.image_size,"family":frame.details["family"]});
        self.captured += 1;
        let frame = Arc::new(frame);
        self.export_latest = image::CachedFrame::new(self.generation, self.captured, frame.clone());
        self.latest = Some(frame);
        self.last_error = None;
    }
    fn clear(&mut self) {
        if let Some(operation) = &self.native_operation {
            operation.cancel();
        }
        self.generation = self.generation.wrapping_add(1);
        self.latest = None;
        self.export_latest = None;
    }
    fn native_pending(&self) -> bool {
        self.native_operation
            .as_ref()
            .is_some_and(|operation| operation.status().native_call_pending)
    }
    fn stop(&mut self, reason: &str) {
        // A later Stop/Drop must not replace the first terminal cause or appear
        // to begin a new cancellation generation. Preserve the observed failure.
        if self.mode == Mode::Stopped {
            return;
        }
        self.mode = Mode::Stopped;
        self.source_retry_at = None;
        self.stop_reason = Some(reason.into());
        self.clear();
    }
    fn expire(&mut self) {
        if self
            .host_view
            .as_mut()
            .is_some_and(|view| view.expire(Instant::now()))
        {
            self.clear();
        }
        if self.mode != Mode::Stopped && Instant::now() >= self.deadline {
            self.stop("duration_expired");
        }
    }
    fn presentation_allowed(&self) -> bool {
        self.host_view
            .as_ref()
            .is_none_or(|view| view.visible(Instant::now()))
    }
    fn control(&mut self, action: LiveAction) {
        self.expire();
        match action {
            LiveAction::Pause
                if matches!(self.mode, Mode::Running | Mode::Recovering | Mode::Paused) =>
            {
                self.mode = Mode::Paused;
                self.last_error = None;
                self.reset_source_recovery();
                self.clear();
            }
            LiveAction::Hide if self.mode != Mode::Stopped => {
                self.mode = Mode::Hidden;
                self.last_error = None;
                self.reset_source_recovery();
                self.clear();
            }
            LiveAction::Resume
                if matches!(self.mode, Mode::Paused | Mode::Recovering | Mode::Hidden) =>
            {
                self.mode = Mode::Running;
                self.last_error = None;
                self.reset_source_recovery();
                self.deadline = Instant::now() + Duration::from_millis(self.options.duration_ms);
                self.clear();
            }
            LiveAction::Stop => self.stop("requested"),
            _ => (),
        }
    }
    fn accepts(&self, generation: u64) -> bool {
        matches!(self.mode, Mode::Running | Mode::Recovering)
            && self.presentation_allowed()
            && self.generation == generation
            && Instant::now() < self.deadline
    }
}
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
}
pub(crate) struct Session {
    controls: Arc<Controls>,
}
pub(crate) struct Controls {
    id: u64,
    shared: Arc<Shared>,
}
impl std::ops::Deref for Session {
    type Target = Controls;
    fn deref(&self) -> &Controls {
        &self.controls
    }
}
impl Session {
    pub(crate) fn start(source: NativeMac, options: LiveOptions) -> Result<Self> {
        options.validate()?;
        let additional_sources = options
            .additional_windows
            .iter()
            .map(|selection| source.additional_visual_source(selection).map(Arc::new))
            .collect::<Result<Vec<_>>>()?;
        if source.pid == std::process::id() {
            return Err(fault(
                "live_self_capture_excluded",
                "Select an application window, not an RWI Agent presenter.",
            ));
        }
        if !INSTALLED.load(Ordering::Acquire) {
            return Err(fault(
                "live_presenter_unavailable",
                "The Agent main-thread presenter is not installed.",
            ));
        }
        if session::permissions()["screen_capture"] != true || !crate::capture::api_available() {
            return Err(fault(
                "screen_capture_permission_missing",
                "Live needs existing ScreenCaptureKit permission.",
            ));
        }
        let list = displays()?;
        let matches = list
            .into_iter()
            .filter(|d| options.display.matches(d.built_in, d.id, d.uuid.as_deref()))
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(fault(
                "live_display_ambiguous",
                "Select one current preview display by UUID or native id.",
            ));
        }
        let display = matches.into_iter().next().unwrap();
        if display.rect.w < 300. || display.rect.h < 300. {
            return Err(fault(
                "live_display_too_small",
                "The allocated preview display must fit a 300-point panel.",
            ));
        }
        let mut registry = sessions()
            .lock()
            .map_err(|_| fault("live_state_unavailable", "Preview registry unavailable."))?;
        registry.retain(|_, weak| {
            weak.upgrade().is_some_and(|s| {
                s.state
                    .lock()
                    .map(|s| !(s.mode == Mode::Stopped && s.worker_done))
                    .unwrap_or(true)
            })
        });
        if registry.len() >= CAPACITY {
            return Err(fault(
                "live_capacity",
                "At most four active or draining preview sessions; stop an existing preview.",
            ));
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                host_view: options.host_view_id.clone().map(ViewGate::new),
                source: Arc::new(source),
                additional_sources,
                generation: 1,
                mode: Mode::Running,
                deadline: Instant::now() + Duration::from_millis(options.duration_ms),
                options,
                display,
                latest: None,
                export_latest: None,
                metadata: Value::Null,
                captured: 0,
                presented: 0,
                capture_pending: false,
                native_operation: None,
                worker_done: false,
                visible: false,
                image_present: false,
                window_id: None,
                last_capture_ms: None,
                last_error: None,
                last_error_detail: None,
                source_retry_count: 0,
                source_retry_at: None,
                presenter_failure: None,
                stop_reason: None,
            }),
            wake: Condvar::new(),
        });
        let worker = shared.clone();
        std::thread::Builder::new()
            .name(format!("rwi-live-{id}"))
            .spawn(move || run(worker))
            .map_err(|_| {
                fault(
                    "live_worker_unavailable",
                    "Could not start the bounded preview worker.",
                )
            })?;
        registry.insert(id, Arc::downgrade(&shared));
        Ok(Self {
            controls: Arc::new(Controls { id, shared }),
        })
    }
    pub(crate) fn controller(&self) -> Arc<Controls> {
        self.controls.clone()
    }
    pub(crate) fn active(&self) -> bool {
        self.shared
            .state
            .lock()
            .map(|mut s| {
                s.expire();
                s.mode != Mode::Stopped
            })
            .unwrap_or(false)
    }
    pub(crate) fn source(&self, source: NativeMac) {
        // Inspecting our own presenter must not turn it into its own input stream.
        if source.pid == std::process::id() {
            return;
        }
        if let Ok(mut s) = self.shared.state.lock() {
            s.expire();
            if s.mode != Mode::Stopped {
                s.source = Arc::new(source);
                // Additional sources belong to the explicitly started root,
                // not a process name. A later root rebind must not keep them.
                s.additional_sources.clear();
                s.options.additional_windows.clear();
                s.metadata = Value::Null;
                s.reset_source_recovery();
                s.clear();
                if matches!(s.mode, Mode::Paused | Mode::Recovering)
                    && s.last_error.take().is_some()
                {
                    s.mode = Mode::Running;
                }
            }
        }
        self.shared.wake.notify_all();
    }
    pub(crate) fn source_failed(&self, error: String) {
        if let Ok(mut s) = self.shared.state.lock()
            && s.mode != Mode::Stopped
        {
            if s.mode != Mode::Hidden {
                s.mode = Mode::Paused;
            }
            s.reset_source_recovery();
            s.last_error = Some(error);
            s.clear();
        }
        self.shared.wake.notify_all();
    }
}
impl Controls {
    pub(crate) fn control(&self, action: LiveAction) -> Value {
        let started = Instant::now();
        if let Ok(mut s) = self.shared.state.lock() {
            s.control(action);
            self.shared.wake.notify_all();
            // Return observed presenter state when it settles promptly. This
            // never fabricates callback completion or waits without a bound.
            let until = stop_drain::deadline(action, started);
            loop {
                let settled = stop_drain::settled(&s, action);
                if settled || Instant::now() >= until {
                    break;
                }
                match self
                    .shared
                    .wake
                    .wait_timeout(s, stop_drain::wait_slice(until))
                {
                    Ok((state, _)) => s = state,
                    Err(error) => {
                        drop(error);
                        return json!({"status":"unavailable","last_error":"live_state_poisoned"});
                    }
                }
            }
        }
        stop_drain::annotate(action, started, self.status())
    }
    pub(crate) fn view_update(&self, update: rwi_core::LiveViewUpdate) -> Result<Value> {
        let mut s = self
            .shared
            .state
            .lock()
            .map_err(|_| fault("live_state_unavailable", "Preview state unavailable."))?;
        s.expire();
        if s.mode == Mode::Stopped {
            return Ok(json!({"accepted":false,"reason":"preview_stopped"}));
        }
        let before = s.presentation_allowed();
        let gate = s.host_view.as_mut().ok_or_else(|| {
            fault(
                "live_view_unbound",
                "This preview has no Host view binding.",
            )
        })?;
        let accepted = gate.update(&update, Instant::now())?;
        let after = s.presentation_allowed();
        if before != after {
            s.clear();
        }
        self.shared.wake.notify_all();
        Ok(json!({"accepted":accepted,"presentation_allowed":after,"sequence":update.sequence}))
    }
    /// AppKit can report orderOut before the window server removes the panel.
    /// Keep that acknowledgement separate from bounded, read-only native
    /// visibility/geometry confirmation. No capture, UI replay or activation.
    pub(crate) fn control_native(&self, action: LiveAction) -> Value {
        self.control_native_with_readback(action, |window| {
            window_inventory()
                .and_then(|inventory| presenter_readback(&inventory, window, std::process::id()))
        })
    }
    fn control_native_with_readback(
        &self,
        action: LiveAction,
        mut readback: impl FnMut(Option<u32>) -> Result<Option<crate::geometry::Rect>>,
    ) -> Value {
        let call_started = Instant::now();
        let mut status = self.control(action);
        let started = Instant::now();
        let mut previous = None;
        let mut checks = 0;
        loop {
            let window = status["presenter_window"]
                .as_u64()
                .and_then(|id| u32::try_from(id).ok());
            let readback = readback(window);
            checks += 1;
            let (bounds, error) = match readback {
                Ok(bounds) => (Some(bounds), None),
                Err(error) => (None, Some(error.code)),
            };
            let consistent = bounds.is_some_and(|bounds| {
                presentation_matches(&status, bounds)
                    && (action == LiveAction::Status || previous == Some(bounds))
            });
            let done = consistent
                || error.is_some()
                || action == LiveAction::Status
                || started.elapsed() >= Duration::from_millis(120);
            if done {
                let on_requested_display = bounds
                    .flatten()
                    .map(|rect| requested_display_contains(&status, rect));
                status["presenter_readback"] = json!({
                    "on_screen":bounds.map(|bounds| bounds.is_some()),
                    "bounds":bounds.flatten().map(|r| [r.x,r.y,r.w,r.h]),
                    "on_requested_display":on_requested_display,
                    "settled":consistent,"checks":checks,
                    "elapsed_ms":started.elapsed().as_millis(),"error":error,
                });
                // Refreshes below deliberately read actual state, not an earlier
                // annotation. Recompute final completion for this same call.
                return stop_drain::annotate(action, call_started, status);
            }
            previous = bounds;
            std::thread::sleep(Duration::from_millis(10));
            status = self.status();
        }
    }
    pub(crate) fn status(&self) -> Value {
        let Ok(mut s) = self.shared.state.lock() else {
            return json!({"status":"unavailable"});
        };
        s.expire();
        json!({
            "session_id":format!("live:{}",self.id),
            "status":format!("{:?}",s.mode).to_lowercase(),
            "ownership":"this_MCP_device_context",
            "presentation":"RWI_Agent_native_nonactivating_panel",
            "follow":"successful_explicit_bind_window_only; RWI Agent windows excluded",
            "requested_preview_display":crate::catalog::DisplayRecord::from_native(&s.display),
            "source":s.metadata, "frames_captured":s.captured, "frames_presented":s.presented,
            "capture_pending":s.capture_pending || s.native_pending(), "worker_done":s.worker_done,
            "native_call_pending":s.native_pending(),
            "presenter_visible":s.visible, "presenter_image_present":s.image_present,
            "user_hidden":s.mode == Mode::Hidden,
            "presentation_suspended":!s.presentation_allowed(),
            "host_view_binding":s.host_view.as_ref().map(|view| &view.id),
            "host_view_lease_ms":s.host_view.as_ref().map(|_| view_gate::VIEW_LEASE.as_millis()),
            "presenter_window":s.window_id, "last_capture_ms":s.last_capture_ms,
            "last_error":s.last_error, "last_error_detail":s.last_error_detail,
            "source_recovery":{
                "pending":s.mode == Mode::Recovering,
                "attempts":s.source_retry_count,"max_attempts":SOURCE_RECOVERY_ATTEMPTS,
                "retry_after_ms":s.source_retry_at.map(|at|at.saturating_duration_since(Instant::now()).as_millis()),
                "read_only":true,"input_replay":false,"source_reselected":false,
                "exhausted":s.mode == Mode::Paused && s.source_retry_count >= SOURCE_RECOVERY_ATTEMPTS,
            }, "stop_reason":s.stop_reason,
            "presenter_failure":s.presenter_failure,
            "expires_in_ms":s.deadline.saturating_duration_since(Instant::now()).as_millis(),
            "frame_created":false,"input_authority":false,"input_sent":false,
            "retention":"one volatile pending frame and one displayed image; no disk recording",
            "capture_protection":"not promised; pause clears the preview",
        })
    }
}

/// A missing panel is conclusive only when the native inventory is complete.
/// A reused CGWindowID owned by another process is never our presenter.
fn presenter_readback(
    inventory: &WindowInventory,
    window: Option<u32>,
    pid: u32,
) -> Result<Option<crate::geometry::Rect>> {
    let Some(window) = window else {
        return Ok(None);
    };
    let mut matches = inventory.windows.iter().filter(|row| row.id == window);
    match (matches.next(), matches.next()) {
        (Some(row), None) if row.pid == pid && row.rect.valid() => Ok(Some(row.rect)),
        (None, None) if inventory.invalid_rows == 0 => Ok(None),
        _ => Err(fault(
            "live_presenter_readback_unknown",
            "The exact presenter window is ambiguous, replaced or missing from an incomplete inventory.",
        )),
    }
}
fn presentation_matches(status: &Value, bounds: Option<crate::geometry::Rect>) -> bool {
    let visible = bounds.is_some();
    if status["presenter_visible"].as_bool() != Some(visible) {
        return false;
    }
    if bounds.is_some_and(|bounds| !requested_display_contains(status, bounds)) {
        return false;
    }
    if status["presentation_suspended"] == true {
        return !visible && status["presenter_image_present"] == false;
    }
    match status["status"].as_str() {
        Some("stopped") => !visible && status["presenter_image_present"] == false,
        Some("hidden") => !visible && status["presenter_image_present"] == false,
        Some("paused" | "recovering") => visible && status["presenter_image_present"] == false,
        Some("running") => visible && status["presenter_image_present"] == true,
        _ => false,
    }
}
fn requested_display_contains(status: &Value, bounds: crate::geometry::Rect) -> bool {
    let Some(values) = status["requested_preview_display"]["bounds"].as_array() else {
        return false;
    };
    let [x, y, w, h] = values.as_slice() else {
        return false;
    };
    let Some((x, y, w, h)) = x
        .as_f64()
        .zip(y.as_f64())
        .zip(w.as_f64())
        .zip(h.as_f64())
        .map(|(((x, y), w), h)| (x, y, w, h))
    else {
        return false;
    };
    let display = crate::geometry::Rect { x, y, w, h };
    display.valid() && display.contains_rect(bounds)
}

#[cfg(test)]
mod tests;
impl Drop for Session {
    fn drop(&mut self) {
        if let Ok(mut s) = self.shared.state.lock() {
            s.stop("context_closed");
        }
        self.shared.wake.notify_all();
    }
}
