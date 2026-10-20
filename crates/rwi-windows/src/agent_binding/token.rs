//! Native token/session reads; moved without changing policy or call order.
use super::*;

pub(super) fn token_identity(process: HANDLE) -> Result<TokenIdentity> {
    let identity = raw_token_identity(process)?;
    validate_user_sid(&identity.sid)?;
    if identity.session == 0 {
        return Err(fault(
            "agent_session_zero_refused",
            "Require an allocated interactive user session, not session zero.",
        ));
    }
    Ok(identity)
}

// Observation-only token facts. Interactive input policy stays in token_identity.
pub(super) fn raw_token_identity(process: HANDLE) -> Result<TokenIdentity> {
    let mut token = HANDLE::default();
    api("agent_token_unavailable", unsafe {
        OpenProcessToken(process, TOKEN_QUERY, &mut token)
    })?;
    let token = owned(token);
    let mut needed = 0;
    let _ = unsafe { GetTokenInformation(raw(&token), TokenUser, None, 0, &mut needed) };
    if needed < size_of::<TOKEN_USER>() as u32 || needed > 65536 {
        return Err(fault(
            "agent_token_invalid",
            "User token exceeds the bounded native buffer.",
        ));
    }
    let mut storage = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
    let capacity = needed;
    api("agent_token_unavailable", unsafe {
        GetTokenInformation(
            raw(&token),
            TokenUser,
            Some(storage.as_mut_ptr().cast()),
            capacity,
            &mut needed,
        )
    })?;
    if needed > capacity || needed < size_of::<TOKEN_USER>() as u32 {
        return Err(fault(
            "agent_token_invalid",
            "Native token length changed unexpectedly.",
        ));
    }
    let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
    let sid = sid_string(user.User.Sid)?;
    let mut stats = TOKEN_STATISTICS::default();
    api("agent_logon_unavailable", unsafe {
        GetTokenInformation(
            raw(&token),
            TokenStatistics,
            Some((&mut stats as *mut TOKEN_STATISTICS).cast()),
            size_of::<TOKEN_STATISTICS>() as u32,
            &mut needed,
        )
    })?;
    let mut session = 0u32;
    api("agent_session_unavailable", unsafe {
        GetTokenInformation(
            raw(&token),
            TokenSessionId,
            Some((&mut session as *mut u32).cast()),
            size_of::<u32>() as u32,
            &mut needed,
        )
    })?;
    Ok(TokenIdentity {
        sid,
        session,
        authentication: AuthenticationId {
            high: stats.AuthenticationId.HighPart,
            low: stats.AuthenticationId.LowPart,
        },
    })
}

fn token_dword(token: HANDLE, kind: TOKEN_INFORMATION_CLASS) -> Result<u32> {
    let mut value = 0u32;
    let mut returned = 0;
    api("agent_token_privilege_unavailable", unsafe {
        // Exact four-byte ABI, not the aligned allocation capacity. In the
        // real v1 diagnostic TokenElevation rejected an eight-byte capacity.
        GetTokenInformation(
            token,
            kind,
            Some((&mut value as *mut u32).cast()),
            size_of::<u32>() as u32,
            &mut returned,
        )
    })?;
    if returned != size_of::<u32>() as u32 {
        return Err(fault(
            "agent_token_privilege_size",
            "Unexpected native DWORD token-information size.",
        ));
    }
    Ok(value)
}

