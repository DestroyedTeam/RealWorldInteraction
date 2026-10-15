//! Agent-side native peer witness; this type is private and never Deserialize.
use super::*;
use std::{
    os::{
        fd::{FromRawFd, OwnedFd},
        unix::{
            ffi::OsStrExt,
            fs::{FileTypeExt, MetadataExt, PermissionsExt},
        },
    },
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};
static NONCE: AtomicU64 = AtomicU64::new(1);
// Expected facts from the Agent's fresh native binding, not a trusted witness.
// Internal parameter grouping only; never Deserialize or accepted from MCP JSON.
pub(crate) struct ExpectedNativeTarget<'a> {
    pub(crate) pid: u32,
    pub(crate) birth: &'a str,
    pub(crate) window: u32,
    pub(crate) frame: [f64; 4],
    pub(crate) title: &'a str,
    pub(crate) native_epoch: &'a str,
}
struct Witness {
    peer: Peer,
    generation: String,
    window: u32,
    frame: [f64; 4],
    title: String,
    native_epoch: String,
}
pub(crate) struct Client {
    endpoint: String,
    registration: String,
    witness: Witness,
}
fn validate_endpoint(path: &Path) -> Result<()> {
    let parent = path.parent().ok_or_else(channel_error)?;
    let dir = std::fs::symlink_metadata(parent).map_err(|_| channel_error())?;
    let socket = std::fs::symlink_metadata(path).map_err(|_| channel_error())?;
    let uid = unsafe { libc::geteuid() };
    if !dir.is_dir()
        || dir.uid() != uid
        || dir.permissions().mode() & 0o777 != 0o700
        || !socket.file_type().is_socket()
        || socket.uid() != uid
        || socket.permissions().mode() & 0o777 != 0o600
    {
        return Err(fault(
            "cooperative_endpoint_not_private",
            "Use only the current owner's short private directory/socket; no fallback or filesystem repair.",
        ));
    }
    Ok(())
}
fn connect(path: &str, end: Instant) -> Result<UnixStream> {
    rwi_core::operation::checkpoint("cooperative_connect")?;
    validate_endpoint(Path::new(path))?;
    let bytes = Path::new(path).as_os_str().as_bytes();
    if bytes.is_empty() || bytes.len() > 100 || bytes.contains(&0) {
        return Err(channel_error());
    }
    let raw = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
    if raw < 0 {
        return Err(channel_error());
    }
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    let old = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    if old < 0
        || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, old | libc::O_NONBLOCK) } < 0
        || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0
    {
        return Err(channel_error());
    }
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (slot, byte) in address.sun_path.iter_mut().zip(bytes) {
        *slot = *byte as libc::c_char;
    }
    address.sun_len = std::mem::size_of::<libc::sockaddr_un>() as u8;
    let rc = unsafe {
        libc::connect(
            fd.as_raw_fd(),
            (&address as *const libc::sockaddr_un).cast(),
            std::mem::size_of_val(&address) as libc::socklen_t,
        )
    };
    if rc != 0 {
        let code = std::io::Error::last_os_error().raw_os_error();
        if !matches!(code, Some(libc::EINPROGRESS) | Some(libc::EAGAIN)) {
            return Err(channel_error());
        }
        loop {
            rwi_core::operation::checkpoint("cooperative_connect_wait")?;
            let left = end
                .checked_duration_since(Instant::now())
                .ok_or_else(channel_error)?;
            let mut poll = libc::pollfd {
                fd: fd.as_raw_fd(),
                events: libc::POLLOUT,
                revents: 0,
            };
            let rc = unsafe { libc::poll(&mut poll, 1, left.as_millis().clamp(1, 20) as i32) };
            if rc < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
                return Err(channel_error());
            }
            if rc > 0 {
                let mut error = 0i32;
                let mut len = std::mem::size_of_val(&error) as libc::socklen_t;
                if unsafe {
                    libc::getsockopt(
                        fd.as_raw_fd(),
                        libc::SOL_SOCKET,
                        libc::SO_ERROR,
                        (&mut error as *mut i32).cast(),
                        &mut len,
                    )
                } != 0
                    || len as usize != std::mem::size_of_val(&error)
                    || error != 0
                {
                    return Err(channel_error());
                }
                break;
            }
        }
    }
    let stream = UnixStream::from(fd);
    stream.set_nonblocking(false).map_err(|_| channel_error())?;
    Ok(stream)
}
impl Client {
    pub(crate) fn prepare(
        endpoint: &str,
        registration: &str,
        expected: ExpectedNativeTarget<'_>,
    ) -> Result<Self> {
        let ExpectedNativeTarget {
            pid,
            birth,
            window,
            frame,
            title,
            native_epoch,
        } = expected;
        rwi_core::MovePointerOptions::validate_cooperative_selector(endpoint, registration)?;
        let end = Instant::now() + IO_BUDGET;
        let mut socket = connect(endpoint, end)?;
        let identity = peer(&socket)?;
        if identity.pid != pid || identity.birth != birth {
            return Err(fault(
                "cooperative_owner_mismatch",
                "Socket belongs to another native process/birth; no input.",
            ));
        }
        let check = || rwi_core::operation::checkpoint("cooperative_describe_readonly");
        send(
            &mut socket,
            &Request::Describe {
                version: 1,
                registration_id: registration.into(),
            },
            end,
            &check,
        )?;
        let reply: Reply = receive(&mut socket, end, &check)?;
        if peer(&socket)? != identity {
            return Err(channel_error());
        }
        let Reply::Registration {
            registration_id,
            generation,
            window: actual,
            frame: actual_frame,
            title: actual_title,
        } = reply
        else {
            return Err(fault(
                "cooperative_registration_unknown",
                "Owner has no matching current registration; no fallback or input.",
            ));
        };
        if registration_id != registration
            || actual != window
            || actual_frame != frame
            || actual_title != title
            || !rwi_core::device::valid_id(&generation)
            || native_epoch.len() != 64
        {
            return Err(fault(
                "cooperative_registration_stale",
                "Native target/context and current owner registration differ; no input.",
            ));
        }
        Ok(Self {
            endpoint: endpoint.into(),
            registration: registration.into(),
            witness: Witness {
                peer: identity,
                generation,
                window,
                frame,
                title: actual_title,
                native_epoch: native_epoch.into(),
            },
        })
    }
    pub(crate) fn matches_context(
        &self,
        window: u32,
        frame: [f64; 4],
        title: &str,
        native_epoch: &str,
    ) -> Result<()> {
        if self.witness.window != window
            || self.witness.frame != frame
            || self.witness.title != title
            || self.witness.native_epoch != native_epoch
        {
            return Err(fault(
                "cooperative_context_changed",
                "Original native witness changed; no owner dispatch.",
            ));
        }
        Ok(())
    }
    pub(crate) fn execute(
        self,
        phase: TrackingPhase,
        window_point: [f64; 2],
        before_write: impl FnOnce() -> Result<rwi_core::operation::NativeCall>,
    ) -> Result<()> {
        let mut mutation_started = false;
        let result = (|| {
            let budget = rwi_core::operation::dispatch_budget()?.min(IO_BUDGET);
            let end = Instant::now() + budget;
            // A new mutation connection is never a reconnect/retry of sent input.
            let mut socket = connect(&self.endpoint, end)?;
            if peer(&socket)? != self.witness.peer {
                return Err(channel_error());
            }
            let clock = monotonic_ns()?;
            let remaining = rwi_core::operation::dispatch_budget()?
                .min(end.saturating_duration_since(Instant::now()));
            let deadline = clock
                .checked_add(u64::try_from(remaining.as_nanos()).map_err(|_| channel_error())?)
                .ok_or_else(channel_error)?;
            let nonce = rwi_core::digest(
                format!(
                    "{}:{}:{}:{}",
                    std::process::id(),
                    crate::native::process_epoch(std::process::id())?,
                    monotonic_ns()?,
                    NONCE.fetch_add(1, Ordering::Relaxed)
                )
                .as_bytes(),
            );
            let request = Request::Execute {
                version: 1,
                nonce: nonce.clone(),
                registration_id: self.registration,
                generation: self.witness.generation,
                window: self.witness.window,
                frame: self.witness.frame,
                title: self.witness.title,
                phase,
                window_point,
                deadline_ns: deadline.to_string(),
            };
            let check = || rwi_core::operation::checkpoint("cooperative_owner_execute_once");
            // Validate encoding before guard/count; no mutation byte exists yet.
            bytes(&request)?;
            // New connect/peer/nonce work is complete. Native binding/TCC/session
            // and original lease are freshly rechecked at this exact boundary.
            let _native = before_write()?;
            mutation_started = true;
            // Durable core reservation already exists. One frame only, never resend.
            send(&mut socket, &request, end, &check)?;
            let reply: Reply = receive(&mut socket, end, &check)?;
            if peer(&socket)? != self.witness.peer {
                return Err(channel_error());
            }
            match reply {
                Reply::Returned {
                    nonce: actual,
                    public_dispatch_count: 1,
                    goal_verified: false,
                } if actual == nonce => Ok(()),
                _ => Err(channel_error()),
            }
        })();
        if mutation_started && result.is_err() {
            rwi_core::operation::resource_release_uncertain();
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn f05_cooperative_tracking_client_unknown_endpoint_never_repairs_paths() {
        assert!(
            validate_endpoint(Path::new("/tmp/rwi-absent-cooperative-owner/source.sock")).is_err()
        );
        assert!(
            rwi_core::MovePointerOptions::validate_cooperative_selector("/tmp/../bad.sock", "r")
                .is_err()
        );
    }
    #[test]
    fn f05_cooperative_tracking_delayed_connect_changed_native_guard_writes_zero_bytes() {
        use std::{
            os::unix::net::UnixListener,
            sync::{Arc, atomic::AtomicBool, mpsc},
            thread,
        };
        let directory = tempfile::Builder::new()
            .prefix("rwi-coop-guard-")
            .tempdir_in("/tmp")
            .unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let endpoint = directory.path().join("owner.sock");
        let listener = UnixListener::bind(&endpoint).unwrap();
        std::fs::set_permissions(&endpoint, std::fs::Permissions::from_mode(0o600)).unwrap();
        let changed = Arc::new(AtomicBool::new(false));
        let server_changed = changed.clone();
        let (accepted, ready) = mpsc::sync_channel(1);
        let server = thread::spawn(move || {
            let (mut describe, _) = listener.accept().unwrap();
            let end = Instant::now() + Duration::from_secs(2);
            let _: Request = receive(&mut describe, end, &|| Ok(())).unwrap();
            send_terminal_reply(
                &mut describe,
                &Reply::Registration {
                    registration_id: "owned".into(),
                    generation: "generation".into(),
                    window: 1,
                    frame: [0., 0., 10., 10.],
                    title: "own".into(),
                },
                end,
                &|| Ok(()),
            )
            .unwrap();
            // Production terminal helper has observed client EOF only after the
            // client completed its unchanged post-reply kernel peer check.
            drop(describe);
            let (mut mutation, _) = listener.accept().unwrap();
            // Complete fixture setup before releasing the client's guard: it
            // can reject and close its peer immediately after this signal.
            mutation
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            thread::sleep(Duration::from_millis(10)); // context changed while connection work was in flight.
            server_changed.store(true, Ordering::Release);
            accepted.send(()).unwrap();
            let mut first = [0u8; 4];
            mutation.read(&mut first).unwrap()
        });
        let called = AtomicBool::new(false);
        let birth = crate::native::process_epoch(std::process::id()).unwrap();
        let client = Client::prepare(
            endpoint.to_str().unwrap(),
            "owned",
            ExpectedNativeTarget {
                pid: std::process::id(),
                birth: &birth,
                window: 1,
                frame: [0., 0., 10., 10.],
                title: "own",
                native_epoch: &"a".repeat(64),
            },
        )
        .unwrap();
        rwi_core::operation::with_dispatch_deadline(
            Instant::now() + Duration::from_secs(2),
            || {
                let result = client.execute(TrackingPhase::Entered, [1., 2.], || {
                    ready.recv_timeout(Duration::from_secs(1)).unwrap();
                    called.store(true, Ordering::Release);
                    assert!(changed.load(Ordering::Acquire));
                    Err(fault(
                        "test_native_binding_TCC_context_changed",
                        "Fresh guard rejects before native count/first mutation byte.",
                    ))
                });
                assert_eq!(
                    result.unwrap_err().code,
                    "test_native_binding_TCC_context_changed"
                );
            },
        );
        assert!(called.load(Ordering::Acquire));
        assert_eq!(server.join().unwrap(), 0);
        assert!(!rwi_core::operation::resource_release_is_uncertain());
    }
}
