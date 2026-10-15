//! Explicit AX foreground route; never invoked by observe or background input.
//! Public AXRaise + writable AXFrontmost, once each, no key/cursor fallback.
use super::*;
use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};
use rwi_core::ForegroundState;
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

const FOREGROUND_OBJECT_TIMEOUT_SECONDS: f32 = 0.05;

// Timeout policy is local to this retained object, not CFEqual objects or the process.
fn checked_foreground_object_timeout<T>(
    object: T,
    set_timeout: impl FnOnce(&T, f32) -> i32,
) -> Result<T> {
    let status = set_timeout(&object, FOREGROUND_OBJECT_TIMEOUT_SECONDS);
    if status != 0 {
        let mut error = fault(
            "foreground_timeout_configuration_failed",
            "Exact foreground AX object timeout setup failed; no retry or fallback.",
        );
        error.diagnostic = Some(Box::new(serde_json::json!({
            "api": "AXUIElementSetMessagingTimeout",
            "native_status": status,
            "timeout_seconds": FOREGROUND_OBJECT_TIMEOUT_SECONDS,
            "scope": "object_local",
            "replay_allowed": false
        })));
        return Err(error);
    }
    Ok(object)
}

fn foreground_object_timeout(object: Cf) -> Result<Cf> {
    checked_foreground_object_timeout(object, |object, seconds| unsafe {
        ffi::AXUIElementSetMessagingTimeout(object.0, seconds)
    })
}

// AXUIElementSetMessagingTimeout(system-wide) changes this process's global
// default. Never set it here. The public Workspace getter runs on the existing
// main run loop; four separately created app/window AX objects remain local.
#[derive(Clone)]
pub(super) struct PublicForegroundApplication {
    object: objc2::rc::Retained<objc2::runtime::AnyObject>,
    pid: u32,
    same_previous: Option<bool>,
}
// SAFETY: Only read_public_application_on_main constructs this private type,
// after checking NSRunningApplication. Apple documents that class as thread safe.
// Worker threads only retain/release it; all Objective-C getters/isEqual run on main.
unsafe impl Send for PublicForegroundApplication {}
impl PublicForegroundApplication {
    pub(super) fn pid(&self) -> u32 {
        self.pid
    }
    pub(super) fn same_previous(&self) -> Option<bool> {
        self.same_previous
    }
}

fn positive_public_pid(pid: Option<i32>) -> Result<u32> {
    pid.filter(|pid| *pid > 0)
        .map(|pid| pid as u32)
        .ok_or_else(|| {
            fault(
                "foreground_identity_unknown",
                "Public frontmost application/PID is absent or invalid; no guess.",
            )
        })
}

fn read_public_application_on_main(
    previous: Option<&PublicForegroundApplication>,
) -> Result<PublicForegroundApplication> {
    use objc2::{
        msg_send,
        rc::Retained,
        runtime::{AnyClass, AnyObject, Bool},
    };
    if unsafe { libc::pthread_main_np() } != 1 {
        return Err(fault(
            "foreground_workspace_thread_unknown",
            "Public foreground metadata must run on the actual main thread.",
        ));
    }
    objc2::rc::autoreleasepool(|_| unsafe {
        let class = AnyClass::get(c"NSWorkspace").ok_or_else(|| {
            fault(
                "foreground_workspace_unavailable",
                "Public Workspace class is unavailable; no system AX fallback.",
            )
        })?;
        let workspace: *mut AnyObject = msg_send![class, sharedWorkspace];
        let workspace = workspace.as_ref().ok_or_else(|| {
            fault(
                "foreground_workspace_unavailable",
                "Public Workspace returned nil.",
            )
        })?;
        let raw: *mut AnyObject = msg_send![workspace, frontmostApplication];
        let object = Retained::retain(raw).ok_or_else(|| {
            fault(
                "foreground_identity_unknown",
                "Public frontmost application is nil.",
            )
        })?;
        let running = AnyClass::get(c"NSRunningApplication").ok_or_else(|| {
            fault(
                "foreground_workspace_unavailable",
                "Public running-application class is unavailable.",
            )
        })?;
        let actual_class: Bool = msg_send![&*object, isKindOfClass: running];
        if !actual_class.as_bool() {
            return Err(fault(
                "foreground_identity_unknown",
                "Public foreground object is not a running application.",
            ));
        }
        let raw_pid: libc::pid_t = msg_send![&*object, processIdentifier];
        let pid = positive_public_pid(Some(raw_pid))?;
        let same_previous = previous.map(|old| {
            let equal: Bool = msg_send![&*old.object, isEqual: &*object];
            equal.as_bool()
        });
        Ok(PublicForegroundApplication {
            object,
            pid,
            same_previous,
        })
    })
}

