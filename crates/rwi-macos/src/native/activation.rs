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

mod object_timeout;
use object_timeout::checked_foreground_object_timeout;

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

mod readback;
pub(crate) use readback::foreground_state;

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
        let request_counts = std::cell::Cell::new([0_u8; 2]);
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
            let mut counts = request_counts.get();
            counts[usize::from(!action)] += 1;
            request_counts.set(counts);
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
        let mut settlement = readback::settlement::Settlement::default();
        let result = (|| {
            dispatch(&window, true)?;
            let after_raise =
                readback::post_dispatch_foreground_state("after_raise", &mut settlement, &check)?;
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
                let now = readback::post_dispatch_foreground_state(
                    "post_request_observation",
                    &mut settlement,
                    &check,
                )?;
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
        })();
        settlement.annotate(result, request_counts.get())
    }
}

#[cfg(test)]
mod object_timeout_tests;

#[cfg(test)]
mod public_foreground_tests;
