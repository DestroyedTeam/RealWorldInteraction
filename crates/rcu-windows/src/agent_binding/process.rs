//! Retained owned/peer process implementations; no PID ownership recovery.
use super::*;

pub(super) fn creation_time(process: HANDLE) -> Result<u64> {
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

pub(super) fn require_alive(process: HANDLE) -> Result<()> {
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

pub(super) fn image_path(process: HANDLE) -> Result<PathBuf> {
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
