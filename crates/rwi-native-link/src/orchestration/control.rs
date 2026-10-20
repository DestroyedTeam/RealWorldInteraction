//! Typed operations over the immutable local Approval. No shell, arbitrary file
//! argument, automatic replay, marker reset, or model-side control surface.
use super::oracle::oracle;
use crate::{
    budget::now_ms,
    invalid,
    private::Directory,
    runtime::Runtime,
    sha,
    types::{ControlOp, ControlRequest, ControlTool, Stage, Step, identifier},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use rwi_core::lifecycle::{
    ActivationReceipt, Inventory, LaunchReceipt, LaunchState, PreflightReceipt, WindowIdentity,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    sync::Arc,
    time::{Duration, Instant},
};

pub const TEMPLATE_DEPTH: usize = 32;
pub const TEMPLATE_NODES: usize = 4096;
pub const TEMPLATE_BYTES: usize = 65536;
pub const TEMPLATE_STRING: usize = 16384;
pub const FILE_CAP: usize = 65536;
const CLEANUP_BOUND: Duration = Duration::from_secs(45);

fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::Prepare => "prepare",
        Stage::Bootstrap => "bootstrap",
        Stage::Preflight => "preflight",
        Stage::Launch => "launch",
        Stage::Activate => "activate",
        Stage::Model => "model",
        Stage::Oracle => "oracle",
        Stage::Cleanup => "cleanup",
    }
}
fn envelope(request: &ControlRequest, result: Value) -> Value {
    json!({"version":1,"request_id":request.request_id,"ok":true,"result":result})
}

/// Status/Reconcile do not touch a task's HOLD or dispatch budget. Only an
/// authenticated controller may call this API; server.rs owns TLS role binding.
pub async fn execute(runtime: &Arc<Runtime>, request: &ControlRequest) -> io::Result<Value> {
    if request.version != 1 || !identifier(&request.request_id, 128) {
        return Err(invalid("control_request_version_or_identifier"));
    }
    let result = match &request.operation {
        ControlOp::Status => status(runtime, false)?,
        ControlOp::Reconcile => status(runtime, true)?,
        ControlOp::Prepare => prepare(runtime).await?,
        ControlOp::RunStage {
            stage,
            previous_receipt,
        } => {
            if !matches!(
                stage,
                Stage::Bootstrap | Stage::Preflight | Stage::Launch | Stage::Activate
            ) {
                return Err(invalid(
                    "typed_run_stage_excludes_model_prepare_oracle_cleanup",
                ));
            }
            run_stage(runtime, *stage, previous_receipt).await?
        }
        ControlOp::Oracle { previous_receipt } => oracle(runtime, previous_receipt).await?,
        ControlOp::Cleanup { previous_receipt } => cleanup(runtime, previous_receipt).await?,
        ControlOp::Cancel => cancel(runtime).await?,
    };
    Ok(envelope(request, result))
}

