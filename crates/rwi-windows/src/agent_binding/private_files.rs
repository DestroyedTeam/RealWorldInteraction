//! Private directory/image implementations; no overwrite or permission repair.
use super::*;

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

impl GuardedFile {
    pub(super) fn open_image(path: &Path) -> Result<Self> {
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
    pub(super) fn verify(&self) -> Result<()> {
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

    pub(super) fn child(&self, name: &str) -> Result<String> {
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
