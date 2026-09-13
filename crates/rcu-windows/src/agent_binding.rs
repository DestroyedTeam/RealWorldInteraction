//! Real Windows bindings for the native-link supervisor. No process discovery,
//! input, desktop switching, remote paths, or termination-by-PID is provided.
mod diagnostics;
mod file_policy;
mod input;
mod installation;
mod private_files;
mod process;
mod token;
use crate::{
    agent_contract::{
        AuthenticationId, OwnerProof, PrivateAclEvidence, PrivateAclScope, PrivilegeMode,
        ProcessBinding, SelectedWindowProof, SessionBinding, TokenPrivilegeFacts,
        WindowUserBinding, validate_local_dos_path, validate_private_name, validate_user_sid,
    },
    desktop::{self, api},
};
pub use diagnostics::{
    current_process_status, current_token_privilege, inspect_process, require_current_privilege,
};
use file_policy::{
    check_open_path, check_private_acl, check_private_acl_scope, file_info, open_literal,
    path_text, read_file, wide,
};
pub use input::foreground_window_id;
pub use installation::{acquire_installation_lock, fixed_agent_installation_root};
pub use private_files::read_private_file_no_reparse;
use process::{creation_time, image_path, require_alive};
pub use process::{current_owner_proof, retain_owned_process};
use rcu_core::{Result, fault};
use sha2::{Digest, Sha256};
use std::{
    ffi::c_void,
    fs::File,
    io::Write,
    mem::size_of,
    os::windows::{
        fs::FileExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
    },
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};
pub use token::current_session;
use token::{process_privilege, raw_token_identity, token_identity};
use tokio::net::windows::named_pipe::{NamedPipeClient, NamedPipeServer};
use windows::{
    Win32::{
        Foundation::*,
        Security::{Authorization::*, *},
        Storage::FileSystem::*,
        System::{Pipes::*, RemoteDesktop::*, StationsAndDesktops::*, Threading::*},
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
    },
    core::{PCWSTR, PWSTR},
};

const IMAGE_CAP: usize = 128 * 1024 * 1024;
const PRIVATE_CAP: usize = 64 * 1024 * 1024;

struct Local(HLOCAL);
impl Drop for Local {
    fn drop(&mut self) {
        unsafe {
            LocalFree(Some(self.0));
        }
    }
}

fn owned(handle: HANDLE) -> OwnedHandle {
    // All callers pass a successfully acquired real (not pseudo) Win32 handle.
    unsafe { OwnedHandle::from_raw_handle(handle.0) }
}

fn raw(handle: &impl AsRawHandle) -> HANDLE {
    HANDLE(handle.as_raw_handle())
}

fn sid_string(sid: PSID) -> Result<String> {
    if !unsafe { IsValidSid(sid) }.as_bool() {
        return Err(fault("agent_sid_invalid", "Native SID validation failed."));
    }
    let mut text = PWSTR::null();
    api("agent_sid_unavailable", unsafe {
        ConvertSidToStringSidW(sid, &mut text)
    })?;
    let _allocation = Local(HLOCAL(text.0.cast()));
    unsafe { text.to_string() }.map_err(|_| fault("agent_sid_invalid", "SID is not valid UTF-16."))
}

struct TokenIdentity {
    sid: String,
    session: u32,
    authentication: AuthenticationId,
}

/// A query/synchronize-only duplicate of a *freshly owned* child handle. The
/// original Child remains the supervisor's authority to cancel/terminate/reap.
/// Keeping this handle prevents PID reuse from turning into process ownership.
pub struct OwnedProcess {
    handle: OwnedHandle,
    image: GuardedFile,
    record: ProcessBinding,
    session: SessionBinding,
    privilege: TokenPrivilegeFacts,
}

/// A query/synchronize HANDLE acquired only from a currently verified HWND, not
/// a historic PID claim. This never owns, terminates or activates a user app.
struct WindowUserGuard {
    handle: OwnedHandle,
    expected: WindowUserBinding,
    session: SessionBinding,
}

