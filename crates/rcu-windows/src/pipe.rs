//! Local-only, one-client transport for an explicitly started interactive server.
//! No scheduled-task launcher, process shell, TCP listener, VM wakeup, or retry.
use crate::desktop::{Handle, api};
use rcu_core::{Result, digest, fault};
use std::{ffi::c_void, mem::size_of, os::windows::io::AsRawHandle, path::Path, time::Duration};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};
use windows::{
    Win32::{
        Foundation::*,
        Security::{Authorization::*, *},
        System::{Pipes::*, Threading::*},
    },
    core::{PCWSTR, PWSTR},
};

pub fn pipe_path(id: &str) -> Result<String> {
    if id.is_empty() || id.len() > 96 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(fault(
            "invalid_pipe_id",
            "Use 1..96 ASCII letters, digits, or hyphens; no absolute/remote pipe paths.",
        ));
    }
    Ok(format!(r"\\.\pipe\RealComputerUse-{id}"))
}
struct Local(HLOCAL);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(Some(self.0));
        }
    }
}
struct Security {
    attributes: SECURITY_ATTRIBUTES,
    _descriptor: Local,
}
impl Security {
    fn user_and_system() -> Result<Self> {
        let mut token = HANDLE::default();
        api("pipe_acl_token", unsafe {
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
        })?;
        let token = Handle(token);
        // Aligned storage for TOKEN_USER with trailing SID, hard upper bound 64 KiB.
        let mut needed = 0;
        let _ = unsafe { GetTokenInformation(token.0, TokenUser, None, 0, &mut needed) };
        if needed < size_of::<TOKEN_USER>() as u32 || needed > 65536 {
            return Err(fault("pipe_acl_token", "Invalid native user-token size."));
        }
        let mut storage = vec![0usize; (needed as usize).div_ceil(size_of::<usize>())];
        api("pipe_acl_token", unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                Some(storage.as_mut_ptr().cast()),
                needed,
                &mut needed,
            )
        })?;
        let user = unsafe { &*storage.as_ptr().cast::<TOKEN_USER>() };
        let mut sid = PWSTR::null();
        api("pipe_acl_sid", unsafe {
            ConvertSidToStringSidW(user.User.Sid, &mut sid)
        })?;
        let _sid = Local(HLOCAL(sid.0.cast()));
        let sid =
            unsafe { sid.to_string() }.map_err(|_| fault("pipe_acl_sid", "Invalid SID string."))?;
        let sddl = format!("D:P(A;;GA;;;SY)(A;;GA;;;{sid})");
        let sddl: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        api("pipe_acl_descriptor", unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        })?;
        Ok(Self {
            attributes: SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: descriptor.0,
                bInheritHandle: false.into(),
            },
            _descriptor: Local(HLOCAL(descriptor.0)),
        })
    }
}
pub async fn accept(id: &str) -> Result<NamedPipeServer> {
    let name = pipe_path(id)?;
    let mut security = Security::user_and_system()?;
    let server = unsafe {
        ServerOptions::new()
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .max_instances(1)
            .in_buffer_size(65536)
            .out_buffer_size(65536)
            .create_with_security_attributes_raw(
                name,
                (&mut security.attributes as *mut SECURITY_ATTRIBUTES).cast::<c_void>(),
            )
    }?;
    // The OS copies the descriptor during creation; retain no raw pointer across await.
    drop(security);
    tokio::time::timeout(Duration::from_secs(120), server.connect())
        .await
        .map_err(|_| {
            fault(
                "pipe_accept_timeout",
                "Candidate exited without a client; explicitly start a new acceptance run.",
            )
        })??;
    Ok(server)
}
fn verify_server(pipe: &impl AsRawHandle, expected_sha: &str) -> Result<u32> {
    if expected_sha.len() != 64 || !expected_sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(fault(
            "invalid_server_digest",
            "Pass the independently recorded ARM64 server SHA-256.",
        ));
    }
    let mut pid = 0;
    api("pipe_server_identity", unsafe {
        GetNamedPipeServerProcessId(HANDLE(pipe.as_raw_handle()), &mut pid)
    })?;
    let process = Handle(api("pipe_server_identity", unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
    })?);
    let mut path = vec![0u16; 32768];
    let mut len = path.len() as u32;
    api("pipe_server_image", unsafe {
        QueryFullProcessImageNameW(
            process.0,
            PROCESS_NAME_FORMAT(0),
            PWSTR(path.as_mut_ptr()),
            &mut len,
        )
    })?;
    let path = String::from_utf16(&path[..len as usize]).map_err(|_| {
        fault(
            "pipe_server_image",
            "Native image path is not valid UTF-16.",
        )
    })?;
    if std::fs::metadata(Path::new(&path))?.len() > 64 * 1024 * 1024 {
        return Err(fault(
            "pipe_server_image_limit",
            "Server image exceeds 64 MiB verification budget.",
        ));
    }
    if digest(&std::fs::read(path)?) != expected_sha.to_ascii_lowercase() {
        return Err(fault(
            "pipe_server_digest_mismatch",
            "Do not forward MCP bytes to another candidate image.",
        ));
    }
    Ok(pid)
}
pub async fn client(id: &str, expected_sha: &str) -> Result<()> {
    let pipe = ClientOptions::new().open(pipe_path(id)?)?;
    let pid = verify_server(&pipe, expected_sha)?;
    eprintln!(
        "RCU local pipe connected; server_pid={pid}; image_sha256={expected_sha}; no reconnect/replay"
    );
    let (mut reader, mut writer) = tokio::io::split(pipe);
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    // End on either direction closing. Never reconnect or replay buffered requests.
    tokio::select! { r=tokio::io::copy(&mut stdin,&mut writer)=>{r?;},r=tokio::io::copy(&mut reader,&mut stdout)=>{r?;} }
    Ok(())
}

