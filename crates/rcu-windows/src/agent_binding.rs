//! Real Windows bindings for the native-link supervisor. No process discovery,
//! input, desktop switching, remote paths, or termination-by-PID is provided.
mod diagnostics;
use crate::{
    agent_contract::{
        AuthenticationId, OwnerProof, PrivateAclEvidence, PrivateAclScope, PrivilegeMode,
        ProcessBinding, SelectedWindowProof, SessionBinding, TokenPrivilegeFacts,
        WindowUserBinding, validate_local_dos_path, validate_private_name, validate_user_sid,
    },
    desktop::{self, api},
};
pub use diagnostics::{current_token_privilege, require_current_privilege};
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

fn token_identity(process: HANDLE) -> Result<TokenIdentity> {
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
    validate_user_sid(&sid)?;
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
    if session == 0 {
        return Err(fault(
            "agent_session_zero_refused",
            "Require an allocated interactive user session, not session zero.",
        ));
    }
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

fn process_privilege(process: HANDLE) -> Result<TokenPrivilegeFacts> {
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

/// Installation authority is derived from the OS Known Folder for the current
/// interactive process token, never LOCALAPPDATA/HOME/config/remote attach data.
/// This resolves a path only; it does not create, move or repair an installation.
pub fn fixed_agent_installation_root() -> Result<PathBuf> {
    use windows::Win32::UI::Shell::{FOLDERID_LocalAppData, KF_FLAG_DONT_VERIFY};
    let session = current_session()?;
    let mut token = HANDLE::default();
    api("agent_installation_token_unavailable", unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_IMPERSONATE,
            &mut token,
        )
    })?;
    let token = owned(token);
    // The high-level generated wrapper hides ppszPath on HRESULT failure, but
    // the API requires CoTaskMemFree even on failure. Retain that pointer here.
    #[link(name = "shell32")]
    unsafe extern "system" {
        #[link_name = "SHGetKnownFolderPath"]
        fn known_folder_path(
            folder: *const windows::core::GUID,
            flags: u32,
            token: HANDLE,
            path: *mut PWSTR,
        ) -> windows::core::HRESULT;
    }
    struct KnownFolderPath(PWSTR);
    impl Drop for KnownFolderPath {
        fn drop(&mut self) {
            unsafe {
                windows::Win32::System::Com::CoTaskMemFree(Some(self.0.0.cast()));
            }
        }
    }
    let mut path = KnownFolderPath(PWSTR::null());
    api("agent_installation_known_folder_unavailable", unsafe {
        known_folder_path(
            &FOLDERID_LocalAppData,
            // Retrieve first, then reject remote/device paths ourselves. Do
            // not let Shell verify/access a redirected network folder first.
            KF_FLAG_DONT_VERIFY.0 as u32,
            raw(&token),
            &mut path.0,
        )
        .ok()
    })?;
    if path.0.is_null() {
        return Err(fault(
            "agent_installation_known_folder_empty",
            "The OS did not return this interactive user's LocalAppData path.",
        ));
    }
    let mut length = 0;
    while length <= 32700 && unsafe { *path.0.0.add(length) } != 0 {
        length += 1;
    }
    if length == 0 || length > 32700 {
        return Err(fault(
            "agent_installation_known_folder_limit",
            "Known Folder path exceeded the explicit native UTF-16 budget.",
        ));
    }
    let text = String::from_utf16(unsafe { std::slice::from_raw_parts(path.0.0, length) })
        .map_err(|_| {
            fault(
                "agent_installation_known_folder_invalid",
                "Known Folder path is not valid Unicode.",
            )
        })?;
    validate_local_dos_path(&text)?;
    let drive = wide(&text[..3]);
    if unsafe { GetDriveTypeW(PCWSTR(drive.as_ptr())) } != 3 {
        return Err(fault(
            "agent_installation_not_local_fixed_drive",
            "Native installation requires OS LocalAppData on a local fixed drive.",
        ));
    }
    session.require_same(&current_session()?)?;
    let root = PathBuf::from(text)
        .join("RealComputerUse")
        .join("native-link-r1");
    path_text(&root)?;
    Ok(root)
}

fn creation_time(process: HANDLE) -> Result<u64> {
    let (mut created, mut exit, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    api("agent_process_epoch_unavailable", unsafe {
        GetProcessTimes(process, &mut created, &mut exit, &mut kernel, &mut user)
    })?;
    Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}

fn require_alive(process: HANDLE) -> Result<()> {
    match unsafe { WaitForSingleObject(process, 0) } {
        WAIT_TIMEOUT => Ok(()),
        WAIT_OBJECT_0 => Err(fault(
            "agent_owned_process_exited",
            "The retained process has exited; do not reconnect or reclaim its PID.",
        )),
        _ => Err(fault(
            "agent_owned_process_state_unknown",
            "The retained process state could not be read; keep HOLD.",
        )),
    }
}

fn image_path(process: HANDLE) -> Result<PathBuf> {
    let mut text = vec![0u16; 32768];
    let mut len = text.len() as u32;
    api("agent_process_image_unavailable", unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            PWSTR(text.as_mut_ptr()),
            &mut len,
        )
    })?;
    if len == 0 || len as usize >= text.len() {
        return Err(fault(
            "agent_process_image_invalid",
            "Process image path exceeds the bounded native buffer.",
        ));
    }
    let text = String::from_utf16(&text[..len as usize]).map_err(|_| {
        fault(
            "agent_process_image_invalid",
            "Process image path is not valid UTF-16.",
        )
    })?;
    validate_local_dos_path(&text)?;
    Ok(PathBuf::from(text))
}

