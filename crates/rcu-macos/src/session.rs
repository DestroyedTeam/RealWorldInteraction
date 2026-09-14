//! Read-only OS context and TCC preflights. Never requests permission or reads UI.
use crate::ffi::{self, Cf};
use rcu_core::{Identity, Result, digest, fault};
use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ContextFacts {
    pub uid: u32,
    pub euid: u32,
    pub security_session: u32,
    pub attributes: u32,
    pub console_uid: i64,
    pub console_seat: Option<i64>,
    pub on_console: Option<bool>,
    pub login_done: Option<bool>,
    pub host: String,
    pub boot: String,
}
impl ContextFacts {
    pub(crate) fn identity(&self, expected_uid: u32) -> Result<Identity> {
        if self.uid != expected_uid || self.euid != expected_uid {
            return Err(fault(
                "native_user_mismatch",
                "Run as the configured macOS UID, without a substituted effective user.",
            ));
        }
        if self.security_session == 0 || self.security_session >= u32::MAX - 1 {
            return Err(fault(
                "session_unknown",
                "No actual SecuritySessionId was observed.",
            ));
        }
        if self.attributes & 0x0010 == 0 || self.attributes & 0x0001 != 0 {
            return Err(fault(
                "session_not_interactive",
                "The process security session has no native graphical access.",
            ));
        }
        if self.on_console != Some(true) || self.login_done != Some(true) {
            return Err(fault(
                "session_not_interactive",
                "No authenticated on-console graphical session; service metadata remains available.",
            ));
        }
        if self.console_uid != i64::from(expected_uid) {
            return Err(fault(
                "session_user_mismatch",
                "The configured UID is not the independently observed console user.",
            ));
        }
        Ok(Identity {
            host: self.host.clone(),
            guest: None,
            session: format!(
                "uid:{}:security:{}:boot:{}",
                self.uid, self.security_session, self.boot
            ),
            // This is a conservative RCU conflict-domain role, not a measured
            // hardware seat. Optional hardware metadata never splits its lock.
            seat: "quartz-on-console-logical-domain".into(),
            desktop: "quartz-console".into(),
        })
    }
}

