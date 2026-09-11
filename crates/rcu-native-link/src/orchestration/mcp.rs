//! Tracks the real unchanged worker's MCP contract and completion evidence.
use crate::{invalid, sha, store::EndProof};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};

pub const VERSION: &str = "0.4.0-m3a.7";
pub const INSTRUCTIONS: &str = include_str!("../../fixtures/instructions.txt");
pub fn schema_sha() -> String {
    serde_json::from_str::<Value>(include_str!("../../fixtures/golden-schema.json")).unwrap()["tools_canonical_sha256"].as_str().unwrap().into()
}
#[derive(Clone, Debug)]
pub struct Pending {
    pub method: String,
    pub mutation: bool,
    pub control: bool,
    pub operation: Option<String>,
    pub args: Value,
}
pub struct Ledger {
    initialized: bool,
    notified: bool,
    schema_checked: bool,
    seen: BTreeSet<String>,
    pub pending: BTreeMap<String, Pending>,
    pub busy: bool,
    busy_operation: Option<String>,
    pub mutation_hold: bool,
    frames: BTreeSet<String>,
    artifacts: BTreeSet<String>,
    views: BTreeSet<String>,
    leases: BTreeSet<String>,
    operations: BTreeMap<String, Value>,
    pub target: String,
    pub identity: Value,
    pub model_only: bool,
    pub request_prefix: String,
    pub expected_goal: String,
    pub goal_verified: bool,
    pub response_digests: Vec<String>,
    pub input_requests: BTreeSet<String>,
    pub frames_seen: usize,
}
fn string<'a>(v: &'a Value, k: &str) -> io::Result<&'a str> {
    v[k].as_str()
        .ok_or_else(|| invalid(&format!("missing_{k}")))
}
impl Ledger {
    pub fn new(target: &str, identity: Value, model_only: bool, prefix: &str, goal: &str) -> Self {
        Self {
            initialized: false,
            notified: false,
            schema_checked: false,
            seen: BTreeSet::new(),
            pending: BTreeMap::new(),
            busy: false,
            busy_operation: None,
            mutation_hold: false,
            frames: BTreeSet::new(),
            artifacts: BTreeSet::new(),
            views: BTreeSet::new(),
            leases: BTreeSet::new(),
            operations: BTreeMap::new(),
            target: target.into(),
            identity,
            model_only,
            request_prefix: prefix.into(),
            expected_goal: goal.into(),
            goal_verified: false,
            response_digests: vec![],
            input_requests: BTreeSet::new(),
            frames_seen: 0,
        }
    }
    pub fn request(&mut self, v: &Value) -> io::Result<Option<Pending>> {
        self.frames_seen += 1;
        if self.frames_seen > 1024 {
            return Err(invalid("session_request_budget"));
        }
        if v["jsonrpc"] != "2.0" {
            return Err(invalid("invalid_jsonrpc"));
        }
        let method = string(v, "method")?;
        if method == "notifications/initialized" {
            if !self.initialized || self.notified || v.get("id").is_some() {
                return Err(invalid("initialize_order"));
            }
            self.notified = true;
            return Ok(None);
        }
        if method == "notifications/cancelled" {
            let id = serde_json::to_string(&v["params"]["requestId"])?;
            if !self.pending.contains_key(&id) {
                return Err(invalid("foreign_cancel_request"));
            }
            return Ok(None);
        }
        let id = v.get("id").ok_or_else(|| invalid("request_id_missing"))?;
        if (!id.is_string() && !id.is_i64()) || id.to_string().len() > 130 {
            return Err(invalid("request_id_shape"));
        }
        let key = serde_json::to_string(id)?;
        if self.seen.contains(&key) || self.seen.len() >= 512 {
            return Err(invalid("jsonrpc_id_reuse_or_limit"));
        }
        let mut p = Pending {
            method: method.into(),
            mutation: false,
            control: false,
            operation: None,
            args: Value::Null,
        };
        match method {
            "initialize"
                if !self.initialized && self.pending.is_empty() && self.seen.is_empty() => {}
            "tools/list" if self.notified => {}
            "tools/call" if self.schema_checked => {
                let name = string(&v["params"], "name")?;
                let a = &v["params"]["arguments"];
                p.method = name.into();
                p.args = a.clone();
                p.control = matches!(name, "computer.operation" | "computer.cancel");
                p.mutation = matches!(
                    name,
                    "computer.act_checked"
                        | "computer.launch_application"
                        | "computer.activate_window"
                );
                if name == "computer.act"
                    || (self.model_only
                        && matches!(
                            name,
                            "computer.launch_application"
                                | "computer.activate_window"
                                | "computer.lifecycle_preflight"
                                | "computer.lifecycle_receipt"
                        ))
                {
                    return Err(invalid("tool_not_granted_in_model_stage"));
                }
                if !matches!(
                    name,
                    "computer.targets"
                        | "computer.observe"
                        | "computer.discover"
                        | "computer.bind_window"
                        | "computer.claim"
                        | "computer.act_checked"
                        | "computer.verify"
                        | "computer.release"
                        | "computer.resolve"
                        | "computer.resolve_view"
                        | "computer.image"
                        | "computer.artifact"
                        | "computer.operation"
                        | "computer.cancel"
                        | "computer.launch_application"
                        | "computer.activate_window"
                        | "computer.lifecycle_preflight"
                        | "computer.lifecycle_receipt"
                ) {
                    return Err(invalid("unknown_tool"));
                }
                if self.mutation_hold && (p.mutation || name == "computer.claim") {
                    return Err(invalid("mutation_uncertain_hold"));
                }
                if let Some(t) = a.get("target")
                    && t != &self.target
                {
                    return Err(invalid("foreign_target"));
                }
                for (field, set) in [
                    ("frame", &self.frames),
                    ("artifact_id", &self.artifacts),
                    ("view_id", &self.views),
                    ("lease", &self.leases),
                ] {
                    if let Some(x) = a.get(field)
                        && !x.as_str().is_some_and(|s| set.contains(s))
                    {
                        return Err(invalid(&format!("foreign_{field}")));
                    }
                }
                if p.control {
                    let op = string(a, "operation_id")?;
                    if !self.operations.contains_key(op) {
                        return Err(invalid("foreign_operation"));
                    }
                    if self.pending.values().filter(|v| v.control).count() >= 4 {
                        return Err(invalid("control_lane_full"));
                    }
                    p.operation = Some(op.into());
                } else if let Some(op) = a.get("operation_id").filter(|v| !v.is_null()) {
                    let op = op.as_str().ok_or_else(|| invalid("operation_id_shape"))?;
                    if op.len() > 128 || self.operations.contains_key(op) {
                        return Err(invalid("operation_id_reuse"));
                    }
                    p.operation = Some(op.into());
                }
                if name == "computer.act_checked" {
                    let request = string(a, "request_id")?;
                    if !request.starts_with(&self.request_prefix)
                        || request.len() > 128
                        || !self.input_requests.insert(request.into())
                    {
                        return Err(invalid("input_request_task_prefix_or_reuse"));
                    }
                }
            }
            _ => return Err(invalid("protocol_order_or_method")),
        }
        if !p.control && (self.busy || self.pending.values().any(|v| !v.control)) {
            return Err(invalid("single_execution_slot_busy"));
        }
        if !p.control {
            self.busy = true;
            self.busy_operation = p.operation.clone();
        }
        if let Some(op) = &p.operation {
            self.operations
                .entry(op.clone())
                .or_insert(json!({"terminal":false,"resources_released":false}));
        }
        if p.mutation {
            self.goal_verified = false;
        }
        self.seen.insert(key.clone());
        self.pending.insert(key, p.clone());
        Ok(Some(p))
    }
    pub fn reply(&mut self, v: &Value, raw: &[u8]) -> io::Result<Pending> {
        if v["jsonrpc"] != "2.0" {
            return Err(invalid("worker_stdout_pollution"));
        }
        let id = serde_json::to_string(v.get("id").ok_or_else(|| invalid("worker_unsolicited"))?)?;
        let p = self
            .pending
            .remove(&id)
            .ok_or_else(|| invalid("worker_unknown_response"))?;
        let result = &v["result"];
        let d = &result["structuredContent"];
        if p.method == "initialize" {
            if result["serverInfo"]["name"] != "real-computer-use"
                || result["serverInfo"]["version"] != VERSION
                || result["instructions"] != INSTRUCTIONS.trim_end()
                || INSTRUCTIONS.trim_end().len() > 512
            {
                return Err(invalid("worker_version_or_instructions_mismatch"));
            }
            self.initialized = true;
        }
        if p.method == "tools/list" {
            let mut tools = result["tools"]
                .as_array()
                .ok_or_else(|| invalid("worker_missing_schema"))?
                .clone();
            tools.sort_by_key(|t| t["name"].as_str().unwrap_or("").to_owned());
            if tools.len() != 19 || sha(&serde_json::to_vec(&tools)?) != schema_sha() {
                return Err(invalid("worker_full_schema_mismatch"));
            }
            self.schema_checked = true;
        }
        if p.method == "computer.targets" {
            let targets = d["targets"]
                .as_array()
                .ok_or_else(|| invalid("worker_targets_unknown"))?;
            if targets.len() != 1
                || targets[0]["target"] != self.target
                || targets[0]["identity"] != self.identity
            {
                return Err(invalid("worker_native_identity_mismatch"));
            }
        }
        let failed = v.get("error").is_some() || result["isError"] == true;
        if p.mutation
            && (failed
                || (p.method == "computer.act_checked"
                    && (d["dispatch"] != "returned" || !d["dispatch_error"].is_null()))
                || d["state"] == "uncertain"
                || d["resource_release_uncertain"] == true)
        {
            self.mutation_hold = true;
        }
        for (field, set) in [
            ("frame", &mut self.frames),
            ("artifact_id", &mut self.artifacts),
            ("view_id", &mut self.views),
        ] {
            if let Some(x) = d[field].as_str() {
                set.insert(x.into());
            }
        }
        if !failed && p.method == "computer.claim" {
            self.leases.insert(string(d, "lease")?.into());
        }
        if !failed
            && p.method == "computer.release"
            && let Some(l) = p.args["lease"].as_str()
        {
            self.leases.remove(l);
        }
        if let Some(op) = d["operation_id"].as_str() {
            let status = if d["operation"].is_object() {
                d["operation"].clone()
            } else {
                d.clone()
            };
            self.operations.insert(op.into(), status);
        }
        if !p.control {
            let held = d["operation"]["resources_released"] == false
                || d["operation"]["terminal"] == false;
            self.busy = held;
            self.busy_operation = if held {
                d["operation_id"].as_str().map(str::to_owned)
            } else {
                None
            };
        } else if d["terminal"] == true
            && d["resources_released"] == true
            && p.operation == self.busy_operation
            && !self.pending.values().any(|v| !v.control)
        {
            self.busy = false;
            self.busy_operation = None;
        }
        if p.method == "computer.verify"
            && !failed
            && d["goal_verified"] == true
            && p.args["predicate"] == "element_value"
            && p.args["expected"] == self.expected_goal
            && p.args["request_id"]
                .as_str()
                .is_some_and(|r| self.input_requests.contains(r))
        {
            self.goal_verified = true;
        }
        self.response_digests.push(sha(raw));
        if self.frames.len()
            + self.artifacts.len()
            + self.views.len()
            + self.operations.len()
            + self.leases.len()
            > 1024
        {
            return Err(invalid("session_handle_budget"));
        }
        Ok(p)
    }
    pub fn proof(&self) -> EndProof {
        let ops = self.operations.values();
        EndProof {
            requests_terminal: self.pending.is_empty() && !self.busy,
            operations_terminal: ops.clone().all(|v| v["terminal"] == true),
            resources_released: ops.clone().all(|v| {
                v["resources_released"] == true && v["resource_release_uncertain"] != true
            }),
            leases_released: self.leases.is_empty(),
            mutation_uncertain: self.mutation_hold,
            model_goal_verified: self.goal_verified,
            operation_ids: self.operations.keys().cloned().collect(),
            response_digests: self.response_digests.clone(),
            ..EndProof::default()
        }
    }
    pub fn active_operation(&self) -> Option<&str> {
        self.busy_operation.as_deref()
    }
}
pub fn response(id: Value, reason: &str) -> Vec<u8> {
    let mut b=serde_json::to_vec(&json!({"jsonrpc":"2.0","id":id,"error":{"code":-32090,"message":reason,"data":{"origin":"native-link-admission","forwarded":false}}})).unwrap();
    b.push(b'\n');
    b
}
#[cfg(test)]
mod policy_tests {
    use super::*;
    fn ready() -> Ledger {
        let mut l = Ledger::new("synthetic-only", Value::Null, true, "task-", "approved");
        l.initialized = true;
        l.notified = true;
        l.schema_checked = true;
        l.frames.insert("frame".into());
        l.leases.insert("lease".into());
        l
    }
    fn action(id: u64, request: &str, op: &str) -> Value {
        json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"computer.act_checked","arguments":{"target":"synthetic-only","frame":"frame","lease":"lease","request_id":request,"operation_id":op}}})
    }
    fn call(id: u64, name: &str, args: Value) -> Value {
        json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}})
    }
    fn reply(l: &mut Ledger, id: u64, data: Value) {
        let v = json!({"jsonrpc":"2.0","id":id,"result":{"structuredContent":data}});
        l.reply(&v, &serde_json::to_vec(&v).unwrap()).unwrap();
    }
    #[test]
    fn terminal_control_reply_cannot_unlock_an_original_pending_mutation() {
        let mut l = ready();
        l.request(&action(10, "task-first", "op")).unwrap();
        l.request(&call(
            11,
            "computer.operation",
            json!({"operation_id":"op"}),
        ))
        .unwrap();
        reply(
            &mut l,
            11,
            json!({"operation_id":"op","terminal":true,"resources_released":true}),
        );
        assert!(l.request(&action(12, "task-second", "op2")).is_err());
        assert!(l.busy);
        assert_eq!(l.pending.len(), 1);
        reply(
            &mut l,
            10,
            json!({"operation_id":"op","dispatch":"returned","operation":{"terminal":true,"resources_released":true}}),
        );
        assert!(!l.busy);
        l.request(&action(13, "task-third", "op3")).unwrap();
    }
    #[test]
    fn only_approved_fresh_request_linked_goal_counts_and_new_mutation_invalidates_it() {
        let mut l = ready();
        l.request(&action(10, "task-input", "op")).unwrap();
        reply(
            &mut l,
            10,
            json!({"operation_id":"op","dispatch":"returned","operation":{"terminal":true,"resources_released":true}}),
        );
        for (id, expected, request) in [
            (20, "wrong", "task-input"),
            (21, "approved", "foreign"),
            (22, "approved", "task-input"),
        ] {
            l.request(&call(id,"computer.verify",json!({"target":"synthetic-only","frame":"frame","predicate":"element_value","expected":expected,"request_id":request}))).unwrap();
            reply(&mut l, id, json!({"goal_verified":true}));
            assert_eq!(l.goal_verified, id == 22);
        }
        l.request(&action(30, "task-next", "op-next")).unwrap();
        assert!(!l.goal_verified);
        assert!(!l.proof().permits(crate::types::Stage::Model, false));
    }
    #[test]
    fn unknown_effect_holds_claims_and_mutations_without_laundering_mcp_is_error() {
        let mut l = ready();
        l.request(&action(10, "task-input", "op")).unwrap();
        reply(
            &mut l,
            10,
            json!({"dispatch":"uncertain","dispatch_error":{"code":"lost_after_effect"},"operation_id":"op","operation":{"terminal":true,"resources_released":true}}),
        );
        assert!(l.mutation_hold);
        assert!(l.proof().mutation_uncertain);
        assert!(
            l.request(&call(
                11,
                "computer.claim",
                json!({"target":"synthetic-only"})
            ))
            .is_err()
        );
        assert!(l.request(&action(12, "task-other", "op-other")).is_err());
        l.request(&call(
            13,
            "computer.operation",
            json!({"operation_id":"op"}),
        ))
        .unwrap();
    }
}