fn process_record(process: HANDLE, image: &GuardedFile) -> Result<ProcessBinding> {
    require_alive(process)?;
    let pid = unsafe { GetProcessId(process) };
    let token = token_identity(process)?;
    let mut session = 0;
    api("agent_process_session_unavailable", unsafe {
        ProcessIdToSessionId(pid, &mut session)
    })?;
    if pid == 0
        || session != token.session
        || !path_text(&image_path(process)?)?.eq_ignore_ascii_case(&image.path)
    {
        return Err(fault(
            "agent_process_binding_changed",
            "The retained process path or native token/session no longer matches.",
        ));
    }
    image.verify()?;
    let record = ProcessBinding {
        pid,
        creation_filetime_100ns: creation_time(process)?,
        user_sid: token.sid,
        session_id: session,
        authentication_id: token.authentication,
        image_path: image.path.clone(),
        image_sha256: image.sha256.clone(),
    };
    require_alive(process)?;
    Ok(record)
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

/// # Safety
/// `raw_handle` must be the valid live process HANDLE borrowed directly from a
/// freshly spawned, still-owned Child. Never pass a handle reopened by old PID.
/// The original owner must keep its handle valid until this function returns.
pub unsafe fn retain_owned_process(
    raw_handle: RawHandle,
    expected_image: &Path,
) -> Result<OwnedProcess> {
    let mut duplicate = HANDLE::default();
    api("agent_owned_handle_duplicate_failed", unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            HANDLE(raw_handle),
            GetCurrentProcess(),
            &mut duplicate,
            (PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE).0,
            false,
            DUPLICATE_HANDLE_OPTIONS(0),
        )
    })?;
    let handle = owned(duplicate);
    let image = GuardedFile::open_image(expected_image)?;
    let record = process_record(raw(&handle), &image)?;
    let session = current_session()?;
    record.matches_session(&session)?;
    let privilege = process_privilege(raw(&handle))?;
    Ok(OwnedProcess {
        handle,
        image,
        record,
        session,
        privilege,
    })
}

impl OwnedProcess {
    pub fn record(&self) -> &ProcessBinding {
        &self.record
    }

    pub fn verify_current(&self, expected_session: &SessionBinding) -> Result<()> {
        self.session.require_same(expected_session)?;
        expected_session.require_same(&current_session()?)?;
        self.record.require_same(
            &process_record(raw(&self.handle), &self.image)?,
            expected_session,
        )?;
        self.privilege
            .require_same(&process_privilege(raw(&self.handle))?)
    }
    pub fn privilege(&self) -> &TokenPrivilegeFacts {
        &self.privilege
    }
    pub fn verify_privilege(&self, mode: PrivilegeMode) -> Result<()> {
        self.verify_current(&self.session)?;
        self.privilege.require_mode(mode)
    }

    pub fn verify_pipe_server(&self, pipe: &NamedPipeClient) -> Result<()> {
        require_alive(raw(&self.handle))?;
        let mut pid = 0;
        api("agent_pipe_server_identity_unavailable", unsafe {
            GetNamedPipeServerProcessId(raw(pipe), &mut pid)
        })?;
        if pid != self.record.pid || unsafe { GetProcessId(raw(&self.handle)) } != pid {
            return Err(fault(
                "agent_pipe_server_not_owned",
                "The pipe server is not the retained freshly owned worker.",
            ));
        }
        self.verify_current(&self.session)
    }

    /// Immediate retained-HANDLE observation only. Exit is not resource/goal proof.
    pub fn observed_exit_code(&self) -> Result<Option<u32>> {
        match unsafe { WaitForSingleObject(raw(&self.handle), 0) } {
            WAIT_TIMEOUT => Ok(None),
            WAIT_OBJECT_0 => {
                let mut code = 0;
                api("agent_owned_exit_unknown", unsafe {
                    GetExitCodeProcess(raw(&self.handle), &mut code)
                })?;
                Ok(Some(code))
            }
            _ => Err(fault(
                "agent_owned_exit_unknown",
                "The retained process state is unavailable; keep HOLD.",
            )),
        }
    }
}

pub fn current_owner_proof() -> Result<OwnerProof> {
    let session = current_session()?;
    let image = GuardedFile::open_image(&image_path(unsafe { GetCurrentProcess() })?)?;
    let process = process_record(unsafe { GetCurrentProcess() }, &image)?;
    let proof = OwnerProof { session, process };
    proof.validate()?;
    Ok(proof)
}

