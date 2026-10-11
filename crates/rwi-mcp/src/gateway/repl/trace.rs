//! Parent-owned receipts and lease cleanup survive JS exceptions/process death.
mod summary;
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use summary::execution_summary;
use tokio::time::{Instant, timeout_at};

#[cfg(all(test, unix))]
mod tests;

#[derive(Clone)]
struct OwnedLease {
    owner: Option<device::OperationRef>,
    release_error: Option<Value>,
}
#[derive(Clone, Copy)]
struct Limits {
    total: Duration,
    settle: Duration,
    poll: Duration,
    queries: usize,
}
const LIMITS: Limits = Limits {
    total: Duration::from_secs(2),
    settle: Duration::from_millis(1500),
    poll: Duration::from_millis(50),
    queries: 64,
};
#[derive(Clone)]
struct Budget {
    end: Instant,
    settle: Instant,
    poll: Duration,
    queries: Arc<AtomicUsize>,
    limit: usize,
}
impl Budget {
    fn new(limits: Limits) -> Self {
        let now = Instant::now();
        Self {
            end: now + limits.total,
            settle: now + limits.settle,
            poll: limits.poll,
            queries: Arc::new(AtomicUsize::new(0)),
            limit: limits.queries,
        }
    }
}
fn execution_ref(value: &Value) -> Option<device::OperationRef> {
    serde_json::from_value::<device::OperationRef>(value.clone())
        .ok()
        .filter(|r| r.valid_shape())
}
fn same_context(a: &device::OperationRef, b: &device::OperationRef) -> bool {
    a.device_id == b.device_id
        && a.agent_instance == b.agent_instance
        && a.context_generation == b.context_generation
}