const PUBLIC_FOREGROUND_READ_BUDGET: Duration = Duration::from_millis(100);
const PUBLIC_FOREGROUND_QUEUE_LIMIT: usize = 4;
static PUBLIC_FOREGROUND_QUEUED: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
fn acquire_public_foreground_slot(counter: &std::sync::atomic::AtomicUsize) -> Result<()> {
    use std::sync::atomic::Ordering;
    counter
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
            if count < PUBLIC_FOREGROUND_QUEUE_LIMIT {
                Some(count + 1)
            } else {
                None
            }
        })
        .map(|_| ())
        .map_err(|_| {
            fault(
                "foreground_workspace_queue_busy",
                "Bounded public metadata queue is full; no fallback.",
            )
        })
}
fn release_public_foreground_slot(counter: &std::sync::atomic::AtomicUsize) {
    counter.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
}
struct PublicForegroundPermit;
impl PublicForegroundPermit {
    fn acquire() -> Result<Self> {
        acquire_public_foreground_slot(&PUBLIC_FOREGROUND_QUEUED).map(|_| Self)
    }
}
impl Drop for PublicForegroundPermit {
    fn drop(&mut self) {
        release_public_foreground_slot(&PUBLIC_FOREGROUND_QUEUED);
    }
}
const PUBLIC_READ_PENDING: u8 = 0;
const PUBLIC_READ_STARTED: u8 = 1;
const PUBLIC_READ_CANCELLED: u8 = 2;
struct PublicForegroundRead {
    deadline: Instant,
    phase: std::sync::atomic::AtomicU8,
}
impl PublicForegroundRead {
    fn cancel(&self) {
        // Pending->Cancelled prevents a later start CAS. Started->Cancelled only
        // discards the result: it is not strong preemption of an ObjC getter.
        self.phase
            .store(PUBLIC_READ_CANCELLED, std::sync::atomic::Ordering::Release);
    }
    fn execute<T>(&self, read: impl FnOnce() -> Result<T>) -> Result<T> {
        self.execute_after_gate(|| {}, read)
    }
    fn execute_after_gate<T>(
        &self,
        before_start: impl FnOnce(),
        read: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        use std::sync::atomic::Ordering;
        // The production hook is empty; the pure barrier test uses this point
        // before the shared start gate, without depending on thread scheduling.
        before_start();
        // One atomic transition is the cancellation/start linearization point.
        // No caller/model supplies phase or bypasses this private ticket.
        if Instant::now() >= self.deadline
            || self
                .phase
                .compare_exchange(
                    PUBLIC_READ_PENDING,
                    PUBLIC_READ_STARTED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_err()
        {
            return Err(fault(
                "foreground_workspace_read_expired",
                "Cancelled, expired or already-started metadata read; no repeat.",
            ));
        }
        self.accept(read())
    }
    fn accept<T>(&self, value: Result<T>) -> Result<T> {
        if self.phase.load(std::sync::atomic::Ordering::Acquire) != PUBLIC_READ_STARTED
            || Instant::now() >= self.deadline
        {
            return Err(fault(
                "foreground_workspace_read_expired",
                "Public metadata arrived after its budget or cancellation; no authority.",
            ));
        }
        value
    }
}

// CFRunLoopPerformBlock copies the block before returning. Only this missing
// Blocks ABI is declared locally; no new runtime, event pump, UI or dependency.
unsafe extern "C" {
    fn CFRunLoopPerformBlock(
        run_loop: core_foundation::runloop::CFRunLoopRef,
        mode: core_foundation::base::CFTypeRef,
        block: *const std::ffi::c_void,
    );
}
pub(super) fn public_foreground_application(
    previous: Option<PublicForegroundApplication>,
) -> Result<PublicForegroundApplication> {
    use std::sync::{
        Arc,
        mpsc::{self, RecvTimeoutError},
    };
    operation::checkpoint("before_public_foreground_read")?;
    let job = Arc::new(PublicForegroundRead {
        deadline: Instant::now() + PUBLIC_FOREGROUND_READ_BUDGET,
        phase: std::sync::atomic::AtomicU8::new(PUBLIC_READ_PENDING),
    });
    if unsafe { libc::pthread_main_np() } == 1 {
        let result = job.execute(|| read_public_application_on_main(previous.as_ref()));
        operation::checkpoint("public_foreground_read_returned")?;
        return job.accept(result);
    }
    let permit = PublicForegroundPermit::acquire()?;
    let (sender, receiver) = mpsc::sync_channel(1);
    let queued = job.clone();
    // Keep the original operation's pending-native scope alive until CF releases
    // its copied block, even if the waiter times out. No metadata request is input.
    let native = operation::native_call("macos_public_foreground_workspace_read")?;
    let block = block2::RcBlock::new(move || {
        let _held = (&permit, &native);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            queued.execute(|| read_public_application_on_main(previous.as_ref()))
        }))
        .unwrap_or_else(|_| {
            Err(fault(
                "foreground_workspace_read_unconfirmed",
                "Public metadata read panicked; no fallback.",
            ))
        });
        let _ = sender.try_send(result);
    });
    unsafe {
        let main = core_foundation::runloop::CFRunLoopGetMain();
        if main.is_null() {
            return Err(fault(
                "foreground_workspace_unavailable",
                "No main run loop; no fallback.",
            ));
        }
        CFRunLoopPerformBlock(
            main,
            core_foundation::runloop::kCFRunLoopCommonModes.cast(),
            block2::RcBlock::as_ptr(&block).cast(),
        );
        core_foundation::runloop::CFRunLoopWakeUp(main);
    }
    // Never run or pump the main loop from this worker. This bounds the waiter,
    // not an already-running Objective-C getter; that scope stays pending.
    loop {
        if let Err(error) = operation::checkpoint("await_public_foreground_read") {
            job.cancel();
            return Err(error);
        }
        let Some(remaining) = job.deadline.checked_duration_since(Instant::now()) else {
            job.cancel();
            return Err(fault(
                "foreground_workspace_read_expired",
                "Main metadata read deadline elapsed; no fallback.",
            ));
        };
        match receiver.recv_timeout(remaining.min(Duration::from_millis(5))) {
            Ok(result) => {
                operation::checkpoint("public_foreground_read_returned")?;
                // A delayed waiter may receive a value computed before expiry.
                // Never accept it after this immutable metadata budget expired.
                return job.accept(result);
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                job.cancel();
                return Err(fault(
                    "foreground_workspace_read_unconfirmed",
                    "Main metadata reply disconnected; no retry.",
                ));
            }
        }
    }
}