/// A read-only foreground token for the supervisor's focus-loss HOLD policy.
/// It does not activate/focus a window. This point observation is not a claim of
/// continuous focus; the original native backend still rechecks before input.
pub fn foreground_window_id() -> Result<String> {
    current_session()?;
    let window = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
    if window.0.is_null() {
        return Err(fault(
            "agent_foreground_unknown",
            "No current foreground window can be proven; hold dispatch.",
        ));
    }
    Ok(desktop::window_id(window))
}

fn window_identity(record: &crate::WindowRecord) -> Result<rcu_core::lifecycle::WindowIdentity> {
    Ok(rcu_core::lifecycle::WindowIdentity {
        window: record.window.clone(),
        pid: record.pid,
        process_created: record.process_created.ok_or_else(|| {
            fault(
                "agent_window_birth_unknown",
                "No process birth for this exact native HWND.",
            )
        })?,
        native_epoch: record.native_epoch.clone().ok_or_else(|| {
            fault(
                "agent_window_epoch_unknown",
                "No native epoch for this exact HWND.",
            )
        })?,
    })
}

fn thread_desktop_name(thread: u32) -> Result<String> {
    // GetThreadDesktop returns a borrowed handle; it must not be closed here.
    let desktop = api("agent_window_desktop_unavailable", unsafe {
        GetThreadDesktop(thread)
    })?;
    let mut text = [0u16; 256];
    let mut bytes = 0;
    api("agent_window_desktop_unavailable", unsafe {
        GetUserObjectInformationW(
            HANDLE(desktop.0),
            UOI_NAME,
            Some(text.as_mut_ptr().cast()),
            size_of::<[u16; 256]>() as u32,
            Some(&mut bytes),
        )
    })?;
    if bytes == 0 || bytes as usize > size_of::<[u16; 256]>() {
        return Err(fault(
            "agent_window_desktop_unknown",
            "Thread desktop name exceeded the native bound.",
        ));
    }
    let length = text.iter().position(|v| *v == 0).ok_or_else(|| {
        fault(
            "agent_window_desktop_unknown",
            "Thread desktop name is not terminated.",
        )
    })?;
    String::from_utf16(&text[..length]).map_err(|_| {
        fault(
            "agent_window_desktop_unknown",
            "Thread desktop name is not valid Unicode.",
        )
    })
}

