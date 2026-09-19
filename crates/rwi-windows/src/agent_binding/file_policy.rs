//! Literal path, retained-handle, bounded-read and ACL verification primitives.
use super::*;

pub(super) fn path_text(path: &Path) -> Result<String> {
    let text = path.to_str().ok_or_else(|| {
        fault(
            "agent_local_path_invalid",
            "The local path is not valid Unicode.",
        )
    })?;
    validate_local_dos_path(text)?;
    Ok(text.into())
}

pub(super) fn wide(path: &str) -> Vec<u16> {
    path.encode_utf16().chain(Some(0)).collect()
}

pub(super) fn file_info(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION> {
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

pub(super) fn check_open_path(
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

pub(super) fn open_literal(path: &str, directory: bool) -> Result<File> {
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

impl PathGuard {
    pub(super) fn parents(path: &str) -> Result<Self> {
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

pub(super) fn check_private_acl(file: &File, sid: &str, directory: bool) -> Result<()> {
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

pub(super) fn check_private_acl_scope(
    file: &File,
    sid: &str,
    scope: PrivateAclScope,
) -> Result<()> {
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

pub(super) fn read_file(file: &File, cap: usize) -> Result<Vec<u8>> {
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