pub(super) struct Trace {
    gateway: Gateway,
    pub calls: Vec<Value>,
    pub outputs: Vec<Value>,
    pub images: Vec<Content>,
    pub output_bytes: usize,
    pub image_bytes: usize,
    leases: BTreeMap<String, OwnedLease>,
    budget: Option<Budget>,
    pending_cleanup: Option<tokio::task::JoinSet<Value>>,
    cleanup: Vec<Value>,
    started: std::time::Instant,
    call_started: std::time::Instant,
}
impl Trace {
    pub fn new(gateway: Gateway) -> Self {
        Self {
            gateway,
            calls: vec![],
            outputs: vec![],
            images: vec![],
            output_bytes: 0,
            image_bytes: 0,
            leases: BTreeMap::new(),
            budget: None,
            pending_cleanup: None,
            cleanup: vec![],
            started: std::time::Instant::now(),
            call_started: std::time::Instant::now(),
        }
    }
    pub fn begin(&mut self, tool: &str, args: &serde_json::Map<String, Value>) -> usize {
        self.call_started = std::time::Instant::now();
        let mut record = json!({"tool":tool,"outcome":"unconfirmed"});
        if let Some(id) = args.get("request_id") {
            record["request_id"] = id.clone();
        }
        self.calls.push(record);
        self.calls.len() - 1
    }
    pub fn finish(
        &mut self,
        index: usize,
        tool: &str,
        args: &serde_json::Map<String, Value>,
        reply: &CallToolResult,
    ) {
        let data = reply.structured_content.clone().unwrap_or(Value::Null);
        let reference = data["presentation"]["result_ref"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| {
                self.gateway
                    .presentation
                    .lock()
                    .ok()
                    .and_then(|mut cache| cache.retain(tool, data.clone()))
            });
        let call = &mut self.calls[index];
        call["elapsed_ms"] = json!(self.call_started.elapsed().as_secs_f64() * 1000.0);
        call["outcome"] = json!("returned");
        call["is_error"] = json!(reply.is_error.unwrap_or(false));
        call["result_ref"] = json!(reference);
        // Preserve exact native execution identity inline, independent of the
        // shorter-lived metadata cache. Never substitute the JS generation.
        for key in [
            "operation_ref",
            "code",
            "system_accepted",
            "goal_verified",
            "cancellation_delivery_confirmed",
            "cancellation_accepted",
            "original_action_replayed",
            "dispatch_error",
            "dispatch",
            "native_dispatch_entered",
        ] {
            if let Some(v) = data.get(key) {
                call[key] = v.clone();
            }
        }
        if tool == "computer.claim"
            && let Some(lease) = data["lease"].as_str()
        {
            self.leases.insert(
                lease.into(),
                OwnedLease {
                    owner: execution_ref(&data["operation_ref"]),
                    release_error: None,
                },
            );
        }
        if tool == "computer.release"
            && reply.is_error != Some(true)
            && let Some(lease) = args.get("lease").and_then(Value::as_str)
        {
            self.leases.remove(lease);
        }
        if tool == "computer.release"
            && reply.is_error == Some(true)
            && let Some(lease) = args.get("lease").and_then(Value::as_str)
            && let Some(owned) = self.leases.get_mut(lease)
        {
            owned.release_error = Some(data);
        }
    }
    pub async fn cleanup(&mut self) {
        self.cleanup_with(LIMITS).await;
    }
    fn cancelled(&self) -> (Vec<device::OperationRef>, bool) {
        let mut refs = vec![];
        let mut invalid = false;
        for call in &self.calls {
            if call["code"] == "operation_cancel_requested"
                && call["cancellation_delivery_confirmed"] == true
                && call["cancellation_accepted"] == true
            {
                match execution_ref(&call["operation_ref"]) {
                    Some(r) if !refs.contains(&r) => refs.push(r),
                    Some(_) => {}
                    None => invalid = true,
                }
            }
        }
        (refs, invalid)
    }
    async fn cleanup_with(&mut self, limits: Limits) {
        // Jobs, accepted release refs and attempt history belong to Trace, not
        // this borrowed future. Cancelling/resuming this await never restarts a
        // lease whose request is already in flight, even if the set is empty.
        if self.pending_cleanup.is_none() {
            let budget = self
                .budget
                .get_or_insert_with(|| Budget::new(limits))
                .clone();
            self.pending_cleanup = Some(cleanup_jobs(
                self.gateway.clone(),
                self.leases.clone(),
                self.cancelled(),
                budget,
            ));
        }
        while let Some(result) = self.pending_cleanup.as_mut().unwrap().join_next().await {
            if let Ok(value) = &result
                && let Some(lease) = value["lease"].as_str()
            {
                self.leases.remove(lease);
            }
            self.cleanup
                .push(result.unwrap_or_else(|e| json!({"is_error":true,"message":e.to_string()})));
        }
        // A panicked/lost job has no confirmed lease result. Keep that unknown
        // explicit; remaining bookkeeping is never permission to send again.
        for lease in self.leases.keys() {
            self.cleanup.push(json!({"lease":lease,"is_error":true,"code":"lease_release_unconfirmed","reason":"cleanup_job_result_unconfirmed"}));
        }
        self.leases.clear();
    }
    pub fn cleanup_failed(&self) -> bool {
        self.cleanup.iter().any(|c| c["is_error"] == true)
    }
    pub fn result(
        &mut self,
        generation: String,
        state_retained: bool,
        error: Option<String>,
    ) -> CallToolResult {
        let summary = execution_summary(&self.calls, error.is_some(), self.cleanup_failed());
        let data = json!({"execution_summary":summary,"generation":generation,"state_retained":state_retained,"error":error,"outputs":self.outputs,"calls":self.calls,"cleanup":self.cleanup,"original_action_replayed":false,"adapter_elapsed_ms":self.started.elapsed().as_secs_f64()*1000.0});
        let mut result = if error.is_some() {
            CallToolResult::structured_error(data)
        } else {
            CallToolResult::structured(data)
        };
        result.content.append(&mut self.images);
        result
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        // Future-drop is not a proof of lease release. Best-effort cleanup is
        // tracked by the same gateway shutdown queue; native expiry is authority.
        if self.leases.is_empty()
            && self
                .pending_cleanup
                .as_ref()
                .is_none_or(|pending| pending.is_empty())
        {
            return;
        }
        let Ok(mut jobs) = self.gateway.cancels.lock() else {
            eprintln!(
                "rwi-mcp: cleanup handoff unavailable; lease release remains unconfirmed; no cleanup replay"
            );
            return;
        };
        jobs.retain(|j| !j.is_finished());
        if jobs.len() >= 64 {
            eprintln!(
                "rwi-mcp: cleanup handoff queue full; lease release remains unconfirmed; no cleanup replay"
            );
            return;
        }
        let mut pending = match self.pending_cleanup.take() {
            // Transfer the SAME tasks. Rebuilding from self.leases would repeat
            // Prepare/Execute while an earlier accepted release is uncertain.
            Some(pending) => pending,
            None => {
                let cancelled = self.cancelled();
                let budget = self.budget.clone().unwrap_or_else(|| Budget::new(LIMITS));
                cleanup_jobs(
                    self.gateway.clone(),
                    std::mem::take(&mut self.leases),
                    cancelled,
                    budget,
                )
            }
        };
        jobs.push(tokio::spawn(async move {
            while pending.join_next().await.is_some() {}
        }));
    }
}