/// A query/synchronize HANDLE acquired only from a currently verified HWND, not
/// a historic PID claim. This never owns, terminates or activates a user app.
struct WindowUserGuard {
    handle: OwnedHandle,
    expected: WindowUserBinding,
    session: SessionBinding,
}
impl WindowUserGuard {
    fn retain(record: &crate::WindowRecord, session: &SessionBinding) -> Result<Self> {
        session.require_same(&current_session()?)?;
        let w = desktop::hwnd(&record.window)?;
        let mut pid = 0;
        let thread = unsafe { GetWindowThreadProcessId(w, Some(&mut pid)) };
        if pid != record.pid || thread == 0 {
            return Err(fault(
                "agent_window_process_changed",
                "Fresh HWND/thread/PID did not match the selected native record.",
            ));
        }
        let handle = owned(api("agent_window_process_unavailable", unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                false,
                pid,
            )
        })?);
        let token = token_identity(raw(&handle))?;
        let expected = WindowUserBinding {
            window: window_identity(record)?,
            thread_id: thread,
            user_sid: token.sid,
            session_id: token.session,
            authentication_id: token.authentication,
            input_desktop: thread_desktop_name(thread)?,
        };
        expected.validate(session)?;
        let guard = Self {
            handle,
            expected,
            session: session.clone(),
        };
        guard.verify()?;
        Ok(guard)
    }
    fn verify(&self) -> Result<WindowUserBinding> {
        self.session.require_same(&current_session()?)?;
        require_alive(raw(&self.handle))?;
        let w = desktop::hwnd(&self.expected.window.window)?;
        let current = desktop::record(w, 0)?;
        let mut pid = 0;
        let thread = unsafe { GetWindowThreadProcessId(w, Some(&mut pid)) };
        let token = token_identity(raw(&self.handle))?;
        let mut os_session = 0;
        api("agent_window_session_unavailable", unsafe {
            ProcessIdToSessionId(pid, &mut os_session)
        })?;
        let observed = WindowUserBinding {
            window: window_identity(&current)?,
            thread_id: thread,
            user_sid: token.sid,
            session_id: token.session,
            authentication_id: token.authentication,
            input_desktop: thread_desktop_name(thread)?,
        };
        observed.validate(&self.session)?;
        if observed != self.expected
            || os_session != observed.session_id
            || unsafe { GetProcessId(raw(&self.handle)) } != pid
            || creation_time(raw(&self.handle))? != observed.window.process_created
        {
            return Err(fault(
                "agent_window_process_changed",
                "Retained HWND/process birth/thread/native user binding changed; never reclaim by PID.",
            ));
        }
        require_alive(raw(&self.handle))?;
        Ok(observed)
    }
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
impl AgentInputAuthority {
    pub(crate) fn new(
        session: SessionBinding,
        root: crate::WindowRecord,
        dispatch_check: Arc<dyn Fn() -> Result<()> + Send + Sync>,
    ) -> Result<Self> {
        let root_process = WindowUserGuard::retain(&root, &session)?;
        Ok(Self {
            session,
            root,
            root_process,
            root_runtime: OnceLock::new(),
            dispatch_check,
        })
    }
    pub(crate) fn root(&self) -> &rcu_core::lifecycle::WindowIdentity {
        &self.root_process.expected.window
    }
    pub(crate) fn pin_root_runtime(&self) -> Result<()> {
        let before = self.root_process.verify()?;
        let walk = crate::uia::walk(
            desktop::hwnd(&before.window.window)?,
            before.window.pid,
            &before.window.native_epoch,
        )?;
        self.root_process.verify()?;
        if let Some(runtime) = self.root_runtime.get() {
            if runtime != &walk.root_runtime_id {
                return Err(fault(
                    "agent_root_uia_changed",
                    "Allocated root UIA identity changed; do not replace the root.",
                ));
            }
        } else {
            self.root_runtime.set(walk.root_runtime_id).map_err(|_| {
                fault(
                    "agent_root_uia_changed",
                    "Allocated UIA root was concurrently replaced.",
                )
            })?;
        }
        Ok(())
    }
    pub(crate) fn verify_root_runtime(&self) -> Result<()> {
        (self.dispatch_check)()?;
        if self.root_runtime.get().is_none() {
            return Err(fault(
                "agent_initial_root_bind_required",
                "First discover and explicitly bind the allocated root before any owned-popup transition.",
            ));
        }
        self.pin_root_runtime()
    }
    pub(crate) fn retain_selection<'a>(
        &'a self,
        selected: &crate::WindowRecord,
    ) -> Result<AgentInputSelection<'a>> {
        let root = self.root_process.verify()?;
        let current_root = desktop::record(desktop::hwnd(&self.root.window)?, 0)?;
        if current_root.owner_chain != self.root.owner_chain
            || current_root.owner_chain_complete != self.root.owner_chain_complete
        {
            return Err(fault(
                "agent_root_owner_changed",
                "Allocated root native owner relation changed.",
            ));
        }
        let selected_process = WindowUserGuard::retain(selected, &self.session)?;
        let mut owners = Vec::new();
        if !selected.owner_chain_complete || selected.owner_chain.len() > 16 {
            return Err(fault(
                "agent_owner_chain_unknown",
                "Require a complete bounded native owner chain.",
            ));
        }
        for key in &selected.owner_chain {
            let record = desktop::record(desktop::hwnd(&key.window)?, 0)?;
            if record.key().as_ref() != Some(key) {
                return Err(fault(
                    "agent_owner_chain_changed",
                    "Owner HWND/PID/process birth changed during native observation.",
                ));
            }
            owners.push(WindowUserGuard::retain(&record, &self.session)?);
        }
        let selection = AgentInputSelection {
            authority: self,
            selected: selected.clone(),
            process: selected_process,
            owners,
        };
        let observed = selection.verify_native()?;
        if observed.0.window != root.window && !observed.1.iter().any(|owner| owner == &root) {
            return Err(fault(
                "agent_foreign_window",
                "Selected HWND is neither the fixed root nor proven native-owned by it.",
            ));
        }
        Ok(selection)
    }
}

pub(crate) struct AgentInputSelection<'a> {
    authority: &'a AgentInputAuthority,
    selected: crate::WindowRecord,
    process: WindowUserGuard,
    owners: Vec<WindowUserGuard>,
}
impl AgentInputSelection<'_> {
    pub(crate) fn verify_root_runtime(&self) -> Result<()> {
        self.authority.verify_root_runtime()
    }
    pub(crate) fn verify_native(&self) -> Result<(WindowUserBinding, Vec<WindowUserBinding>)> {
        (self.authority.dispatch_check)()?;
        self.authority.root_process.verify()?;
        let selected = self.process.verify()?;
        let current = desktop::record(desktop::hwnd(&self.selected.window)?, 0)?;
        if !current.foreground
            || !current.enabled
            || !current.visible
            || current.minimized
            || current.cloaked != Some(false)
            || current.owner_chain != self.selected.owner_chain
            || !current.owner_chain_complete
        {
            return Err(fault(
                "agent_selected_focus_changed",
                "Exact selected foreground/owner/visibility facts changed; no auto-refocus.",
            ));
        }
        let owners = self
            .owners
            .iter()
            .map(WindowUserGuard::verify)
            .collect::<Result<Vec<_>>>()?;
        if desktop::window_id(unsafe { GetForegroundWindow() }) != selected.window.window {
            return Err(fault(
                "agent_selected_focus_changed",
                "Foreground changed while native selected-user proof was checked.",
            ));
        }
        Ok((selected, owners))
    }
    pub(crate) fn proof(
        &self,
        selected_runtime: Vec<i32>,
        catalog_id: String,
    ) -> Result<SelectedWindowProof> {
        let (selected, owner_chain) = self.verify_native()?;
        let root_runtime = self.authority.root_runtime.get().ok_or_else(|| {
            fault(
                "agent_initial_root_bind_required",
                "No pinned allocated UIA root.",
            )
        })?;
        let proof = SelectedWindowProof {
            session: self.authority.session.clone(),
            allocated_root: self.authority.root_process.verify()?,
            foreground: selected.window.clone(),
            selected,
            owner_chain,
            owner_chain_complete: true,
            root_uia_runtime_id: root_runtime.clone(),
            selected_uia_runtime_id: selected_runtime,
            catalog_id,
        };
        proof.require_scope(&self.authority.session, self.authority.root(), root_runtime)?;
        Ok(proof)
    }
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

