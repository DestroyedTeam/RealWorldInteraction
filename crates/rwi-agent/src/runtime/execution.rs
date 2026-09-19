//! Service-owned execution identities. Correlation labels are never lookup keys.
use super::*;
use serde_json::Value;
use std::time::Duration;

const ACCEPT_TTL: Duration = Duration::from_secs(30);
const HISTORY_TTL: Duration = Duration::from_secs(300);
const EXECUTION_LIMIT: usize = 64;
enum Phase {
    Accepted,
    Cancelled,
    Running {
        operation: Operation,
        terminal_at: Option<Instant>,
    },
}

#[cfg(all(test, feature = "fixture"))]
mod tests {
    use super::*;
    #[tokio::test]
    async fn pending_native_ownership_survives_quota_cancel_and_context_close() {
        let temp = tempfile::tempdir().unwrap();
        let pin = PeerPin::parse(&"a".repeat(64)).unwrap();
        let policy = Principal {
            certificate: pin.clone(),
            observe: true,
            input: true,
        };
        let r = Runtime::new(Config {
            version: 1,
            device_id: "device".into(),
            bind: "127.0.0.1:0".parse().unwrap(),
            identity: rwi_link_trust::IdentityPaths::in_profile(temp.path()),
            principals: vec![policy.clone()],
            state_root: temp.path().join("state"),
            platform: crate::config::PlatformPolicy::Fixture,
        })
        .unwrap();
        let open = r.call(&pin, &policy, None, Call::OpenContext).await;
        let token = open.data["context"].as_str().unwrap();
        let arguments = json!({"target":open.data["targets"][0]["target"],"inline_image":false});
        let accepted = r.prepare(
            &pin,
            &policy,
            Some(token),
            "computer.observe".into(),
            arguments.clone(),
        );
        let reference: OperationRef =
            serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
        let (context, entry) = r.resolve_execution(&pin, &reference).unwrap();
        let operation = entry.begin(&context.registry).unwrap();
        let scope = rwi_core::operation::run(&operation, || {
            rwi_core::operation::native_call("explicit_held_test_scope").unwrap()
        });
        operation.finish();
        for _ in 0..80 {
            let accepted = r.prepare(
                &pin,
                &policy,
                Some(token),
                "computer.observe".into(),
                arguments.clone(),
            );
            assert!(!accepted.is_error, "{:?}", accepted.data);
            let reference = serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
            r.cancel(&pin, &reference);
        }
        r.cancel(&pin, &reference);
        assert!(r.resolve_execution(&pin, &reference).is_ok());
        let status = entry.status();
        assert_eq!(status.data["native_call_pending"], true);
        assert_eq!(status.data["terminal"], false);
        assert_eq!(
            r.call(&pin, &policy, Some(token), Call::CloseContext)
                .await
                .data["code"],
            "native_resources_pending"
        );
        drop(scope);
        assert_eq!(entry.status().data["resources_released"], true);
        if let Phase::Running { terminal_at, .. } = &mut *entry.phase.lock().unwrap() {
            *terminal_at = Some(Instant::now() - HISTORY_TTL - Duration::from_secs(1));
        }
        assert!(
            r.resolve_execution(&pin, &reference).is_err(),
            "terminal age eviction must return stale, never another execution"
        );
        assert!(
            !r.call(&pin, &policy, Some(token), Call::CloseContext)
                .await
                .is_error
        );
    }
}
pub(super) struct Execution {
    pub reference: OperationRef,
    pub tool: String,
    pub arguments: Value,
    pub correlation: Option<String>,
    pub writes: bool,
    accepted_at: Instant,
    phase: Mutex<Phase>,
}
impl Execution {
    pub fn status(&self) -> DeviceResult {
        let Ok(mut phase) = self.phase.lock() else {
            return DeviceResult::error(
                "execution_state_unavailable",
                "Native completion is unknown.",
            );
        };
        match &mut *phase {
            Phase::Accepted if self.accepted_at.elapsed() >= ACCEPT_TTL => {
                *phase = Phase::Cancelled;
                DeviceResult::value(
                    json!({"phase":"expired_before_execute","terminal":true,"resources_released":true,"native_call_pending":false,"dispatch_started":false}),
                )
            }
            Phase::Accepted => DeviceResult::value(
                json!({"phase":"accepted","terminal":false,"resources_released":true,"native_call_pending":false,"dispatch_started":false}),
            ),
            Phase::Cancelled => DeviceResult::value(
                json!({"phase":"cancelled_before_execute","terminal":true,"cancel_requested":true,"resources_released":true,"native_call_pending":false,"dispatch_started":false}),
            ),
            Phase::Running {
                operation,
                terminal_at,
            } => {
                let status = operation.status();
                if status.terminal {
                    terminal_at.get_or_insert_with(Instant::now);
                }
                let mut data = json!(status);
                data.as_object_mut().unwrap().remove("operation_id");
                data["phase"] = json!(if status.terminal {
                    "finished"
                } else {
                    "executing"
                });
                DeviceResult::value(data)
            }
        }
    }
    pub fn retirable(&self, fenced: bool, aged_only: bool) -> bool {
        let _ = self.status();
        let Ok(phase) = self.phase.lock() else {
            return false;
        };
        match &*phase {
            Phase::Accepted => false,
            Phase::Cancelled => !aged_only || self.accepted_at.elapsed() >= ACCEPT_TTL,
            Phase::Running {
                operation,
                terminal_at,
            } => {
                let s = operation.status();
                s.terminal
                    && !s.native_call_pending
                    && (s.resources_released || fenced)
                    && (!aged_only || terminal_at.is_some_and(|t| t.elapsed() >= HISTORY_TTL))
            }
        }
    }
    pub fn begin(&self, registry: &Registry) -> Result<Operation, DeviceResult> {
        let mut phase = self.phase.lock().map_err(|_| {
            DeviceResult::error(
                "execution_state_unavailable",
                "No execution can be authorized.",
            )
        })?;
        match &*phase {
            Phase::Accepted if self.accepted_at.elapsed() < ACCEPT_TTL => {}
            Phase::Accepted | Phase::Cancelled => {
                return Err(DeviceResult::error(
                    "execution_cancelled_before_dispatch",
                    "This accepted instance cannot execute; no automatic replacement or replay.",
                ));
            }
            Phase::Running { .. } => {
                return Err(DeviceResult::error(
                    "execution_already_consumed",
                    "Execute is one-shot. Query this reference or the durable request; never retry Execute.",
                ));
            }
        }
        let operation = match registry.begin(Some(self.reference.execution_nonce.clone())) {
            Ok(operation) => operation,
            Err(e) => {
                *phase = Phase::Cancelled;
                return Err(DeviceResult::error(&e.code, &e.next_step));
            }
        };
        *phase = Phase::Running {
            operation: operation.clone(),
            terminal_at: None,
        };
        Ok(operation)
    }
    pub fn cancel(&self) -> DeviceResult {
        let Ok(mut phase) = self.phase.lock() else {
            return DeviceResult::error(
                "execution_state_unavailable",
                "Cancellation delivery is unconfirmed.",
            );
        };
        match &*phase {
            Phase::Accepted => *phase = Phase::Cancelled,
            Phase::Running { operation, .. } => operation.cancel(),
            Phase::Cancelled => {}
        }
        drop(phase);
        self.envelope(self.status())
    }
    pub fn envelope(&self, mut result: DeviceResult) -> DeviceResult {
        if !result.data.is_object() {
            result.data = json!({"value":result.data});
        }
        result.data.as_object_mut().unwrap().remove("operation_id");
        result.data["operation_ref"] = json!(self.reference);
        result.data["correlation_id"] = json!(self.correlation);
        result
    }
}
impl Context {
    pub(super) fn native_quiescent(&self) -> bool {
        self.executions.lock().is_ok_and(|entries| {
            entries.values().all(|entry| {
                let Ok(phase) = entry.phase.lock() else {
                    return false;
                };
                match &*phase {
                    Phase::Accepted | Phase::Cancelled => true,
                    Phase::Running { operation, .. } => {
                        let s = operation.status();
                        s.terminal
                            && !s.native_call_pending
                            && (s.resources_released || self.domain.fenced.load(Ordering::Acquire))
                    }
                }
            })
        })
    }
    fn collect(&self, entries: &mut BTreeMap<String, Arc<Execution>>) {
        let fenced = self.domain.fenced.load(Ordering::Acquire);
        entries.retain(|_, e| !e.retirable(fenced, true));
        while entries.len() >= EXECUTION_LIMIT {
            let oldest = entries
                .iter()
                .filter(|(_, e)| e.retirable(fenced, false))
                .min_by_key(|(_, e)| e.accepted_at)
                .map(|(key, _)| key.clone());
            if let Some(key) = oldest {
                entries.remove(&key);
            } else {
                break;
            }
        }
    }
}
impl Runtime {
    pub(super) fn prepare(
        &self,
        peer: &PeerPin,
        policy: &Principal,
        token: Option<&str>,
        tool: String,
        mut arguments: Value,
    ) -> DeviceResult {
        if !catalog::executable(&tool) {
            return DeviceResult::error(
                "prepare_not_supported",
                "Use the explicit metadata/query/cached-read API.",
            );
        }
        if let Err(e) = catalog::validate(&tool, &arguments) {
            return DeviceResult::error("invalid_arguments", &e.to_string());
        }
        if serde_json::to_vec(&arguments).map_or(true, |v| v.len() > 65536) {
            return DeviceResult::error(
                "acceptance_argument_limit",
                "Arguments exceed the bounded acceptance budget.",
            );
        }
        let Some(context) = self.context(peer, token) else {
            return DeviceResult::error(
                "context_unknown",
                "Read targets to obtain a fresh authorized context.",
            );
        };
        if arguments
            .get("target")
            .is_some_and(|v| v != &context.target)
        {
            return DeviceResult::error(
                "foreign_target",
                "Target does not belong to this context.",
            );
        }
        if arguments
            .get("device_id")
            .is_some_and(|v| !v.is_null() && v != &self.config.device_id)
        {
            return DeviceResult::error("foreign_device", "Arguments name another device.");
        }
        let writes = catalog::definitions()
            .iter()
            .any(|d| d.name == tool && d.writes);
        if !policy.observe
            || (writes || matches!(tool.as_str(), "computer.claim" | "computer.recover_input"))
                && !policy.input
        {
            return DeviceResult::error("permission_denied", "Principal has no capability grant.");
        }
        if writes && !arguments["request_id"].as_str().is_some_and(valid_id) {
            return DeviceResult::error(
                "request_id_required",
                "Mutations require an explicit stable request_id.",
            );
        }
        let correlation = arguments
            .get("correlation_id")
            .or_else(|| arguments.get("operation_id"))
            .and_then(Value::as_str)
            .map(str::to_owned);
        if correlation.as_ref().is_some_and(|s| s.len() > 256) {
            return DeviceResult::error("correlation_limit", "Use a bounded correlation label.");
        }
        // Correlation is not part of execution identity or the replay fingerprint.
        if let Some(args) = arguments.as_object_mut() {
            args.remove("operation_id");
            args.remove("correlation_id");
        }
        let Ok(mut entries) = context.executions.lock() else {
            return DeviceResult::error("execution_store_unavailable", "No execution accepted.");
        };
        if context.closed.load(Ordering::Acquire) {
            return DeviceResult::error(
                "context_closed",
                "No execution accepted after context closure.",
            );
        }
        context.collect(&mut entries);
        if entries.len() >= EXECUTION_LIMIT {
            return DeviceResult::error(
                "execution_capacity",
                "Pending native ownership is retained; cancel/query existing references.",
            );
        }
        let Ok(seq) = context
            .sequence
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        else {
            return DeviceResult::error("execution_capacity", "Context sequence exhausted.");
        };
        let reference = OperationRef {
            ref_version: EXECUTION_REF_VERSION,
            device_id: self.config.device_id.clone(),
            agent_instance: self.instance.clone(),
            context_generation: context.generation.clone(),
            execution_nonce: format!("{seq:x}:{}", uuid::Uuid::new_v4()),
        };
        let entry = Arc::new(Execution {
            reference: reference.clone(),
            tool,
            arguments,
            correlation,
            writes,
            accepted_at: Instant::now(),
            phase: Mutex::new(Phase::Accepted),
        });
        entries.insert(reference.execution_nonce.clone(), entry.clone());
        entry.envelope(DeviceResult::value(json!({"accepted":true,"native_dispatch_started":false,"input_resources_acquired":false})))
    }
    pub(super) fn resolve_execution(
        &self,
        peer: &PeerPin,
        reference: &OperationRef,
    ) -> Result<(Arc<Context>, Arc<Execution>), DeviceResult> {
        let stale = || {
            DeviceResult::error(
                "execution_ref_expired",
                "Reference unknown, retired or from another generation. This does not prove no effect or native termination; query the explicit durable request if applicable.",
            )
        };
        if !reference.valid_shape()
            || reference.device_id != self.config.device_id
            || reference.agent_instance != self.instance
        {
            return Err(stale());
        }
        let context = self
            .context(peer, Some(&reference.context_generation))
            .ok_or_else(stale)?;
        let mut entries = context.executions.lock().map_err(|_| stale())?;
        // Do not evict a live execution merely because a new query arrived.
        let fenced = context.domain.fenced.load(Ordering::Acquire);
        entries.retain(|_, e| !e.retirable(fenced, true));
        let entry = entries
            .get(&reference.execution_nonce)
            .filter(|e| e.reference == *reference)
            .cloned()
            .ok_or_else(stale)?;
        drop(entries);
        Ok((context, entry))
    }
    pub(super) fn cancel(&self, peer: &PeerPin, reference: &OperationRef) -> DeviceResult {
        match self.resolve_execution(peer, reference) {
            Ok((_, entry)) => entry.cancel(),
            Err(e) => e,
        }
    }
    pub(super) fn query(&self, peer: &PeerPin, selector: &OperationSelector) -> DeviceResult {
        match selector {
            OperationSelector::Execution { operation_ref } => {
                match self.resolve_execution(peer, operation_ref) {
                    Ok((_, e)) => e.envelope(e.status()),
                    Err(e) => e,
                }
            }
            OperationSelector::Request {
                device_id,
                request_id,
            } => {
                if device_id != &self.config.device_id || !valid_id(request_id) {
                    return DeviceResult::error(
                        "request_binding",
                        "Use an explicit paired device and stable request_id.",
                    );
                }
                let Ok(storage) = self.ledger.lock() else {
                    return DeviceResult::error(
                        "operation_storage_unavailable",
                        "Durable request state is unknown.",
                    );
                };
                let Ok(ledger) = storage.as_ref() else {
                    return DeviceResult::error(
                        "operation_storage_unavailable",
                        "Preserve the journal; do not replay.",
                    );
                };
                match ledger.request_status(peer.as_str(), request_id) {
                    Some(mut value) => {
                        value["receipt_ref"] =
                            json!({"device_id":device_id,"request_id":request_id});
                        DeviceResult::value(value)
                    }
                    None => DeviceResult::error(
                        "request_unknown",
                        "No durable record found. Do not infer that an uncertain action had no effect or automatically retry.",
                    ),
                }
            }
        }
    }
}
