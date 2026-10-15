//! Authenticated cooperating-owner IPC, not a business fixture/oracle or foreign tag injector.
//! No caller can deserialize a trusted native witness. The raw-tag lane stays off.
use rwi_core::{Result, TrackingPhase, fault};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    os::{fd::AsRawFd, unix::net::UnixStream},
    time::{Duration, Instant},
};
pub(crate) mod client;
mod owner;
pub use owner::{BusyState, OwnedRegistration, OwnerServer, ProviderStats};
const LIMIT: usize = 4096;
const IO_BUDGET: Duration = Duration::from_millis(750);
const HISTORY_LIMIT: usize = 128;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Describe {
        version: u8,
        registration_id: String,
    },
    Execute {
        version: u8,
        nonce: String,
        registration_id: String,
        generation: String,
        window: u32,
        frame: [f64; 4],
        title: String,
        phase: TrackingPhase,
        window_point: [f64; 2],
        deadline_ns: String,
    },
    Status {
        version: u8,
        nonce: String,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum Reply {
    Registration {
        registration_id: String,
        generation: String,
        window: u32,
        frame: [f64; 4],
        title: String,
    },
    Returned {
        nonce: String,
        public_dispatch_count: u32,
        goal_verified: bool,
    },
    Rejected {
        code: String,
    },
    Unknown {
        nonce: String,
    },
}
fn channel_error() -> rwi_core::Fault {
    fault(
        "cooperative_owner_channel_unconfirmed",
        "Inspect the original operation; never reconnect/replay a mutation after partial write, timeout or lost reply.",
    )
}
fn reject(code: &str) -> Reply {
    Reply::Rejected { code: code.into() }
}
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    if bytes.is_empty() || bytes.len() > LIMIT {
        return Err(fault(
            "cooperative_frame_limit",
            "Bound owner protocol frame to 4096 bytes.",
        ));
    }
    serde_json::from_slice(bytes).map_err(|_| {
        fault(
            "cooperative_frame_invalid",
            "Strict versioned owner frame required; no authority inferred from JSON.",
        )
    })
}
fn bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    let payload = serde_json::to_vec(value).map_err(|_| channel_error())?;
    if payload.is_empty() || payload.len() > LIMIT {
        return Err(channel_error());
    }
    let mut out = (payload.len() as u32).to_be_bytes().to_vec();
    out.extend(payload);
    Ok(out)
}
fn read_n(
    stream: &mut UnixStream,
    n: usize,
    end: Instant,
    check: &impl Fn() -> Result<()>,
) -> Result<Vec<u8>> {
    if n > LIMIT {
        return Err(channel_error());
    }
    let mut out = vec![0; n];
    let mut at = 0;
    while at < n {
        check()?;
        let left = end
            .checked_duration_since(Instant::now())
            .ok_or_else(channel_error)?;
        stream
            .set_read_timeout(Some(left.min(Duration::from_millis(20))))
            .map_err(|_| channel_error())?;
        match stream.read(&mut out[at..]) {
            Ok(0) => return Err(channel_error()),
            Ok(k) => at += k,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err(channel_error()),
        }
    }
    Ok(out)
}
fn receive<T: serde::de::DeserializeOwned>(
    stream: &mut UnixStream,
    end: Instant,
    check: &impl Fn() -> Result<()>,
) -> Result<T> {
    let h = read_n(stream, 4, end, check)?;
    let n = u32::from_be_bytes(h.try_into().map_err(|_| channel_error())?) as usize;
    if n == 0 || n > LIMIT {
        return Err(channel_error());
    }
    decode(&read_n(stream, n, end, check)?)
}
fn send(
    stream: &mut UnixStream,
    value: &impl Serialize,
    end: Instant,
    check: &impl Fn() -> Result<()>,
) -> Result<()> {
    let data = bytes(value)?;
    let mut at = 0;
    while at < data.len() {
        check()?;
        let left = end
            .checked_duration_since(Instant::now())
            .ok_or_else(channel_error)?;
        stream
            .set_write_timeout(Some(left.min(Duration::from_millis(20))))
            .map_err(|_| channel_error())?;
        match stream.write(&data[at..]) {
            Ok(0) => return Err(channel_error()),
            Ok(n) => at += n,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err(channel_error()),
        }
    }
    Ok(())
}
// Keep the authenticated peer alive through the client's post-reply identity
// check. One request/reply only: EOF finishes, any extra byte rejects, and the
// original total deadline/cancellation is never renewed for this terminal phase.
fn send_terminal_reply(
    stream: &mut UnixStream,
    value: &Reply,
    end: Instant,
    check: &impl Fn() -> Result<()>,
) -> Result<()> {
    send(stream, value, end, check)?;
    let mut extra = [0u8; 1];
    loop {
        check()?;
        let left = end
            .checked_duration_since(Instant::now())
            .ok_or_else(channel_error)?;
        stream
            .set_read_timeout(Some(left.min(Duration::from_millis(20))))
            .map_err(|_| channel_error())?;
        match stream.read(&mut extra) {
            Ok(0) => return Ok(()),
            Ok(_) => return Err(channel_error()), // No next frame or mutation resend.
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err(channel_error()),
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct AuditToken {
    val: [u32; 8],
}
#[link(name = "bsm")]
unsafe extern "C" {
    fn audit_token_to_pid(token: AuditToken) -> libc::pid_t;
    fn audit_token_to_euid(token: AuditToken) -> libc::uid_t;
    fn audit_token_to_egid(token: AuditToken) -> libc::gid_t;
    fn audit_token_to_pidversion(token: AuditToken) -> libc::c_int;
}
#[derive(Clone, PartialEq, Eq)]
struct Peer {
    pid: u32,
    uid: u32,
    gid: u32,
    birth: String,
    version: i32,
    token: AuditToken,
}
fn peer(stream: &UnixStream) -> Result<Peer> {
    let fd = stream.as_raw_fd();
    let mut pid: libc::pid_t = -1;
    let mut n = std::mem::size_of_val(&pid) as libc::socklen_t;
    let mut token = AuditToken { val: [u32::MAX; 8] };
    let mut t = std::mem::size_of_val(&token) as libc::socklen_t;
    let mut uid = libc::uid_t::MAX;
    let mut gid = libc::gid_t::MAX;
    unsafe {
        if libc::getsockopt(
            fd,
            libc::SOL_LOCAL,
            libc::LOCAL_PEERPID,
            (&mut pid as *mut libc::pid_t).cast(),
            &mut n,
        ) != 0
            || n as usize != std::mem::size_of_val(&pid)
            || pid <= 0
            || libc::getsockopt(
                fd,
                libc::SOL_LOCAL,
                libc::LOCAL_PEERTOKEN,
                (&mut token as *mut AuditToken).cast(),
                &mut t,
            ) != 0
            || t as usize != std::mem::size_of_val(&token)
            || libc::getpeereid(fd, &mut uid, &mut gid) != 0
            || audit_token_to_pid(token) != pid
            || audit_token_to_euid(token) != uid
            || audit_token_to_egid(token) != gid
            || uid != libc::geteuid()
        {
            return Err(fault(
                "cooperative_peer_identity_unknown",
                "Kernel peer PID/token/credentials mismatch; no owner authority.",
            ));
        }
        Ok(Peer {
            pid: pid as u32,
            uid,
            gid,
            birth: crate::native::process_epoch(pid as u32)?,
            version: audit_token_to_pidversion(token),
            token,
        })
    }
}
fn connected(stream: &UnixStream) -> bool {
    let mut byte = 0u8;
    let n = unsafe {
        libc::recv(
            stream.as_raw_fd(),
            (&mut byte as *mut u8).cast(),
            1,
            libc::MSG_PEEK | libc::MSG_DONTWAIT,
        )
    };
    n > 0 || (n < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::WouldBlock)
}
fn monotonic_ns() -> Result<u64> {
    let mut time = libc::timespec {
        tv_sec: -1,
        tv_nsec: -1,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0
        || time.tv_sec < 0
        || !(0..1_000_000_000).contains(&time.tv_nsec)
    {
        return Err(channel_error());
    }
    (time.tv_sec as u64)
        .checked_mul(1_000_000_000)
        .and_then(|v| v.checked_add(time.tv_nsec as u64))
        .ok_or_else(channel_error)
}
fn deadline(value: &str) -> Result<u64> {
    let n = value.parse::<u64>().map_err(|_| channel_error())?;
    if n.to_string() != value {
        return Err(channel_error());
    }
    let now = monotonic_ns()?;
    if n <= now || n - now > 2_000_000_000 {
        return Err(fault(
            "cooperative_deadline_expired",
            "Original bounded monotonic deadline is absent/expired; no dispatch or renewal.",
        ));
    }
    Ok(n)
}
pub(crate) fn capability() -> serde_json::Value {
    serde_json::json!({"implemented":true,"available":owner::factory_available(),"opt_in_required":true,
        "route":"native-authenticated cooperative owner; synchronous owner-main-thread NSApplication.sendEvent",
        "input_selector":["phase","owner_endpoint","registration_id"],"caller_tag_or_trusted_witness":false,
        "raw_tag_lane_enabled":false,"unknown_registration":"fail_closed","fallback":false,"retry_input":false,
        "mutual_peer":"kernel PID + audit token/pidversion + native process birth, repeated before execution",
        "user_data":"NULL","foreign_tag_CGPost":false,"activation_requested":false,"cursor_warp_requested":false,
        "native_acknowledgement":"owner public dispatch return only; not View/tooltip outcome",
        "application_effect_attested":false,"ordinary_third_party_or_modern_tracking_verified":false,
        "registration_lifetime":"owner UI reservation; caller integration must defer close/reparent/remove while busy"})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn f05_cooperative_tracking_protocol_bounds_and_untrusted_fields() {
        assert!(decode::<Request>(&vec![b'x'; LIMIT + 1]).is_err());
        assert!(
            decode::<Request>(
                br#"{"op":"describe","version":1,"registration_id":"x","trusted_witness":true}"#
            )
            .is_err()
        );
        assert!(decode::<Request>(br#"{"op":"execute","tracking_number":"1"}"#).is_err());
        assert!(deadline("0").is_err());
        assert!(deadline("+10").is_err());
        let message = Request::Describe {
            version: 1,
            registration_id: "own-1".into(),
        };
        let encoded = bytes(&message).unwrap();
        assert_eq!(decode::<Request>(&encoded[4..]).unwrap(), message);
    }
    #[test]
    fn f05_cooperative_tracking_terminal_reply_keeps_peer_through_validation() {
        // Both describe and mutation ACK use the same production terminal path.
        // Synthetic protocol values only: no AppKit window or native dispatch.
        for expected in [
            Reply::Registration {
                registration_id: "owned".into(),
                generation: "generation".into(),
                window: 1,
                frame: [0., 0., 10., 10.],
                title: "own".into(),
            },
            Reply::Returned {
                nonce: "original".into(),
                public_dispatch_count: 1,
                goal_verified: false,
            },
        ] {
            let (mut client, mut owner) = UnixStream::pair().unwrap();
            let identity = peer(&client).unwrap();
            let reply = expected.clone();
            let end = Instant::now() + IO_BUDGET;
            let worker = std::thread::spawn(move || {
                send_terminal_reply(&mut owner, &reply, end, &|| Ok(()))
            });
            let actual: Reply = receive(&mut client, end, &|| Ok(())).unwrap();
            assert_eq!(actual, expected);
            assert!(peer(&client).unwrap() == identity);
            assert!(!worker.is_finished()); // Owner cannot close before client EOF.
            drop(client);
            assert!(worker.join().unwrap().is_ok());
        }
    }
    #[test]
    fn f05_cooperative_tracking_terminal_no_eof_has_original_bounded_deadline() {
        let (mut client, mut owner) = UnixStream::pair().unwrap();
        let end = Instant::now() + Duration::from_millis(100);
        let worker = std::thread::spawn(move || {
            send_terminal_reply(&mut owner, &reject("readonly-test"), end, &|| Ok(()))
        });
        let reply: Reply = receive(&mut client, Instant::now() + IO_BUDGET, &|| Ok(())).unwrap();
        assert_eq!(reply, reject("readonly-test"));
        // Client intentionally stays connected. Terminal hold must expire without
        // opening another channel, renewing its budget, or executing any input.
        assert!(worker.join().unwrap().is_err());
        assert!(Instant::now() >= end);
        drop(client);
    }
    #[test]
    fn f05_cooperative_tracking_kernel_peer_is_not_a_json_pid() {
        let (a, _b) = UnixStream::pair().unwrap();
        let fact = peer(&a).unwrap();
        assert_eq!(fact.pid, std::process::id());
        assert_eq!(
            fact.birth,
            crate::native::process_epoch(std::process::id()).unwrap()
        );
        assert_eq!(peer(&a).unwrap().version, fact.version);
    }
}
