use super::*;
use rwi_windows::{
    InputGrant, NativeWindows,
    agent_binding::{PeerProcess, PrivateDirectory, current_session, read_private_file_no_reparse},
};
use std::{path::PathBuf, sync::OnceLock};

static GUARD: OnceLock<Arc<WindowsGuard>> = OnceLock::new();
pub struct WindowsGuard {
    spec: WorkerGuardSpec,
    gate: GuardGate,
    source_sha256: String,
    _directory: PrivateDirectory,
    peer: OnceLock<Arc<PeerProcess>>,
    model_ready: OnceLock<(String, Budget)>,
}
impl WindowsGuard {
    pub fn load(path: &Path) -> Result<Arc<Self>> {
        let bytes = read_private_file_no_reparse(path, 65536)?;
        let spec: WorkerGuardSpec = serde_json::from_slice(&bytes).map_err(|_| fault("agent_worker_spec_decode", "Worker guard uses the exact locally approved schema; no unknown fields or remote replacement."))?;
        validate_spec(
            &spec,
            path.to_str().ok_or_else(|| {
                fault(
                    "agent_worker_guard_path_invalid",
                    "Guard path is not valid Unicode.",
                )
            })?,
        )?;
        spec.owner.session.require_same(&current_session()?)?;
        let directory = PrivateDirectory::open(path.parent().ok_or_else(|| {
            fault(
                "agent_worker_guard_path_invalid",
                "Guard must live under a private run directory.",
            )
        })?)?;
        let budget = Budget::start(spec.budget.clone())
            .map_err(|e| fault("agent_worker_budget_invalid", &e.to_string()))?;
        let gate = GuardGate::new(spec.stage, budget);
        let guard = Arc::new(Self {
            spec,
            gate,
            source_sha256: rwi_core::digest(&bytes),
            _directory: directory,
            peer: OnceLock::new(),
            model_ready: OnceLock::new(),
        });
        guard.check_before_peer()?;
        Ok(guard)
    }
    pub fn owner(&self) -> &rwi_windows::agent_contract::OwnerProof {
        &self.spec.owner
    }
    pub fn connection_budget(&self) -> Duration {
        self.gate.budget.remaining().min(Duration::from_secs(120))
    }
    fn check_before_peer(&self) -> Result<()> {
        rwi_windows::agent_binding::require_current_privilege(self.spec.privilege_mode)?;
        self.gate
            .budget
            .check()
            .map_err(|e| fault("agent_worker_budget_hold", &e.to_string()))?;
        match std::fs::symlink_metadata(&self.spec.input_blocked_path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => {
                return Err(fault(
                    "agent_worker_revocation_hold",
                    "Revocation marker exists or its absence cannot be proven; do not dispatch.",
                ));
            }
        }
        self.spec.owner.session.require_same(&current_session()?)
    }
    pub fn bind_peer(&self, peer: PeerProcess) -> Result<()> {
        self.check_before_peer()?;
        peer.verify_current()?;
        peer.verify_privilege(self.spec.privilege_mode)?;
        if peer.record() != &self.spec.owner.process {
            return Err(fault(
                "agent_worker_owner_mismatch",
                "Connected pipe client differs from the exact local owner proof.",
            ));
        }
        self.peer.set(Arc::new(peer)).map_err(|_| {
            fault(
                "agent_worker_peer_already_bound",
                "The native worker permits exactly one pipe owner; no reconnect.",
            )
        })?;
        self.check(GuardOperation::Read)
    }
    pub fn constrain_grants(
        &self,
        input: &mut Option<InputGrant>,
        lifecycle: &mut Option<LifecycleGrant>,
    ) -> Result<()> {
        if self.spec.stage == Stage::Model {
            let grant = input.as_ref().ok_or_else(|| {
                fault(
                    "agent_worker_input_grant_required",
                    "Model stage requires its separate original native input grant.",
                )
            })?;
            if Some(&grant.root) != self.spec.expected_foreground.as_ref()
                || grant.expected_identity != self.spec.owner.session.native
            {
                return Err(fault(
                    "agent_worker_input_scope_mismatch",
                    "The native input grant and guard must bind the same root and native session.",
                ));
            }
        } else if input.is_some() {
            return Err(fault(
                "agent_worker_input_wrong_stage",
                "Only Model stage can carry an input grant.",
            ));
        }
        // Narrow only: original per-gesture and lifecycle checks retain a
        // monotonic deadline no later than this worker's remaining budget.
        let expires = now_ms()
            .saturating_add(self.gate.budget.remaining().as_millis() as u64)
            .min(self.spec.budget.expires_unix_ms);
        if let Some(grant) = input {
            grant.expires_unix_ms = grant.expires_unix_ms.min(expires);
        }
        if let Some(grant) = lifecycle {
            if grant.expected_identity != self.spec.owner.session.native {
                return Err(fault(
                    "agent_worker_lifecycle_scope_mismatch",
                    "Lifecycle and worker guard native identities differ.",
                ));
            }
            grant.expires_unix_ms = grant.expires_unix_ms.min(expires);
        }
        Ok(())
    }
}
impl WindowsGuard {
    fn check_model_ready(&self) -> Result<()> {
        let path = self.spec.model_ready_path.as_ref().ok_or_else(|| {
            fault(
                "agent_worker_model_not_ready",
                "Model has no private ready record; no input before MCP handshake completes.",
            )
        })?;
        let bytes = read_private_file_no_reparse(path, 16384)?;
        let hash = rwi_core::digest(&bytes);
        if let Some((expected, budget)) = self.model_ready.get() {
            if expected != &hash {
                return Err(fault(
                    "agent_worker_ready_changed",
                    "Ready authority changed after first observation; never renew or replace the model budget.",
                ));
            }
            return budget
                .check()
                .map_err(|e| fault("agent_worker_ready_budget_hold", &e.to_string()));
        }
        let ready: ModelReadySpec = serde_json::from_slice(&bytes).map_err(|_| {
            fault(
                "agent_worker_ready_decode",
                "Private ready record must use the exact strict schema.",
            )
        })?;
        validate_model_ready(&self.spec, &self.source_sha256, &ready, now_ms())?;
        // Absolute expires-now limits a late first read; it cannot grant a
        // fresh full duration merely because no input was attempted yet.
        let budget = Budget::start(ready.budget)
            .map_err(|e| fault("agent_worker_ready_budget_hold", &e.to_string()))?;
        let _ = self.model_ready.set((hash.clone(), budget));
        let (expected, budget) = self.model_ready.get().ok_or_else(|| {
            fault(
                "agent_worker_ready_unknown",
                "Could not retain immutable model ready budget.",
            )
        })?;
        if expected != &hash {
            return Err(fault(
                "agent_worker_ready_changed",
                "Concurrent ready observations differed.",
            ));
        }
        budget
            .check()
            .map_err(|e| fault("agent_worker_ready_budget_hold", &e.to_string()))
    }
}
impl GuardChecks for WindowsGuard {
    fn check(&self, operation: GuardOperation) -> Result<()> {
        self.gate.check(operation, || {
            self.check_before_peer()?;
            self.peer
                .get()
                .ok_or_else(|| {
                    fault(
                        "agent_worker_peer_not_bound",
                        "No original native call is allowed before owned pipe binding.",
                    )
                })?
                .verify_current()?;
            // HWND root equality is not a selected-window proof: a legitimate
            if operation == GuardOperation::Input {
                self.check_model_ready()?;
            }
            // owned dialog has a different HWND. The actual NativeWindows
            // callback and its per-gesture checks validate that exact scope.
            Ok(())
        })
    }
    fn hold(&self) {
        self.gate.hold();
    }
    fn deadline(&self) -> Instant {
        self.gate.budget.deadline()
    }
    fn metadata(&self) -> Value {
        json!({"version":1,"task":self.spec.task,"stage":self.spec.stage,"request_prefix":self.spec.request_prefix,"expected_ui_goal":self.spec.expected_ui_value,"guard_sha256":self.source_sha256,"held":self.gate.held.load(Ordering::Acquire),"pipe_owner_bound":self.peer.get().is_some(),"model_foreground_policy":"immutable allocated root; explicit fresh native selected root/owned-popup/return, full owner chain, token/session/desktop and UIA proof; per-balanced-gesture revalidation","native_runtime_verified":false})
    }
}
pub fn install(guard: Arc<WindowsGuard>) -> Result<()> {
    GUARD.set(guard).map_err(|_| {
        fault(
            "agent_worker_guard_already_installed",
            "Guard authority is immutable for this single worker process.",
        )
    })
}
pub fn installed() -> Option<&'static Arc<WindowsGuard>> {
    GUARD.get()
}
pub fn wrap(
    mut inner: NativeWindows,
    guard: Arc<WindowsGuard>,
) -> Result<GuardedBackend<NativeWindows>> {
    if guard.spec.stage == Stage::Model {
        let dispatch_guard = guard.clone();
        inner.configure_agent_input_scope(
            guard.spec.owner.session.clone(),
            guard.spec.expected_root_native_epoch.clone(),
            Arc::new(move || dispatch_guard.check(GuardOperation::Input)),
        )?;
        Ok(GuardedBackend::new(inner, guard)
            .with_input_proof(NativeWindows::fresh_agent_selected_proof))
    } else {
        Ok(GuardedBackend::new(inner, guard))
    }
}
pub fn private_grant(path: PathBuf, cap: usize) -> Result<Vec<u8>> {
    read_private_file_no_reparse(&path, cap)
}