fn status(runtime: &Runtime, reconcile: bool) -> io::Result<Value> {
    let journal = runtime.journal()?.public_status();
    let records = runtime
        .live
        .lock()
        .map_err(|_| invalid("owned_registry_lock"))?
        .iter()
        .map(|worker| {
            worker
                .record
                .lock()
                .map(|record| record.clone())
                .map_err(|_| invalid("owned_worker_record_lock"))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let file_readback = if reconcile {
        let directory = runtime.files();
        let rows = runtime.approval.files.iter().map(|file| {
            let returned = match &directory {
                Ok(directory) => directory.read(&file.name, FILE_CAP),
                Err(error) => Err(invalid(&format!("reconciliation_directory_unavailable:{error}"))),
            };
            match returned {
                Ok(bytes) => {
                    let hash = sha(&bytes);
                    json!({"name":file.name,"bytes":bytes.len(),"sha256":hash,"expected_sha256":file.expected_sha256,"matches":hash==file.expected_sha256,"bytes_base64":STANDARD.encode(&bytes)})
                }
                Err(error) => json!({"name":file.name,"expected_sha256":file.expected_sha256,"matches":false,"error":error.to_string()}),
            }
        }).collect::<Vec<_>>();
        json!({"files":rows,"all_match":rows.iter().all(|row|row["matches"]==true),"independent_of_uia":true,"stage_advanced":false,"journal_or_marker_written":false})
    } else {
        Value::Null
    };
    Ok(
        json!({"status":journal,"reconciliation_requested":reconcile,"readonly":true,"hold_cleared":false,"owned_records":records,"native_process_liveness_not_inferred_from_record":true,"file_readback":file_readback}),
    )
}

/// Called before ownership begins: a rejected concurrent/out-of-order request
/// must never HOLD or clean up another active connection's worker.
pub(super) async fn no_active_or_live(runtime: &Runtime) -> io::Result<()> {
    if runtime
        .journal()?
        .state
        .as_ref()
        .is_some_and(|state| state.active.is_some())
    {
        return Err(invalid("another_stage_already_active_no_interference"));
    }
    let workers = runtime
        .live
        .lock()
        .map_err(|_| invalid("owned_registry_lock"))?
        .clone();
    for worker in workers {
        if tokio::time::timeout(Duration::from_secs(2), worker.still_owned_alive())
            .await
            .map_err(|_| invalid("owned_liveness_unconfirmed_no_stage_started"))??
        {
            return Err(invalid("previous_owned_worker_alive_no_stage_started"));
        }
    }
    Ok(())
}

async fn bounded_cleanup(runtime: &Runtime) -> Value {
    match tokio::time::timeout(CLEANUP_BOUND, runtime.cleanup_owned()).await {
        Ok(value) => value,
        Err(_) => {
            json!({"cleanup_unknown":true,"reason":"owned_cleanup_total_deadline","user_application_killed":false,"historical_pid_kill":false,"does_not_clear_hold":true})
        }
    }
}
pub(super) async fn owned_failure(runtime: &Runtime, reason: &str) {
    let held = runtime.hold(reason);
    let cleanup = bounded_cleanup(runtime).await;
    let _ = runtime.journal().and_then(|mut journal| {
        journal.event(
            "typed_control_owned_failure_cleanup",
            json!({"hold_persisted":held.is_ok(),"cleanup":cleanup}),
        )
    });
}

async fn prepare(runtime: &Runtime) -> io::Result<Value> {
    runtime.dispatch_allowed()?;
    no_active_or_live(runtime).await?;
    {
        let mut journal = runtime.journal()?;
        journal.start_task(&runtime.approval, runtime.config.backend.is_windows())?;
        // Both admission and intent are serialized under the journal lock.
        if let Err(error) = journal.begin(&runtime.approval, Stage::Prepare, "") {
            let _ = journal.hold("prepare_intent_persistence_failed");
            return Err(error);
        }
    }
    let result = async {
        let files = Directory::create(&runtime.files_path)?;
        let mut rows = Vec::new();
        for file in &runtime.approval.files {
            files.create_new(&file.name, file.initial_utf8.as_bytes(), FILE_CAP)?;
            let returned = files.read(&file.name, FILE_CAP)?;
            if returned != file.initial_utf8.as_bytes() { return Err(invalid("prepared_file_exact_readback_mismatch")); }
            rows.push(json!({"name":file.name,"bytes":returned.len(),"initial_sha256":sha(&returned)}));
        }
        runtime.prepare_fixture()?;
        let data = json!({"new_private_run_files":rows,"only_allowlisted_initial_files_written":true});
        let mut refs = BTreeMap::new(); refs.insert("files".into(), runtime.save_response(&data)?);
        let mut proof = runtime.base_proof();
        proof.detail = data;
        let receipt = runtime.finish(Stage::Prepare, proof, refs)?;
        Ok(json!({"stage":"prepare","stage_receipt":receipt,"complete":true,"native_goal_verified":false}))
    }.await;
    if let Err(error) = &result {
        owned_failure(runtime, &format!("prepare_owned_failure:{error}")).await;
    }
    result
}

fn plan(runtime: &Runtime, stage: Stage) -> io::Result<Vec<Step>> {
    let steps = runtime
        .approval
        .stages
        .get(&stage)
        .cloned()
        .ok_or_else(|| invalid("stage_plan_not_locally_approved"))?;
    if steps.is_empty() || steps.len() > 32 || steps.iter().any(|step| !step.tool.allowed(stage)) {
        return Err(invalid("empty_or_invalid_local_stage_plan"));
    }
    if runtime.config.backend.is_windows() {
        let required = match stage {
            Stage::Bootstrap | Stage::Preflight => ControlTool::LifecyclePreflight,
            Stage::Launch => ControlTool::LaunchApplication,
            Stage::Activate => ControlTool::ActivateWindow,
            _ => return Err(invalid("native_control_stage_invalid")),
        };
        if !steps.iter().any(|step| step.tool.name() == required.name()) {
            return Err(invalid("native_stage_required_semantic_step_missing"));
        }
        if stage == Stage::Activate {
            let catalogs = steps
                .iter()
                .enumerate()
                .filter(|(_, s)| matches!(s.tool, ControlTool::Catalog))
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            let activations = steps
                .iter()
                .enumerate()
                .filter(|(_, s)| matches!(s.tool, ControlTool::ActivateWindow))
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            if catalogs.len() != 1
                || activations.len() != 1
                || catalogs[0] >= activations[0]
                || steps[catalogs[0] + 1..activations[0]].iter().any(|s| {
                    matches!(
                        s.tool,
                        ControlTool::Discover
                            | ControlTool::BindWindow
                            | ControlTool::LifecyclePreflight
                    )
                })
            {
                return Err(invalid(
                    "native_activate_requires_one_unchanged_complete_catalog_chain",
                ));
            }
        }
        if matches!(stage, Stage::Launch | Stage::Activate)
            && (!steps
                .iter()
                .any(|step| matches!(step.tool, ControlTool::Claim))
                || !steps
                    .iter()
                    .any(|step| matches!(step.tool, ControlTool::Release)))
        {
            return Err(invalid(
                "native_mutation_stage_requires_explicit_claim_and_release",
            ));
        }
    }
    Ok(steps)
}

async fn run_stage(runtime: &Runtime, stage: Stage, previous: &str) -> io::Result<Value> {
    let steps = plan(runtime, stage)?;
    no_active_or_live(runtime).await?;
    runtime.begin(stage, previous)?;
    // Only exceptions after successful begin may affect this admitted stage.
    let result = async {
        let expires = runtime.live_budget()?.stage_spec(stage.limit_ms())?.expires_unix_ms;
        let local = BTreeMap::new();
        let lifecycle = expand(runtime, lifecycle_template(runtime, stage)?, &local, expires)?;
        let lifecycle_authority = lifecycle.clone();
        // No input capability is passed to a control-stage worker.
        let mut worker = runtime.make_worker(stage, Value::Null, lifecycle).await?;
        worker.initialize().await?;
        let mut results = BTreeMap::new();
        let mut refs = BTreeMap::new();
        let mut semantics = BTreeMap::new();
        let mut rpc_id = 10u64;
        let mut catalog: Option<(Value, Instant)> = None;
        for step in &steps {
            runtime.dispatch_allowed()?;
            let args = expand(runtime, &step.arguments, &results, expires)?;
            if matches!(step.tool, ControlTool::Catalog) {
                let started = Instant::now();
                let aggregate = collect_catalog(runtime, &mut worker, &args, &step.id, &mut rpc_id).await?;
                refs.insert(step.id.clone(), runtime.save_response(&aggregate)?);
                semantics.insert(step.id.clone(), json!({"original_complete_catalog_chain":true,"selection":aggregate["selection"]}));
                results.insert(step.id.clone(), aggregate.clone());
                catalog = Some((aggregate, started));
                continue;
            }
            if matches!(step.tool, ControlTool::ActivateWindow) && runtime.config.backend.is_windows() {
                let (aggregate, started) = catalog.as_ref().ok_or_else(|| invalid("activation_without_same_worker_catalog"))?;
                if started.elapsed() >= Duration::from_millis(CATALOG_TTL_MS) { return Err(invalid("activation_catalog_monotonic_expired")); }
                let selection = catalog_selection(runtime, aggregate)?;
                let permissions = lifecycle_authority["activations"].as_array().ok_or_else(|| invalid("activation_authority_missing"))?;
                let permissions = permissions.iter().filter(|p| p["authorization_id"] == args["authorization_id"]).collect::<Vec<_>>();
                if permissions.len() != 1 || permissions[0]["window"] != serde_json::to_value(&selection.activation_target)?
                    || args["expected_foreground"] != serde_json::to_value(&selection.foreground)? {
                    return Err(invalid("activation_catalog_target_or_foreground_not_granted"));
                }
            }
            let rpc = json!({"jsonrpc":"2.0","id":rpc_id,"method":"tools/call","params":{"name":step.tool.name(),"arguments":args}});
            rpc_id += 1;
            let mut bytes = serde_json::to_vec(&rpc)?; bytes.push(b'\n');
            let mutation = matches!(step.tool, ControlTool::LaunchApplication | ControlTool::ActivateWindow);
            let reservation = if mutation {
                let id = args["request_id"].as_str().ok_or_else(|| invalid("mutation_request_id_required"))?;
                if !id.starts_with(&runtime.approval.request_prefix) || !identifier(id, 128) { return Err(invalid("mutation_request_id_task_scope")); }
                id.to_owned()
            } else {
                format!("control-{}", sha(&serde_json::to_vec(&(runtime.approval.task.as_str(), stage, &step.id))?))
            };
            runtime.journal()?.reserve(&reservation, &bytes)?;
            let response = worker.rpc(rpc).await?;
            // Observe and store failures too, before semantic interpretation.
            // An uncommitted stage never exposes them as future template authority.
            runtime.journal()?.event("typed_step_raw_reply_observed", json!({"stage":stage,"step":step.id,"raw_sha256":worker.ledger.response_digests.last()}))?;
            let wire_name = runtime.save_response(&response)?;
            let content = response["result"].get("structuredContent").cloned().unwrap_or(Value::Null);
            let content_name = runtime.save_response(&content)?;
            runtime.journal()?.event("typed_step_response_blobs", json!({"stage":stage,"step":step.id,"wire_blob":wire_name,"structured_blob":content_name}))?;
            refs.insert(step.id.clone(), content_name);
            // '@' cannot occur in an approved Step.id, avoiding namespace alias.
            refs.insert(format!("@wire:{}", step.id), wire_name);
            if response.get("error").is_some() || response["result"]["isError"] == true { return Err(invalid("approved_step_worker_error")); }
            if !content.is_object() { return Err(invalid("approved_step_structured_content_missing")); }
            let semantic = validate_semantic(runtime, stage, step.tool, &content)?;
            results.insert(step.id.clone(), content);
            semantics.insert(step.id.clone(), semantic);
        }
        runtime.dispatch_allowed()?;
        let mut proof = worker.finish(&runtime.config, &runtime.approval).await?;
        let native_detail = proof.detail.clone();
        proof.detail = json!({"worker":native_detail,"steps":semantics,"actual_worker_version":crate::mcp::VERSION,"actual_19tool_schema_sha256":crate::mcp::schema_sha(),"semantic_native_stage_claimed":runtime.config.backend.is_windows()});
        let receipt = runtime.finish(stage, proof, refs.clone())?;
        Ok(json!({"stage":stage,"stage_receipt":receipt,"complete":true,"response_files":refs,"native_goal_verified":false}))
    }.await;
    if let Err(error) = &result {
        owned_failure(
            runtime,
            &format!("{}_owned_failure:{error}", stage_name(stage)),
        )
        .await;
    }
    result
}

fn lifecycle_template(runtime: &Runtime, stage: Stage) -> io::Result<&Value> {
    let template = &runtime.approval.lifecycle_grant_template;
    if template.is_null() && !runtime.config.backend.is_windows() {
        return Ok(template);
    }
    let stages = template
        .as_object()
        .ok_or_else(|| invalid("lifecycle_templates_require_local_stage_map"))?;
    if stages
        .keys()
        .any(|key| !["bootstrap", "preflight", "launch", "activate"].contains(&key.as_str()))
    {
        return Err(invalid("lifecycle_template_unknown_stage_key"));
    }
    stages
        .get(stage_name(stage))
        .ok_or_else(|| invalid("lifecycle_template_current_stage_missing"))
}

fn cleanup_confirmed(value: &Value) -> bool {
    value["cleanup_unknown"] != true
        && value["owned_workers"].as_array().is_some_and(|rows| {
            rows.iter().all(|row| {
                row["exit_observed"] == true
                    && row["exit_code"] == 0
                    && row["forced"] == false
                    && row["error"].is_null()
            })
        })
}
async fn cleanup(runtime: &Runtime, previous: &str) -> io::Result<Value> {
    let held = {
        let journal = runtime.journal()?;
        let state = journal.ensure_task(&runtime.approval)?;
        let last = state
            .receipts
            .values()
            .last()
            .map(|r| r.digest.as_str())
            .unwrap_or("");
        if previous != last {
            return Err(invalid("cleanup_previous_receipt_mismatch_no_interference"));
        }
        state.hold
    };
    if held {
        // Explicit cleanup of an already held task never claims stage progress.
        let report = bounded_cleanup(runtime).await;
        let _ = runtime
            .journal()?
            .event("held_task_explicit_owned_cleanup", report.clone());
        return Ok(
            json!({"held":true,"complete":false,"stage_receipt":null,"cleanup":report,"hold_cleared":false}),
        );
    }
    no_active_or_live(runtime).await?;
    runtime.begin(Stage::Cleanup, previous)?;
    let result = async {
        runtime.completion_budget_check(Stage::Cleanup)?;
        let report = bounded_cleanup(runtime).await;
        let mut proof = runtime.base_proof();
        proof.resources_released = cleanup_confirmed(&report);
        proof.detail = report;
        let receipt = runtime.finish(Stage::Cleanup, proof, BTreeMap::new())?;
        Ok(json!({"stage":"cleanup","stage_receipt":receipt,"complete":true,"user_application_killed":false,"historical_pid_kill":false}))
    }.await;
    if let Err(error) = &result {
        owned_failure(runtime, &format!("cleanup_owned_failure:{error}")).await;
    }
    result
}
async fn cancel(runtime: &Runtime) -> io::Result<Value> {
    let already_complete = {
        let journal = runtime.journal()?;
        let state = journal.ensure_task(&runtime.approval)?;
        state.complete && !state.hold && state.active.is_none()
    };
    if already_complete {
        // A late Cancel must not revoke a fully completed, matching task or
        // obstruct approve_next. Existing HOLD/active states use the old path.
        return Ok(
            json!({"cancel_requested":false,"already_complete":true,"complete":true,"hold_cleared":false,"native_resources_not_inferred_from_cancel_ack":true}),
        );
    }
    let persisted = runtime.hold("explicit_controller_cancel");
    let report = bounded_cleanup(runtime).await;
    let _ = runtime
        .journal()?
        .event("explicit_cancel_owned_cleanup", report.clone());
    persisted?;
    Ok(
        json!({"cancel_requested":true,"held":true,"complete":false,"cleanup":report,"native_resources_not_inferred_from_cancel_ack":true}),
    )
}

fn native_identity(runtime: &Runtime) -> io::Result<Value> {
    if runtime.config.backend.is_windows() {
        serde_json::to_value(
            &runtime
                .approval
                .native_binding
                .as_ref()
                .ok_or_else(|| invalid("template_native_binding_missing"))?
                .native,
        )
        .map_err(io::Error::other)
    } else {
        Ok(
            json!({"host":"fixture-host","guest":null,"session":"session-1","seat":"seat-0","desktop":"desktop-1"}),
        )
    }
}
fn window(window: &WindowIdentity) -> io::Result<()> {
    if !window.window.starts_with("hwnd:")
        || window.window.len() > 128
        || window.pid == 0
        || window.process_created == 0
        || window.native_epoch.is_empty()
        || window.native_epoch.len() > 256
    {
        return Err(invalid("exact_native_window_identity_required"));
    }
    Ok(())
}
fn inventory(runtime: &Runtime, value: &Value) -> io::Result<Inventory> {
    let inventory: Inventory = serde_json::from_value(value.clone())?;
    if !inventory.complete
        || inventory.windows.len() > rwi_core::lifecycle::MAX_WINDOWS
        || serde_json::to_value(&inventory.identity)? != native_identity(runtime)?
        || inventory.native_windows_enumerated < inventory.windows.len()
    {
        return Err(invalid("complete_bound_native_inventory_required"));
    }
    let mut keys = BTreeSet::new();
    for item in &inventory.windows {
        window(&item.key)?;
        if !keys.insert(serde_json::to_string(&item.key)?) {
            return Err(invalid("duplicate_inventory_window_identity"));
        }
    }
    Ok(inventory)
}
fn preflight(runtime: &Runtime, stage: Stage, value: &Value) -> io::Result<PreflightReceipt> {
    let receipt: PreflightReceipt = serde_json::from_value(value.clone())?;
    let inventory_value = super::receipt::preflight_inventory(&receipt, value)?;
    inventory(runtime, inventory_value)?;
    if !receipt.resources_released
        || receipt.resource_release_uncertain
        || !receipt.native_events.complete
        || !receipt.native_events.hooks_released
        || receipt.reservation_created
        || receipt.launch_dispatched
        || receipt.activation_dispatched
        || receipt.input_dispatched
    {
        return Err(invalid(
            "preflight_readonly_resource_or_coverage_proof_missing",
        ));
    }
    if stage == Stage::Bootstrap {
        if !receipt.ready_for_launch
            && receipt
                .error
                .as_ref()
                .is_none_or(|error| error.code != "existing_window_not_authorized")
        {
            return Err(invalid(
                "bootstrap_not_ready_reason_not_reconcilable_inventory",
            ));
        }
        if receipt.ready_for_launch && receipt.error.is_some() {
            return Err(invalid("bootstrap_ready_error_contradiction"));
        }
    } else if !receipt.ready_for_launch || receipt.error.is_some() {
        return Err(invalid("preflight_ready_true_required"));
    }
    Ok(receipt)
}
fn launch(runtime: &Runtime, value: &Value) -> io::Result<LaunchReceipt> {
    let receipt: LaunchReceipt = serde_json::from_value(value.clone())?;
    if !matches!(
        receipt.state,
        LaunchState::NewWindowObserved | LaunchState::ReusedExistingWindow
    ) || receipt.candidates.len() != 1
        || receipt.error.is_some()
        || !receipt.resources_released
        || receipt.resource_release_uncertain
        || !receipt.native_events.complete
        || !receipt.native_events.hooks_released
        || receipt.input_authority_granted
        || receipt.native_launch.as_ref().is_none_or(|native| {
            !native.accepted || !native.process_handles_released || native.error.is_some()
        })
    {
        return Err(invalid(
            "launch_requires_single_certain_window_and_released_resources",
        ));
    }
    inventory(runtime, &value["before"])?;
    let after = inventory(runtime, &value["after"])?;
    window(&receipt.candidates[0])?;
    if after
        .windows
        .iter()
        .filter(|item| item.key == receipt.candidates[0] && item.interactive_root())
        .count()
        != 1
    {
        return Err(invalid(
            "launch_candidate_not_unique_in_complete_after_inventory",
        ));
    }
    Ok(receipt)
}
fn activation(value: &Value) -> io::Result<ActivationReceipt> {
    let receipt: ActivationReceipt = serde_json::from_value(value.clone())?;
    window(&receipt.window)?;
    if !matches!(
        receipt.state.as_str(),
        "foreground_observed" | "already_foreground"
    ) || receipt.after_foreground.as_ref() != Some(&receipt.window)
        || receipt.user_intervened
        || receipt.error.is_some()
        || !receipt.resources_released
        || receipt.resource_release_uncertain
        || receipt.input_authority_granted
    {
        return Err(invalid("activation_foreground_confirmation_missing"));
    }
    Ok(receipt)
}

/// Epoch authority is the committed, fully verified activation receipt, not a
/// fabricated fourth field attached to the three-field native input root.
pub fn model_root_epoch(runtime: &Runtime, input: &Value) -> io::Result<String> {
    let refs = runtime
        .journal()?
        .ensure_task(&runtime.approval)?
        .receipts
        .get(&Stage::Activate)
        .ok_or_else(|| invalid("model_activation_receipt_missing"))?
        .response_files
        .clone();
    let mut epochs = BTreeSet::new();
    for (step, blob) in refs {
        if step.starts_with('@') {
            continue;
        }
        let value = runtime.response(&blob)?;
        if let Ok(receipt) = activation(&value) {
            let key = json!({"window":receipt.window.window,"pid":receipt.window.pid,"process_created":receipt.window.process_created});
            if key == input["root"] {
                epochs.insert(receipt.window.native_epoch);
            }
        }
    }
    if epochs.len() != 1 {
        return Err(invalid(
            "model_root_requires_unique_committed_activation_epoch",
        ));
    }
    Ok(epochs.into_iter().next().unwrap())
}

pub const CATALOG_MAX_PAGES: usize = 32;
pub const CATALOG_MAX_WINDOWS: usize = 256;
pub const CATALOG_TTL_MS: u64 = 10_000;

#[derive(Clone, Debug, serde::Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogSelection {
    pub catalog_id: String,
    pub foreground: WindowIdentity,
    pub activation_target: WindowIdentity,
    pub matched_windows: usize,
    pub pages: usize,
}

/// Validates original native pages without changing offsets or treating a
/// partial page as evidence of absence. Completion is a property of this chain.
pub struct CatalogChain {
    identity: Value,
    target: WindowIdentity,
    catalog_id: Option<String>,
    header: Option<Value>,
    next: Option<String>,
    count: usize,
    total: usize,
    pages: usize,
    closed: bool,
    windows: BTreeSet<String>,
    foreground: Vec<WindowIdentity>,
    target_count: usize,
}
impl CatalogChain {
    pub fn new(identity: Value, target: WindowIdentity) -> io::Result<Self> {
        window(&target)?;
        Ok(Self {
            identity,
            target,
            catalog_id: None,
            header: None,
            next: None,
            count: 0,
            total: 0,
            pages: 0,
            closed: false,
            windows: BTreeSet::new(),
            foreground: Vec::new(),
            target_count: 0,
        })
    }
    pub fn next_cursor(&self) -> Option<&str> {
        self.next.as_deref()
    }
    pub fn push_at(
        &mut self,
        value: &Value,
        request_cursor: Option<&str>,
        elapsed: Duration,
    ) -> io::Result<()> {
        let rows = value["windows"]
            .as_array()
            .ok_or_else(|| invalid("catalog_rows_missing"))?;
        let catalog = value["catalog_id"]
            .as_str()
            .ok_or_else(|| invalid("catalog_id_missing"))?;
        let total = value["matched_windows"]
            .as_u64()
            .ok_or_else(|| invalid("catalog_total_missing"))? as usize;
        let next = match value.get("next_cursor") {
            Some(Value::Null) => None,
            Some(Value::String(s)) if s.len() <= 256 => Some(s.clone()),
            _ => return Err(invalid("catalog_next_cursor_shape")),
        };
        // MCP adds bounded operation metadata after the backend's 12 KiB page
        // formatter. Check each layer's own bound without altering saved bytes.
        let mut native_page = value.clone();
        if let Some(object) = native_page.as_object_mut() {
            for key in ["operation_id", "operation", "cancellation"] {
                object.remove(key);
            }
        }
        if self.closed
            || self.pages >= CATALOG_MAX_PAGES
            || elapsed >= Duration::from_millis(CATALOG_TTL_MS)
            || value["identity"] != self.identity
            || value["enumeration_complete"] != true
            || value["catalog_ttl_ms"] != CATALOG_TTL_MS
            || !crate::types::digest_shape(catalog)
            || value["offset"].as_u64() != Some(self.count as u64)
            || value["page_count"].as_u64() != Some(rows.len() as u64)
            || value["page_limit"] != 8
            || value["structured_byte_limit"] != 12 * 1024
            || rows.is_empty()
            || rows.len() > 8
            || total == 0
            || total > CATALOG_MAX_WINDOWS
            || self.count + rows.len() > total
            || serde_json::to_vec(&native_page)?.len() > 12 * 1024
            || serde_json::to_vec(value)?.len() > 16 * 1024
            || request_cursor != self.next.as_deref()
        {
            return Err(invalid("catalog_identity_offset_rows_cursor_or_expiry"));
        }
        let mut header = Map::new();
        for field in [
            "identity",
            "enumeration_complete",
            "native_windows_enumerated",
            "catalog_ttl_ms",
            "displays",
            "selected",
            "filter",
            "input_grant_scoped",
            "native_api",
            "coverage_note",
        ] {
            header.insert(field.into(), value[field].clone());
        }
        // This operation deliberately asks for an unfiltered catalog. Filters
        // cannot hide a competing foreground or discard the activation target.
        if value["filter"]
            .as_object()
            .is_none_or(|f| f.values().any(|v| !v.is_null()))
            || value["input_grant_scoped"] != false
            || value["native_windows_enumerated"].as_u64() != Some(total as u64)
        {
            return Err(invalid(
                "catalog_must_be_complete_unfiltered_native_enumeration",
            ));
        }
        let header = Value::Object(header);
        if let Some(id) = &self.catalog_id {
            if id != catalog || self.total != total || self.header.as_ref() != Some(&header) {
                return Err(invalid("catalog_changed_mid_chain"));
            }
        } else {
            self.catalog_id = Some(catalog.into());
            self.total = total;
            self.header = Some(header);
        }
        let end = self.count + rows.len();
        let expected_next = (end < total).then(|| format!("{catalog}/{end}"));
        if next != expected_next || value["complete"] != (self.count == 0 && end == total) {
            return Err(invalid(
                "catalog_missing_page_or_original_completion_mismatch",
            ));
        }
        for row in rows {
            let item = record_window(row)?;
            if !row["foreground"].is_boolean() || !self.windows.insert(item.window.clone()) {
                return Err(invalid("catalog_duplicate_window_or_missing_foreground"));
            }
            if row["foreground"] == true {
                self.foreground.push(item.clone());
            }
            if item == self.target {
                self.target_count += 1;
            }
        }
        self.pages += 1;
        self.count = end;
        self.closed = next.is_none();
        self.next = next;
        Ok(())
    }
    pub fn finish_at(&self, elapsed: Duration) -> io::Result<CatalogSelection> {
        if !self.closed
            || self.count != self.total
            || self.pages == 0
            || elapsed >= Duration::from_millis(CATALOG_TTL_MS)
            || self.foreground.len() != 1
            || self.target_count != 1
        {
            return Err(invalid(
                "catalog_incomplete_expired_ambiguous_or_target_missing",
            ));
        }
        Ok(CatalogSelection {
            catalog_id: self.catalog_id.clone().unwrap(),
            foreground: self.foreground[0].clone(),
            activation_target: self.target.clone(),
            matched_windows: self.total,
            pages: self.pages,
        })
    }
}
fn record_window(row: &Value) -> io::Result<WindowIdentity> {
    let key = serde_json::from_value(
        json!({"window":row["window"],"pid":row["pid"],"process_created":row["process_created"],"native_epoch":row["native_epoch"]}),
    )?;
    window(&key)?;
    Ok(key)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogArguments {
    target: String,
    activation_target: WindowIdentity,
}
#[derive(serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogPageRef {
    request_cursor: Option<String>,
    structured_blob: String,
    wire_blob: String,
    raw_sha256: String,
}
#[derive(serde::Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogAggregate {
    kind: String,
    started_unix_ms: u64,
    worker_binding_sha256: String,
    selection: CatalogSelection,
    pages: Vec<CatalogPageRef>,
}
fn worker_binding(runtime: &Runtime) -> io::Result<String> {
    let j = runtime.journal()?;
    let state = j.ensure_task(&runtime.approval)?;
    if state.active != Some(Stage::Activate) {
        return Err(invalid("catalog_only_current_activate_worker"));
    }
    let worker = state
        .live_worker_record
        .as_ref()
        .ok_or_else(|| invalid("catalog_live_worker_missing"))?;
    Ok(sha(&serde_json::to_vec(worker)?))
}
async fn collect_catalog(
    runtime: &Runtime,
    worker: &mut crate::worker::Worker,
    args: &Value,
    step: &str,
    rpc_id: &mut u64,
) -> io::Result<Value> {
    let arguments: CatalogArguments = serde_json::from_value(args.clone())?;
    if arguments.target != runtime.config.backend.target() {
        return Err(invalid("catalog_foreign_target"));
    }
    let started = Instant::now();
    let started_unix_ms = now_ms();
    let binding = worker_binding(runtime)?;
    let mut chain = CatalogChain::new(native_identity(runtime)?, arguments.activation_target)?;
    let mut pages = Vec::new();
    loop {
        runtime.dispatch_allowed()?;
        worker.budget.check()?;
        if started.elapsed() >= Duration::from_millis(CATALOG_TTL_MS) {
            return Err(invalid("catalog_expired_before_next_page"));
        }
        let cursor = chain.next_cursor().map(str::to_owned);
        let mut arguments = json!({"target":arguments.target});
        if let Some(cursor) = &cursor {
            arguments["cursor"] = cursor.clone().into();
        }
        let rpc = json!({"jsonrpc":"2.0","id":*rpc_id,"method":"tools/call","params":{"name":"computer.discover","arguments":arguments}});
        *rpc_id += 1;
        let mut request = serde_json::to_vec(&rpc)?;
        request.push(b'\n');
        let reservation = format!(
            "catalog-{}",
            sha(&serde_json::to_vec(&(
                runtime.approval.task.as_str(),
                step,
                pages.len()
            ))?)
        );
        runtime.journal()?.reserve(&reservation, &request)?;
        let response = tokio::time::timeout(
            Duration::from_millis(CATALOG_TTL_MS).saturating_sub(started.elapsed()),
            worker.rpc(rpc),
        )
        .await
        .map_err(|_| invalid("catalog_whole_chain_deadline"))??;
        let content = response["result"]
            .get("structuredContent")
            .cloned()
            .unwrap_or(Value::Null);
        let page = CatalogPageRef {
            request_cursor: cursor.clone(),
            structured_blob: runtime.save_response(&content)?,
            wire_blob: runtime.save_response(&response)?,
            raw_sha256: worker
                .ledger
                .response_digests
                .last()
                .cloned()
                .ok_or_else(|| invalid("catalog_raw_sha_missing"))?,
        };
        runtime.journal()?.event(
            "catalog_original_page_saved_before_validation",
            json!({"step":step,"page":pages.len(),"reference":page}),
        )?;
        if response.get("error").is_some() || response["result"]["isError"] == true {
            return Err(invalid("catalog_original_worker_error"));
        }
        chain.push_at(&content, cursor.as_deref(), started.elapsed())?;
        pages.push(page);
        if chain.next_cursor().is_none() {
            break;
        }
    }
    let selection = chain.finish_at(started.elapsed())?;
    serde_json::to_value(CatalogAggregate {
        kind: "catalog_chain_v2".into(),
        started_unix_ms,
        worker_binding_sha256: binding,
        selection,
        pages,
    })
    .map_err(io::Error::other)
}
fn catalog_selection(runtime: &Runtime, value: &Value) -> io::Result<CatalogSelection> {
    runtime.dispatch_allowed()?;
    let aggregate: CatalogAggregate = serde_json::from_value(value.clone())?;
    let elapsed = now_ms()
        .checked_sub(aggregate.started_unix_ms)
        .ok_or_else(|| invalid("catalog_wall_rollback"))?;
    if aggregate.kind != "catalog_chain_v2"
        || aggregate.pages.is_empty()
        || aggregate.pages.len() > CATALOG_MAX_PAGES
        || aggregate.worker_binding_sha256 != worker_binding(runtime)?
    {
        return Err(invalid("catalog_aggregate_kind_worker_or_bound"));
    }
    let mut chain = CatalogChain::new(
        native_identity(runtime)?,
        aggregate.selection.activation_target.clone(),
    )?;
    for page in &aggregate.pages {
        let wire = runtime.response(&page.wire_blob)?;
        let content = runtime.response(&page.structured_blob)?;
        if wire["result"]["structuredContent"] != content
            || !crate::types::digest_shape(&page.raw_sha256)
        {
            return Err(invalid("catalog_page_wire_content_binding"));
        }
        // The raw response is content-addressed separately by Worker::rpc.
        let raw = runtime.responses.read(
            &format!("wire-{}.ndjson", page.raw_sha256),
            crate::frame::OUTBOUND,
        )?;
        if sha(&raw) != page.raw_sha256 || serde_json::from_slice::<Value>(&raw)? != wire {
            return Err(invalid("catalog_original_raw_binding"));
        }
        chain.push_at(
            &content,
            page.request_cursor.as_deref(),
            Duration::from_millis(elapsed),
        )?;
    }
    let selection = chain.finish_at(Duration::from_millis(elapsed))?;
    if selection != aggregate.selection {
        return Err(invalid("catalog_aggregate_not_original_pages"));
    }
    Ok(selection)
}
fn foreground_window(runtime: &Runtime, value: &Value) -> io::Result<WindowIdentity> {
    Ok(catalog_selection(runtime, value)?.foreground)
}

