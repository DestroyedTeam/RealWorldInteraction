//! Native window/user allocation and selected-input guards; policy unchanged.
use super::*;

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
