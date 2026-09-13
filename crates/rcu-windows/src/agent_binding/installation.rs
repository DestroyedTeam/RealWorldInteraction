//! Fixed installation root and lifetime locking; exact original native operations.
use super::*;

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