pub(super) fn process_privilege(process: HANDLE) -> Result<TokenPrivilegeFacts> {
    let mut token = HANDLE::default();
    api("agent_token_privilege_unavailable", unsafe {
        OpenProcessToken(process, TOKEN_QUERY, &mut token)
    })?;
    let token = owned(token);
    let elevated = token_dword(raw(&token), TokenElevation)?;
    let elevation_type = token_dword(raw(&token), TokenElevationType)?;
    let ui_access = token_dword(raw(&token), TokenUIAccess)?;
    let app_container = token_dword(raw(&token), TokenIsAppContainer)?;
    if elevated > 1 || ui_access > 1 || app_container > 1 || !(1..=3).contains(&elevation_type) {
        return Err(fault(
            "agent_token_privilege_invalid",
            "Native token returned unknown privilege flags; no inferred mode.",
        ));
    }
    let mut needed = 0;
    let _ = unsafe { GetTokenInformation(raw(&token), TokenIntegrityLevel, None, 0, &mut needed) };
    if needed < size_of::<TOKEN_MANDATORY_LABEL>() as u32 || needed > 65536 {
        return Err(fault(
            "agent_token_integrity_invalid",
            "Native integrity label exceeds its bounded ABI buffer.",
        ));
    }
    let exact_bytes = needed;
    let mut buffer = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
    api("agent_token_integrity_unavailable", unsafe {
        GetTokenInformation(
            raw(&token),
            TokenIntegrityLevel,
            Some(buffer.as_mut_ptr().cast()),
            exact_bytes,
            &mut needed,
        )
    })?;
    if needed != exact_bytes {
        return Err(fault(
            "agent_token_integrity_invalid",
            "Integrity label size changed during query.",
        ));
    }
    let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>()).Label.Sid };
    if !unsafe { IsValidSid(sid) }.as_bool() {
        return Err(fault(
            "agent_token_integrity_invalid",
            "Invalid native integrity SID.",
        ));
    }
    let count = unsafe { *GetSidSubAuthorityCount(sid) };
    if count == 0 {
        return Err(fault(
            "agent_token_integrity_invalid",
            "Integrity SID has no RID.",
        ));
    }
    let integrity_rid = unsafe { *GetSidSubAuthority(sid, u32::from(count) - 1) };
    Ok(TokenPrivilegeFacts {
        elevated: elevated != 0,
        elevation_type,
        integrity_rid,
        ui_access: ui_access != 0,
        app_container: app_container != 0,
    })
}

/// Includes the frozen backend's WTSActive/unlocked/WinSta0/input-desktop checks.
/// The machine digest is local Windows evidence, not VM/host attestation.
pub fn current_session() -> Result<SessionBinding> {
    let native = desktop::identity()?;
    let token = token_identity(unsafe { GetCurrentProcess() })?;
    let mut pointer = PWSTR::null();
    let mut count = 0;
    api("agent_logon_unavailable", unsafe {
        WTSQuerySessionInformationW(
            None,
            token.session,
            WTSSessionInfoEx,
            &mut pointer,
            &mut count,
        )
    })?;
    struct Wts(PWSTR);
    impl Drop for Wts {
        fn drop(&mut self) {
            unsafe {
                WTSFreeMemory(self.0.0.cast());
            }
        }
    }
    let info = Wts(pointer);
    if info.0.is_null() || count < size_of::<WTSINFOEXW>() as u32 {
        return Err(fault(
            "agent_logon_unavailable",
            "Extended WTS session record is incomplete.",
        ));
    }
    let info = unsafe { &*info.0.0.cast::<WTSINFOEXW>() };
    if info.Level != 1 {
        return Err(fault(
            "agent_logon_unavailable",
            "Unexpected WTS record level.",
        ));
    }
    let info = unsafe { info.Data.WTSInfoExLevel1 };
    if info.SessionId != token.session
        || info.SessionState != WTSActive
        || info.SessionFlags != WTS_SESSIONSTATE_UNLOCK as i32
    {
        return Err(fault(
            "agent_session_not_interactive",
            "The allocated native session is inactive or locked.",
        ));
    }
    let binding = SessionBinding {
        input_desktop: native.desktop.clone(),
        native,
        user_sid: token.sid,
        session_id: token.session,
        authentication_id: token.authentication,
        logon_time_100ns: info.LogonTime,
    };
    binding.validate()?;
    // A second original observation closes changes between the WTS/token reads.
    if desktop::identity()? != binding.native {
        return Err(fault(
            "agent_session_binding_changed",
            "The native desktop changed during binding.",
        ));
    }
    Ok(binding)
}
