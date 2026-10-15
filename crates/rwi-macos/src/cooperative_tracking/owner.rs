//! Generic owner integration: only the application's UI thread creates/uses its own tag.
//! Socket workers cannot invoke AppKit. Applications MUST defer all close/remove/
//! reparent operations while BusyState is true, including reentrant callbacks.
use super::*;
use objc2::{
    msg_send,
    rc::{Retained, autoreleasepool},
    runtime::{AnyClass, AnyObject},
    sel,
};
use objc2_foundation::{NSPoint, NSRect, NSString};
use std::{
    cell::Cell,
    collections::BTreeMap,
    os::unix::{
        fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
        net::UnixListener,
    },
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};
static GENERATION: AtomicU64 = AtomicU64::new(1);
#[derive(Clone)]
pub struct BusyState(Rc<Cell<bool>>);
impl BusyState {
    pub fn is_busy(&self) -> bool {
        self.0.get()
    }
}
// Private dispatch expectations retain the original native frame/title/deadline.
// This data grouping confers no authority and is not deserializable.
struct ExpectedDispatch<'a> {
    frame: [f64; 4],
    title: &'a str,
    deadline_ns: &'a str,
}
struct Reservation(BusyState);
impl Drop for Reservation {
    fn drop(&mut self) {
        self.0.0.set(false);
    }
}
/// Created only by a real public addTrackingRect on this process's exact attached
/// visible view/window. No Deserialize, numeric-tag adoption or foreign pointer API.
pub struct OwnedRegistration {
    view: Retained<AnyObject>,
    window: Retained<AnyObject>,
    tag: isize,
    number: u32,
    id: String,
    generation: String,
    birth: String,
    busy: BusyState,
    registered: bool,
}
fn require_class(object: &AnyObject, name: &std::ffi::CStr) -> Result<()> {
    let expected = AnyClass::get(name).ok_or_else(channel_error)?;
    let mut actual = Some(object.class());
    while let Some(class) = actual {
        if std::ptr::eq(class, expected) {
            return Ok(());
        }
        actual = class.superclass();
    }
    Err(fault(
        "cooperative_owner_object_type",
        "Use this process's actual live NSView/NSWindow/NSApplication object, never arbitrary objects or decoded foreign pointers.",
    ))
}
fn require_selectors(object: &AnyObject, selectors: &[objc2::runtime::Sel]) -> Result<()> {
    if selectors
        .iter()
        .any(|s| object.class().instance_method(*s).is_none())
    {
        return Err(fault(
            "cooperative_owner_selector_unavailable",
            "Required public selector is unavailable; no ObjC exception/fallback.",
        ));
    }
    Ok(())
}
#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    static NSApp: *mut AnyObject;
}
fn require_application(app: &AnyObject) -> Result<()> {
    require_class(app, c"NSApplication")?;
    require_selectors(app, &[sel!(sendEvent:), sel!(modalWindow)])?;
    if !std::ptr::eq(unsafe { NSApp }.cast_const(), app) {
        return Err(channel_error());
    }
    Ok(())
}
fn main_thread() -> Result<()> {
    if unsafe { libc::pthread_main_np() } != 1 {
        return Err(fault(
            "cooperative_owner_main_thread_required",
            "Only the owner UI thread may register or dispatch.",
        ));
    }
    Ok(())
}
pub(super) fn factory_available() -> bool {
    AnyClass::get(c"NSEvent").is_some_and(|c|c.class_method(sel!(enterExitEventWithType:location:modifierFlags:timestamp:windowNumber:context:eventNumber:trackingNumber:userData:)).is_some())
}
impl OwnedRegistration {
    pub fn register(view: &AnyObject, window: &AnyObject) -> Result<Self> {
        main_thread()?;
        require_class(view, c"NSView")?;
        require_class(window, c"NSWindow")?;
        require_selectors(
            view,
            &[
                sel!(window),
                sel!(bounds),
                sel!(addTrackingRect:owner:userData:assumeInside:),
                sel!(removeTrackingRect:),
            ],
        )?;
        require_selectors(
            window,
            &[
                sel!(windowNumber),
                sel!(isVisible),
                sel!(title),
                sel!(attachedSheet),
                sel!(isMiniaturized),
            ],
        )?;
        let birth = crate::native::process_epoch(std::process::id())?;
        unsafe {
            let attached: *mut AnyObject = msg_send![view, window];
            let number: isize = msg_send![window, windowNumber];
            let visible: bool = msg_send![window, isVisible];
            if number <= 0
                || number > u32::MAX as isize
                || !visible
                || !std::ptr::eq(attached.cast_const(), window)
            {
                return Err(fault(
                    "cooperative_registration_owner_invalid",
                    "A displayed exact own view/window is required.",
                ));
            }
            let own = crate::native::exact_window_inventory(number as u32)?.windows;
            if own.len() != 1 || own[0].pid != std::process::id() {
                return Err(fault(
                    "cooperative_registration_process_invalid",
                    "Public window belongs to another native process.",
                ));
            }
            let generation = rwi_core::digest(
                format!(
                    "{}:{}:{}",
                    birth,
                    GENERATION.fetch_add(1, Ordering::Relaxed),
                    monotonic_ns()?
                )
                .as_bytes(),
            );
            let retained_view = Retained::retain(view as *const AnyObject as *mut AnyObject)
                .ok_or_else(channel_error)?;
            let retained_window = Retained::retain(window as *const AnyObject as *mut AnyObject)
                .ok_or_else(channel_error)?;
            let bounds: NSRect = msg_send![view, bounds];
            let tag: isize = msg_send![view,addTrackingRect:bounds,owner:view,userData:std::ptr::null_mut::<std::ffi::c_void>(),assumeInside:false];
            if tag == 0 {
                return Err(fault(
                    "cooperative_registration_failed",
                    "No nonzero public legacy tag returned; no fabricated tag.",
                ));
            }
            Ok(Self {
                view: retained_view,
                window: retained_window,
                tag,
                number: number as u32,
                id: format!("legacy-{generation}"),
                generation,
                birth,
                busy: BusyState(Rc::new(Cell::new(false))),
                registered: true,
            })
        }
    }
    pub fn registration_id(&self) -> &str {
        &self.id
    }
    pub fn public_tag_decimal(&self) -> Option<String> {
        self.registered.then(|| self.tag.to_string())
    }
    /// Call before close/hide/reparent. Hidden-but-attached is NOT invalidation.
    pub fn unregister(&mut self) -> Result<bool> {
        main_thread()?;
        if !self.registered {
            return Ok(false);
        }
        if self.busy.is_busy() {
            return Err(fault(
                "cooperative_registration_busy",
                "Defer removal/close until the synchronous owner reservation ends.",
            ));
        }
        let attached: *mut AnyObject = unsafe { msg_send![&*self.view, window] };
        if !std::ptr::eq(attached.cast_const(), &*self.window) {
            self.registered = false;
            return Err(channel_error());
        }
        unsafe {
            let _: () = msg_send![&*self.view,removeTrackingRect:self.tag];
        }
        self.registered = false;
        Ok(true)
    }
    pub fn busy_state(&self) -> BusyState {
        self.busy.clone()
    }
    fn describe(&self) -> Result<Reply> {
        main_thread()?;
        if !self.registered
            || self.busy.is_busy()
            || crate::native::process_epoch(std::process::id())? != self.birth
        {
            return Err(channel_error());
        }
        unsafe {
            let attached: *mut AnyObject = msg_send![&*self.view, window];
            let number: isize = msg_send![&*self.window, windowNumber];
            let visible: bool = msg_send![&*self.window, isVisible];
            if !visible
                || number != self.number as isize
                || !std::ptr::eq(attached.cast_const(), &*self.window)
            {
                return Err(channel_error());
            }
            let rows = crate::native::exact_window_inventory(self.number)?.windows;
            if rows.len() != 1 || rows[0].pid != std::process::id() || !rows[0].rect.valid() {
                return Err(channel_error());
            }
            let f = rows[0].rect;
            let title: Retained<NSString> = msg_send![&*self.window, title];
            Ok(Reply::Registration {
                registration_id: self.id.clone(),
                generation: self.generation.clone(),
                window: self.number,
                frame: [f.x, f.y, f.w, f.h],
                title: title.to_string(),
            })
        }
    }
    fn dispatch(
        &mut self,
        app: &AnyObject,
        phase: TrackingPhase,
        point: [f64; 2],
        expected: ExpectedDispatch<'_>,
        check: impl Fn() -> Result<()>,
    ) -> Result<()> {
        let ExpectedDispatch {
            frame: expected_frame,
            title: expected_title,
            deadline_ns,
        } = expected;
        require_application(app)?;
        let end = deadline(deadline_ns)?;
        let Reply::Registration { frame, title, .. } = self.describe()? else {
            return Err(channel_error());
        };
        if frame != expected_frame
            || title != expected_title
            || !point.into_iter().all(f64::is_finite)
            || point[0] < 0.0
            || point[1] < 0.0
            || point[0] >= frame[2]
            || point[1] >= frame[3]
        {
            return Err(channel_error());
        }
        if !factory_available() {
            return Err(channel_error());
        }
        // Reservation is live throughout synchronous AppKit dispatch and callbacks.
        self.busy.0.set(true);
        let _reservation = Reservation(self.busy.clone());
        autoreleasepool(|_| unsafe {
            let class = AnyClass::get(c"NSEvent").ok_or_else(channel_error)?;
            let info: *mut AnyObject = msg_send![
                AnyClass::get(c"NSProcessInfo").ok_or_else(channel_error)?,
                processInfo
            ];
            let time: f64 = msg_send![info, systemUptime];
            let raw: *mut AnyObject = msg_send![class,enterExitEventWithType:phase.native_kind() as usize,
                location:NSPoint::new(point[0],point[1]),modifierFlags:0usize,timestamp:time,windowNumber:self.number as isize,
                context:std::ptr::null_mut::<AnyObject>(),eventNumber:0isize,trackingNumber:self.tag,
                userData:std::ptr::null_mut::<std::ffi::c_void>()];
            let event = Retained::retain(raw).ok_or_else(channel_error)?;
            let kind: usize = msg_send![&*event, type];
            if kind != phase.native_kind() as usize || !matches!(kind, 8 | 9) {
                return Err(channel_error());
            }
            let tag: isize = msg_send![&*event, trackingNumber];
            let data: *mut std::ffi::c_void = msg_send![&*event, userData];
            let window: isize = msg_send![&*event, windowNumber];
            if tag != self.tag
                || !data.is_null()
                || window != self.number as isize
                || monotonic_ns()? >= end
            {
                return Err(channel_error());
            }
            let modal: *mut AnyObject = msg_send![app, modalWindow];
            let sheet: *mut AnyObject = msg_send![&*self.window, attachedSheet];
            let minimized: bool = msg_send![&*self.window, isMiniaturized];
            if !modal.is_null() || !sheet.is_null() || minimized {
                return Err(channel_error());
            }
            check()?;
            if monotonic_ns()? >= end {
                return Err(channel_error());
            }
            // Documented NSApplication entry, NEVER NSWindow.sendEvent or owner callbacks.
            // Caller must release all UI RefCell borrows but keep UI installed for callbacks.
            let _: () = msg_send![app,sendEvent:&*event];
            Ok(())
        })
    }
}
impl Drop for OwnedRegistration {
    fn drop(&mut self) {
        let _ = self.unregister();
    }
}
#[derive(Default, Clone, Serialize)]
pub struct ProviderStats {
    pub read_only_describes: u64,
    pub public_dispatches: u64,
    pub duplicates: u64,
    pub rejected: u64,
    pub last_outcome: Option<String>,
}
mod transport_state;
use transport_state::{CreatedPaths, History, Task, cleanup_owned};
/// Generic authenticated socket provider. Workers only read/write bytes; poll()
/// must run on the owner's UI thread outside all app UI RefCell borrows.
pub struct OwnerServer {
    endpoint: PathBuf,
    directory: PathBuf,
    directory_inode: u64,
    inode: u64,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    queue: mpsc::Receiver<Task>,
    history: History,
    stats: ProviderStats,
}
impl OwnerServer {
    pub fn start(directory: &Path, agent_pid: u32, agent_birth: &str) -> Result<Self> {
        main_thread()?;
        if agent_pid == 0 || agent_birth != crate::native::process_epoch(agent_pid)? {
            return Err(fault(
                "cooperative_agent_grant_invalid",
                "Owner must explicitly grant the exact live native Agent PID/process birth.",
            ));
        }
        let endpoint = directory.join("owner.sock");
        rwi_core::MovePointerOptions::validate_cooperative_selector(
            endpoint.to_str().ok_or_else(channel_error)?,
            "r",
        )?;
        if !directory.is_absolute() || directory.exists() {
            return Err(fault(
                "cooperative_owner_directory_exists",
                "Use a new short private directory; never unlink or repair another owner's path.",
            ));
        }
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(directory)
            .map_err(|_| channel_error())?;
        let directory_inode = std::fs::symlink_metadata(directory)
            .map_err(|_| channel_error())?
            .ino();
        let mut created = CreatedPaths {
            directory: directory.into(),
            directory_inode,
            endpoint: endpoint.clone(),
            socket_inode: None,
            armed: true,
        };
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| channel_error())?;
        let listener = UnixListener::bind(&endpoint).map_err(|_| channel_error())?;
        let inode = std::fs::symlink_metadata(&endpoint)
            .map_err(|_| channel_error())?
            .ino();
        created.socket_inode = Some(inode);
        std::fs::set_permissions(&endpoint, std::fs::Permissions::from_mode(0o600))
            .map_err(|_| channel_error())?;
        listener
            .set_nonblocking(true)
            .map_err(|_| channel_error())?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let (tx, queue) = mpsc::sync_channel(8);
        let birth = agent_birth.to_owned();
        let worker = thread::spawn(move || {
            while !stopping.load(Ordering::Acquire) {
                let Ok((mut stream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                };
                let end = Instant::now() + IO_BUDGET;
                let check = || {
                    if stopping.load(Ordering::Acquire) {
                        Err(channel_error())
                    } else {
                        Ok(())
                    }
                };
                let result = (|| -> Result<()> {
                    let identity = peer(&stream)?;
                    if identity.pid != agent_pid || identity.birth != birth {
                        return Err(channel_error());
                    }
                    let request: Request = receive(&mut stream, end, &check)?;
                    if peer(&stream)? != identity {
                        return Err(channel_error());
                    }
                    let (reply, rx) = mpsc::sync_channel(1);
                    tx.try_send(Task {
                        request,
                        identity,
                        stream: stream.try_clone().map_err(|_| channel_error())?,
                        reply,
                    })
                    .map_err(|_| channel_error())?;
                    loop {
                        check()?;
                        if Instant::now() >= end {
                            return Err(channel_error());
                        }
                        match rx.recv_timeout(Duration::from_millis(10)) {
                            Ok(reply) => {
                                send_terminal_reply(&mut stream, &reply, end, &check)?;
                                return Ok(());
                            }
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                            Err(_) => return Err(channel_error()),
                        }
                    }
                })();
                let _ = result; // Fail closed: no requeue, input resend or GUI worker fallback.
            }
        });
        created.armed = false;
        Ok(Self {
            endpoint,
            directory: directory.into(),
            directory_inode,
            inode,
            stop,
            worker: Some(worker),
            queue,
            history: History(BTreeMap::new()),
            stats: ProviderStats::default(),
        })
    }
    pub fn endpoint(&self) -> &Path {
        &self.endpoint
    }
    pub fn stats(&self) -> ProviderStats {
        self.stats.clone()
    }
    pub fn poll(&mut self, registration: &mut OwnedRegistration, app: &AnyObject) -> Result<()> {
        main_thread()?;
        require_application(app)?;
        // One request per tick, not an unbounded drain or hidden Native replay.
        let Ok(task) = self.queue.try_recv() else {
            return Ok(());
        };
        let response = (|| -> Result<Reply> {
            if self.stop.load(Ordering::Acquire)
                || !connected(&task.stream)
                || peer(&task.stream)? != task.identity
            {
                return Err(channel_error());
            }
            match &task.request {
                Request::Describe {
                    version: 1,
                    registration_id,
                } if *registration_id == registration.id => {
                    self.stats.read_only_describes += 1;
                    registration.describe()
                }
                Request::Status { version: 1, nonce } => Ok(self
                    .history
                    .0
                    .get(nonce)
                    .map(|(_, r)| r.clone())
                    .unwrap_or_else(|| Reply::Unknown {
                        nonce: nonce.clone(),
                    })),
                Request::Execute {
                    version: 1,
                    nonce,
                    registration_id,
                    generation,
                    window,
                    frame,
                    title,
                    phase,
                    window_point,
                    deadline_ns,
                } => {
                    if *registration_id != registration.id
                        || *generation != registration.generation
                        || *window != registration.number
                    {
                        return Ok(reject("unknown_or_stale_registration"));
                    }
                    deadline(deadline_ns)?;
                    if let Err(reply) = self.history.reserve(nonce, &task.request) {
                        self.stats.duplicates += 1;
                        return Ok(reply);
                    }
                    if peer(&task.stream)? != task.identity || !connected(&task.stream) {
                        return Err(channel_error());
                    }
                    registration.dispatch(
                        app,
                        *phase,
                        *window_point,
                        ExpectedDispatch {
                            frame: *frame,
                            title,
                            deadline_ns,
                        },
                        || {
                            if peer(&task.stream)? != task.identity || !connected(&task.stream) {
                                return Err(channel_error());
                            }
                            Ok(())
                        },
                    )?;
                    self.stats.public_dispatches += 1;
                    let reply = Reply::Returned {
                        nonce: nonce.clone(),
                        public_dispatch_count: 1,
                        goal_verified: false,
                    };
                    if let Some((_, value)) = self.history.0.get_mut(nonce) {
                        *value = reply.clone();
                    }
                    Ok(reply)
                }
                _ => Ok(reject("unknown_registration")),
            }
        })()
        .unwrap_or_else(|_| reject("execution_unconfirmed_no_retry"));
        if matches!(response, Reply::Rejected { .. }) {
            self.stats.rejected += 1;
        }
        self.stats.last_outcome = Some(
            match &response {
                Reply::Returned { .. } => "public_dispatch_returned",
                Reply::Registration { .. } => "registration_described",
                Reply::Unknown { .. } => "unknown_operation",
                Reply::Rejected { .. } => "rejected_or_uncertain",
            }
            .into(),
        );
        let _ = task.reply.try_send(response);
        Ok(())
    }
}
impl Drop for OwnerServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        cleanup_owned(
            &self.directory,
            self.directory_inode,
            &self.endpoint,
            Some(self.inode),
        );
    }
}
#[cfg(test)]
mod tests;