/// Native-only immutable allocation. The UIA root identity is first pinned by
/// the explicit initial root bind after the owned pipe guard has admitted reads.
pub(crate) struct AgentInputAuthority {
    session: SessionBinding,
    root: crate::WindowRecord,
    root_process: WindowUserGuard,
    root_runtime: OnceLock<Vec<i32>>,
    dispatch_check: Arc<dyn Fn() -> Result<()> + Send + Sync>,
}

pub(crate) struct AgentInputSelection<'a> {
    authority: &'a AgentInputAuthority,
    selected: crate::WindowRecord,
    process: WindowUserGuard,
    owners: Vec<WindowUserGuard>,
}

/// Query-only pipe peer guard, never an owned/killable process. Opened only from
/// the currently connected pipe's OS PID, compared to the local OwnerProof, and
/// retained for the entire service lifetime. No historical PID recovery API.
pub struct PeerProcess {
    handle: OwnedHandle,
    image: GuardedFile,
    expected: OwnerProof,
    privilege: TokenPrivilegeFacts,
}

/// Protected, explicit one-user ACL. This changes only freshly created objects;
/// it never repairs or widens an existing object's permissions.
pub(crate) struct UserSecurity {
    pub(crate) attributes: SECURITY_ATTRIBUTES,
    _allocation: Local,
}

struct PathGuard {
    _ancestors: Vec<File>,
}

struct GuardedFile {
    _parents: PathGuard,
    file: File,
    path: String,
    sha256: String,
    identity: (u32, u32, u32, u32, u32),
}

/// A single fresh/private directory. All methods accept one literal child name,
/// never an arbitrary path. No delete, overwrite, chmod/ACL repair, or shell API.
pub struct PrivateDirectory {
    _parents: PathGuard,
    directory: File,
    path: String,
    user_sid: String,
}

/// Cross-process installation lifetime guard. The file persists; only this
/// OS-exclusive HANDLE means the installation is actively owned. There is no
/// PID claim, marker deletion, automatic HOLD reset or process termination.
pub struct InstallationLock {
    _file: File,
    _directory: File,
    _ancestors: Vec<File>,
    path: PathBuf,
}

#[cfg(test)]
mod abi_tests {
    use super::*;
    // ABI/type checks only. No process/session/pipe/file/native function is run.
    #[test]
    fn retained_guards_are_send_and_sync() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<OwnedProcess>();
        send_sync::<PeerProcess>();
        send_sync::<PrivateDirectory>();
        send_sync::<InstallationLock>();
        assert_eq!(size_of::<FILETIME>(), 8);
        assert_eq!(size_of::<LUID>(), 8);
        assert_eq!(FILE_ALL_ACCESS.0, 0x001f_01ff);
        assert_eq!(OBJECT_INHERIT_ACE.0 | CONTAINER_INHERIT_ACE.0, 3);
        assert_eq!(INHERITED_ACE.0, 16);
        let _: fn() -> Result<PathBuf> = fixed_agent_installation_root;
        let _: fn(&PrivateDirectory) -> Result<InstallationLock> = acquire_installation_lock;
    }

    #[test]
    fn owned_pipe_futures_have_send_abi_without_polling_native_code() {
        fn send<T: Send>(_: T) {}
        fn accept_type(owner: &OwnerProof) {
            send(crate::pipe::accept_owned(
                "not-executed",
                owner,
                std::time::Duration::from_secs(1),
            ));
        }
        fn connect_type(worker: &OwnedProcess, session: &SessionBinding) {
            send(crate::pipe::connect_owned(
                "not-executed",
                worker,
                session,
                std::time::Duration::from_secs(1),
            ));
        }
        let _: fn(&OwnerProof) = accept_type;
        let _: fn(&OwnedProcess, &SessionBinding) = connect_type;
    }
}
