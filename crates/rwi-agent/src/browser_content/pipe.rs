//! macOS-only owned Chrome, JSON/NUL debugging PIPE; no listening debugging port.
//! Transport errors never resend a command. This is not a native hover implementation.
mod cleanup;
#[cfg(test)]
mod close_tests;
use super::{ChildHandle, StartFailure, policy::Policy};
use cleanup::{ExitObservation, SignalState, graceful_exit_budget};
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
            net::UnixStream,
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
const CAP: usize = 1024 * 1024;
const DEADLINE: Duration = Duration::from_secs(8);
fn invalid(s: impl Into<String>) -> io::Error {
    let mut s = s.into();
    if s.len() > 1024 {
        let mut n = 1024;
        while !s.is_char_boundary(n) {
            n -= 1;
        }
        s.truncate(n);
        s.push_str(" [truncated]");
    }
    io::Error::other(s)
}
struct Framer {
    buffered: Vec<u8>,
}
impl Framer {
    fn new() -> Self {
        Self {
            buffered: Vec::new(),
        }
    }
    fn push(&mut self, bytes: &[u8]) -> io::Result<()> {
        if self.buffered.len() + bytes.len() > CAP + 8192 {
            return Err(invalid("browser_pipe_buffer_limit"));
        }
        self.buffered.extend_from_slice(bytes);
        if self
            .buffered
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(self.buffered.len())
            > CAP
        {
            return Err(invalid("browser_pipe_frame_limit"));
        }
        Ok(())
    }
    fn next(&mut self) -> io::Result<Option<Value>> {
        let Some(end) = self.buffered.iter().position(|b| *b == 0) else {
            return Ok(None);
        };
        if end == 0 || end > CAP {
            return Err(invalid("browser_pipe_frame_invalid"));
        }
        let value = serde_json::from_slice(&self.buffered[..end])?;
        self.buffered.drain(..=end);
        Ok(Some(value))
    }
}
struct Pipe {
    tx: UnixStream,
    rx: UnixStream,
    framer: Framer,
    id: u64,
    poisoned: bool,
}
impl Pipe {
    fn rpc(&mut self, method: &str, params: Value, until: Instant) -> io::Result<Value> {
        if self.poisoned {
            return Err(invalid("browser_pipe_poisoned_no_replay"));
        }
        let id = self
            .id
            .checked_add(1)
            .ok_or_else(|| invalid("browser_pipe_id_exhausted"))?;
        self.id = id;
        let mut raw = serde_json::to_vec(&json!({"id":id,"method":method,"params":params}))?;
        raw.push(0);
        if raw.len() > CAP {
            return Err(invalid("browser_pipe_request_limit"));
        }
        // Poison before first attempted write, including partial write/EOF/lost ACK.
        // Only a correctly bound complete reply clears it. No command is resent.
        self.poisoned = true;
        let mut written = 0;
        while written < raw.len() {
            if Instant::now() >= until {
                return Err(invalid(format!(
                    "browser_pipe_write_timeout_effect_unknown:written={written}"
                )));
            }
            match self.tx.write(&raw[written..]) {
                Ok(0) => return Err(invalid("browser_pipe_write_zero_effect_unknown")),
                Ok(n) => written += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(e) => return Err(invalid(format!("browser_pipe_write_effect_unknown:{e}"))),
            }
        }
        let mut events = 0;
        loop {
            if Instant::now() >= until {
                return Err(invalid("browser_pipe_reply_timeout_effect_unknown"));
            }
            if let Some(v) = self.framer.next()? {
                if v.get("id").is_some() {
                    if v["id"].as_u64() != Some(id) {
                        return Err(invalid("browser_pipe_reply_identity_mismatch"));
                    }
                    if let Some(e) = v.get("error") {
                        return Err(invalid(format!("browser_protocol_refused:{e}")));
                    }
                    let Some(result) = v.get("result") else {
                        return Err(invalid("browser_pipe_result_missing"));
                    };
                    self.poisoned = false;
                    return Ok(result.clone());
                }
                if !v.get("method").is_some_and(Value::is_string) || events >= 64 {
                    return Err(invalid("browser_pipe_event_limit_or_shape"));
                }
                events += 1;
                continue;
            }
            let mut b = [0u8; 8192];
            match self.rx.read(&mut b) {
                Ok(0) => return Err(invalid("browser_pipe_eof_effect_unknown")),
                Ok(n) => self.framer.push(&b[..n])?,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(e) => return Err(invalid(format!("browser_pipe_read_effect_unknown:{e}"))),
            }
        }
    }
}
fn high_fd(s: &UnixStream) -> io::Result<OwnedFd> {
    // Sources >=10 cannot be overwritten by child dup2 to Chromium's fd3/fd4.
    let fd = unsafe { libc::fcntl(s.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 10) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}
fn sticky_parent_protects_child(
    mode: u32,
    directory: bool,
    child: Option<(u32, u32)>,
    uid: u32,
) -> bool {
    // Sticky parents protect an owned, non-writable child against foreign rename/unlink.
    // Ordinary writable ancestors, writable files and unknown child owners stay refused.
    directory
        && mode & 0o1000 != 0
        && child.is_some_and(|(owner, child_mode)| {
            (owner == 0 || owner == uid) && child_mode & 0o022 == 0
        })
}
fn executable(policy: &Policy) -> io::Result<File> {
    let path = &policy.chrome_executable;
    if path.canonicalize()? != *path {
        return Err(invalid("browser_executable_noncanonical"));
    }
    let mut child = None;
    for p in path.ancestors() {
        let m = std::fs::symlink_metadata(p)?;
        let writable_unprotected = m.mode() & 0o022 != 0
            && !sticky_parent_protects_child(m.mode(), m.is_dir(), child, policy.user_uid);
        if m.file_type().is_symlink()
            || writable_unprotected
            || !(m.uid() == 0 || m.uid() == policy.user_uid)
        {
            return Err(invalid("browser_executable_path_untrusted"));
        }
        child = Some((m.uid(), m.mode()));
    }
    let mut f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let m = f.metadata()?;
    if !m.is_file() || m.mode() & 0o111 == 0 || m.len() > 64 * 1024 * 1024 {
        return Err(invalid("browser_executable_invalid"));
    }
    let mut raw = Vec::new();
    std::io::Read::by_ref(&mut f)
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut raw)?;
    if raw.len() > 64 * 1024 * 1024 || rwi_core::digest(&raw) != policy.chrome_sha256 {
        return Err(invalid("browser_executable_hash_mismatch"));
    }
    Ok(f)
}
fn birth(pid: u32, uid: u32) -> io::Result<String> {
    // Same public libproc structure as rwi-macos native::process_epoch.
    let mut v = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let n = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            v.as_mut_ptr().cast(),
            std::mem::size_of::<libc::proc_bsdinfo>() as i32,
        )
    };
    if n != std::mem::size_of::<libc::proc_bsdinfo>() as i32 {
        return Err(invalid("browser_child_birth_unknown"));
    }
    let v = unsafe { v.assume_init() };
    if v.pbi_uid != uid {
        return Err(invalid("browser_child_user_mismatch"));
    }
    Ok(format!(
        "{pid}:{}:{}",
        v.pbi_start_tvsec, v.pbi_start_tvusec
    ))
}
pub(super) struct OwnedChrome {
    child: Option<Child>,
    pipe: Pipe,
    profile: PathBuf,
    pid: u32,
    birth: String,
    image_hash: String,
    context: Option<String>,
    target: Option<String>,
    version: Value,
    close_started: bool,
    signal: SignalState,
    user_uid: u32,
    exit: Option<Value>,
}
impl OwnedChrome {
    pub fn start(p: &Policy, profile: &Path, url: &str) -> Result<Self, StartFailure> {
        if unsafe { libc::geteuid() } != p.user_uid || unsafe { libc::getuid() } != p.user_uid {
            return Err(invalid("browser_child_user_mismatch").into());
        }
        // Pin checked immediately before creating the fresh profile and spawn.
        // Trusted path is required; this does not claim an atomic exec-by-hash API.
        let _image = executable(p)?;
        std::fs::DirBuilder::new().mode(0o700).create(profile)?;
        let _private = rwi_link_trust::PrivateDirectory::open(profile)?;
        let (tx, child_read) = UnixStream::pair()?;
        let (child_write, rx) = UnixStream::pair()?;
        tx.set_nonblocking(true)?;
        rx.set_nonblocking(true)?;
        let in_fd = high_fd(&child_read)?;
        let out_fd = high_fd(&child_write)?;
        let mut command = Command::new(&p.chrome_executable);
        command
            .args([
                "--remote-debugging-pipe",
                "--no-first-run",
                "--no-default-browser-check",
                "--no-startup-window",
                "--window-size=960,720",
            ])
            .arg(format!("--user-data-dir={}", profile.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // Only async-signal-safe fd operations after fork; no environment discovery,
        // shell, platform permission change, or user profile fallback.
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(in_fd.as_raw_fd(), 3) < 0 || libc::dup2(out_fd.as_raw_fd(), 4) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn()?;
        drop(command);
        drop(child_read);
        drop(child_write);
        let pid = child.id();
        let mut owned = Self {
            child: Some(child),
            pipe: Pipe {
                tx,
                rx,
                framer: Framer::new(),
                id: 0,
                poisoned: false,
            },
            profile: profile.into(),
            pid,
            birth: String::new(),
            image_hash: p.chrome_sha256.clone(),
            context: None,
            target: None,
            version: Value::Null,
            close_started: false,
            signal: SignalState::default(),
            user_uid: p.user_uid,
            exit: None,
        };
        let result = (|| {
            owned.birth = birth(pid, p.user_uid)?;
            let until = Instant::now() + DEADLINE;
            owned.version = owned.pipe.rpc("Browser.getVersion", json!({}), until)?;
            for field in ["product", "protocolVersion"] {
                if !owned.version[field]
                    .as_str()
                    .is_some_and(|s| !s.is_empty() && s.len() <= 256)
                {
                    return Err(invalid("browser_version_identity_invalid"));
                }
            }
            let context = owned.pipe.rpc(
                "Target.createBrowserContext",
                json!({"disposeOnDetach":true}),
                until,
            )?["browserContextId"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() < 512)
                .ok_or_else(|| invalid("browser_context_identity_missing"))?
                .to_owned();
            owned.context = Some(context.clone());
            let target = owned.pipe.rpc(
                "Target.createTarget",
                json!({"url":url,"browserContextId":context,"background":true}),
                until,
            )?["targetId"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() < 512)
                .ok_or_else(|| invalid("browser_target_identity_missing"))?
                .to_owned();
            owned.target = Some(target.clone());
            let info = owned
                .pipe
                .rpc("Target.getTargetInfo", json!({"targetId":target}), until)?;
            if info["targetInfo"]["targetId"] != target
                || info["targetInfo"]["browserContextId"] != context
                || info["targetInfo"]["type"] != "page"
            {
                return Err(invalid("browser_target_binding_mismatch"));
            }
            Ok(())
        })();
        if let Err(e) = result {
            let cleanup = owned.close();
            return Err(StartFailure {
                detail: format!("{e};owned_startup_cleanup={cleanup}"),
                retained: if cleanup["process_exit_observed"] == true {
                    None
                } else {
                    Some(Box::new(owned))
                },
            });
        }
        Ok(owned)
    }
    fn observe_exit(&mut self, until: Instant) -> ExitObservation {
        use std::os::unix::process::ExitStatusExt;
        if let Some(v) = &self.exit {
            return ExitObservation::Exited(v.clone());
        }
        while let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let value = json!({"process_exit_observed":true,"code":status.code(),"signal":status.signal()});
                    self.exit = Some(value.clone());
                    self.child.take();
                    return ExitObservation::Exited(value);
                }
                Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(5)),
                Ok(None) => return ExitObservation::Running,
                Err(error) => {
                    self.signal.authority_lost = true;
                    return ExitObservation::WaitUnknown(error.to_string());
                }
            }
        }
        self.signal.authority_lost = true;
        ExitObservation::WaitUnknown("owned_child_handle_missing_without_exit_status".into())
    }
}
impl ChildHandle for OwnedChrome {
    fn facts(&self) -> Value {
        json!({"pid":self.pid,"process_start":self.birth,"process_user_uid":self.user_uid,"executable_sha256":self.image_hash,"profile_kind":"new_exclusive_private","transport":"debugging_pipe","product":self.version["product"],"protocol_version":self.version["protocolVersion"],"stderr":"not_captured","profile_retained_after_close":true,"initial_window_size_requested":[960,720]})
    }
    fn close(&mut self) -> Value {
        let started = Instant::now();
        let first_close = !self.close_started;
        let mut protocol = json!({"attempted_this_call":first_close});
        if first_close {
            self.close_started = true;
            // Close only this owned context and this exclusively spawned browser.
            // An ambiguous pipe write poisons the connection: no second CDP command.
            let until = Instant::now() + Duration::from_millis(600);
            if let Some(context) = self.context.take() {
                let disposed = self.pipe.rpc(
                    "Target.disposeBrowserContext",
                    json!({"browserContextId":context}),
                    until,
                );
                protocol["dispose_ack"] = json!(disposed.is_ok());
                if let Err(error) = disposed {
                    protocol["dispose_error"] = json!(error.to_string());
                }
            }
            protocol["browser_close_attempted"] = json!(!self.pipe.poisoned);
            if !self.pipe.poisoned {
                let closed = self.pipe.rpc(
                    "Browser.close",
                    json!({}),
                    Instant::now() + Duration::from_millis(600),
                );
                protocol["browser_close_ack"] = json!(closed.is_ok());
                if let Err(error) = closed {
                    protocol["browser_close_error"] = json!(error.to_string());
                }
            }
        }
        let observed = self.observe_exit(Instant::now() + graceful_exit_budget(first_close));
        let previously_attempted = self.signal.attempted;
        let pid = self.pid;
        let uid = self.user_uid;
        let expected_birth = self.birth.clone();
        let signal_error = {
            let mut child = self.child.as_mut();
            let child_pid = child.as_ref().map(|c| c.id());
            self.signal.try_once(
                &observed,
                || {
                    // Adjacent public libproc birth/UID read, not an atomic PID handle.
                    // No await, sleep or alternate lookup between guard and signal.
                    if child_pid != Some(pid)
                        || expected_birth.is_empty()
                        || birth(pid, uid)? != expected_birth
                    {
                        return Err(invalid("browser_signal_identity_unknown_or_changed"));
                    }
                    Ok(())
                },
                || {
                    child
                        .as_mut()
                        .ok_or_else(|| invalid("owned_child_missing"))?
                        .kill()
                },
            )
        };
        let mut exit = observed.value();
        if self.signal.attempted && !previously_attempted {
            exit = self
                .observe_exit(Instant::now() + Duration::from_millis(500))
                .value();
        }
        if let Some(error) = signal_error {
            exit["kill_error"] = json!(error);
        }
        exit["forced_own_child_kill_attempted"] = json!(self.signal.attempted);
        exit["forced_own_child_kill_sent"] = json!(self.signal.sent);
        exit["signal_authority_lost"] = json!(self.signal.authority_lost);
        exit["close_elapsed_ms"] = json!(started.elapsed().as_millis());
        exit["graceful_wait_budget_ms"] = json!(graceful_exit_budget(first_close).as_millis());
        exit["protocol_close"] = protocol;
        exit["profile_deleted"] = json!(false);
        exit["references_invalidated"] = json!(true);
        exit
    }
}
impl Drop for OwnedChrome {
    fn drop(&mut self) {
        let outcome = self.close();
        if let Some(mut child) = self.child.take() {
            // Transfer the unreaped own Child to one reaper, rather than pretending
            // an absent ACK is exit0 or signaling an arbitrary/recycled PID later.
            eprintln!(
                "rwi-agent: owned browser cleanup unconfirmed: {outcome}; profile={}",
                self.profile.display()
            );
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn f14_browser_sticky_parent_requires_private_trusted_child_and_directory() {
        assert!(sticky_parent_protects_child(
            0o1777,
            true,
            Some((501, 0o700)),
            501
        ));
        assert!(sticky_parent_protects_child(
            0o1777,
            true,
            Some((0, 0o755)),
            501
        ));
        for (mode, directory, child) in [
            (0o777, true, Some((501, 0o700))),
            (0o1777, false, Some((501, 0o700))),
            (0o1777, true, None),
            (0o1777, true, Some((502, 0o700))),
            (0o1777, true, Some((501, 0o777))),
            (0o1777, true, Some((501, 0o775))),
        ] {
            assert!(!sticky_parent_protects_child(mode, directory, child, 501));
        }
    }
    #[test]
    fn f14_browser_pipe_split_coalesced_and_frame_cap() {
        let mut f = Framer::new();
        f.push(b"{\"id\":1").unwrap();
        assert!(f.next().unwrap().is_none());
        f.push(b",\"result\":{}}\0{\"method\":\"Target.changed\"}\0")
            .unwrap();
        assert_eq!(f.next().unwrap().unwrap()["id"], 1);
        assert_eq!(f.next().unwrap().unwrap()["method"], "Target.changed");
        assert!(f.next().unwrap().is_none());
        assert!(Framer::new().push(&vec![b'x'; CAP + 1]).is_err());
    }
    #[test]
    fn f14_browser_pipe_eof_poison_means_no_second_send() {
        let (a, b) = UnixStream::pair().unwrap();
        let (c, d) = UnixStream::pair().unwrap();
        a.set_nonblocking(true).unwrap();
        d.set_nonblocking(true).unwrap();
        drop(c);
        let mut p = Pipe {
            tx: a,
            rx: d,
            framer: Framer::new(),
            id: 0,
            poisoned: false,
        };
        assert!(
            p.rpc(
                "Target.createTarget",
                json!({}),
                Instant::now() + Duration::from_millis(30)
            )
            .is_err()
        );
        assert!(p.poisoned);
        let id = p.id;
        assert!(
            p.rpc(
                "Target.createTarget",
                json!({}),
                Instant::now() + Duration::from_millis(30)
            )
            .is_err()
        );
        assert_eq!(p.id, id);
        drop(b);
    }
}
