//! One-shot dispatch after service acceptance. Async waiter lifetime does not own native work.
use super::execution::Execution;
use super::*;
use serde_json::Value;

struct Finish(Operation);
fn fault_result(error: rwi_core::Fault) -> DeviceResult {
    let mut result = DeviceResult::error(&error.code, &error.next_step);
    if let Some(diagnostic) = error.diagnostic {
        result.data["diagnostic"] = *diagnostic;
    }
    result
}
#[cfg(test)]
#[test]
fn native_fault_diagnostic_survives_dispatch_transport() {
    let mut error = rwi_core::fault("stale_reference", "Reobserve.");
    error.diagnostic = Some(Box::new(json!({"content_changed": true})));
    let result = fault_result(error);
    assert!(result.is_error);
    assert_eq!(result.data["code"], "stale_reference");
    assert_eq!(result.data["diagnostic"]["content_changed"], true);
    assert!(
        fault_result(rwi_core::fault("unknown", "Inspect."))
            .data
            .get("diagnostic")
            .is_none()
    );
}
fn needs_input_fence(
    writes: bool,
    entered: bool,
    resources_released: bool,
    result: &DeviceResult,
) -> bool {
    writes
        && (!resources_released
            || result.data["resource_release_uncertain"] == true
            || (entered
                && (result.is_error
                    || result.data["dispatch"] == "uncertain"
                    || result.data["state"] == "uncertain")))
}
#[cfg(test)]
#[test]
fn cleanup_uncertainty_is_not_mistaken_for_a_clean_pre_dispatch_rejection() {
    let rejection = DeviceResult::error("foreground_changed", "Reobserve, no dispatch.");
    assert!(!needs_input_fence(true, false, true, &rejection));
    assert!(needs_input_fence(true, false, false, &rejection));
    assert!(needs_input_fence(true, true, true, &rejection));
    assert!(!needs_input_fence(false, false, true, &rejection));
    let mut cleanup = rejection.clone();
    cleanup.data["resource_release_uncertain"] = true.into();
    assert!(needs_input_fence(true, false, true, &cleanup));
}
impl Drop for Finish {
    fn drop(&mut self) {
        self.0.finish();
    }
}
struct WorkerOwnership {
    domain: Arc<InputDomain>,
    writes: bool,
    _context: tokio::sync::OwnedSemaphorePermit,
    _input: Option<tokio::sync::OwnedSemaphorePermit>,
}
impl Drop for WorkerOwnership {
    fn drop(&mut self) {
        // Drop runs before our permit fields are released. Another writer must
        // observe the fence even if the original async waiter already vanished.
        if self.writes && std::thread::panicking() {
            self.domain.fenced.store(true, Ordering::Release);
        }
    }
}
#[cfg(test)]
#[test]
fn panic_fences_the_domain_before_owned_permits_are_dropped() {
    let domain = Arc::new(InputDomain {
        key: "synthetic".into(),
        gate: Arc::new(tokio::sync::Semaphore::new(1)),
        fenced: AtomicBool::new(false),
    });
    let context = Arc::new(tokio::sync::Semaphore::new(1));
    let ownership = WorkerOwnership {
        domain: domain.clone(),
        writes: true,
        _context: context.clone().try_acquire_owned().unwrap(),
        _input: Some(domain.gate.clone().try_acquire_owned().unwrap()),
    };
    assert_eq!(domain.gate.available_permits(), 0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _owned = ownership;
        panic!("synthetic worker panic");
    }));
    assert!(result.is_err());
    assert!(domain.fenced.load(Ordering::Acquire));
    assert_eq!(domain.gate.available_permits(), 1);
    assert_eq!(context.available_permits(), 1);
}
impl Runtime {
    pub(super) async fn execute(
        self: &Arc<Self>,
        peer: &PeerPin,
        policy: &Principal,
        reference: &OperationRef,
    ) -> DeviceResult {
        let (context, entry) = match self.resolve_execution(peer, reference) {
            Ok(v) => v,
            Err(e) => return e,
        };
        let op = match entry.begin(&context.registry) {
            Ok(op) => op,
            Err(e) => return entry.envelope(e),
        };
        let finish = Finish(op.clone());
        let result = self
            .dispatch(peer, policy, context, entry.clone(), finish)
            .await;
        let mut result = entry.envelope(result);
        result.data["operation"] = entry.status().data;
        result
    }
    async fn dispatch(
        self: &Arc<Self>,
        peer: &PeerPin,
        policy: &Principal,
        context: Arc<Context>,
        entry: Arc<Execution>,
        finish: Finish,
    ) -> DeviceResult {
        let Ok(permit) = context.gate.clone().try_acquire_owned() else {
            return DeviceResult::error(
                "context_busy",
                "Only this context is busy; this execution was not dispatched.",
            );
        };
        if context.closed.load(Ordering::Acquire) {
            return DeviceResult::error("context_closed", "No dispatch after context closure.");
        }
        if let Err(e) = rwi_core::operation::run(&finish.0, || {
            rwi_core::operation::checkpoint("before_dispatch")
        }) {
            return DeviceResult::error(&e.code, &e.next_step);
        }
        let input_permit = if entry.writes {
            if context.domain.fenced.load(Ordering::Acquire) {
                return DeviceResult::error(
                    "input_domain_fenced",
                    "Inspect uncertain native resources; no new input was dispatched.",
                );
            }
            match context.domain.gate.clone().try_acquire_owned() {
                Ok(p) => {
                    if context.domain.fenced.load(Ordering::Acquire) {
                        return DeviceResult::error(
                            "input_domain_fenced",
                            "Reconcile the previous writer before a new action.",
                        );
                    }
                    Some(p)
                }
                Err(_) => {
                    return DeviceResult::error(
                        "input_busy",
                        "This desktop has an active writer; do not steal focus.",
                    );
                }
            }
        } else {
            None
        };
        let mut reservation = None;
        if entry.writes {
            let id = entry.arguments["request_id"]
                .as_str()
                .expect("validated at acceptance");
            let Ok(mut storage) = self.ledger.lock() else {
                return DeviceResult::error(
                    "operation_storage_unavailable",
                    "No dispatch without durable reservation.",
                );
            };
            let ledger = match storage.as_mut() {
                Ok(l) => l,
                Err(e) => {
                    return DeviceResult::error("operation_storage_unavailable", &e.to_string());
                }
            };
            match ledger.reserve_execution(
                peer.as_str(),
                id,
                &json!({"tool":entry.tool,"arguments":entry.arguments}),
                &entry.reference,
                &context.domain.key,
            ) {
                Ok(Reservation::New(key)) => reservation = Some(key),
                Ok(Reservation::Existing(r)) => return r,
                Err(e) => {
                    return DeviceResult::error("operation_reservation_failed", &e.to_string());
                }
            }
        }
        let service = self.clone();
        let worker_context = context.clone();
        let worker_entry = entry.clone();
        let policy = policy.clone();
        let result=tokio::task::spawn_blocking(move || {
            // Both permits and finish guard belong to the worker, not its async waiter.
            let _ownership=WorkerOwnership{domain:worker_context.domain.clone(),writes:worker_entry.writes,_context:permit,_input:input_permit};
            let entered=&worker_context.entered;
            entered.store(false,Ordering::Release);
            let operation=finish.0.clone();
            let result=rwi_core::operation::run(&operation, || -> rwi_core::Result<DeviceResult> {
                rwi_core::operation::checkpoint("accepted_execute")?;
                if worker_entry.tool=="computer.recover_input" {
                    return Ok(service.recover(&policy,worker_entry.arguments["resource_domain"].as_str().unwrap_or_default(),Some(&worker_entry.reference.execution_nonce)));
                }
                worker_context.executor.lock().map_err(|_|rwi_core::fault("executor_unavailable","Native context failed; other contexts remain available."))?
                    .invoke(&worker_entry.tool,worker_entry.arguments.clone())
            });
            let mut result=match result {Ok(r)=>r,Err(e)=>fault_result(e)};
            drop(finish);
            result=worker_entry.envelope(result);
            result.data["operation"]=worker_entry.status().data;
            result.data["native_dispatch_entered"]=json!(entered.load(Ordering::Acquire));
            if needs_input_fence(worker_entry.writes, entered.load(Ordering::Acquire), operation.status().resources_released, &result) {
                worker_context.domain.fenced.store(true,Ordering::Release);
            }
            result.data["input_domain_fenced"]=json!(worker_context.domain.fenced.load(Ordering::Acquire));
            if let Some(key)=reservation {
                result.data["receipt_ref"]=json!({"device_id":worker_entry.reference.device_id,"request_id":worker_entry.arguments["request_id"]});
                let saved=service.ledger.lock().ok().and_then(|l|l.as_ref().ok().map(|l|l.finish(&key,&result).is_ok())).unwrap_or(false);
                if !saved {
                    worker_context.domain.fenced.store(true,Ordering::Release);
                    return DeviceResult::error("effect_recording_uncertain","Effect may have occurred; query the durable request without replay.");
                }
            }
            result
        }).await;
        match result {
            Ok(r) => r,
            Err(_) => {
                if entry.writes {
                    context.domain.fenced.store(true, Ordering::Release);
                }
                DeviceResult::error(
                    "native_worker_failed",
                    "Effect may be unknown; inspect the execution and target without replay.",
                )
            }
        }
    }
    pub(super) async fn cached(
        &self,
        peer: &PeerPin,
        policy: &Principal,
        token: Option<&str>,
        tool: String,
        arguments: Value,
    ) -> DeviceResult {
        if !catalog::cached_read(&tool) {
            return DeviceResult::error(
                "cached_read_not_supported",
                "Native work requires service acceptance.",
            );
        }
        if let Err(e) = catalog::validate(&tool, &arguments) {
            return DeviceResult::error("invalid_arguments", &e.to_string());
        }
        if !policy.observe {
            return DeviceResult::error("permission_denied", "No observation authorization.");
        }
        let Some(context) = self.context(peer, token) else {
            return DeviceResult::error("context_unknown", "Read targets for a fresh context.");
        };
        if arguments
            .get("target")
            .is_some_and(|v| v != &context.target)
        {
            return DeviceResult::error("foreign_target", "Use this context's cached handles.");
        }
        let Ok(_permit) = context.gate.clone().try_acquire_owned() else {
            return DeviceResult::error(
                "context_busy",
                "Cached data is in use; no native call started.",
            );
        };
        if context.closed.load(Ordering::Acquire) {
            return DeviceResult::error("context_closed", "The cache context has closed.");
        }
        let Ok(mut executor) = context.executor.try_lock() else {
            return DeviceResult::error("context_busy", "No native call started.");
        };
        let mut result = match executor.invoke(&tool, arguments) {
            Ok(r) => r,
            Err(e) => fault_result(e),
        };
        result.data["result_kind"] = json!("cached_read");
        result.data["native_execution"] = json!(false);
        result
    }
}