struct PublicForegroundReadback {
    original_pid: u32,
    public_pid: u32,
    ax_pid: u32,
    birth_unchanged: bool,
    public_application_equal: Option<bool>,
    ax_application_equal: bool,
    ax_window_equal: bool,
    geometry_equal: bool,
}
fn require_public_foreground_unchanged(read: PublicForegroundReadback) -> Result<()> {
    if read.public_pid != read.original_pid
        || read.ax_pid != read.original_pid
        || !read.birth_unchanged
        || read.public_application_equal != Some(true)
        || !read.ax_application_equal
        || !read.ax_window_equal
        || !read.geometry_equal
    {
        return Err(fault(
            "foreground_changed",
            "Independent public application, birth or exact AX window changed; no foreground conclusion.",
        ));
    }
    Ok(())
}

pub(crate) fn foreground_state() -> Result<ForegroundState> {
    let _call = operation::native_call("macos_foreground_metadata")?;
    if unsafe { ffi::AXIsProcessTrusted() } == 0 {
        return Err(fault(
            "accessibility_permission_missing",
            "Foreground identity requires existing AX permission.",
        ));
    }
    let public = public_foreground_application(None)?;
    let pid = public.pid;
    let process_start = process_epoch(pid)?;
    let app = foreground_object_timeout(unsafe {
        Cf::owned(ffi::AXUIElementCreateApplication(pid as i32))?
    })?;
    if app.pid()? != pid {
        return Err(fault(
            "foreground_identity_unknown",
            "Public PID and exact AX application disagree.",
        ));
    }
    let focused = foreground_object_timeout(app.attr("AXFocusedWindow")?)?;
    if focused.pid()? != pid {
        return Err(fault(
            "foreground_identity_unknown",
            "Focused window belongs to another provider.",
        ));
    }
    let id = ffi::window_number_for_ax_element(&focused).ok_or_else(|| {
        fault(
            "foreground_identity_unknown",
            "No exact foreground window number; do not guess.",
        )
    })?;
    let row = windows()?
        .into_iter()
        .find(|w| w.pid == pid && w.id == id)
        .ok_or_else(|| {
            fault(
                "foreground_identity_unknown",
                "Foreground has no current exact on-screen native window.",
            )
        })?;
    let after_public = public_foreground_application(Some(public.clone()))?;
    if after_public.pid != pid || after_public.same_previous != Some(true) {
        return Err(fault(
            "foreground_changed",
            "Public frontmost application changed; no replacement-app probe.",
        ));
    }
    let after_app = foreground_object_timeout(unsafe {
        Cf::owned(ffi::AXUIElementCreateApplication(after_public.pid as i32))?
    })?;
    let after_window = foreground_object_timeout(after_app.attr("AXFocusedWindow")?)?;
    require_public_foreground_unchanged(PublicForegroundReadback {
        original_pid: pid,
        public_pid: after_public.pid,
        ax_pid: after_app.pid()?,
        birth_unchanged: process_epoch(pid)? == process_start,
        public_application_equal: after_public.same_previous,
        ax_application_equal: unsafe { ffi::CFEqual(app.0, after_app.0) } != 0,
        ax_window_equal: unsafe { ffi::CFEqual(focused.0, after_window.0) } != 0,
        geometry_equal: ax_rect(&focused)?.same(row.rect),
    })?;
    let native_epoch = digest(
        format!(
            "{pid}:{process_start}:{id}:{:?}:{}",
            row.rect,
            digest(row.title.as_deref().unwrap_or("").as_bytes())
        )
        .as_bytes(),
    );
    Ok(ForegroundState {
        pid,
        window: format!("cgwindow:{id}"),
        process_start,
        native_epoch,
    })
}
impl NativeMac {
    pub(crate) fn focus_exact_window(
        &self,
        reference: &Resolved,
        expected: &ForegroundState,
    ) -> Result<serde_json::Value> {
        expected.validate()?;
        if !self.allow_input
            || self.semantic_only
            || !reference.placement_only
            || reference.scope.pid != self.pid
            || reference.scope.window != self.window_name()
        {
            return Err(fault(
                "foreground_scope_unavailable",
                "Explicit foreground requires an input-authorized exact visual window-level reference.",
            ));
        }
        let _call = operation::native_call("macos_explicit_foreground")?;
        let check = || -> Result<()> {
            operation::checkpoint("foreground_guard")?;
            if console_identity()? != self.identity || process_epoch(self.pid)? != self.process {
                return Err(fault(
                    "session_or_process_changed",
                    "Pinned session/process changed; no fallback.",
                ));
            }
            // AX owns this route. CGEvent posting permission is unrelated; only
            // the existing input-observation permission is needed for HID yield.
            if !unsafe { ffi::CGPreflightListenEventAccess() } {
                return Err(fault(
                    "listen_event_permission_missing",
                    "Foreground requires existing listen authorization for cooperative HID yield.",
                ));
            }
            // Foreground changes affect the global keyboard receiver, unlike scoped
            // background pointer actions. Yield to recent HID input everywhere.
            if crate::activity::seconds_since_hid_input().is_none_or(|s| s < 1.2) {
                return Err(fault(
                    "user_active",
                    "Explicit foreground yields to recent/unknown HID input; never replay.",
                ));
            }
            Ok(())
        };
        check()?;
        let before = foreground_state()?;
        if &before != expected {
            return Err(fault(
                "foreground_changed",
                "Foreground changed after the supplied observation; no focus request sent.",
            ));
        }
        let row = self.window_record()?;
        let app = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(app.0, 0.05);
        }
        let window = self.resolve_window_element(&app, row.rect)?;
        if window.pid()? != self.pid
            || !ax_rect(&window)?.same(row.rect)
            || ffi::window_number_for_ax_element(&window) != Some(self.window)
        {
            return Err(fault(
                "window_epoch_changed",
                "Exact AX/CG target identity is not stable.",
            ));
        }
        let is_target = |f: &ForegroundState| {
            f.pid == self.pid && f.window == self.window_name() && f.process_start == self.process
        };
        if is_target(&before) {
            return Ok(
                serde_json::json!({"state":"already_foreground","foreground":before,"native_requests":0,"goal_verified":false,"input_authority_granted":false,"coverage":"sampled_native_identity; not continuous human-input attestation"}),
            );
        }
        if !action_names(&window).iter().any(|a| a == "AXRaise")
            || !app.attribute_settable("AXFrontmost")?
        {
            return Err(fault(
                "foreground_api_unavailable",
                "Exact window must advertise AXRaise and application AXFrontmost must be writable; no shortcut or activation fallback.",
            ));
        }
        let dispatch = |object: &Cf, action: bool| -> Result<()> {
            check()?;
            // Recheck both receivers at each side-effect boundary. These are
            // sequential observations, not an atomic or continuous input proof.
            if foreground_state()? != before {
                return Err(fault(
                    "foreground_changed",
                    "Foreground changed at the dispatch boundary; no further request sent.",
                ));
            }
            let current_row = self.window_record()?;
            let current_window = self.resolve_window_element(&app, current_row.rect)?;
            if !current_row.rect.same(row.rect)
                || unsafe { ffi::CFEqual(window.0, current_window.0) } == 0
            {
                return Err(fault(
                    "window_epoch_changed",
                    "Target changed at the dispatch boundary; no further request sent.",
                ));
            }
            self.dispatches.fetch_add(1, Ordering::AcqRel);
            let code = unsafe {
                if action {
                    let name = CFString::new("AXRaise");
                    ffi::AXUIElementPerformAction(object.0, name.as_concrete_TypeRef().cast())
                } else {
                    let name = CFString::new("AXFrontmost");
                    let yes = CFBoolean::true_value();
                    ffi::AXUIElementSetAttributeValue(
                        object.0,
                        name.as_concrete_TypeRef().cast(),
                        yes.as_concrete_TypeRef().cast(),
                    )
                }
            };
            if code != 0 {
                return Err(fault(
                    "foreground_native_error",
                    "A native foreground request returned an error; an effect may still occur. Observe, never retry.",
                ));
            }
            Ok(())
        };
        dispatch(&window, true)?;
        let after_raise = foreground_state()?;
        if !is_target(&after_raise) && after_raise != before {
            return Err(fault(
                "foreground_changed",
                "Another foreground appeared after raise; no further request sent.",
            ));
        }
        if !is_target(&after_raise) {
            dispatch(&app, false)?;
        }
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            check()?;
            let now = foreground_state()?;
            let current_row = self.window_record()?;
            let current_window = self.resolve_window_element(&app, current_row.rect)?;
            if !current_row.rect.same(row.rect)
                || unsafe { ffi::CFEqual(window.0, current_window.0) } == 0
            {
                return Err(fault(
                    "window_epoch_changed",
                    "Target changed after a foreground request; never replay.",
                ));
            }
            if is_target(&now) {
                return Ok(
                    serde_json::json!({"state":"foreground_observed","foreground":now,"before_foreground":before,"after_foreground":now,
                    "goal_verified":false,"input_authority_granted":false,"coverage":"sampled_native_identity; not continuous human-input attestation",
                    "route":"public_AXRaise_then_AXFrontmost_once; no keyboard/warp/unhide/restore"}),
                );
            }
            if now != before {
                return Err(fault(
                    "foreground_changed",
                    "Another foreground appeared; yield without restoration or replay.",
                ));
            }
            if Instant::now() >= deadline {
                return Err(fault(
                    "foreground_observation_timeout",
                    "Foreground not observed within 500ms; effect may arrive later, no retry.",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
mod object_timeout_tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn foreground_object_timeout_sets_each_of_four_distinct_objects_once() {
        // All four can represent equal AX objects; each retained object still needs policy.
        let objects = [(1, "app"), (2, "window"), (3, "app"), (4, "window")];
        let mut calls = Vec::new();
        for object in objects {
            let returned = checked_foreground_object_timeout(object, |exact, seconds| {
                calls.push((*exact, seconds));
                0
            })
            .unwrap();
            assert_eq!(returned, object);
        }
        assert_eq!(calls, objects.map(|object| (object, 0.05)));
    }

    #[test]
    fn foreground_object_timeout_native_errors_are_preserved_without_retry() {
        for status in [-25201, -25202, -25204, 17] {
            let calls = Cell::new(0);
            let error = checked_foreground_object_timeout(73, |object, seconds| {
                assert_eq!(*object, 73);
                assert_eq!(seconds, 0.05);
                calls.set(calls.get() + 1);
                status
            })
            .unwrap_err();
            assert_eq!(calls.get(), 1);
            assert_eq!(error.code, "foreground_timeout_configuration_failed");
            let diagnostic = error.diagnostic.unwrap();
            assert_eq!(diagnostic["native_status"], status);
            assert_eq!(diagnostic["scope"], "object_local");
            assert_eq!(diagnostic["replay_allowed"], false);
        }
    }

    #[test]
    fn foreground_object_timeout_failure_stops_before_object_use() {
        let reads = Cell::new(0);
        let result = checked_foreground_object_timeout(37, |_, _| -25202).inspect(|_| {
            reads.set(reads.get() + 1);
        });
        assert!(result.is_err());
        assert_eq!(reads.get(), 0);
    }

    #[test]
    fn foreground_object_timeout_returns_same_owned_object_without_clone() {
        struct Object(String);
        let object = Object("fresh retained object".to_owned());
        let address = object.0.as_ptr();
        let returned = checked_foreground_object_timeout(object, |exact, seconds| {
            assert_eq!(exact.0.as_ptr(), address);
            assert_eq!(seconds, 0.05);
            0
        })
        .ok()
        .unwrap();
        assert_eq!(returned.0.as_ptr(), address);
    }
}

#[cfg(test)]
mod public_foreground_tests {
    use super::*;
    use std::{cell::Cell, sync::atomic::AtomicU8};
    #[test]
    fn foreground_public_nil_and_nonpositive_pid_refuse_before_ax() {
        for raw in [None, Some(-1), Some(0), Some(i32::MIN)] {
            let ax_reads = Cell::new(0);
            let result = positive_public_pid(raw).inspect(|_| ax_reads.set(ax_reads.get() + 1));
            assert_eq!(result.unwrap_err().code, "foreground_identity_unknown");
            assert_eq!(ax_reads.get(), 0);
        }
        assert_eq!(positive_public_pid(Some(17)).unwrap(), 17);
    }
    fn stable() -> PublicForegroundReadback {
        PublicForegroundReadback {
            original_pid: 17,
            public_pid: 17,
            ax_pid: 17,
            birth_unchanged: true,
            public_application_equal: Some(true),
            ax_application_equal: true,
            ax_window_equal: true,
            geometry_equal: true,
        }
    }
    #[test]
    fn foreground_public_independent_identity_axes_all_fail_closed() {
        require_public_foreground_unchanged(stable()).unwrap();
        for axis in 0..8 {
            let mut read = stable();
            match axis {
                0 => read.public_pid = 18,
                1 => read.ax_pid = 18,
                2 => read.birth_unchanged = false,
                3 => read.public_application_equal = None,
                4 => read.public_application_equal = Some(false),
                5 => read.ax_application_equal = false,
                6 => read.ax_window_equal = false,
                _ => read.geometry_equal = false,
            }
            assert_eq!(
                require_public_foreground_unchanged(read).unwrap_err().code,
                "foreground_changed"
            );
        }
    }
    #[test]
    fn foreground_public_cancelled_expired_or_duplicate_queue_reads_never_run() {
        let reads = Cell::new(0);
        let job = PublicForegroundRead {
            deadline: Instant::now() + Duration::from_secs(1),
            phase: AtomicU8::new(PUBLIC_READ_PENDING),
        };
        job.cancel();
        assert!(
            job.execute(|| {
                reads.set(reads.get() + 1);
                Ok(17)
            })
            .is_err()
        );
        let expired = PublicForegroundRead {
            deadline: Instant::now() - Duration::from_millis(1),
            phase: AtomicU8::new(PUBLIC_READ_PENDING),
        };
        assert!(
            expired
                .execute(|| {
                    reads.set(reads.get() + 1);
                    Ok(17)
                })
                .is_err()
        );
        assert_eq!(reads.get(), 0);
        let valid = PublicForegroundRead {
            deadline: Instant::now() + Duration::from_secs(1),
            phase: AtomicU8::new(PUBLIC_READ_PENDING),
        };
        assert_eq!(
            valid
                .execute(|| {
                    reads.set(reads.get() + 1);
                    Ok(17)
                })
                .unwrap(),
            17
        );
        assert!(
            valid
                .execute(|| {
                    reads.set(reads.get() + 1);
                    Ok(18)
                })
                .is_err()
        );
        assert_eq!(reads.get(), 1);
        // The UI read can complete in time while the waiter resumes too late.
        assert!(expired.accept(Ok(17)).is_err());
        valid.cancel();
        assert!(valid.accept(Ok(17)).is_err());
        let during = PublicForegroundRead {
            deadline: Instant::now() + Duration::from_secs(1),
            phase: AtomicU8::new(PUBLIC_READ_PENDING),
        };
        assert!(
            during
                .execute(|| {
                    during.cancel();
                    Ok(17)
                })
                .is_err()
        );
    }
    #[test]
    fn foreground_public_queued_native_scope_retains_original_operation() {
        fn send<T: Send>() {}
        send::<PublicForegroundApplication>();
        send::<operation::NativeCall>();
        let slots = std::sync::atomic::AtomicUsize::new(0);
        for _ in 0..PUBLIC_FOREGROUND_QUEUE_LIMIT {
            acquire_public_foreground_slot(&slots).unwrap();
        }
        assert_eq!(
            acquire_public_foreground_slot(&slots).unwrap_err().code,
            "foreground_workspace_queue_busy"
        );
        assert_eq!(slots.load(std::sync::atomic::Ordering::Acquire), 4);
        release_public_foreground_slot(&slots);
        acquire_public_foreground_slot(&slots).unwrap();
        assert_eq!(slots.load(std::sync::atomic::Ordering::Acquire), 4);
        let registry = operation::Registry::default();
        let op = registry.begin(None).unwrap();
        let queued = operation::run(&op, || {
            operation::native_call("offline_queued_read").unwrap()
        });
        op.finish();
        assert!(op.status().native_call_pending);
        assert!(!op.status().resources_released);
        drop(queued);
        assert!(op.status().terminal && op.status().resources_released);
    }
    #[test]
    fn foreground_public_cancel_before_start_cas_never_reads() {
        use std::sync::{
            Arc, Barrier,
            atomic::{AtomicUsize, Ordering},
        };
        let at_start = Arc::new(Barrier::new(2));
        let cancelled = Arc::new(Barrier::new(2));
        let reads = Arc::new(AtomicUsize::new(0));
        let job = Arc::new(PublicForegroundRead {
            deadline: Instant::now() + Duration::from_secs(5),
            phase: AtomicU8::new(PUBLIC_READ_PENDING),
        });
        let worker = job.clone();
        let first = at_start.clone();
        let second = cancelled.clone();
        let count = reads.clone();
        let result = std::thread::spawn(move || {
            worker.execute_after_gate(
                || {
                    first.wait();
                    second.wait();
                },
                || {
                    count.fetch_add(1, Ordering::AcqRel);
                    Ok(17)
                },
            )
        });
        at_start.wait();
        job.cancel(); // cancel has completed before the worker's start CAS.
        assert_eq!(job.phase.load(Ordering::Acquire), PUBLIC_READ_CANCELLED);
        cancelled.wait();
        assert_eq!(
            result.join().unwrap().unwrap_err().code,
            "foreground_workspace_read_expired"
        );
        assert_eq!(reads.load(Ordering::Acquire), 0);
        assert_eq!(job.phase.load(Ordering::Acquire), PUBLIC_READ_CANCELLED);
    }
}