fn cleanup_jobs(
    gateway: Gateway,
    leases: BTreeMap<String, OwnedLease>,
    cancelled: (Vec<device::OperationRef>, bool),
    budget: Budget,
) -> tokio::task::JoinSet<Value> {
    let mut jobs = tokio::task::JoinSet::new();
    for (lease, owned) in leases {
        let refs: Vec<_> = cancelled
            .0
            .iter()
            .filter(|r| owned.owner.as_ref().is_some_and(|o| same_context(o, r)))
            .cloned()
            .collect();
        let unknown = cancelled.1 || owned.owner.is_none() && !cancelled.0.is_empty();
        let gateway = gateway.clone();
        let budget = budget.clone();
        jobs.spawn(
            async move { cleanup_lease(&gateway, lease, owned, refs, unknown, &budget).await },
        );
    }
    jobs
}

struct Sample {
    quiet: bool,
    failed: bool,
    data: Value,
}
async fn sample(
    gateway: &Gateway,
    r: &device::OperationRef,
    deadline: Instant,
    budget: &Budget,
) -> Sample {
    let unconfirmed = |reason: &str, evidence: Value| Sample {
        quiet: false,
        failed: true,
        data: json!({"operation_ref":r,"confirmed":false,"reason":reason,"evidence":evidence}),
    };
    if Instant::now() >= deadline {
        return unconfirmed("cleanup_deadline", Value::Null);
    }
    if budget
        .queries
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
            (n < budget.limit).then_some(n + 1)
        })
        .is_err()
    {
        return unconfirmed("cleanup_query_limit", Value::Null);
    }
    // This is the independent control lane: no context mutex/OpenContext,
    // native execution, bare nonce or durable-request fallback.
    let reply = match timeout_at(
        deadline,
        gateway.request(
            &r.device_id,
            None,
            Call::Operation {
                selector: device::OperationSelector::Execution {
                    operation_ref: r.clone(),
                },
            },
        ),
    )
    .await
    {
        Ok(Ok(reply)) => reply,
        Ok(Err(error)) => return unconfirmed("execution_read_failed", error.data),
        Err(_) => return unconfirmed("execution_read_timeout", Value::Null),
    };
    if reply.instance != r.agent_instance {
        return unconfirmed("execution_identity_changed", reply.result.data);
    }
    if reply.result.is_error {
        return unconfirmed("execution_read_rejected", reply.result.data);
    }
    if execution_ref(&reply.result.data["operation_ref"]).as_ref() != Some(r) {
        return unconfirmed("execution_reference_changed", reply.result.data);
    }
    let data = reply.result.data;
    if ["terminal", "native_call_pending", "resources_released"]
        .iter()
        .any(|k| !data[*k].is_boolean())
    {
        return unconfirmed("execution_status_unproven", data);
    }
    let quiet = data["terminal"] == true
        && data["native_call_pending"] == false
        && data["resources_released"] == true;
    Sample {
        quiet,
        failed: false,
        data: json!({"operation_ref":r,"confirmed":quiet,"status":data}),
    }
}
async fn wait_quiet(gateway: &Gateway, r: &device::OperationRef, budget: &Budget) -> Sample {
    loop {
        let mut result = sample(gateway, r, budget.settle, budget).await;
        if result.quiet || result.failed {
            return result;
        }
        if Instant::now() + budget.poll >= budget.settle {
            result.failed = true;
            result.data["reason"] = json!("cleanup_deadline");
            return result;
        }
        tokio::time::sleep(budget.poll).await;
    }
}