/// Stronger native-agent route. The original `accept` / `client` are unchanged.
/// No MCP byte is accepted until the actual connected client is bound to the
/// local owner proof. The caller must retain and recheck the returned peer guard.
pub async fn accept_owned(
    id: &str,
    owner: &crate::agent_contract::OwnerProof,
    budget: Duration,
) -> Result<(NamedPipeServer, crate::agent_binding::PeerProcess)> {
    use crate::agent_binding::{PeerProcess, UserSecurity, current_session};
    if budget.is_zero() || budget > Duration::from_secs(120) {
        return Err(fault(
            "owned_pipe_budget_invalid",
            "Explicit pipe acceptance budget must be within 1 ns..120 s.",
        ));
    }
    let deadline = tokio::time::Instant::now() + budget;
    owner.validate()?;
    owner.session.require_same(&current_session()?)?;
    let server = {
        let mut security = UserSecurity::new(&owner.session.user_sid, false)?;
        unsafe {
            ServerOptions::new()
                .first_pipe_instance(true)
                .reject_remote_clients(true)
                .max_instances(1)
                .in_buffer_size(65536)
                .out_buffer_size(65536)
                .create_with_security_attributes_raw(
                    pipe_path(id)?,
                    (&mut security.attributes as *mut SECURITY_ATTRIBUTES).cast::<c_void>(),
                )
        }?
    };
    // The lexical scope guarantees no descriptor pointer crosses this await.
    tokio::time::timeout_at(deadline, server.connect()).await
        .map_err(|_| fault("owned_pipe_accept_timeout", "No approved agent connected inside this stage budget; do not start another instance automatically."))??;
    let peer = PeerProcess::from_pipe_client(&server, owner)?;
    if tokio::time::Instant::now() >= deadline {
        return Err(fault(
            "owned_pipe_accept_timeout",
            "Pipe peer binding exceeded the acceptance budget; no MCP bytes were accepted.",
        ));
    }
    Ok((server, peer))
}

/// Wait only for the freshly owned worker to publish its one local pipe. This
/// initial startup wait is bounded; after a connection exists there is no retry,
/// reconnect, byte forwarding, or request replay in this helper.
pub async fn connect_owned(
    id: &str,
    worker: &crate::agent_binding::OwnedProcess,
    session: &crate::agent_contract::SessionBinding,
    budget: Duration,
) -> Result<tokio::net::windows::named_pipe::NamedPipeClient> {
    if budget.is_zero() || budget > Duration::from_secs(120) {
        return Err(fault(
            "owned_pipe_budget_invalid",
            "Explicit pipe connection budget must be within 1 ns..120 s.",
        ));
    }
    let name = pipe_path(id)?;
    let deadline = tokio::time::Instant::now() + budget;
    loop {
        worker.verify_current(session)?;
        if tokio::time::Instant::now() >= deadline {
            return Err(fault(
                "owned_pipe_connect_timeout",
                "The owned worker did not publish its approved pipe inside the stage budget.",
            ));
        }
        match ClientOptions::new().open(&name) {
            Ok(pipe) => {
                worker.verify_pipe_server(&pipe)?;
                worker.verify_current(session)?;
                if tokio::time::Instant::now() >= deadline {
                    return Err(fault(
                        "owned_pipe_connect_timeout",
                        "Pipe identity verification exceeded the stage budget; no MCP bytes were forwarded.",
                    ));
                }
                return Ok(pipe);
            }
            Err(error) if matches!(error.raw_os_error(), Some(2 | 231)) => {
                // ERROR_FILE_NOT_FOUND / ERROR_PIPE_BUSY: initial publication
                // only, never an attempt to recover a disconnected session.
                let wake = (tokio::time::Instant::now() + Duration::from_millis(20)).min(deadline);
                tokio::time::sleep_until(wake).await;
            }
            Err(error) => return Err(error.into()),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_and_absolute_pipe_names_rejected() {
        for id in ["", r"\\other\pipe\x", r"a\b", "a/b", "a:b"] {
            assert!(pipe_path(id).is_err());
        }
        assert_eq!(
            pipe_path("M1-123").unwrap(),
            r"\\.\pipe\RealComputerUse-M1-123"
        );
    }
}
