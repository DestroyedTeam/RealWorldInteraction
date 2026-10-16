use crate::{budget::BudgetSpec, invalid, sha};
use rwi_link_trust::IdentityPaths;
use rwi_windows::agent_contract::OwnerProof;
use rwi_windows::agent_contract::SessionBinding;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, io, net::SocketAddr, path::PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Prepare,
    Bootstrap,
    Preflight,
    Launch,
    Activate,
    Model,
    Oracle,
    Cleanup,
}
impl Stage {
    pub fn next(self) -> Option<Self> {
        match self {
            Self::Prepare => Some(Self::Bootstrap),
            Self::Bootstrap => Some(Self::Preflight),
            Self::Preflight => Some(Self::Launch),
            Self::Launch => Some(Self::Activate),
            Self::Activate => Some(Self::Model),
            Self::Model => Some(Self::Oracle),
            Self::Oracle => Some(Self::Cleanup),
            Self::Cleanup => None,
        }
    }
    pub fn worker_stage(self) -> bool {
        matches!(
            self,
            Self::Bootstrap | Self::Preflight | Self::Launch | Self::Activate | Self::Model
        )
    }
    pub fn limit_ms(self) -> u64 {
        if self == Self::Model {
            1_200_000
        } else if self == Self::Cleanup {
            600_000
        } else {
            90_000
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Controller,
    Model,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendMode {
    Windows,
    #[cfg(feature = "fixture")]
    Fixture,
}
impl BackendMode {
    pub fn is_windows(self) -> bool {
        matches!(self, Self::Windows)
    }
    pub fn target(self) -> &'static str {
        if self.is_windows() {
            "windows-local"
        } else {
            "synthetic-only"
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub path: PathBuf,
    pub sha256: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentConfig {
    /// Required for native Windows, absent only in explicit offline fixtures.
    #[serde(default)]
    pub privilege_mode: Option<rwi_windows::agent_contract::PrivilegeMode>,
    pub version: u8,
    pub install_root: PathBuf,
    pub state_root: PathBuf,
    pub approval: PathBuf,
    pub bind: SocketAddr,
    pub server_name: String,
    pub trust: IdentityPaths,
    pub controller_peer_sha256: String,
    pub model_peer_sha256: String,
    pub agent_image: Image,
    pub worker_image: Image,
    pub backend: BackendMode,
}
impl AgentConfig {
    pub fn native_privilege_mode(&self) -> io::Result<rwi_windows::agent_contract::PrivilegeMode> {
        self.privilege_mode
            .ok_or_else(|| invalid("native_privilege_mode_required"))
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientConfig {
    #[serde(default)]
    pub model_output: crate::presentation::OutputMode,
    pub version: u8,
    pub root: PathBuf,
    pub endpoint: SocketAddr,
    pub server_name: String,
    pub trust: IdentityPaths,
    pub agent_peer_sha256: String,
    pub task: String,
    pub approval_sha256: String,
    pub role: Role,
    pub relay_image: Image,
    pub entry_sha256: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileSpec {
    pub name: String,
    pub initial_utf8: String,
    pub expected_sha256: String,
}
pub fn identifier(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
pub fn file_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 96
        && s != "."
        && s != ".."
        && !s.contains("..")
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}
pub fn digest_shape(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub id: String,
    pub tool: ControlTool,
    pub arguments: Value,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlTool {
    Targets,
    Discover,
    Catalog,
    BindWindow,
    LifecyclePreflight,
    Claim,
    LaunchApplication,
    ActivateWindow,
    Release,
    Operation,
    LifecycleReceipt,
    Observe,
    Verify,
}
impl ControlTool {
    pub fn name(self) -> &'static str {
        match self {
            Self::Targets => "computer.targets",
            Self::Discover => "computer.discover",
            Self::Catalog => "local.discover_catalog",
            Self::BindWindow => "computer.bind_window",
            Self::LifecyclePreflight => "computer.lifecycle_preflight",
            Self::Claim => "computer.claim",
            Self::LaunchApplication => "computer.launch_application",
            Self::ActivateWindow => "computer.activate_window",
            Self::Release => "computer.release",
            Self::Operation => "computer.operation",
            Self::LifecycleReceipt => "computer.lifecycle_receipt",
            Self::Observe => "computer.observe",
            Self::Verify => "computer.verify",
        }
    }
    pub fn allowed(self, stage: Stage) -> bool {
        match self {
            Self::LaunchApplication => stage == Stage::Launch,
            Self::ActivateWindow => stage == Stage::Activate,
            Self::Catalog => stage == Stage::Activate,
            Self::LifecyclePreflight => matches!(stage, Stage::Bootstrap | Stage::Preflight),
            Self::Claim | Self::Release => matches!(stage, Stage::Launch | Stage::Activate),
            _ => stage.worker_stage() && stage != Stage::Model,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub version: u8,
    pub task: String,
    pub controller_host: String,
    pub guest_claim: Option<String>,
    pub native_binding: Option<SessionBinding>,
    pub budget: BudgetSpec,
    pub request_prefix: String,
    pub files: Vec<FileSpec>,
    pub expected_ui_value: String,
    pub stages: BTreeMap<Stage, Vec<Step>>,
    pub input_grant_template: Value,
    pub lifecycle_grant_template: Value,
    #[serde(default)]
    pub fixture_control: Value,
}
impl Approval {
    pub fn validate(&self, mode: BackendMode) -> io::Result<()> {
        if self.version != 1
            || !identifier(&self.task, 64)
            || self.controller_host.is_empty()
            || self.controller_host.len() > 256
            || self.request_prefix != format!("{}-", self.task)
            || self.expected_ui_value.len() > 16384
            || self.files.is_empty()
            || self.files.len() > 4
            || self.files.iter().any(|f| {
                !file_name(&f.name)
                    || f.initial_utf8.len() > 65536
                    || !digest_shape(&f.expected_sha256)
            })
            || self
                .files
                .iter()
                .map(|f| &f.name)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.files.len()
        {
            return Err(invalid("invalid_local_task_manifest"));
        }
        if mode.is_windows() {
            self.native_binding
                .as_ref()
                .ok_or_else(|| invalid("native_binding_required"))?
                .validate()
                .map_err(|e| invalid(&e.to_string()))?;
            if !self.fixture_control.is_null() {
                return Err(invalid("fixture_control_forbidden_in_native"));
            }
        }
        for (stage, steps) in &self.stages {
            if !stage.worker_stage() || *stage == Stage::Model || steps.len() > 32 {
                return Err(invalid("invalid_stage_plan"));
            }
            let mut ids = std::collections::BTreeSet::new();
            for step in steps {
                if !identifier(&step.id, 64)
                    || !ids.insert(&step.id)
                    || !step.tool.allowed(*stage)
                    || serde_json::to_vec(&step.arguments)?.len() > 65536
                {
                    return Err(invalid("unapproved_control_step"));
                }
            }
        }
        // Expired approvals may still authorize read-only status/reconciliation.
        // A live Budget is separately mandatory before any write/stage dispatch.
        self.budget.validate(self.budget.issued_unix_ms)
    }
    pub fn digest(&self) -> String {
        sha(&serde_json::to_vec(self).unwrap())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attach {
    pub version: u8,
    pub task: String,
    pub approval_sha256: String,
    pub role: Role,
    pub nonce: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlRequest {
    pub version: u8,
    pub request_id: String,
    pub operation: ControlOp,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlOp {
    Prepare,
    RunStage {
        stage: Stage,
        previous_receipt: String,
    },
    Status,
    Reconcile,
    Oracle {
        previous_receipt: String,
    },
    Cleanup {
        previous_receipt: String,
    },
    Cancel,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEnd {
    pub transport_version: u8,
    pub model_end: bool,
    pub task: String,
    pub nonce: String,
}

/// Written by the locally admitted agent, never accepted as a remote grant.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerGuardSpec {
    pub privilege_mode: rwi_windows::agent_contract::PrivilegeMode,
    pub version: u8,
    pub task: String,
    pub stage: Stage,
    pub budget: BudgetSpec,
    pub owner: OwnerProof,
    pub input_blocked_path: PathBuf,
    pub expected_foreground: Option<rwi_windows::NativeWindowKey>,
    #[serde(default)]
    pub expected_root_native_epoch: Option<String>,
    #[serde(default)]
    pub model_ready_path: Option<PathBuf>,
    pub expected_ui_value: String,
    pub request_prefix: String,
}

/// Immutable ready record written by the admitted agent after MCP initialization.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelReadySpec {
    pub version: u8,
    pub task: String,
    pub stage: Stage,
    pub guard_sha256: String,
    pub budget: BudgetSpec,
}