async fn release_owned(
    gateway: &Gateway,
    lease: &str,
    owner: &device::OperationRef,
    accepted: &mut Option<device::OperationRef>,
    budget: &Budget,
) -> DeviceResult {
    if *gateway.shutdown.borrow() {
        return DeviceResult::error("host_disconnected", "No cleanup execution after Host EOF.");
    }
    // Never consult the adapter's mutable current-context pointer. This lease
    // belongs to the original claim context, even if a newer context was opened.
    let prepared = match gateway
        .request(
            &owner.device_id,
            Some(owner.context_generation.clone()),
            Call::Prepare {
                tool: "computer.release".into(),
                arguments: json!({"lease":lease}),
            },
        )
        .await
    {
        Ok(r) => r,
        Err(e) => return e,
    };
    if prepared.result.is_error {
        return prepared.result;
    }
    let Some(reference) = execution_ref(&prepared.result.data["operation_ref"])
        .filter(|r| same_context(owner, r) && prepared.instance == owner.agent_instance)
    else {
        let mut error = DeviceResult::error(
            "lease_release_identity_unconfirmed",
            "Release acceptance did not bind the original claim context; no Execute.",
        );
        error.data["prepare_result"] = prepared.result.data;
        return error;
    };
    *accepted = Some(reference.clone());
    if Instant::now() >= budget.end || *gateway.shutdown.borrow() {
        return DeviceResult::error(
            "lease_release_unconfirmed",
            "Cleanup ended after acceptance; no Execute was sent.",
        );
    }
    let reply = match gateway
        .request(
            &owner.device_id,
            None,
            Call::Execute {
                operation_ref: reference.clone(),
            },
        )
        .await
    {
        Ok(r) => r,
        Err(e) => return e,
    };
    if reply.instance != owner.agent_instance
        || execution_ref(&reply.result.data["operation_ref"]).as_ref() != Some(&reference)
    {
        let mut error = DeviceResult::error(
            "lease_release_identity_unconfirmed",
            "Release response did not retain the original accepted execution identity.",
        );
        error.data["release_result"] = reply.result.data;
        return error;
    }
    reply.result
}
async fn release_once(
    gateway: &Gateway,
    lease: &str,
    owner: Option<&device::OperationRef>,
    budget: &Budget,
) -> DeviceResult {
    if Instant::now() >= budget.end {
        return DeviceResult::error(
            "lease_release_unconfirmed",
            "Shared cleanup deadline already elapsed; no new release was sent.",
        );
    }
    let mut accepted = None;
    let mut result = timeout_at(budget.end, async {
        match owner {
            Some(owner) => release_owned(gateway, lease, owner, &mut accepted, budget).await,
            // Legacy/unknown owner: retain the original safe owned-lease attempt,
            // but callers never use this branch as cancellation quiescence proof.
            None => {
                gateway
                    .invoke("computer.release", json!({"lease":lease}), None)
                    .await
            }
        }
    })
    .await
    .unwrap_or_else(|_| {
        DeviceResult::error(
            "lease_release_unconfirmed",
            "Shared cleanup deadline; lease expiry remains authoritative.",
        )
    });
    if let Some(reference) = accepted {
        // Preserve this cleanup execution locator even after a timeout. Its
        // uncertainty is not a reason to replay Execute or the original action.
        result.data["operation_ref"] = json!(reference);
    }
    result
}
async fn cleanup_lease(
    gateway: &Gateway,
    lease: String,
    owned: OwnedLease,
    refs: Vec<device::OperationRef>,
    unknown: bool,
    budget: &Budget,
) -> Value {
    let needs_proof = unknown || !refs.is_empty();
    let mut quiet = !unknown;
    let mut evidence = vec![];
    for r in &refs {
        let result = wait_quiet(gateway, r, budget).await;
        quiet &= result.quiet;
        evidence.push(result.data);
        if !quiet {
            break;
        }
    }
    // Unknown stop is never promoted to success. Still retain the original
    // safe, owned-lease release attempt (the native context gate can reject it).
    let mut released = release_once(gateway, &lease, owned.owner.as_ref(), budget).await;
    let mut attempts = vec![json!({"is_error":released.is_error,"result":released.data})];
    if quiet
        && !refs.is_empty()
        && released.is_error
        && released.data["code"] == "context_busy"
        && Instant::now() + budget.poll < budget.end
    {
        tokio::time::sleep(budget.poll).await;
        // Terminal status can precede context-permit/ledger cleanup briefly.
        // One fresh control sample per exact ref, then at most one release retry.
        for r in &refs {
            let result = sample(gateway, r, budget.end, budget).await;
            quiet &= result.quiet;
            evidence.push(result.data);
            if !quiet {
                break;
            }
        }
        if quiet {
            released = release_once(gateway, &lease, owned.owner.as_ref(), budget).await;
            attempts.push(json!({"is_error":released.is_error,"result":released.data}));
        }
    }
    json!({"lease":lease,"is_error":released.is_error || released.data["released"] != true || needs_proof && !quiet,
        "code":if needs_proof && !quiet {Some("lease_release_unconfirmed")} else {None},
        "result":released.data,"release_attempts":attempts,"original_release_error":owned.release_error,
        "native_quiescence_confirmed":if needs_proof {Some(quiet)} else {None},
        "quiescence":evidence,"association_unconfirmed":unknown,"original_action_replayed":false})
}