impl PeerProcess {
    pub(crate) fn from_pipe_client(pipe: &NamedPipeServer, expected: &OwnerProof) -> Result<Self> {
        expected.validate()?;
        expected.session.require_same(&current_session()?)?;
        let mut pid = 0;
        api("agent_pipe_client_identity_unavailable", unsafe {
            GetNamedPipeClientProcessId(raw(pipe), &mut pid)
        })?;
        if pid == 0 || pid != expected.process.pid {
            return Err(fault(
                "agent_pipe_client_unapproved",
                "Only the exact locally approved agent process may connect.",
            ));
        }
        let handle = owned(api("agent_pipe_client_handle_unavailable", unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                false,
                pid,
            )
        })?);
        let image = GuardedFile::open_image(Path::new(&expected.process.image_path))?;
        let privilege = process_privilege(raw(&handle))?;
        let peer = Self {
            handle,
            image,
            expected: expected.clone(),
            privilege,
        };
        peer.verify_pipe_client(pipe)?;
        Ok(peer)
    }

    pub fn record(&self) -> &ProcessBinding {
        &self.expected.process
    }

    pub fn verify_current(&self) -> Result<()> {
        self.expected.session.require_same(&current_session()?)?;
        self.expected.process.require_same(
            &process_record(raw(&self.handle), &self.image)?,
            &self.expected.session,
        )?;
        self.privilege
            .require_same(&process_privilege(raw(&self.handle))?)
    }
    pub fn privilege(&self) -> &TokenPrivilegeFacts {
        &self.privilege
    }
    pub fn verify_privilege(&self, mode: PrivilegeMode) -> Result<()> {
        self.verify_current()?;
        self.privilege.require_mode(mode)
    }

    pub fn verify_pipe_client(&self, pipe: &NamedPipeServer) -> Result<()> {
        self.verify_current()?;
        let mut pid = 0;
        api("agent_pipe_client_identity_unavailable", unsafe {
            GetNamedPipeClientProcessId(raw(pipe), &mut pid)
        })?;
        if pid != self.expected.process.pid || unsafe { GetProcessId(raw(&self.handle)) } != pid {
            return Err(fault(
                "agent_pipe_client_changed",
                "The pipe client no longer matches the retained query handle.",
            ));
        }
        Ok(())
    }
}

/// Protected, explicit one-user ACL. This changes only freshly created objects;
/// it never repairs or widens an existing object's permissions.
pub(crate) struct UserSecurity {
    pub(crate) attributes: SECURITY_ATTRIBUTES,
    _allocation: Local,
}

impl UserSecurity {
    pub(crate) fn new(sid: &str, directory: bool) -> Result<Self> {
        validate_user_sid(sid)?;
        let flags = if directory { "OICI" } else { "" };
        let text = format!("O:{sid}G:{sid}D:P(A;{flags};FA;;;{sid})");
        let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        api("agent_private_acl_create_failed", unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(text.as_ptr()),
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
            _allocation: Local(HLOCAL(descriptor.0)),
        })
    }
}

fn path_text(path: &Path) -> Result<String> {
    let text = path.to_str().ok_or_else(|| {
        fault(
            "agent_local_path_invalid",
            "The local path is not valid Unicode.",
        )
    })?;
    validate_local_dos_path(text)?;
    Ok(text.into())
}

fn wide(path: &str) -> Vec<u16> {
    path.encode_utf16().chain(Some(0)).collect()
}