pub(crate) fn current_facts() -> Result<ContextFacts> {
    let mut security_session = 0;
    let mut attributes = 0;
    let result = unsafe { ffi::SessionGetInfo(u32::MAX, &mut security_session, &mut attributes) };
    if result != 0 {
        return Err(fault(
            "session_unknown",
            "SessionGetInfo could not read this process security session.",
        ));
    }
    let dictionary = unsafe { Cf::owned(ffi::CGSessionCopyCurrentDictionary()) }?;
    let mut host = [0u8; 16];
    let timeout = libc::timespec {
        tv_sec: 1,
        tv_nsec: 0,
    };
    if unsafe { libc::gethostuuid(host.as_mut_ptr(), &timeout) } != 0 {
        return Err(fault(
            "host_identity_unknown",
            "Cannot read the local host UUID.",
        ));
    }
    let mut boot = libc::timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    let mut len = std::mem::size_of_val(&boot);
    if unsafe {
        libc::sysctlbyname(
            c"kern.boottime".as_ptr(),
            (&mut boot as *mut libc::timeval).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    } != 0
        || len != std::mem::size_of_val(&boot)
        || boot.tv_sec <= 0
    {
        return Err(fault(
            "session_unknown",
            "Cannot bind the current native boot epoch.",
        ));
    }
    let number = |key| {
        unsafe { ffi::number(ffi::field(dictionary.0, key)) }.ok_or_else(|| {
            fault(
                "session_unknown",
                &format!("Missing native numeric console field {key}."),
            )
        })
    };
    Ok(ContextFacts {
        uid: unsafe { libc::getuid() },
        euid: unsafe { libc::geteuid() },
        security_session,
        attributes,
        console_uid: number("kCGSSessionUserIDKey")?,
        console_seat: unsafe { ffi::number(ffi::field(dictionary.0, "kCGSSessionConsoleSetKey")) }
            .filter(|value| (0..=i64::from(u32::MAX)).contains(value)),
        on_console: unsafe { ffi::boolean(ffi::field(dictionary.0, "kCGSSessionOnConsoleKey")) },
        login_done: unsafe { ffi::boolean(ffi::field(dictionary.0, "kCGSessionLoginDoneKey")) },
        host: format!("macos-{}", digest(&host)),
        // Some Apple Silicon systems recompute the sub-second part of
        // `kern.boottime` after time-sync/sleep adjustments, so an identity that
        // embedded tv_usec flipped and fenced every operation with a false
        // `session_mismatch`. Real context changes (reboot, login, seat) are
        // already fenced by the boot epoch second, security session and host.
        boot: boot.tv_sec.to_string(),
    })
}

pub(crate) fn console_identity() -> Result<Identity> {
    let facts = current_facts()?;
    facts.identity(facts.euid)
}
pub(crate) fn identity_for_uid(uid: u32) -> Result<Identity> {
    current_facts()?.identity(uid)
}

pub(crate) fn permissions() -> Value {
    json!({
        "accessibility":unsafe { ffi::AXIsProcessTrusted() != 0 },
        "screen_capture":unsafe { ffi::CGPreflightScreenCaptureAccess() },
        "post_event":unsafe { ffi::CGPreflightPostEventAccess() },
        "listen_event":unsafe { ffi::CGPreflightListenEventAccess() },
        "screenshot_api":crate::capture::api_available(),
        "requests_permission":false
    })
}

fn context_probe() -> Value {
    let mut session = 0;
    let mut attributes = 0;
    let code = unsafe { ffi::SessionGetInfo(u32::MAX, &mut session, &mut attributes) };
    let mut result = json!({"security_session":{"api_status":code,"id":if code==0{Some(session.to_string())}else{None},"attributes":if code==0{Some(attributes)}else{None}}});
    match unsafe { Cf::owned(ffi::CGSessionCopyCurrentDictionary()) } {
        Ok(dictionary) => {
            for (name, key) in [
                ("console_uid", "kCGSSessionUserIDKey"),
                ("console_set", "kCGSSessionConsoleSetKey"),
                ("on_console", "kCGSSessionOnConsoleKey"),
                ("login_done", "kCGSessionLoginDoneKey"),
            ] {
                let value = unsafe { ffi::field(dictionary.0, key) };
                let number = unsafe { ffi::number(value) };
                let boolean = unsafe { ffi::boolean(value) };
                result[name] = json!({"present":!value.is_null(),"type":if value.is_null(){"absent"}else if number.is_some(){"CFNumber"}else if boolean.is_some(){"CFBoolean"}else{"other"},"numeric":number,"boolean":boolean});
            }
        }
        Err(error) => {
            result["console_dictionary_error"] = json!(error);
        }
    }
    result
}

/// No window enumeration, AX content, image capture, input or service startup.
pub fn process_status(expected_uid: u32) -> Value {
    let facts = current_facts();
    let mut status = json!({
        "platform":"macos","pid":std::process::id(),
        "uid":unsafe { libc::getuid() },"euid":unsafe { libc::geteuid() },
        "configured_uid":expected_uid,"permissions":permissions(),
        "context_probe":context_probe(),
        "evidence":"native_api_observed","hardware_attestation":false,
        "seat_kind":"logical_on_console_domain","native_console_set_id":null,"native_hardware_seat_observed":false,
        "lock_state":"unknown","guest_identity":"unknown",
        "screen_captured":false,"input_sent":false,"window_list_read":false
    });
    match facts {
        Ok(facts) => {
            status["native_console_set_id"] = json!(facts.console_seat);
            status["native_hardware_seat_observed"] = json!(facts.console_seat.is_some());
            status["security_session_id"] = json!(facts.security_session.to_string());
            status["security_session_attributes"] = json!(facts.attributes);
            status["console"] = json!({"uid":facts.console_uid,"native_console_set_id":facts.console_seat,"on_console":facts.on_console,"login_done":facts.login_done});
            match facts.identity(expected_uid) {
                Ok(identity) => {
                    status["status"] = json!("available");
                    status["native_context"] = json!(identity);
                }
                Err(error) => {
                    status["status"] = json!("unavailable");
                    status["error"] = json!(error);
                }
            }
        }
        Err(error) => {
            status["status"] = json!("unavailable");
            status["error"] = json!(error);
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) fn facts() -> ContextFacts {
        ContextFacts {
            uid: 501,
            euid: 501,
            security_session: 1001,
            attributes: 0x10,
            console_uid: 501,
            console_seat: Some(1),
            on_console: Some(true),
            login_done: Some(true),
            host: "mac-test-host".into(),
            boot: "100:1".into(),
        }
    }
    #[test]
    fn mandatory_native_security_login_boot_host_and_uid_isolate_the_input_domain() {
        let first = facts();
        let key = first.identity(501).unwrap().key();
        for change in 0..4 {
            let mut other = first.clone();
            match change {
                0 => other.security_session += 1,
                1 => other.boot = "101:1".into(),
                2 => other.host = "different-native-host".into(),
                _ => {
                    other.uid = 502;
                    other.euid = 502;
                    other.console_uid = 502;
                }
            }
            assert_ne!(other.identity(other.uid).unwrap().key(), key);
        }
        let identity = first.identity(501).unwrap();
        assert!(identity.guest.is_none());
        assert!(!identity.session.contains("SID"));
    }
    #[test]
    fn optional_hardware_console_metadata_never_splits_a_logical_input_domain() {
        let key = facts().identity(501).unwrap().key();
        for console_seat in [Some(1), None, Some(2)] {
            let mut observed = facts();
            observed.console_seat = console_seat;
            assert_eq!(observed.identity(501).unwrap().key(), key);
            assert_eq!(
                observed.identity(501).unwrap().seat,
                "quartz-on-console-logical-domain"
            );
        }
    }
    #[test]
    fn unknown_or_nonconsole_or_substituted_user_context_is_rejected() {
        for change in 0..7 {
            let mut f = facts();
            match change {
                0 => f.security_session = 0,
                1 => f.attributes = 0,
                2 => f.attributes |= 1,
                3 => f.on_console = None,
                4 => f.login_done = Some(false),
                5 => f.euid = 0,
                _ => f.console_uid = 502,
            }
            assert!(f.identity(501).is_err());
        }
        assert!(facts().identity(502).is_err());
    }
}