fn validate_semantic(
    runtime: &Runtime,
    stage: Stage,
    tool: ControlTool,
    value: &Value,
) -> io::Result<Value> {
    match tool {
        ControlTool::LifecyclePreflight => {
            let receipt = preflight(runtime, stage, value)?;
            Ok(
                json!({"readonly_preflight":true,"ready_for_launch":receipt.ready_for_launch,"complete_inventory":true}),
            )
        }
        ControlTool::LaunchApplication => {
            let receipt = launch(runtime, value)?;
            Ok(
                json!({"launch_state":receipt.state,"candidate_count":1,"native_goal_verified":false}),
            )
        }
        ControlTool::ActivateWindow => {
            activation(value)?;
            Ok(json!({"foreground_confirmed":true,"native_goal_verified":false}))
        }
        ControlTool::Verify if value["goal_verified"] != true => {
            Err(invalid("approved_verify_predicate_not_satisfied"))
        }
        _ => Ok(json!({"structured_response_received":true,"native_goal_inferred":false})),
    }
}

#[derive(Default)]
struct Limits {
    nodes: usize,
    bytes: usize,
    visits: usize,
}
impl Limits {
    fn visit(&mut self, depth: usize) -> io::Result<()> {
        self.visits += 1;
        if depth > TEMPLATE_DEPTH || self.visits > TEMPLATE_NODES {
            return Err(invalid("template_depth_or_node_limit"));
        }
        Ok(())
    }
    fn scalar(&mut self, value: &Value, depth: usize) -> io::Result<()> {
        self.nodes += 1;
        if depth > TEMPLATE_DEPTH || self.nodes > TEMPLATE_NODES {
            return Err(invalid("template_output_depth_or_node_limit"));
        }
        match value {
            Value::String(s) => {
                if s.len() > TEMPLATE_STRING {
                    return Err(invalid("template_string_limit"));
                }
                self.bytes += s.len();
            }
            Value::Number(n) => self.bytes += n.to_string().len(),
            _ => self.bytes += 4,
        }
        if self.bytes > TEMPLATE_BYTES {
            return Err(invalid("template_output_byte_limit"));
        }
        Ok(())
    }
    fn opaque(&mut self, value: &Value, depth: usize) -> io::Result<()> {
        self.scalar(value, depth)?;
        match value {
            Value::Array(values) => {
                for value in values {
                    self.opaque(value, depth + 1)?;
                }
            }
            Value::Object(values) => {
                for (key, value) in values {
                    self.scalar(&Value::String(key.clone()), depth + 1)?;
                    self.opaque(value, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResultRef {
    stage: String,
    step: String,
    pointer: String,
}
fn result_ref(
    runtime: &Runtime,
    value: &Value,
    local: &BTreeMap<String, Value>,
) -> io::Result<Value> {
    let reference: ResultRef = serde_json::from_value(value.clone())?;
    if !identifier(&reference.step, 64)
        || reference.pointer.len() > 2048
        || (!reference.pointer.is_empty() && !reference.pointer.starts_with('/'))
        || reference.pointer.split('/').count() > 33
    {
        return Err(invalid("template_result_reference_shape"));
    }
    let mut chars = reference.pointer.chars();
    while let Some(c) = chars.next() {
        if c == '~' && !matches!(chars.next(), Some('0' | '1')) {
            return Err(invalid("template_json_pointer_escape"));
        }
    }
    let value = if reference.stage == "current" {
        local
            .get(&reference.step)
            .cloned()
            .ok_or_else(|| invalid("template_current_result_not_completed"))?
    } else {
        let stage: Stage = serde_json::from_value(Value::String(reference.stage))?;
        let name = {
            let journal = runtime.journal()?;
            let state = journal.ensure_task(&runtime.approval)?;
            state
                .receipts
                .get(&stage)
                .and_then(|receipt| receipt.response_files.get(&reference.step))
                .cloned()
                .ok_or_else(|| invalid("template_prior_stage_result_not_committed"))?
        };
        runtime.response(&name)?
    };
    value
        .pointer(&reference.pointer)
        .cloned()
        .ok_or_else(|| invalid("template_json_pointer_missing"))
}

/// Expand only the local Approval's template language. A returned result is
/// opaque data even if it contains '$...' keys: it is never evaluated again.
pub fn expand(
    runtime: &Runtime,
    template: &Value,
    local_results: &BTreeMap<String, Value>,
    expires_ms: u64,
) -> io::Result<Value> {
    runtime.live_budget()?;
    if expires_ms <= now_ms() || expires_ms > runtime.approval.budget.expires_unix_ms {
        return Err(invalid("template_expiry_not_within_local_approval"));
    }
    let mut limits = Limits::default();
    let value = walk(runtime, template, local_results, expires_ms, 0, &mut limits)?;
    if serde_json::to_vec(&value)?.len() > TEMPLATE_BYTES {
        return Err(invalid("template_serialized_output_limit"));
    }
    Ok(value)
}
fn walk(
    runtime: &Runtime,
    template: &Value,
    local: &BTreeMap<String, Value>,
    expires: u64,
    depth: usize,
    limits: &mut Limits,
) -> io::Result<Value> {
    limits.visit(depth)?;
    match template {
        Value::Object(fields) if fields.keys().any(|key| key.starts_with('$')) => {
            if fields.len() != 1 {
                return Err(invalid("template_tag_must_be_exact_single_key"));
            }
            let (tag, argument) = fields.iter().next().unwrap();
            let value = match tag.as_str() {
                "$file" => {
                    let name = argument
                        .as_str()
                        .ok_or_else(|| invalid("template_file_name_string"))?;
                    if !runtime.approval.files.iter().any(|file| file.name == name) {
                        return Err(invalid("template_file_not_allowlisted"));
                    }
                    serde_json::to_value(runtime.files()?.child(name)?)?
                }
                "$lease" if argument == &Value::Bool(true) => {
                    let leases = local
                        .values()
                        .filter_map(|value| value["lease"].as_str())
                        .collect::<BTreeSet<_>>();
                    if leases.len() != 1 {
                        return Err(invalid("template_current_lease_missing_or_ambiguous"));
                    }
                    Value::String((*leases.first().unwrap()).into())
                }
                "$native_identity" if argument == &Value::Bool(true) => native_identity(runtime)?,
                "$expires" if argument == &Value::Bool(true) => json!(expires),
                "$guest_claim" if argument == &Value::Bool(true) => {
                    serde_json::to_value(&runtime.approval.guest_claim)?
                }
                "$result" => result_ref(runtime, argument, local)?,
                "$window_keys" => {
                    let source = walk(runtime, argument, local, expires, depth + 1, limits)?;
                    let value = if source.get("inventory").is_some() {
                        &source["inventory"]
                    } else {
                        &source
                    };
                    let inventory = inventory(runtime, value)?;
                    serde_json::to_value(
                        inventory
                            .windows
                            .into_iter()
                            .map(|window| window.key)
                            .collect::<Vec<_>>(),
                    )?
                }
                "$one_candidate" => {
                    let source = walk(runtime, argument, local, expires, depth + 1, limits)?;
                    serde_json::to_value(&launch(runtime, &source)?.candidates[0])?
                }
                "$input_root" => {
                    let source = walk(runtime, argument, local, expires, depth + 1, limits)?;
                    let receipt = activation(&source)?;
                    json!({"window":receipt.window.window,"pid":receipt.window.pid,"process_created":receipt.window.process_created})
                }
                "$foreground_window" => {
                    let source = walk(runtime, argument, local, expires, depth + 1, limits)?;
                    serde_json::to_value(foreground_window(runtime, &source)?)?
                }
                _ => return Err(invalid("unknown_or_malformed_local_template_tag")),
            };
            limits.opaque(&value, depth)?;
            Ok(value)
        }
        Value::Object(fields) => {
            limits.scalar(&Value::Null, depth)?;
            let mut result = Map::new();
            for (key, value) in fields {
                limits.scalar(&Value::String(key.clone()), depth + 1)?;
                result.insert(
                    key.clone(),
                    walk(runtime, value, local, expires, depth + 1, limits)?,
                );
            }
            Ok(Value::Object(result))
        }
        Value::Array(values) => {
            limits.scalar(&Value::Null, depth)?;
            Ok(Value::Array(
                values
                    .iter()
                    .map(|value| walk(runtime, value, local, expires, depth + 1, limits))
                    .collect::<io::Result<_>>()?,
            ))
        }
        value => {
            limits.scalar(value, depth)?;
            Ok(value.clone())
        }
    }
}