fn file_info(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION> {
    if unsafe { GetFileType(raw(file)) } != FILE_TYPE_DISK {
        return Err(fault(
            "agent_private_not_disk",
            "Only a bounded local disk file/directory is permitted.",
        ));
    }
    let mut tag = FILE_ATTRIBUTE_TAG_INFO::default();
    api("agent_private_attributes_unavailable", unsafe {
        GetFileInformationByHandleEx(
            raw(file),
            FileAttributeTagInfo,
            (&mut tag as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
            size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    })?;
    if tag.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 || tag.ReparseTag != 0 {
        return Err(fault(
            "agent_private_reparse_refused",
            "Reparse points are refused by opened-handle inspection.",
        ));
    }
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    api("agent_private_identity_unavailable", unsafe {
        GetFileInformationByHandle(raw(file), &mut info)
    })?;
    Ok(info)
}

fn final_path(file: &File) -> Result<String> {
    let mut text = vec![0u16; 32768];
    let count = unsafe {
        GetFinalPathNameByHandleW(
            raw(file),
            &mut text,
            GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_DOS.0),
        )
    };
    if count == 0 || count as usize >= text.len() {
        return Err(fault(
            "agent_private_final_path_unknown",
            "Opened-handle final path could not be resolved within budget.",
        ));
    }
    let text = String::from_utf16(&text[..count as usize]).map_err(|_| {
        fault(
            "agent_private_final_path_invalid",
            "Opened-handle final path is not valid Unicode.",
        )
    })?;
    Ok(text.strip_prefix(r"\\?\").unwrap_or(&text).into())
}

fn check_open_path(
    file: &File,
    expected: &str,
    directory: bool,
) -> Result<BY_HANDLE_FILE_INFORMATION> {
    let info = file_info(file)?;
    if (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0) != directory
        || !final_path(file)?.eq_ignore_ascii_case(expected)
        || (!directory && info.nNumberOfLinks != 1)
    {
        return Err(fault(
            "agent_private_path_changed",
            "Opened handle type/final path/link count disagrees with the approved literal path.",
        ));
    }
    Ok(info)
}

fn open_literal(path: &str, directory: bool) -> Result<File> {
    let text = wide(path);
    let flags = FILE_FLAG_OPEN_REPARSE_POINT
        | if directory {
            FILE_FLAG_BACKUP_SEMANTICS
        } else {
            FILE_FLAGS_AND_ATTRIBUTES(0)
        };
    // No FILE_SHARE_WRITE or FILE_SHARE_DELETE: retained ancestors and file
    // handles prevent replacement/reparse changes during the operation.
    let handle = api("agent_private_open_failed", unsafe {
        CreateFileW(
            PCWSTR(text.as_ptr()),
            GENERIC_READ.0,
            FILE_SHARE_READ,
            None,
            OPEN_EXISTING,
            flags,
            None,
        )
    })?;
    let file = File::from(owned(handle));
    check_open_path(&file, path, directory)?;
    Ok(file)
}

struct PathGuard {
    _ancestors: Vec<File>,
}
impl PathGuard {
    fn parents(path: &str) -> Result<Self> {
        validate_local_dos_path(path)?;
        let mut prefix = path[..3].to_string();
        // DRIVE_FIXED == 3. Reject mapped/network/unknown drives before any
        // CreateFile call, rather than learning about UNC after opening it.
        let root = wide(&prefix);
        if unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) } != 3 {
            return Err(fault(
                "agent_private_local_fixed_drive_required",
                "Only a local fixed drive is accepted; mapped/remote/unknown drives are refused before opening files.",
            ));
        }
        let mut ancestors = vec![open_literal(&prefix, true)?];
        let parts: Vec<_> = path[3..].split('\\').collect();
        for name in &parts[..parts.len() - 1] {
            if !prefix.ends_with('\\') {
                prefix.push('\\');
            }
            prefix.push_str(name);
            ancestors.push(open_literal(&prefix, true)?);
        }
        Ok(Self {
            _ancestors: ancestors,
        })
    }
}

fn check_private_acl(file: &File, sid: &str, directory: bool) -> Result<()> {
    check_private_acl_scope(
        file,
        sid,
        if directory {
            PrivateAclScope::Directory
        } else {
            PrivateAclScope::StandaloneFile
        },
    )
}

fn check_private_acl_scope(file: &File, sid: &str, scope: PrivateAclScope) -> Result<()> {
    let mut owner = PSID::default();
    let mut dacl = std::ptr::null_mut();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    api("agent_private_acl_unavailable", unsafe {
        GetSecurityInfo(
            raw(file),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            Some(&mut owner),
            None,
            Some(&mut dacl),
            None,
            Some(&mut descriptor),
        )
        .ok()
    })?;
    let _allocation = Local(HLOCAL(descriptor.0));
    let mut control = 0;
    let mut revision = 0;
    api("agent_private_acl_unavailable", unsafe {
        GetSecurityDescriptorControl(descriptor, &mut control, &mut revision)
    })?;
    if owner.0.is_null()
        || dacl.is_null()
        || !unsafe { IsValidAcl(dacl) }.as_bool()
        || unsafe { (*dacl).AceCount } != 1
    {
        return Err(fault(
            "agent_private_acl_not_user_only",
            "Existing object must have a valid owner SID and exactly one user ACE; it is never repaired in place.",
        ));
    }
    let mut ace = std::ptr::null_mut::<c_void>();
    api("agent_private_acl_unavailable", unsafe {
        GetAce(dacl, 0, &mut ace)
    })?;
    if ace.is_null() {
        return Err(fault("agent_private_acl_invalid", "Missing sole user ACE."));
    }
    let header = unsafe { &*ace.cast::<ACE_HEADER>() };
    // ACCESS_ALLOWED_ACE_TYPE == 0. Never reinterpret object/callback ACE layouts.
    if header.AceType != 0 || header.AceSize < size_of::<ACCESS_ALLOWED_ACE>() as u16 {
        return Err(fault(
            "agent_private_acl_not_user_only",
            "Require a simple user-only access-allowed ACE; object/callback ACEs are refused.",
        ));
    }
    let ace = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
    let ace_sid = PSID((&ace.SidStart as *const u32).cast_mut().cast());
    PrivateAclEvidence {
        owner_sid: sid_string(owner)?,
        dacl_protected: control & SE_DACL_PROTECTED.0 != 0,
        ace_count: unsafe { (*dacl).AceCount },
        access_allowed: true,
        ace_flags: header.AceFlags,
        ace_mask: ace.Mask,
        ace_sid: sid_string(ace_sid)?,
    }
    .validate(sid, scope)
}

fn read_file(file: &File, cap: usize) -> Result<Vec<u8>> {
    let info = file_info(file)?;
    let len = (u64::from(info.nFileSizeHigh) << 32) | u64::from(info.nFileSizeLow);
    if cap == 0 || cap > IMAGE_CAP || len > cap as u64 {
        return Err(fault(
            "agent_private_file_budget",
            "File length exceeds the explicit bounded read budget.",
        ));
    }
    let mut bytes = vec![0u8; len as usize];
    let mut offset = 0;
    while offset < bytes.len() {
        let n = file.seek_read(&mut bytes[offset..], offset as u64)?;
        if n == 0 {
            return Err(fault(
                "agent_private_file_changed",
                "File became incomplete during its bounded read.",
            ));
        }
        offset += n;
    }
    let after = file_info(file)?;
    if after.nFileSizeHigh != info.nFileSizeHigh
        || after.nFileSizeLow != info.nFileSizeLow
        || after.ftLastWriteTime.dwHighDateTime != info.ftLastWriteTime.dwHighDateTime
        || after.ftLastWriteTime.dwLowDateTime != info.ftLastWriteTime.dwLowDateTime
    {
        return Err(fault(
            "agent_private_file_changed",
            "Opened file metadata changed during its bounded read.",
        ));
    }
    Ok(bytes)
}

struct GuardedFile {
    _parents: PathGuard,
    file: File,
    path: String,
    sha256: String,
    identity: (u32, u32, u32, u32, u32),
}
impl GuardedFile {
    fn open_image(path: &Path) -> Result<Self> {
        let path = path_text(path)?;
        let parents = PathGuard::parents(&path)?;
        let file = open_literal(&path, false)?;
        let info = file_info(&file)?;
        let bytes = read_file(&file, IMAGE_CAP)?;
        Ok(Self {
            _parents: parents,
            file,
            path,
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            identity: (
                info.dwVolumeSerialNumber,
                info.nFileIndexHigh,
                info.nFileIndexLow,
                info.ftLastWriteTime.dwHighDateTime,
                info.ftLastWriteTime.dwLowDateTime,
            ),
        })
    }
    fn verify(&self) -> Result<()> {
        let info = check_open_path(&self.file, &self.path, false)?;
        let observed = (
            info.dwVolumeSerialNumber,
            info.nFileIndexHigh,
            info.nFileIndexLow,
            info.ftLastWriteTime.dwHighDateTime,
            info.ftLastWriteTime.dwLowDateTime,
        );
        if observed != self.identity {
            return Err(fault(
                "agent_image_changed",
                "Pinned image handle identity changed; keep HOLD.",
            ));
        }
        Ok(())
    }
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
impl InstallationLock {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

pub fn acquire_installation_lock(directory: &PrivateDirectory) -> Result<InstallationLock> {
    let fixed = fixed_agent_installation_root()?;
    if directory.path() != fixed {
        return Err(fault(
            "agent_installation_lock_wrong_root",
            "Installation locking only uses this token's fixed OS Known Folder root, never another task/state/config directory.",
        ));
    }
    let path = directory.child("installation.lock")?;
    // Pin independently so dropping the caller's Directory cannot release our
    // path ancestry. Duplicates refer to the already verified open objects.
    let root = directory.directory.try_clone()?;
    let ancestors = directory
        ._parents
        ._ancestors
        .iter()
        .map(File::try_clone)
        .collect::<std::io::Result<Vec<_>>>()?;
    let security = UserSecurity::new(&directory.user_sid, false)?;
    let text = wide(&path);
    let handle = match unsafe {
        CreateFileW(
            PCWSTR(text.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_MODE(0),
            Some(&security.attributes),
            OPEN_ALWAYS,
            FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    } {
        Ok(handle) => handle,
        Err(error)
            if error.code() == windows::core::HRESULT::from_win32(ERROR_SHARING_VIOLATION.0)
                || error.code() == windows::core::HRESULT::from_win32(ERROR_LOCK_VIOLATION.0) =>
        {
            return Err(fault(
                "agent_installation_lock_busy",
                "The fixed installation lock is already open incompatibly; do not start/approve another agent or infer ownership from its filename.",
            ));
        }
        Err(error) => return api("agent_installation_lock_unavailable", Err(error)),
    };
    let file = File::from(owned(handle));
    check_open_path(&file, &path, false)?;
    check_private_acl(&file, &directory.user_sid, false)?;
    directory.child("installation.lock")?;
    Ok(InstallationLock {
        _file: file,
        _directory: root,
        _ancestors: ancestors,
        path: PathBuf::from(path),
    })
}

impl PrivateDirectory {
    pub fn create(path: &Path) -> Result<Self> {
        let path = path_text(path)?;
        let parents = PathGuard::parents(&path)?;
        let sid = token_identity(unsafe { GetCurrentProcess() })?.sid;
        let security = UserSecurity::new(&sid, true)?;
        let text = wide(&path);
        api("agent_private_directory_create_failed", unsafe {
            CreateDirectoryW(PCWSTR(text.as_ptr()), Some(&security.attributes))
        })?;
        let directory = open_literal(&path, true)?;
        check_private_acl(&directory, &sid, true)?;
        Ok(Self {
            _parents: parents,
            directory,
            path,
            user_sid: sid,
        })
    }

    pub fn open(path: &Path) -> Result<Self> {
        let path = path_text(path)?;
        let parents = PathGuard::parents(&path)?;
        let sid = token_identity(unsafe { GetCurrentProcess() })?.sid;
        let directory = open_literal(&path, true)?;
        check_private_acl(&directory, &sid, true)?;
        Ok(Self {
            _parents: parents,
            directory,
            path,
            user_sid: sid,
        })
    }

    pub fn path(&self) -> &Path {
        Path::new(&self.path)
    }

    fn child(&self, name: &str) -> Result<String> {
        validate_private_name(name)?;
        check_open_path(&self.directory, &self.path, true)?;
        check_private_acl(&self.directory, &self.user_sid, true)?;
        if token_identity(unsafe { GetCurrentProcess() })?.sid != self.user_sid {
            return Err(fault(
                "agent_private_owner_changed",
                "The current user no longer owns the private directory.",
            ));
        }
        Ok(format!("{}\\{name}", self.path))
    }

    /// Oracle-only reader: a file may retain the strict explicit user ACL or
    /// inherit exactly one full-access ACE for the same user. This exception is
    /// available only under this retained, protected private directory. It does
    /// not relax standalone trust/config/owner readers or repair any ACL.
    pub fn read(&self, name: &str, cap: usize) -> Result<Vec<u8>> {
        if cap == 0 || cap > PRIVATE_CAP {
            return Err(fault(
                "agent_private_file_budget",
                "Private oracle reads require an explicit 1..64 MiB cap.",
            ));
        }
        let path = self.child(name)?;
        let file = open_literal(&path, false)?;
        check_private_acl_scope(
            &file,
            &self.user_sid,
            PrivateAclScope::FileInPinnedPrivateDirectory,
        )?;
        let bytes = read_file(&file, cap)?;
        self.child(name)?;
        check_open_path(&file, &path, false)?;
        check_private_acl_scope(
            &file,
            &self.user_sid,
            PrivateAclScope::FileInPinnedPrivateDirectory,
        )?;
        Ok(bytes)
    }

    pub fn create_new(&self, name: &str, bytes: &[u8], cap: usize) -> Result<()> {
        if cap == 0 || cap > PRIVATE_CAP || bytes.len() > cap {
            return Err(fault(
                "agent_private_file_budget",
                "New file exceeds the explicit byte budget.",
            ));
        }
        let path = self.child(name)?;
        let text = wide(&path);
        let security = UserSecurity::new(&self.user_sid, false)?;
        let handle = api("agent_private_create_new_failed", unsafe {
            CreateFileW(
                PCWSTR(text.as_ptr()),
                GENERIC_READ.0 | GENERIC_WRITE.0,
                FILE_SHARE_READ,
                Some(&security.attributes),
                CREATE_NEW,
                FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        })?;
        let mut file = File::from(owned(handle));
        check_open_path(&file, &path, false)?;
        check_private_acl(&file, &self.user_sid, false)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        if read_file(&file, cap)? != bytes {
            return Err(fault(
                "agent_private_create_readback_failed",
                "New file readback differed; preserve the partial file and HOLD.",
            ));
        }
        Ok(())
    }
}

/// Private trust/manifest reader for the TLS crate's `load_with` callback. The
/// root must still select the exact approved path; this is not a model tool.
pub fn read_private_file_no_reparse(path: &Path, cap: usize) -> Result<Vec<u8>> {
    if cap == 0 || cap > PRIVATE_CAP {
        return Err(fault(
            "agent_private_file_budget",
            "Private reads require an explicit 1..64 MiB cap.",
        ));
    }
    let path = path_text(path)?;
    let _parents = PathGuard::parents(&path)?;
    let file = open_literal(&path, false)?;
    let sid = token_identity(unsafe { GetCurrentProcess() })?.sid;
    check_private_acl(&file, &sid, false)?;
    let bytes = read_file(&file, cap)?;
    check_open_path(&file, &path, false)?;
    check_private_acl(&file, &sid, false)?;
    Ok(bytes)
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
