//! Native token diagnostics; no launch, policy mutation or OS input.
use super::*;

/// Service identity diagnostics do not require a desktop/window or grant OS rights.
/// Inspect this PID against the actual listener; never report the launcher's token.
pub fn current_process_status() -> Result<serde_json::Value> {
    process_status(unsafe { GetCurrentProcess() })
}

/// Explicit read-only diagnostic: retain the process handle and match its birth,
/// rather than guessing that a PID is still the same process. Never terminates it.
pub fn inspect_process(pid: u32, expected_created: u64) -> Result<serde_json::Value> {
    if pid == 0 || expected_created == 0 {
        return Err(fault(
            "invalid_process_selector",
            "Positive PID and exact creation FILETIME required.",
        ));
    }
    rwi_core::operation::checkpoint("process_diagnostic_open")?;
    let handle = api("process_diagnostic_unavailable", unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            false,
            pid,
        )
    })?;
    let handle = owned(handle);
    if creation_time(raw(&handle))? != expected_created {
        return Err(fault(
            "process_diagnostic_birth_changed",
            "PID has a different birth; no replacement or retry.",
        ));
    }
    process_status(raw(&handle))
}

fn process_status(process: HANDLE) -> Result<serde_json::Value> {
    rwi_core::operation::checkpoint("agent_process_diagnostics")?;
    require_alive(process)?;
    let pid = unsafe { GetProcessId(process) };
    if pid == 0 {
        return Err(fault(
            "process_diagnostic_pid_unavailable",
            "Native process PID could not be measured.",
        ));
    }
    let created = creation_time(process)?;
    let image = image_path(process)?;
    let before = raw_token_identity(process)?;
    let privilege = process_privilege(process)?;
    let after = raw_token_identity(process)?;
    privilege.require_same(&process_privilege(process)?)?;
    if before.sid != after.sid
        || before.session != after.session
        || before.authentication != after.authentication
        || created != creation_time(process)?
        || image != image_path(process)?
    {
        return Err(fault(
            "agent_process_diagnostics_changed",
            "Process/token identity changed during the read-only sample; no privilege inferred.",
        ));
    }
    require_alive(process)?;
    rwi_core::operation::checkpoint("agent_process_diagnostics_returned")?;
    Ok(serde_json::json!({
        "status":"available", "source":"retained process handle/OpenProcessToken/GetTokenInformation; not launcher or configured privilege",
        "pid":pid, "creation_filetime_100ns":created.to_string(), "image_path":image,
        "user_sid":after.sid, "session_id":after.session,
        "authentication_id":after.authentication, "token_privilege":privilege,
        "input_sent":false, "permissions_changed":false,
        "desktop_availability_inferred":false
    }))
}

/// Read actual process token facts. No mode is inferred from configuration,
/// launcher, administrator group name, command line or the caller's claims.
pub fn current_token_privilege() -> Result<TokenPrivilegeFacts> {
    let session = current_session()?;
    let facts = process_privilege(unsafe { GetCurrentProcess() })?;
    session.require_same(&current_session()?)?;
    Ok(facts)
}
pub fn require_current_privilege(mode: PrivilegeMode) -> Result<TokenPrivilegeFacts> {
    let facts = current_token_privilege()?;
    facts.require_mode(mode)?;
    Ok(facts)
}
