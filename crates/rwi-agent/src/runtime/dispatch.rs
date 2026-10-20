//! One-shot dispatch after service acceptance. Async waiter lifetime does not own native work.
use super::execution::Execution;
use super::*;
use serde_json::Value;

// Passive device-call wall time, not CPU or pure native API time. Record only
// calls actually entered, after acquiring the executor and before transport or
// durable serialization. Integer microseconds round down (saturating at u64::MAX)
// and round-trip exactly; durable results retain the original sample.
pub(super) fn append_device_call_timing(
    result: &mut DeviceResult,
    scope: &'static str,
    elapsed: Option<std::time::Duration>,
) {
    let (Some(elapsed), Some(data)) = (elapsed, result.data.as_object_mut()) else {
        return;
    };
    data.entry("device_call_wall").or_insert_with(|| {
        json!({"scope":scope,"elapsed_us":u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX),
            "cpu_measured":false})
    });
}

pub(super) struct Finish(pub(super) Operation);
pub(super) fn fault_result(error: rwi_core::Fault) -> DeviceResult {
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
// The semantic focus route returns its durable receipt as a nested object.
// Normalize only this exact tool for fence policy, never for model presentation.
fn needs_execution_input_fence(
    tool: &str,
    writes: bool,
    entered: bool,
    resources_released: bool,
    native_call_pending: bool,
    result: &DeviceResult,
) -> bool {
    if !matches!(
        tool,
        "computer.focus_element" | "computer.select_window_local" | "computer.set_text"
    ) {
        return needs_input_fence(writes, entered, resources_released, result);
    }
    if !writes {
        return false;
    }
    if !resources_released
        || native_call_pending
        || result.data["resource_release_uncertain"] == true
        || result
            .data
            .get("cleanup")
            .is_some_and(|v| v["resources_released"] != true)
    {
        return true;
    }
    let mut normalized = result.clone();
    if let Some(receipt) = result.data.get("receipt").and_then(Value::as_object) {
        for key in ["dispatch", "dispatch_error"] {
            if let Some(value) = receipt.get(key) {
                normalized.data[key] = value.clone();
            }
        }
    }
    // A missing counter/protocol receipt cannot turn entered native work into a
    // clean predispatch refusal. The precise no-HID exception requires positive
    // backend output, exact one-write counters and the durable no-replay receipt.
    let returned = normalized.data["dispatch"] == "returned";
    let uncertain = entered && !returned;
    let ordinary = needs_input_fence(writes, entered, resources_released, &normalized);
    let no_held_input = match tool {
        "computer.set_text" => set_text_no_held_input_proof(&normalized),
        "computer.focus_element" => semantic_focus_no_held_input_proof(&normalized),
        "computer.select_window_local" => window_local_no_held_input_proof(&normalized),
        _ => false,
    };
    (uncertain || ordinary) && !no_held_input
}

fn semantic_focus_no_held_input_proof(result: &DeviceResult) -> bool {
    let error = &result.data["dispatch_error"];
    let proof = &error["diagnostic"];
    let before = proof["native_dispatch_count_before"].as_u64();
    let after = proof["native_dispatch_count_after"].as_u64();
    error["code"] == "semantic_focus_unconfirmed"
        && proof["version"] == 1
        && proof["operation_kind"] == "semantic_focus"
        && proof["native_setter_attempts"] == 1
        && proof["native_dispatch_counter_known"] == true
        && proof["native_call_pending"] == false
        && proof["native_resources_released"] == true
        && proof["api"] == "AXUIElementSetAttributeValue"
        && proof["attribute"] == "AXFocused"
        && proof["native_setter_attempted"] == true
        && proof["native_status"]
            .as_i64()
            .is_some_and(|status| i32::try_from(status).is_ok())
        && proof["held_input_resources"] == false
        && proof["replay_allowed"] == false
        && before
            .zip(after)
            .is_some_and(|(n, next)| n.checked_add(1) == Some(next))
        && result.data["receipt"]["replay"] == "forbidden"
        && result.data["resource_release_uncertain"] != true
}

// Actual operation resources_released/native_call_pending are mandatory
// in needs_execution_input_fence BEFORE this AX-only no-held-input proof.
fn set_text_no_held_input_proof(result: &DeviceResult) -> bool {
    let error = &result.data["dispatch_error"];
    let proof = &error["diagnostic"];
    let before = proof["native_dispatch_count_before"].as_u64();
    let after = proof["native_dispatch_count_after"].as_u64();
    error["code"] == "set_text_unconfirmed"
        && proof["version"] == 1
        && proof["operation_kind"] == "set_text"
        && proof["native_setter_attempts"] == 1
        && proof["native_dispatch_counter_known"] == true
        && proof["api"] == "AXUIElementSetAttributeValue"
        && proof["attribute"] == "AXValue"
        && proof["native_setter_attempted"] == true
        && proof["native_status"]
            .as_i64()
            .is_some_and(|status| i32::try_from(status).is_ok())
        && proof["held_input_resources"] == false
        && proof["replay_allowed"] == false
        && before
            .zip(after)
            .is_some_and(|(n, next)| n.checked_add(1) == Some(next))
        && result.data["receipt"]["replay"] == "forbidden"
        && result.data["resource_release_uncertain"] != true
}

fn window_local_no_held_input_proof(result: &DeviceResult) -> bool {
    let error = &result.data["dispatch_error"];
    let proof = &error["diagnostic"];
    let before = proof["native_dispatch_count_before"].as_u64();
    let after = proof["native_dispatch_count_after"].as_u64();
    let exact_kind = matches!(
        (proof["kind"].as_str(), proof["attribute"].as_str()),
        (Some("main"), Some("AXMain")) | (Some("key"), Some("AXFocused"))
    );
    error["code"] == "window_local_selection_unconfirmed"
        && proof["version"] == 1
        && proof["operation_kind"] == "window_local_selection"
        && exact_kind
        && proof["native_window_role"] == "AXWindow"
        && proof["native_setter_attempts"] == 1
        && proof["native_dispatch_counter_known"] == true
        && proof["native_call_pending"] == false
        && proof["native_resources_released"] == true
        && proof["api"] == "AXUIElementSetAttributeValue"
        && proof["native_setter_attempted"] == true
        && proof["native_status"]
            .as_i64()
            .is_some_and(|s| i32::try_from(s).is_ok())
        && proof["held_input_resources"] == false
        && proof["replay_allowed"] == false
        && before
            .zip(after)
            .is_some_and(|(n, next)| n.checked_add(1) == Some(next))
        && result.data["receipt"]["replay"] == "forbidden"
        && result.data["resource_release_uncertain"] != true
}

fn needs_input_fence(
    writes: bool,
    entered: bool,
    resources_released: bool,
    result: &DeviceResult,
) -> bool {
    // Positive backend-issued evidence can distinguish uncertain application
    // effects from a possibly held key/button. Cleanup uncertainty still fences
    // and durable receipts still forbid replay, including preparation-only calls.
    let ax_without_held_input = result.data["dispatch_error"]["code"]
        == "native_action_unconfirmed"
        && result.data["dispatch_error"]["diagnostic"]["api"] == "AXUIElementPerformAction"
        && result.data["dispatch_error"]["diagnostic"]["held_input_resources"] == false
        && result.data["dispatch_error"]["diagnostic"]["replay_allowed"] == false;
    let preparation = &result.data["dispatch_error"]["diagnostic"]["dispatch_evidence"];
    let preparation_without_requested_input = preparation["version"] == 1
        && preparation["kind"] == "macos_background_preparation_only"
        && preparation["api"] == "CGEventPostToPid"
        && preparation["preparation_event"] == "NSApplicationActivatedEventType"
        && preparation["preparation_events"] == 1
        && preparation["requested_input_events"] == 0
        && preparation["held_input_resources"] == false
        && preparation["replay_allowed"] == false;
    writes
        && (!resources_released
            || result.data["resource_release_uncertain"] == true
            || (entered
                && !(ax_without_held_input || preparation_without_requested_input)
                && (result.is_error
                    || result.data["dispatch"] == "uncertain"
                    || result.data["state"] == "uncertain")))
}
#[cfg(test)]
#[test]
fn preparation_only_uncertainty_requires_complete_native_evidence_and_clean_release() {
    let mut result = DeviceResult::value(json!({
        "dispatch": "uncertain",
        "dispatch_error": {
            "code": "stale_reference",
            "diagnostic": {"dispatch_evidence": {
                "version": 1,
                "kind": "macos_background_preparation_only",
                "api": "CGEventPostToPid",
                "preparation_event": "NSApplicationActivatedEventType",
                "preparation_events": 1,
                "requested_input_events": 0,
                "held_input_resources": false,
                "replay_allowed": false
            }}
        }
    }));
    assert!(!needs_input_fence(true, true, true, &result));
    assert!(needs_input_fence(true, true, false, &result));
    result.data["resource_release_uncertain"] = json!(true);
    assert!(needs_input_fence(true, true, true, &result));
    result.data["resource_release_uncertain"] = json!(false);
    for (field, wrong) in [
        ("version", json!(2)),
        ("kind", json!("unknown")),
        ("api", json!("AXUIElementPerformAction")),
        ("preparation_event", json!("mouseDown")),
        ("preparation_events", json!(0)),
        ("requested_input_events", json!(1)),
        ("requested_input_events", json!("0")),
        ("held_input_resources", json!(true)),
        ("held_input_resources", json!("false")),
        ("replay_allowed", json!(true)),
    ] {
        let mut invalid = result.clone();
        invalid.data["dispatch_error"]["diagnostic"]["dispatch_evidence"][field] = wrong;
        assert!(needs_input_fence(true, true, true, &invalid), "{field}");
    }
    for field in [
        "version",
        "kind",
        "api",
        "preparation_event",
        "preparation_events",
        "requested_input_events",
        "held_input_resources",
        "replay_allowed",
    ] {
        let mut invalid = result.clone();
        invalid.data["dispatch_error"]["diagnostic"]["dispatch_evidence"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            needs_input_fence(true, true, true, &invalid),
            "missing {field}"
        );
    }
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
#[cfg(test)]
#[test]
fn ax_effect_uncertainty_does_not_require_unrelated_hardware_to_be_idle() {
    let mut result = DeviceResult::error("native_action_unconfirmed", "Inspect; never replay.");
    result.data["dispatch"] = json!("uncertain");
    result.data["dispatch_error"] = json!({
        "code": "native_action_unconfirmed",
        "diagnostic": {"api":"AXUIElementPerformAction", "held_input_resources":false, "replay_allowed":false}
    });
    assert!(!needs_input_fence(true, true, true, &result));
    assert!(needs_input_fence(true, true, false, &result));
    result.data["resource_release_uncertain"] = json!(true);
    assert!(needs_input_fence(true, true, true, &result));
    result.data["resource_release_uncertain"] = json!(false);
    for evidence in [Value::Null, json!(true), json!("false")] {
        result.data["dispatch_error"]["diagnostic"]["held_input_resources"] = evidence;
        assert!(needs_input_fence(true, true, true, &result));
    }
    result.data["dispatch_error"]["diagnostic"]["held_input_resources"] = json!(false);
    result.data["dispatch_error"]["diagnostic"]["api"] = json!("CGEventPostToPid");
    assert!(needs_input_fence(true, true, true, &result));
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
        if entry.tool == "computer.live" && entry.arguments["action"] != "start" {
            return self.dispatch_live(context, entry, finish).await;
        }
        let permit = match read_admission::acquire(
            context.gate.clone(),
            context.read_waiters.clone(),
            read_admission::eligible(&entry.tool, entry.writes),
            &finish.0,
            &context.closed,
            read_admission::WAIT,
        )
        .await
        {
            Ok(permit) => permit,
            Err(error) => return error,
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
            let mut call_elapsed = None;
            let result=rwi_core::operation::run(&operation, || -> rwi_core::Result<DeviceResult> {
                rwi_core::operation::checkpoint("accepted_execute")?;
                if worker_entry.tool=="computer.recover_input" {
                    return Ok(service.recover(&policy,worker_entry.arguments["resource_domain"].as_str().unwrap_or_default(),Some(&worker_entry.reference.execution_nonce)));
                }
                let mut executor = worker_context.executor.lock().map_err(|_|rwi_core::fault("executor_unavailable","Native context failed; other contexts remain available."))?;
                let arguments = worker_entry.arguments.clone();
                let start = Instant::now();
                let result = executor.invoke(&worker_entry.tool, arguments);
                call_elapsed = Some(start.elapsed());
                result
            });
            let mut result=match result {Ok(r)=>r,Err(e)=>fault_result(e)};
            drop(finish);
            result=worker_entry.envelope(result);
            append_device_call_timing(&mut result, "executor_call", call_elapsed);
            result.data["operation"]=worker_entry.status().data;
            result.data["native_dispatch_entered"]=json!(entered.load(Ordering::Acquire));
            let native_status = operation.status();
            if needs_execution_input_fence(&worker_entry.tool, worker_entry.writes, entered.load(Ordering::Acquire), native_status.resources_released, native_status.native_call_pending, &result) {
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
        if tool == live_image::METHOD {
            return self.cached_live_image(peer, policy, token, arguments).await;
        }
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
            .is_some_and(|target| !context.owns_target(target))
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

#[cfg(test)]
mod semantic_focus_fence_tests;

#[cfg(test)]
mod window_local_fence_tests;

#[cfg(test)]
mod explicit_set_text_fence_tests;

#[cfg(test)]
mod device_call_timing_tests;
