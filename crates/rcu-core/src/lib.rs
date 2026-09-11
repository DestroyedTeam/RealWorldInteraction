//! Rust-only intent engine. OS input lives behind a target-bound native Backend.
mod checked_action;
pub use checked_action::CheckedAction;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    time::{Duration, Instant},
};
use uuid::Uuid;
pub mod device;
#[cfg(any(test, feature = "fixture"))]
pub mod fixture;
pub mod lease;
pub mod lifecycle;
pub mod operation;
#[cfg(test)]
mod tests;

pub const INLINE_BYTES: usize = 8 * 1024 * 1024;
pub const ARTIFACT_BUDGET: usize = 64 * 1024 * 1024;
pub const MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;
pub const SNAPSHOT_LIMIT: usize = 64;
pub type Result<T> = std::result::Result<T, Fault>;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Fault {
    pub code: String,
    pub next_step: String,
    /// Optional structured evidence, never authorization or a retry instruction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<Box<serde_json::Value>>,
}
impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.next_step)
    }
}
impl std::error::Error for Fault {}
pub fn fault(code: &str, next: &str) -> Fault {
    Fault {
        code: code.into(),
        next_step: next.into(),
        diagnostic: None,
    }
}
impl From<std::io::Error> for Fault {
    fn from(_: std::io::Error) -> Self {
        fault(
            "storage_or_transport_error",
            "Inspect state; never replay an uncertain action.",
        )
    }
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn id() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub host: String,
    pub guest: Option<String>,
    pub session: String,
    pub seat: String,
    pub desktop: String,
}
impl Identity {
    pub fn key(&self) -> String {
        digest(&serde_json::to_vec(self).expect("identity serialization"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Scope {
    pub window: String,
    pub pid: u32,
    pub element: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Binding {
    pub scope: Scope,
    pub focus_window: String,
    pub layout: String,
    pub content: String,
    pub native_epoch: String,
    /// Bounded optimistic document precondition, not a filesystem/tab identity proof.
    #[serde(default)]
    pub mutation_context: Option<MutationContext>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MutationContext {
    pub window_title: String,
    pub document_state: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Element {
    pub id: String,
    pub name: String,
    pub point: [f64; 2],
    pub visible: Option<bool>,
    pub occluded: Option<bool>,
    pub state_revision: Option<String>,
}
impl Element {
    pub fn revision(&self) -> String {
        digest(&serde_json::to_vec(self).expect("finite element metadata"))
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Scene {
    pub identity: Identity,
    pub assurance: String,
    pub binding: Option<Binding>,
    pub mutation_ready: bool,
    pub blocked: Option<bool>,
    pub image_size: [u32; 2],
    pub pixel_to_desktop: Option<[f64; 6]>,
    pub elements: Vec<Element>,
    pub captured_unix_ms: u64,
    pub atomicity: String,
    pub image_normalization: String,
    #[serde(skip)]
    pub png: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Click,
    DoubleClick,
    ContextMenu,
    Text,
    KeyChord,
    Scroll,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Locator {
    Semantic { name: String },
    Element { id: String },
    Pixel { x: f64, y: f64 },
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Predicate {
    WindowTitle,
    FocusedWindow,
    ElementValue,
    Modal,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Resolved {
    pub reference: String,
    pub target: String,
    pub frame: String,
    pub point: [f64; 2],
    pub scope: Scope,
    pub element_revision: Option<String>,
}
/// Semantic references bind native geometry/focus and the selected element, not
/// unrelated image pixels. Pixel-only references remain deliberately conservative.
pub fn grounding_matches(expected: &Binding, current: &Scene, target: &Resolved) -> bool {
    let Some(now) = &current.binding else {
        return false;
    };
    if now.scope != expected.scope
        || now.focus_window != expected.focus_window
        || now.layout != expected.layout
        || now.native_epoch != expected.native_epoch
        || now.mutation_context != expected.mutation_context
    {
        return false;
    }
    match &target.scope.element {
        Some(id) => current
            .elements
            .iter()
            .find(|e| &e.id == id)
            .is_some_and(|e| {
                e.point == target.point
                    && e.visible == Some(true)
                    && e.occluded == Some(false)
                    && target.element_revision.as_ref() == Some(&e.revision())
            }),
        None => now.content == expected.content,
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Oracle {
    pub identity: Identity,
    pub scope: Scope,
    pub assurance: String,
    pub matched: bool,
    pub source: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Receipt {
    pub request_id: String,
    pub frame: String,
    pub scope: Scope,
    pub dispatch: String,
    pub dispatch_error: Option<Fault>,
    pub system_accepted: Option<bool>,
    pub goal_verified: bool,
    pub replay: String,
    pub deduplicated: bool,
}
pub struct Verification<'a> {
    pub target: &'a str,
    pub frame: &'a str,
    pub element: Option<String>,
    pub request: Option<String>,
    pub predicate: Predicate,
    pub expected: &'a str,
    pub timeout_ms: u64,
    pub stable_samples: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Journal {
    fingerprint: String,
    receipt: Receipt,
}
pub use backend::Backend;
mod backend;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WindowSelection {
    pub window: String,
    pub pid: u32,
    pub native_epoch: String,
    pub relation: WindowRelation,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryQuery {
    pub cursor: Option<String>,
    pub pid: Option<u32>,
    pub title_contains: Option<String>,
}
impl DiscoveryQuery {
    pub fn validate(&self) -> Result<()> {
        if self
            .cursor
            .as_ref()
            .is_some_and(|c| c.len() > 96 || c.is_empty())
            || self.pid == Some(0)
            || self
                .title_contains
                .as_ref()
                .is_some_and(|s| s.is_empty() || s.len() > 256)
            || (self.cursor.is_some() && (self.pid.is_some() || self.title_contains.is_some()))
        {
            return Err(fault(
                "invalid_discovery_query",
                "Use a bounded PID/title filter on the first page, or its opaque cursor alone.",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WindowRelation {
    Initial,
    OwnedPopup,
    ReturnToOwner,
}
struct Snapshot {
    target: String,
    at: Instant,
    scene: Scene,
    artifact_id: Option<String>,
}
#[derive(Debug, Serialize, JsonSchema)]
pub struct Observation {
    pub frame: String,
    pub artifact_id: Option<String>,
    pub png_sha256: Option<String>,
    pub scene: Scene,
    pub adapter_status: serde_json::Value,
}
struct Artifact {
    id: String,
    bytes: Vec<u8>,
    hash: String,
}
struct RegisteredTarget {
    identity: Identity,
    backend: Box<dyn Backend>,
}

pub struct Engine {
    targets: BTreeMap<String, RegisteredTarget>,
    state: PathBuf,
    snapshots: VecDeque<(String, Snapshot)>,
    refs: VecDeque<Resolved>,
    artifacts: VecDeque<Artifact>,
    pub leases: lease::LeaseBook,
}
impl Engine {
    pub fn new(state: PathBuf) -> Result<Self> {
        Self::with_lease_root(state.clone(), state)
    }
    /// Receipt namespaces may differ by principal; physical desktop leases must
    /// still share one root across every cooperating principal and context.
    pub fn with_lease_root(state: PathBuf, lease_root: PathBuf) -> Result<Self> {
        fs::create_dir_all(state.join("receipts"))?;
        Ok(Self {
            targets: BTreeMap::new(),
            state: state.clone(),
            snapshots: VecDeque::new(),
            refs: VecDeque::new(),
            artifacts: VecDeque::new(),
            leases: lease::LeaseBook::new(lease_root),
        })
    }
    pub fn register(&mut self, target: String, backend: Box<dyn Backend>) -> Result<()> {
        if target.is_empty() || self.targets.contains_key(&target) || self.targets.len() >= 16 {
            return Err(fault(
                "invalid_target",
                "Register at most 16 explicit, unique targets.",
            ));
        }
        self.targets.insert(
            target,
            RegisteredTarget {
                identity: backend.identity(),
                backend,
            },
        );
        Ok(())
    }
    pub fn targets(&self) -> serde_json::Value {
        serde_json::json!(self.targets.iter().map(|(k,b)|serde_json::json!({"target":k,"identity":b.identity,"capabilities":b.backend.capabilities()})).collect::<Vec<_>>())
    }
    pub fn discover(&mut self, target: &str) -> Result<serde_json::Value> {
        operation::checkpoint("discover")?;
        self.backend(target)?.discover()
    }
    pub fn discover_with(
        &mut self,
        target: &str,
        query: DiscoveryQuery,
    ) -> Result<serde_json::Value> {
        query.validate()?;
        operation::checkpoint("discover")?;
        self.backend(target)?.discover_with(&query)
    }
    pub fn bind_window(
        &mut self,
        target: &str,
        selection: &WindowSelection,
    ) -> Result<serde_json::Value> {
        operation::checkpoint("bind_window")?;
        let result = self.backend(target)?.bind_window(selection)?;
        self.snapshots.retain(|(_, s)| s.target != target);
        self.refs.retain(|r| r.target != target);
        Ok(result)
    }
    fn lifecycle_execute<T>(
        &mut self,
        target: &str,
        lease: &str,
        f: impl FnOnce(
            &std::path::Path,
            &lifecycle::LifecycleGrant,
            &mut dyn lifecycle::LifecycleDriver,
        ) -> Result<T>,
    ) -> Result<T> {
        let identity = self.identity(target)?;
        let grant = self.backend(target)?.lifecycle_grant()?;
        grant.validate(&identity, lifecycle::now_ms())?;
        let book = self.leases.shared();
        let held = book
            .lock()
            .map_err(|_| fault("lease_error", "Restart observation session."))?;
        let key = identity.key();
        let deadline = held
            .deadline(lease, target, &key)?
            .min(self.backend(target)?.lifecycle_deadline()?);
        let root = self.state.clone();
        let result = operation::with_dispatch_deadline(deadline, || {
            operation::checkpoint("lifecycle_worker")?;
            let mut driver = self.backend(target)?.lifecycle_driver()?;
            f(&root, &grant, driver.as_mut())
        });
        // Side effects and uncertain dispatch invalidate every alias on this desktop.
        let aliases = self
            .targets
            .iter()
            .filter(|(_, t)| t.identity.key() == key)
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>();
        self.snapshots.retain(|(_, s)| !aliases.contains(&s.target));
        self.refs.retain(|r| !aliases.contains(&r.target));
        drop(held);
        result
    }
    pub fn launch_application(
        &mut self,
        target: &str,
        lease: &str,
        req: lifecycle::LaunchRequest<'_>,
    ) -> Result<lifecycle::LaunchReceipt> {
        self.lifecycle_execute(target, lease, |root, g, d| {
            lifecycle::launch(root, g, d, req)
        })
    }
    pub fn activate_window(
        &mut self,
        target: &str,
        lease: &str,
        req: lifecycle::ActivateRequest<'_>,
    ) -> Result<lifecycle::ActivationReceipt> {
        let grant = self.backend(target)?.lifecycle_grant()?;
        let permission = grant
            .activations
            .iter()
            .find(|p| p.authorization_id == req.authorization_id)
            .ok_or_else(|| {
                fault(
                    "unknown_lifecycle_permission",
                    "Choose an exact preauthorized window.",
                )
            })?;
        self.backend(target)?
            .validate_activation(&permission.window)?;
        self.lifecycle_execute(target, lease, |root, g, d| {
            lifecycle::activate(root, g, d, req)
        })
    }
    pub fn lifecycle_preflight(
        &mut self,
        target: &str,
        authorization: &str,
    ) -> Result<lifecycle::PreflightReceipt> {
        let cleanup = operation::CleanupAudit::begin();
        let identity = self.identity(target)?;
        let grant = self.backend(target)?.lifecycle_grant()?;
        grant.validate(&identity, lifecycle::now_ms())?;
        let deadline = self.backend(target)?.lifecycle_deadline()?;
        let mut result = operation::with_dispatch_deadline(deadline, || {
            operation::checkpoint("lifecycle_preflight_worker")?;
            let mut driver = self.backend(target)?.lifecycle_driver()?;
            lifecycle::preflight(&grant, driver.as_mut(), authorization)
        })?;
        // Include driver construction/destruction signals, not only individual closers.
        if cleanup.uncertain() {
            result.ready_for_launch = false;
            result.resources_released = false;
            result.resource_release_uncertain = true;
            if result.error.is_none() {
                result.error = Some(fault(
                    "native_resource_release_uncertain",
                    "Lifecycle preflight cleanup is unproven; no action was dispatched.",
                ));
            }
        }
        Ok(result)
    }
    pub fn lifecycle_receipt(
        &mut self,
        target: &str,
        kind: &str,
        authorization: &str,
        request: &str,
    ) -> Result<serde_json::Value> {
        let grant = self.backend(target)?.lifecycle_grant()?;
        lifecycle::receipt(&self.state, &grant, kind, authorization, request)
    }
    fn backend(&mut self, target: &str) -> Result<&mut Box<dyn Backend>> {
        self.targets
            .get_mut(target)
            .map(|r| &mut r.backend)
            .ok_or_else(|| {
                fault(
                    "unknown_target",
                    "Use computer.targets; never infer host/guest routing.",
                )
            })
    }
    fn identity(&self, target: &str) -> Result<Identity> {
        self.targets
            .get(target)
            .map(|t| t.identity.clone())
            .ok_or_else(|| {
                fault(
                    "unknown_target",
                    "Use computer.targets; never infer host/guest routing.",
                )
            })
    }
    fn scene(&mut self, target: &str, region: Option<[u32; 4]>) -> Result<Scene> {
        let expected = self.identity(target)?;
        let scene = match region {
            Some(region) => self.backend(target)?.observe_region(region)?,
            None => self.backend(target)?.observe()?,
        };
        if scene.identity != expected {
            return Err(fault(
                "session_mismatch",
                "Stop and bind the intended session explicitly.",
            ));
        }
        Ok(scene)
    }
    pub fn observe(&mut self, target: &str) -> Result<Observation> {
        self.observe_with_region(target, None)
    }
    pub fn observe_with_region(
        &mut self,
        target: &str,
        region: Option<[u32; 4]>,
    ) -> Result<Observation> {
        operation::checkpoint("observe")?;
        let mut scene = self.scene(target, region)?;
        operation::checkpoint("capture_returned")?;
        let frame = id();
        let mut artifact = None;
        let mut hash = None;
        if scene.png.len() > MAX_FRAME_BYTES {
            return Err(fault(
                "screenshot_size_limit",
                "Native PNG exceeds the 32 MiB original-artifact limit; distinguish capture from encoding/transport. Do not change desktop resolution.",
            ));
        }
        if !scene.png.is_empty() {
            let bytes = std::mem::take(&mut scene.png);
            let h = digest(&bytes);
            let aid = id();
            while self.artifact_bytes() + bytes.len() > ARTIFACT_BUDGET
                || self.artifacts.len() >= SNAPSHOT_LIMIT
            {
                self.artifacts.pop_front();
            }
            self.artifacts.push_back(Artifact {
                id: aid.clone(),
                bytes,
                hash: h.clone(),
            });
            artifact = Some(aid);
            hash = Some(h);
        }
        if serde_json::to_vec(&scene)
            .map_err(|_| fault("invalid_scene", "Inspect adapter metadata."))?
            .len()
            > 256 * 1024
        {
            return Err(fault(
                "observation_budget",
                "Narrow the accessibility tree.",
            ));
        }
        self.snapshots.push_back((
            frame.clone(),
            Snapshot {
                target: target.into(),
                at: Instant::now(),
                scene: scene.clone(),
                artifact_id: artifact.clone(),
            },
        ));
        while self.snapshots.len() > SNAPSHOT_LIMIT {
            self.snapshots.pop_front();
        }
        Ok(Observation {
            frame,
            artifact_id: artifact,
            png_sha256: hash,
            scene,
            adapter_status: self.backend(target)?.capabilities(),
        })
    }
    pub fn artifact_bytes(&self) -> usize {
        self.artifacts.iter().map(|a| a.bytes.len()).sum()
    }
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }
    pub fn artifact(
        &self,
        aid: &str,
        offset: usize,
        length: usize,
    ) -> Result<(Vec<u8>, String, usize)> {
        let a = self.artifacts.iter().find(|a| a.id == aid).ok_or_else(|| {
            fault(
                "artifact_expired",
                "Capture again; artifacts are held under a 64 MiB total budget.",
            )
        })?;
        if length == 0 || length > INLINE_BYTES || offset > a.bytes.len() {
            return Err(fault(
                "invalid_range",
                "Use a bounded byte range within the raw PNG.",
            ));
        }
        let end = offset.saturating_add(length).min(a.bytes.len());
        Ok((a.bytes[offset..end].to_vec(), a.hash.clone(), a.bytes.len()))
    }
    /// Internal full-original access for bounded view encoding; not a raw transport chunk.
    pub fn original(&self, target: &str, frame: &str, aid: &str) -> Result<(&[u8], &Scene)> {
        let s = self.snapshot(target, frame)?;
        let a = self
            .artifacts
            .iter()
            .find(|a| a.id == aid)
            .ok_or_else(|| fault("artifact_expired", "Observe again."))?;
        if s.artifact_id.as_deref() != Some(aid) {
            return Err(fault(
                "artifact_frame_mismatch",
                "Use the original artifact from this exact frame.",
            ));
        }
        Ok((&a.bytes, &s.scene))
    }
    fn snapshot(&self, target: &str, frame: &str) -> Result<&Snapshot> {
        self.snapshots
            .iter()
            .find(|(id, s)| id == frame && s.target == target)
            .map(|(_, s)| s)
            .ok_or_else(|| fault("unknown_frame", "Observe this exact target again."))
    }
    pub fn resolve(&mut self, target: &str, frame: &str, locator: Locator) -> Result<Resolved> {
        let scene = &self.snapshot(target, frame)?.scene;
        let binding = scene.binding.as_ref().ok_or_else(|| {
            fault(
                "target_binding_unknown",
                "Native window/PID binding is required.",
            )
        })?;
        let mut scope = binding.scope.clone();
        let mut element_revision = None;
        let point = match locator {
            locator @ (Locator::Semantic { .. } | Locator::Element { .. }) => {
                let matches = scene
                    .elements
                    .iter()
                    .filter(|e| match &locator {
                        Locator::Semantic { name } => e.name == *name,
                        Locator::Element { id } => e.id == *id,
                        _ => false,
                    })
                    .collect::<Vec<_>>();
                if matches.len() != 1 {
                    return Err(fault(
                        "ambiguous_target",
                        "Choose one grounded visible element.",
                    ));
                }
                let e = matches[0];
                if e.visible != Some(true) || e.occluded != Some(false) {
                    return Err(fault(
                        "visibility_unknown",
                        "Resolve occlusion before input.",
                    ));
                }
                scope.element = Some(e.id.clone());
                element_revision = Some(e.revision());
                e.point
            }
            Locator::Pixel { x, y } => {
                if !x.is_finite()
                    || !y.is_finite()
                    || x < 0.0
                    || y < 0.0
                    || x >= scene.image_size[0] as f64
                    || y >= scene.image_size[1] as f64
                {
                    return Err(fault(
                        "outside_frame",
                        "Use coordinates inside the original image.",
                    ));
                }
                let [a, b, c, d, e, f] = scene.pixel_to_desktop.ok_or_else(|| {
                    fault(
                        "coordinate_mapping_unknown",
                        "Do not assume scale=1 or host coordinates for a guest.",
                    )
                })?;
                scope.element = None;
                [a * x + c * y + e, b * x + d * y + f]
            }
        };
        if !point.iter().all(|p| p.is_finite()) {
            return Err(fault(
                "invalid_transform",
                "Inspect native layout metadata.",
            ));
        }
        let r = Resolved {
            reference: id(),
            target: target.into(),
            frame: frame.into(),
            point,
            scope,
            element_revision,
        };
        self.refs.push_back(r.clone());
        while self.refs.len() > 256 {
            self.refs.pop_front();
        }
        Ok(r)
    }
    pub fn claim(&mut self, target: &str, ttl: u64) -> Result<String> {
        let key = self.identity(target)?.key();
        self.leases.claim(target, &key, ttl)
    }
    pub fn release(&mut self, token: &str) -> Result<()> {
        self.leases.release(token)
    }
    fn journal_path(&self, key: &str, request: &str) -> PathBuf {
        self.state
            .join("receipts")
            .join(digest(format!("{key}:{request}").as_bytes()) + ".jsonl")
    }
    pub fn act(
        &mut self,
        target: &str,
        reference: &str,
        lease: &str,
        request: &str,
        intent: Intent,
        value: Option<String>,
    ) -> Result<Receipt> {
        self.act_inner(target, reference, lease, request, intent, value, None)
    }
    #[allow(clippy::too_many_arguments)]
    fn act_inner(
        &mut self,
        target: &str,
        reference: &str,
        lease: &str,
        request: &str,
        intent: Intent,
        value: Option<String>,
        checked_fingerprint: Option<String>,
    ) -> Result<Receipt> {
        if request.is_empty()
            || request.len() > 128
            || value.as_ref().is_some_and(|v| v.len() > 16384)
        {
            return Err(fault(
                "invalid_arguments",
                "Bound request_id and value lengths.",
            ));
        }
        let key = self.identity(target)?.key();
        let path = self.journal_path(&key, request);
        let fingerprint = checked_fingerprint.unwrap_or_else(|| {
            digest(&serde_json::to_vec(&(target, reference, &intent, &value)).unwrap())
        });
        if path.exists() {
            let mut text = String::new();
            File::open(&path)?.take(65536).read_to_string(&mut text)?;
            let mut records = text
                .split_inclusive('\n')
                .filter(|s| s.ends_with('\n'))
                .filter_map(|s| serde_json::from_str::<Journal>(s).ok());
            let first = records.next().ok_or_else(|| {
                fault(
                    "uncertain_journal",
                    "Interrupted reservation exists; do not replay.",
                )
            })?;
            if first.fingerprint != fingerprint {
                return Err(fault(
                    "idempotency_conflict",
                    "Do not reuse request_id for another action or reference.",
                ));
            }
            let mut result = records.next_back().unwrap_or(first).receipt;
            result.deduplicated = true;
            return Ok(result);
        }
        operation::checkpoint("before_revalidation")?;
        let resolved = self
            .refs
            .iter()
            .find(|r| r.reference == reference && r.target == target)
            .cloned()
            .ok_or_else(|| fault("unknown_reference", "Observe and resolve this target."))?;
        let snapshot = self.snapshot(target, &resolved.frame)?;
        if snapshot.at.elapsed() > Duration::from_secs(10) {
            return Err(fault("stale_reference", "Reobserve; no input sent."));
        }
        let before = snapshot.scene.clone();
        let current = self.backend(target)?.revalidate(&resolved)?;
        if current.identity != self.identity(target)? {
            return Err(fault(
                "session_mismatch",
                "Native revalidation belongs to another session.",
            ));
        }
        if !current.mutation_ready || !before.mutation_ready {
            return Err(fault(
                "capability_unavailable",
                "Native input binding is not ready.",
            ));
        }
        if !before
            .binding
            .as_ref()
            .is_some_and(|binding| grounding_matches(binding, &current, &resolved))
        {
            return Err(fault(
                "stale_reference",
                "Focus/layout/content changed; reobserve.",
            ));
        }
        if current.blocked != Some(false) {
            return Err(fault("target_blocked", "Inspect modal/occlusion state."));
        }
        let book = self.leases.shared();
        let held = book
            .lock()
            .map_err(|_| fault("lease_error", "Restart observation session."))?;
        held.check(lease, target, &key)?;
        operation::checkpoint("before_durable_reservation")?;
        let mut receipt = Receipt {
            request_id: request.into(),
            frame: resolved.frame.clone(),
            scope: resolved.scope.clone(),
            dispatch: "uncertain".into(),
            dispatch_error: None,
            system_accepted: None,
            goal_verified: false,
            replay: "forbidden".into(),
            deduplicated: false,
        };
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let write = |f: &mut File, r: &Receipt| -> Result<()> {
            let data = serde_json::to_vec(&Journal {
                fingerprint: fingerprint.clone(),
                receipt: r.clone(),
            })
            .unwrap();
            f.write_all(&data)?;
            f.write_all(b"\n")?;
            f.sync_all()?;
            Ok(())
        };
        write(&mut file, &receipt)?;
        // Persist the directory entry before dispatch as well as the reservation bytes.
        #[cfg(unix)]
        File::open(path.parent().unwrap())?.sync_all()?;
        let expected = before
            .binding
            .as_ref()
            .ok_or_else(|| fault("target_binding_unknown", "Native binding required."))?;
        match operation::with_dispatch_deadline(held.deadline(lease, target, &key)?, || {
            self.backend(target)?
                .act(expected, &resolved, &intent, value.as_deref())
        }) {
            Ok(accepted) => {
                receipt.dispatch = "returned".into();
                receipt.system_accepted = accepted;
            }
            Err(error) => receipt.dispatch_error = Some(error),
        }
        write(&mut file, &receipt)?;
        drop(held);
        let aliases = self
            .targets
            .iter()
            .filter(|(_, t)| t.identity.key() == key)
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        self.refs.retain(|r| !aliases.contains(&r.target));
        Ok(receipt)
    }
    pub fn verify(&mut self, verification: Verification<'_>) -> Result<serde_json::Value> {
        let Verification {
            target,
            frame,
            element,
            request,
            predicate,
            expected,
            timeout_ms,
            stable_samples,
        } = verification;
        if timeout_ms > 10000 || !(1..=5).contains(&stable_samples) || expected.len() > 16384 {
            return Err(fault(
                "invalid_arguments",
                "Bound verification timeout, sample count and value.",
            ));
        }
        let scene = &self.snapshot(target, frame)?.scene;
        let binding = scene
            .binding
            .as_ref()
            .ok_or_else(|| fault("target_binding_unknown", "Window/PID attestation required."))?;
        let mut scope = binding.scope.clone();
        scope.element = element.clone();
        if matches!(predicate, Predicate::ElementValue) && element.is_none() {
            return Err(fault(
                "element_required",
                "Bind the expected value to a specific element.",
            ));
        }
        if element
            .as_ref()
            .is_some_and(|id| !scene.elements.iter().any(|e| &e.id == id))
        {
            return Err(fault(
                "unknown_element",
                "Use an element present in this frame.",
            ));
        }
        let identity = scene.identity.clone();
        if let Some(req) = &request {
            let text = fs::read_to_string(self.journal_path(&identity.key(), req))?;
            let record: Journal = serde_json::from_str(text.lines().next().unwrap_or(""))
                .map_err(|_| fault("unknown_operation", "Use a valid action request ID."))?;
            if record.receipt.scope != scope {
                return Err(fault(
                    "verification_scope_mismatch",
                    "Operation targeted another window/PID/element.",
                ));
            }
        }
        let start = Instant::now();
        let mut consecutive = 0;
        let mut samples = 0;
        loop {
            operation::checkpoint("verify_poll")?;
            let oracle = self
                .backend(target)?
                .inspect(&scope, &predicate, expected)?;
            if oracle.identity != identity || oracle.scope != scope {
                return Err(fault(
                    "verification_scope_mismatch",
                    "Same value in another target is not success.",
                ));
            }
            samples += 1;
            consecutive = if oracle.matched { consecutive + 1 } else { 0 };
            if consecutive >= stable_samples || start.elapsed() >= Duration::from_millis(timeout_ms)
            {
                let attested = oracle.assurance == "native-attested"
                    || oracle.assurance == "synthetic-fixture";
                return Ok(
                    serde_json::json!({"goal_verified":attested&&consecutive>=stable_samples,"predicate_matched":consecutive>=stable_samples,"frame":frame,"request_id":request,"scope":scope,"samples":samples,"oracle":oracle}),
                );
            }
            std::thread::sleep(
                Duration::from_millis(25)
                    .min(Duration::from_millis(timeout_ms).saturating_sub(start.elapsed())),
            );
        }
    }
}
