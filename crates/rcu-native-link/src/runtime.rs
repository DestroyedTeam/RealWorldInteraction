#[cfg(windows)]
use crate::private::core_error;
use crate::{
    budget::{Budget, BudgetSpec, ModelAdmission},
    invalid,
    private::{Directory, read_private},
    sha,
    store::{EndProof, Store},
    types::{AgentConfig, Approval, Stage},
    worker::{Registry, Worker},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

pub struct Runtime {
    _installation_lease: crate::installation::Lease,
    pub config: AgentConfig,
    pub approval: Approval,
    pub budget: Option<Budget>,
    pub store: Mutex<Store>,
    pub grants: Directory,
    pub responses: Directory,
    pub run_parent: Directory,
    pub files_path: PathBuf,
    pub native_state: Directory,
    pub live: Registry,
    pub cancelled: AtomicBool,
    model_admission: Mutex<Option<ModelAdmission>>,
    completion_budget: Mutex<Option<(Stage, Budget)>>,
}
fn open_or_create(path: &Path) -> io::Result<Directory> {
    if path.exists() {
        Directory::open(path)
    } else {
        Directory::create(path)
    }
}
impl Runtime {
    pub fn load(path: &Path) -> io::Result<Arc<Self>> {
        let cfg: AgentConfig = serde_json::from_slice(&read_private(path, 65536)?)?;
        Self::new(cfg)
    }
    pub fn new(config: AgentConfig) -> io::Result<Arc<Self>> {
        if config.version != 1
            || config.state_root != config.install_root.join("state")
            || config.approval != config.install_root.join("approved-task.json")
            || config.controller_peer_sha256 == config.model_peer_sha256
            || config.bind.ip().is_unspecified()
            || config.bind.ip().is_multicast()
        {
            return Err(invalid("fixed_installation_config_boundary"));
        }
        let install = Directory::open(&config.install_root)?;
        let installation_lease = crate::installation::Lease::acquire(&install)?;
        if config.backend.is_windows() {
            #[cfg(windows)]
            {
                let fixed = rcu_windows::agent_binding::fixed_agent_installation_root()
                    .map_err(core_error)?;
                let trust = rcu_link_trust::IdentityPaths::in_profile(&fixed.join("trust/agent"));
                if config.install_root != fixed
                    || config.agent_image.path != fixed.join("bin/rcu-native-agent.exe")
                    || config.worker_image.path != fixed.join("bin/rcu-mcp.exe")
                    || config.trust.ca != trust.ca
                    || config.trust.certificate != trust.certificate
                    || config.trust.private_key != trust.private_key
                {
                    return Err(invalid("fixed_native_installation_anchor_no_root_override"));
                }
            }
            #[cfg(not(windows))]
            return Err(invalid("native_runtime_requires_windows_user_agent"));
        }
        let approval: Approval =
            serde_json::from_slice(&read_private(&config.approval, 256 * 1024)?)?;
        approval.validate(config.backend)?;
        let state = open_or_create(&config.state_root)?;
        let grants = open_or_create(&state.path().join("grants"))?;
        let responses = open_or_create(&state.path().join("responses"))?;
        let run_parent = open_or_create(&state.path().join("runs"))?;
        let native_state = open_or_create(&state.path().join("native-state"))?;
        let store = Store::open(&state.path().join("journal"))?;
        let files_path = run_parent.path().join(&approval.task);
        let budget = Budget::start(approval.budget.clone()).ok();
        Ok(Arc::new(Self {
            _installation_lease: installation_lease,
            config,
            approval,
            budget,
            store: Mutex::new(store),
            grants,
            responses,
            run_parent,
            files_path,
            native_state,
            live: Arc::new(Mutex::new(Vec::new())),
            cancelled: AtomicBool::new(false),
            model_admission: Mutex::new(None),
            completion_budget: Mutex::new(None),
        }))
    }
    pub fn journal(&self) -> io::Result<MutexGuard<'_, Store>> {
        self.store
            .lock()
            .map_err(|_| invalid("journal_lock_poisoned_hold"))
    }
    pub fn live_budget(&self) -> io::Result<&Budget> {
        let b = self
            .budget
            .as_ref()
            .ok_or_else(|| invalid("expired_approval_readonly_only"))?;
        b.check()?;
        Ok(b)
    }
    pub fn check_native(&self) -> io::Result<()> {
        if self.config.backend.is_windows() {
            #[cfg(windows)]
            {
                let current = rcu_windows::agent_binding::current_session().map_err(core_error)?;
                self.approval
                    .native_binding
                    .as_ref()
                    .ok_or_else(|| invalid("native_binding_missing"))?
                    .require_same(&current)
                    .map_err(core_error)?;
            }
            #[cfg(not(windows))]
            {
                return Err(invalid("native_execution_requires_windows"));
            }
        }
        Ok(())
    }
    pub fn dispatch_allowed(&self) -> io::Result<()> {
        let budget = self.live_budget()?;
        self.check_native()?;
        if self.config.backend.is_windows()
            && budget.remaining() <= std::time::Duration::from_secs(600)
        {
            return Err(invalid(
                "native_last_ten_minutes_reserved_for_oracle_cleanup",
            ));
        }
        if self.cancelled.load(Ordering::SeqCst)
            || self.journal()?.state.as_ref().is_some_and(|s| s.hold)
        {
            return Err(invalid("task_cancelled_or_held"));
        }
        Ok(())
    }
    pub fn hold(&self, reason: &str) -> io::Result<()> {
        self.cancelled.store(true, Ordering::SeqCst);
        let bytes = serde_json::to_vec(&json!({"held":true,"reason":reason}))?;
        let name = format!("blocked-{}.json", self.approval.task);
        if !self.grants.path().join(&name).exists() {
            let _ = self.grants.create_new(&name, &bytes, 4096);
        }
        self.journal()?.hold(reason)
    }
    pub fn begin(&self, stage: Stage, previous: &str) -> io::Result<()> {
        if matches!(stage, Stage::Oracle | Stage::Cleanup) {
            self.live_budget()?;
            self.check_native()?;
        } else {
            self.dispatch_allowed()?;
        }
        let completion = match stage {
            Stage::Oracle => Some(Budget::start(self.live_budget()?.exact_spec(
                crate::budget::MODEL_ORACLE_MS,
                crate::budget::MODEL_CLEANUP_MS,
            )?)?),
            Stage::Cleanup => Some(Budget::start(
                self.live_budget()?
                    .exact_spec(crate::budget::MODEL_CLEANUP_MS, 0)?,
            )?),
            _ => None,
        };
        if stage == Stage::Model {
            // No intent, child, or forwarded byte exists if the full usable
            // interval and fixed setup/end/oracle/cleanup reserves do not fit.
            let admission = ModelAdmission::reserve(self.live_budget()?)?;
            let mut slot = self
                .model_admission
                .lock()
                .map_err(|_| invalid("model_budget_lock"))?;
            if slot.is_some() {
                return Err(invalid("model_admission_not_renewable"));
            }
            self.journal()?.begin(&self.approval, stage, previous)?;
            *slot = Some(admission);
            Ok(())
        } else {
            let mut slot = self
                .completion_budget
                .lock()
                .map_err(|_| invalid("completion_budget_lock"))?;
            if slot.is_some() {
                return Err(invalid("another_completion_phase_active"));
            }
            self.journal()?.begin(&self.approval, stage, previous)?;
            *slot = completion.map(|budget| (stage, budget));
            Ok(())
        }
    }
    pub fn completion_budget_check(&self, stage: Stage) -> io::Result<()> {
        self.live_budget()?;
        let slot = self
            .completion_budget
            .lock()
            .map_err(|_| invalid("completion_budget_lock"))?;
        let (owned_stage, budget) = slot
            .as_ref()
            .ok_or_else(|| invalid("completion_phase_not_admitted"))?;
        if *owned_stage != stage {
            return Err(invalid("completion_phase_mismatch"));
        }
        budget.check()
    }
    pub fn model_worker_spec(&self) -> io::Result<BudgetSpec> {
        self.model_admission
            .lock()
            .map_err(|_| invalid("model_budget_lock"))?
            .as_ref()
            .ok_or_else(|| invalid("model_requires_full_admission_before_spawn"))?
            .worker_spec()
    }
    pub fn model_ready(&self) -> io::Result<BudgetSpec> {
        self.model_admission
            .lock()
            .map_err(|_| invalid("model_budget_lock"))?
            .as_mut()
            .ok_or_else(|| invalid("model_not_admitted"))?
            .ready()
    }
    pub fn model_end(&self) -> io::Result<()> {
        self.model_admission
            .lock()
            .map_err(|_| invalid("model_budget_lock"))?
            .as_mut()
            .ok_or_else(|| invalid("model_not_admitted"))?
            .end()
    }
    pub fn model_budget_check(&self) -> io::Result<()> {
        self.model_admission
            .lock()
            .map_err(|_| invalid("model_budget_lock"))?
            .as_ref()
            .ok_or_else(|| invalid("model_not_admitted"))?
            .check()
    }
    pub fn previous(&self) -> io::Result<String> {
        Ok(self
            .journal()?
            .ensure_task(&self.approval)?
            .receipts
            .values()
            .last()
            .map(|r| r.digest.clone())
            .unwrap_or_default())
    }
    pub fn files(&self) -> io::Result<Directory> {
        Directory::open(&self.files_path)
    }
    pub fn prepare_fixture(&self) -> io::Result<()> {
        #[cfg(feature = "fixture")]
        if !self.config.backend.is_windows() {
            if self.approval.files.len() != 1 {
                return Err(invalid("fixture_effect_requires_exact_one_new_file"));
            }
            let mut value = if self.approval.fixture_control.is_null() {
                serde_json::Map::new()
            } else {
                self.approval
                    .fixture_control
                    .as_object()
                    .ok_or_else(|| invalid("fixture_control_object"))?
                    .clone()
            };
            if value.contains_key("effect_file") {
                return Err(invalid("fixture_remote_effect_path_forbidden"));
            }
            value.insert(
                "effect_file".into(),
                serde_json::to_value(self.files_path.join(&self.approval.files[0].name))?,
            );
            let bytes = serde_json::to_vec(&value)?;
            if self
                .native_state
                .path()
                .join("fixture-control.json")
                .exists()
            {
                // Only reached after start_task accepted a fully Complete
                // predecessor and begin(Prepare) owns this new task. Retain the
                // old fixture metadata; never reset its app model or journal.
                let old = self.native_state.read("fixture-control.json", 65536)?;
                crate::private::content(
                    &self.responses,
                    &format!("r-{}.json", sha(&old)),
                    &old,
                    65536,
                )?;
                let staged = format!("fixture-cfg-{}.json", self.approval.task);
                self.native_state.create_new(&staged, &bytes, 65536)?;
                std::fs::rename(
                    self.native_state.path().join(staged),
                    self.native_state.path().join("fixture-control.json"),
                )?;
            } else {
                self.native_state
                    .create_new("fixture-control.json", &bytes, 65536)?;
            }
        }
        Ok(())
    }
    pub fn response(&self, name: &str) -> io::Result<Value> {
        let b = self.responses.read(name, 1024 * 1024)?;
        let hash = name
            .strip_prefix("r-")
            .and_then(|s| s.strip_suffix(".json"))
            .ok_or_else(|| invalid("response_blob_name"))?;
        if sha(&b) != hash {
            return Err(invalid("response_blob_digest"));
        }
        Ok(serde_json::from_slice(&b)?)
    }
    pub fn save_response(&self, value: &Value) -> io::Result<String> {
        let b = serde_json::to_vec(value)?;
        let name = format!("r-{}.json", sha(&b));
        crate::private::content(&self.responses, &name, &b, 1024 * 1024)?;
        Ok(name)
    }
    pub fn save_wire(&self, raw: &[u8]) -> io::Result<String> {
        let name = format!("wire-{}.ndjson", sha(raw));
        crate::private::content(&self.responses, &name, raw, crate::frame::OUTBOUND)?;
        Ok(name)
    }
    pub fn finish(
        &self,
        stage: Stage,
        proof: EndProof,
        refs: BTreeMap<String, String>,
    ) -> io::Result<String> {
        if matches!(stage, Stage::Oracle | Stage::Cleanup) {
            self.completion_budget_check(stage)?;
            let receipt = self.journal()?.finish(stage, proof, refs)?;
            *self
                .completion_budget
                .lock()
                .map_err(|_| invalid("completion_budget_lock"))? = None;
            Ok(receipt)
        } else {
            self.journal()?.finish(stage, proof, refs)
        }
    }
    pub async fn make_worker(
        &self,
        stage: Stage,
        input: Value,
        lifecycle: Value,
    ) -> io::Result<Worker> {
        self.dispatch_allowed()?;
        let workers = self
            .live
            .lock()
            .map_err(|_| invalid("owned_registry_lock"))?
            .clone();
        for worker in workers {
            if worker.still_owned_alive().await? {
                return Err(invalid("previous_owned_worker_still_alive"));
            }
        }
        let mut spec = if stage == Stage::Model {
            self.model_worker_spec()?
        } else {
            self.live_budget()?.stage_spec(stage.limit_ms())?
        };
        if self.config.backend.is_windows() && stage != Stage::Model {
            spec.expires_unix_ms = spec
                .expires_unix_ms
                .min(self.approval.budget.expires_unix_ms.saturating_sub(600_000));
            spec.maximum_elapsed_ms = spec.maximum_elapsed_ms.min(
                self.live_budget()?
                    .remaining()
                    .as_millis()
                    .saturating_sub(600_000) as u64,
            );
        }
        let budget = Budget::start(spec)?;
        let root_epoch = if stage == Stage::Model && self.config.backend.is_windows() {
            Some(crate::control::model_root_epoch(self, &input)?)
        } else {
            None
        };
        let mut worker = Worker::spawn(
            &self.config,
            &self.approval,
            stage,
            budget,
            self.native_state.path(),
            &self.grants,
            input,
            lifecycle,
            root_epoch,
            &self.live,
        )
        .await?;
        worker.audit_root = Some(self.responses.path().into());
        self.journal()?.worker(
            worker
                .owned
                .record
                .lock()
                .map_err(|_| invalid("worker_record_lock"))?
                .clone(),
        )?;
        Ok(worker)
    }
    pub async fn cleanup_owned(&self) -> Value {
        let workers = match self.live.try_lock() {
            Ok(v) => v.clone(),
            Err(_) => {
                return json!({"cleanup_unknown":true,"reason":"owned_registry_inaccessible","user_application_killed":false,"historical_pid_kill":false,"does_not_clear_hold":true});
            }
        };
        let mut rows = Vec::new();
        for worker in workers {
            let result = worker.stop().await;
            rows.push(json!({"owned_record":worker.record.lock().ok().map(|v|v.clone()),"exit_observed":result.observed,"exit_code":result.code,"forced":result.forced,"error":result.error}));
        }
        json!({"owned_workers":rows,"user_application_killed":false,"historical_pid_kill":false,"does_not_clear_hold":true})
    }
    pub fn base_proof(&self) -> EndProof {
        EndProof {
            requests_terminal: true,
            operations_terminal: true,
            resources_released: true,
            leases_released: true,
            native_binding_unchanged: self.check_native().is_ok(),
            evidence_level: if self.config.backend.is_windows() {
                "windows_native"
            } else {
                "simulated_policy_and_scoped_files"
            }
            .into(),
            ..EndProof::default()
        }
    }
}
