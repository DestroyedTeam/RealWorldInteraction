//! Append-only hash-chained stage journal. Native effects cannot be reset by a new task.
use crate::{
    invalid,
    private::Directory,
    sha,
    types::{Approval, Stage},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::Path,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EndProof {
    pub requests_terminal: bool,
    pub operations_terminal: bool,
    pub resources_released: bool,
    pub leases_released: bool,
    pub mutation_uncertain: bool,
    pub owned_worker_exit_observed: bool,
    pub pipe_peer_bound: bool,
    pub native_binding_unchanged: bool,
    pub model_goal_verified: bool,
    pub file_oracle_verified: bool,
    pub worker_exit_code: Option<i32>,
    pub evidence_level: String,
    pub operation_ids: Vec<String>,
    pub response_digests: Vec<String>,
    pub detail: Value,
}
impl EndProof {
    pub fn permits(&self, stage: Stage, native: bool) -> bool {
        self.requests_terminal
            && self.operations_terminal
            && self.resources_released
            && self.leases_released
            && !self.mutation_uncertain
            && (!stage.worker_stage()
                || self.owned_worker_exit_observed
                    && self.worker_exit_code == Some(0)
                    && self.pipe_peer_bound)
            && (!native || self.native_binding_unchanged && self.evidence_level == "windows_native")
            && (stage != Stage::Model || self.model_goal_verified)
            && (stage != Stage::Oracle || self.file_oracle_verified)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageReceipt {
    pub stage: Stage,
    pub proof: EndProof,
    pub response_files: BTreeMap<String, String>,
    pub digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub task: String,
    pub approval_sha256: String,
    pub next: Stage,
    pub active: Option<Stage>,
    pub hold: bool,
    pub hold_reason: Option<String>,
    pub complete: bool,
    pub receipts: BTreeMap<Stage, StageReceipt>,
    pub reservations: BTreeSet<String>,
    pub live_worker_record: Option<Value>,
    pub native_mode: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    version: u8,
    sequence: u64,
    previous: String,
    kind: String,
    detail: Value,
    state: State,
}
pub struct Store {
    directory: Directory,
    sequence: u64,
    last: String,
    pub state: Option<State>,
}
impl Store {
    pub fn open(path: &Path) -> io::Result<Self> {
        let directory = if path.exists() {
            Directory::open(path)?
        } else {
            Directory::create(path)?
        };
        let mut names = Vec::new();
        for e in std::fs::read_dir(path)? {
            let e = e?;
            let name = e
                .file_name()
                .into_string()
                .map_err(|_| invalid("journal_name_encoding"))?;
            if !name.starts_with("event-") || !name.ends_with(".json") || name.len() != 19 {
                return Err(invalid("unexpected_journal_file_hold"));
            }
            names.push(name);
        }
        names.sort();
        if names.len() > 8192 {
            return Err(invalid("journal_limit_hold"));
        }
        let mut s = Self {
            directory,
            sequence: 0,
            last: String::new(),
            state: None,
        };
        for name in names {
            let bytes = s.directory.read(&name, 1024 * 1024)?;
            let e: Event = serde_json::from_slice(&bytes)?;
            if e.version != 1
                || e.sequence != s.sequence + 1
                || e.previous != s.last
                || name != format!("event-{:08}.json", e.sequence)
            {
                return Err(invalid("journal_chain_or_gap_hold"));
            }
            s.sequence = e.sequence;
            s.last = sha(&bytes);
            s.state = Some(e.state);
        }
        if s.state.as_ref().is_some_and(|v| !v.complete && !v.hold) {
            s.hold("agent_restart_unfinished_task")?;
        }
        Ok(s)
    }
    fn persist(&mut self, kind: &str, detail: Value) -> io::Result<()> {
        if self.sequence >= 8192 {
            return Err(invalid("journal_event_limit_hold"));
        }
        let state = self.state.clone().ok_or_else(|| invalid("no_task_state"))?;
        let bytes = serde_json::to_vec(&Event {
            version: 1,
            sequence: self.sequence + 1,
            previous: self.last.clone(),
            kind: kind.into(),
            detail,
            state,
        })?;
        if let Err(e) = self.directory.create_new(
            &format!("event-{:08}.json", self.sequence + 1),
            &bytes,
            1024 * 1024,
        ) {
            if let Some(s) = &mut self.state {
                s.hold = true;
                s.hold_reason = Some("journal_write_failed_unknown".into());
            }
            return Err(e);
        }
        self.sequence += 1;
        self.last = sha(&bytes);
        Ok(())
    }
    pub fn start_task(&mut self, a: &Approval, native: bool) -> io::Result<()> {
        if let Some(s) = &self.state {
            if !s.complete {
                return Err(invalid("installation_has_unfinished_or_held_task"));
            }
            if s.task == a.task {
                return Err(invalid("completed_task_cannot_be_replayed"));
            }
        }
        self.state = Some(State {
            task: a.task.clone(),
            approval_sha256: a.digest(),
            next: Stage::Prepare,
            active: None,
            hold: false,
            hold_reason: None,
            complete: false,
            receipts: BTreeMap::new(),
            reservations: BTreeSet::new(),
            live_worker_record: None,
            native_mode: native,
        });
        self.persist("local_task_admitted", Value::Null)
    }
    pub fn ensure_task(&self, a: &Approval) -> io::Result<&State> {
        let s = self
            .state
            .as_ref()
            .ok_or_else(|| invalid("task_not_prepared"))?;
        if s.task != a.task || s.approval_sha256 != a.digest() {
            return Err(invalid("task_or_manifest_mismatch"));
        }
        Ok(s)
    }
    pub fn begin(&mut self, a: &Approval, stage: Stage, previous: &str) -> io::Result<()> {
        let s = self.ensure_task(a)?;
        if s.hold || s.complete || s.active.is_some() || s.next != stage {
            return Err(invalid("stage_held_active_or_out_of_order"));
        }
        let last = s
            .receipts
            .values()
            .last()
            .map(|v| v.digest.as_str())
            .unwrap_or("");
        if last != previous {
            return Err(invalid("previous_stage_proof_mismatch"));
        }
        self.state.as_mut().unwrap().active = Some(stage);
        self.persist(
            "stage_intent_before_worker_or_files",
            serde_json::json!({"stage":stage}),
        )
    }
    pub fn worker(&mut self, record: Value) -> io::Result<()> {
        self.state
            .as_mut()
            .ok_or_else(|| invalid("task_missing"))?
            .live_worker_record = Some(record.clone());
        self.persist("owned_worker_retained", record)
    }
    pub fn reserve(&mut self, request: &str, bytes: &[u8]) -> io::Result<()> {
        let s = self.state.as_mut().ok_or_else(|| invalid("task_missing"))?;
        if s.hold
            || s.active.is_none()
            || request.is_empty()
            || request.len() > 128
            || s.reservations.len() >= 1024
            || !s.reservations.insert(request.into())
        {
            return Err(invalid("request_reuse_or_hold"));
        }
        self.persist(
            "request_reserved_before_forward",
            serde_json::json!({"request":request,"sha256":sha(bytes),"bytes":bytes.len()}),
        )
    }
    pub fn event(&mut self, kind: &str, detail: Value) -> io::Result<()> {
        self.persist(kind, detail)
    }
    pub fn hold(&mut self, reason: &str) -> io::Result<()> {
        if let Some(s) = &mut self.state {
            s.hold = true;
            s.hold_reason = Some(reason.chars().take(256).collect());
        } else {
            return Err(invalid("hold_without_task"));
        }
        self.persist(
            "held_no_automatic_reconciliation",
            serde_json::json!({"reason":reason}),
        )
    }
    pub fn finish(
        &mut self,
        stage: Stage,
        proof: EndProof,
        response_files: BTreeMap<String, String>,
    ) -> io::Result<String> {
        let s = self.state.as_ref().ok_or_else(|| invalid("task_missing"))?;
        if s.hold || s.active != Some(stage) || !proof.permits(stage, s.native_mode) {
            self.hold("stage_end_evidence_incomplete")?;
            return Err(invalid("stage_end_evidence_incomplete"));
        }
        if stage == Stage::Cleanup
            && !s
                .receipts
                .get(&Stage::Oracle)
                .is_some_and(|r| r.proof.file_oracle_verified)
        {
            self.hold("cleanup_without_final_file_oracle")?;
            return Err(invalid("cleanup_without_final_file_oracle"));
        }
        let digest = sha(&serde_json::to_vec(&(
            s.task.as_str(),
            stage,
            &proof,
            &response_files,
        ))?);
        let s = self.state.as_mut().unwrap();
        s.receipts.insert(
            stage,
            StageReceipt {
                stage,
                proof,
                response_files,
                digest: digest.clone(),
            },
        );
        s.active = None;
        s.live_worker_record = None;
        if let Some(next) = stage.next() {
            s.next = next;
        } else {
            s.complete = true;
        }
        self.persist(
            "normal_stage_complete_with_evidence",
            serde_json::json!({"stage":stage,"receipt_sha256":digest}),
        )?;
        Ok(digest)
    }
    pub fn public_status(&self) -> Value {
        serde_json::json!({"state":self.state,"journal_sequence":self.sequence,"journal_head_sha256":self.last,"reconciliation":"read_only; does not clear HOLD","resource_release_from_process_exit_alone":false})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        budget::{BudgetSpec, now_ms},
        types::{BackendMode, FileSpec},
    };
    fn approval() -> Approval {
        let now = now_ms();
        Approval {
            version: 1,
            task: "offline-task".into(),
            controller_host: "simulated-controller".into(),
            guest_claim: None,
            native_binding: None,
            budget: BudgetSpec {
                issued_unix_ms: now,
                expires_unix_ms: now + 10000,
                maximum_elapsed_ms: 10000,
                clock_drift_tolerance_ms: 100,
            },
            request_prefix: "offline-task-".into(),
            files: vec![FileSpec {
                name: "goal.txt".into(),
                initial_utf8: String::new(),
                expected_sha256: sha(b"goal"),
            }],
            expected_ui_value: "goal".into(),
            stages: BTreeMap::new(),
            input_grant_template: Value::Null,
            lifecycle_grant_template: Value::Null,
            fixture_control: Value::Null,
        }
    }
    fn good() -> EndProof {
        EndProof {
            requests_terminal: true,
            operations_terminal: true,
            resources_released: true,
            leases_released: true,
            mutation_uncertain: false,
            owned_worker_exit_observed: true,
            pipe_peer_bound: true,
            native_binding_unchanged: true,
            model_goal_verified: true,
            file_oracle_verified: true,
            worker_exit_code: Some(0),
            evidence_level: "simulated_policy_only".into(),
            operation_ids: vec![],
            response_digests: vec![],
            detail: Value::Null,
        }
    }
    #[test]
    fn complete_stage_sequence_needs_more_than_eof_and_allows_new_approved_task() {
        let tmp = tempfile::tempdir().unwrap();
        let mut s = Store::open(&tmp.path().canonicalize().unwrap().join("journal")).unwrap();
        let a = approval();
        let _ = BackendMode::Windows;
        s.start_task(&a, false).unwrap();
        let mut previous = String::new();
        let mut stage = Stage::Prepare;
        loop {
            s.begin(&a, stage, &previous).unwrap();
            previous = s.finish(stage, good(), BTreeMap::new()).unwrap();
            if let Some(n) = stage.next() {
                stage = n
            } else {
                break;
            }
        }
        assert!(s.state.as_ref().unwrap().complete);
        assert!(s.start_task(&a, false).is_err());
        let mut new = a.clone();
        new.task = "new-task".into();
        new.request_prefix = "new-task-".into();
        s.start_task(&new, false).unwrap();
    }
    #[test]
    fn native_exit_code_without_resource_proof_never_finishes() {
        let tmp = tempfile::tempdir().unwrap();
        let mut s = Store::open(&tmp.path().canonicalize().unwrap().join("journal")).unwrap();
        let a = approval();
        s.start_task(&a, true).unwrap();
        s.begin(&a, Stage::Prepare, "").unwrap();
        let mut p = good();
        p.evidence_level = "windows_native".into();
        p.resources_released = false;
        assert!(s.finish(Stage::Prepare, p, BTreeMap::new()).is_err());
        assert!(s.state.as_ref().unwrap().hold);
        assert!(s.start_task(&a, true).is_err());
    }
    #[test]
    fn restart_and_duplicate_payload_are_durable_hold_not_new_root_permission() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().canonicalize().unwrap().join("journal");
        let a = approval();
        {
            let mut s = Store::open(&path).unwrap();
            s.start_task(&a, false).unwrap();
            s.begin(&a, Stage::Prepare, "").unwrap();
            s.reserve("one", b"first").unwrap();
            assert!(s.reserve("one", b"different").is_err());
        }
        let s = Store::open(&path).unwrap();
        assert!(s.state.unwrap().hold);
    }
}
