//! One-shot dispatch after service acceptance. Async waiter lifetime does not own native work.
use super::execution::Execution;
use super::*;
use serde_json::Value;

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
mod semantic_focus_fence_tests {
    use super::*;
    fn uncertain() -> DeviceResult {
        DeviceResult::value(
            json!({"receipt":{"dispatch":"uncertain","replay":"forbidden","goal_verified":false,"system_accepted":null,"dispatch_error":{"code":"focus_setter_uncertain"}},"focus_readback":{"state":"unknown","setter_status":null},"input_domain_fenced":false}),
        )
    }
    fn exact_proof() -> DeviceResult {
        let mut result = uncertain();
        result.data["receipt"]["dispatch_error"] = json!({"code":"semantic_focus_unconfirmed","diagnostic":{"version":1,"operation_kind":"semantic_focus","native_setter_attempts":1,"native_dispatch_counter_known":true,"native_call_pending":false,"native_resources_released":true,"api":"AXUIElementSetAttributeValue","attribute":"AXFocused","native_setter_attempted":true,"native_status":-25202,"native_dispatch_count_before":4,"native_dispatch_count_after":5,"held_input_resources":false,"replay_allowed":false}});
        result
    }
    fn fence(result: &DeviceResult, entered: bool, release: bool, pending: bool) -> bool {
        needs_execution_input_fence(
            "computer.focus_element",
            true,
            entered,
            release,
            pending,
            result,
        )
    }
    #[test]
    fn semantic_focus_nested_uncertainty_now_fences_but_old_tool_policy_is_unchanged() {
        let result = uncertain();
        assert!(
            !needs_input_fence(true, true, true, &result),
            "old top-level predicate reproduces the gap"
        );
        assert!(fence(&result, true, true, false));
        assert!(!needs_execution_input_fence(
            "computer.act_checked",
            true,
            true,
            true,
            false,
            &result
        ));
        assert!(fence(&result, false, false, false));
        assert!(fence(&result, false, true, true));
        let mut missing = DeviceResult::value(json!({"focus_readback":null}));
        assert!(fence(&missing, true, true, false));
        missing.data["receipt"] =
            json!({"dispatch":"rejected_before_dispatch","replay":"forbidden"});
        assert!(!fence(&missing, false, true, false));
    }
    #[test]
    fn semantic_focus_clean_return_or_no_write_is_not_a_goal_or_held_input_uncertainty() {
        for (entered, accepted, goal) in [(true, json!(true), false), (false, Value::Null, true)] {
            let result = DeviceResult::value(
                json!({"receipt":{"dispatch":"returned","replay":"forbidden","system_accepted":accepted,"goal_verified":goal}}),
            );
            assert!(!fence(&result, entered, true, false));
            assert!(fence(&result, entered, false, false));
            assert!(fence(&result, entered, true, true));
        }
    }
    #[test]
    fn semantic_focus_no_hid_exception_requires_complete_exact_native_counter_evidence() {
        let exact = exact_proof();
        assert!(!fence(&exact, true, true, false));
        assert!(fence(&exact, true, false, false));
        assert!(fence(&exact, true, true, true));
        for key in [
            "version",
            "operation_kind",
            "native_setter_attempts",
            "native_dispatch_counter_known",
            "native_call_pending",
            "native_resources_released",
            "api",
            "attribute",
            "native_setter_attempted",
            "native_status",
            "native_dispatch_count_before",
            "native_dispatch_count_after",
            "held_input_resources",
            "replay_allowed",
        ] {
            let mut incomplete = exact.clone();
            incomplete.data["receipt"]["dispatch_error"]["diagnostic"]
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(fence(&incomplete, true, true, false), "{key}");
        }
        for (key, value) in [
            ("version", json!(2)),
            ("operation_kind", json!("click")),
            ("native_setter_attempts", json!(2)),
            ("native_dispatch_counter_known", json!(false)),
            ("native_call_pending", json!(true)),
            ("native_resources_released", json!(false)),
            ("api", json!("AXUIElementPerformAction")),
            ("attribute", json!("AXFocusedUIElement")),
            ("native_setter_attempted", json!(false)),
            ("held_input_resources", json!(true)),
            ("replay_allowed", json!(true)),
            ("native_status", json!(i64::from(i32::MAX) + 1)),
            ("native_status", json!(i64::from(i32::MIN) - 1)),
            ("native_status", json!("unknown")),
            ("native_dispatch_count_after", json!(4)),
            ("native_dispatch_count_before", json!(u64::MAX)),
        ] {
            let mut wrong = exact.clone();
            wrong.data["receipt"]["dispatch_error"]["diagnostic"][key] = value;
            assert!(fence(&wrong, true, true, false), "{key}");
        }
        let mut cleanup = exact.clone();
        cleanup.data["resource_release_uncertain"] = json!(true);
        assert!(fence(&cleanup, true, true, false));
        let mut cleanup_unknown = exact.clone();
        cleanup_unknown.data["cleanup"] = json!({"code":"release_uncertain"});
        assert!(fence(&cleanup_unknown, true, true, false));
        let mut replay = exact.clone();
        replay.data["receipt"]["replay"] = json!("allowed");
        assert!(fence(&replay, true, true, false));
        let mut saturated_unknown = exact.clone();
        saturated_unknown.data["receipt"]["dispatch_error"]["diagnostic"]["native_dispatch_count_before"] =
            json!(u64::MAX);
        saturated_unknown.data["receipt"]["dispatch_error"]["diagnostic"]["native_dispatch_count_after"] =
            Value::Null;
        assert!(fence(&saturated_unknown, true, true, false));
        let mut wrong_code = exact;
        wrong_code.data["receipt"]["dispatch_error"]["code"] = json!("native_action_unconfirmed");
        assert!(fence(&wrong_code, true, true, false));
    }
}

#[cfg(test)]
mod window_local_fence_tests {
    use super::*;
    fn exact(kind: &str) -> DeviceResult {
        DeviceResult::value(
            json!({"receipt":{"dispatch":"uncertain","replay":"forbidden","dispatch_error":{"code":"window_local_selection_unconfirmed","diagnostic":{"version":1,"operation_kind":"window_local_selection","kind":kind,"native_window_role":"AXWindow","api":"AXUIElementSetAttributeValue","attribute":if kind=="main"{"AXMain"}else{"AXFocused"},"native_setter_attempted":true,"native_setter_attempts":1,"native_status":-25202,"native_dispatch_counter_known":true,"native_dispatch_count_before":4,"native_dispatch_count_after":5,"native_call_pending":false,"native_resources_released":true,"held_input_resources":false,"replay_allowed":false}}}}),
        )
    }
    fn fence(r: &DeviceResult, release: bool, pending: bool) -> bool {
        needs_execution_input_fence(
            "computer.select_window_local",
            true,
            true,
            release,
            pending,
            r,
        )
    }
    #[test]
    fn window_local_no_hid_exception_requires_exact_kind_window_and_one_setter_counter() {
        for kind in ["main", "key"] {
            let r = exact(kind);
            assert!(!fence(&r, true, false));
            assert!(fence(&r, false, false));
            assert!(fence(&r, true, true));
            for key in [
                "version",
                "operation_kind",
                "kind",
                "native_window_role",
                "api",
                "attribute",
                "native_setter_attempted",
                "native_setter_attempts",
                "native_status",
                "native_dispatch_counter_known",
                "native_dispatch_count_before",
                "native_dispatch_count_after",
                "native_call_pending",
                "native_resources_released",
                "held_input_resources",
                "replay_allowed",
            ] {
                let mut bad = r.clone();
                bad.data["receipt"]["dispatch_error"]["diagnostic"]
                    .as_object_mut()
                    .unwrap()
                    .remove(key);
                assert!(fence(&bad, true, false), "{kind} missing {key}");
            }
            for (key, value) in [
                ("native_setter_attempts", json!(2)),
                ("native_window_role", json!("AXTextField")),
                ("kind", json!("focused")),
                ("native_dispatch_count_before", json!(u64::MAX)),
                ("native_dispatch_count_after", json!(6)),
                ("native_call_pending", json!(true)),
                ("native_resources_released", json!(false)),
                ("held_input_resources", json!(true)),
                ("replay_allowed", json!(true)),
            ] {
                let mut bad = r.clone();
                bad.data["receipt"]["dispatch_error"]["diagnostic"][key] = value;
                assert!(fence(&bad, true, false), "{kind} {key}");
            }
            let mut wrong = r.clone();
            wrong.data["receipt"]["dispatch_error"]["diagnostic"]["attribute"] =
                json!(if kind == "main" {
                    "AXFocused"
                } else {
                    "AXMain"
                });
            assert!(fence(&wrong, true, false));
            assert!(
                needs_execution_input_fence("computer.focus_element", true, true, true, false, &r),
                "window proof must not exempt original element-focus route"
            );
        }
    }
    #[test]
    fn window_local_nested_unknown_or_cleanup_fences_while_returned_partial_has_no_held_input() {
        let unknown =
            DeviceResult::value(json!({"receipt":{"dispatch":"uncertain","replay":"forbidden"}}));
        assert!(fence(&unknown, true, false));
        let returned = DeviceResult::value(
            json!({"receipt":{"dispatch":"returned","replay":"forbidden","goal_verified":false},"window_readback":{"state":"partial","typing_authorized":false}}),
        );
        assert!(!fence(&returned, true, false));
        assert!(fence(&returned, false, false));
        assert!(fence(&returned, true, true));
        let mut cleanup = exact("main");
        cleanup.data["cleanup"] = json!({"resources_released":null});
        assert!(fence(&cleanup, true, false));
        let mut release = exact("key");
        release.data["resource_release_uncertain"] = json!(true);
        assert!(fence(&release, true, false));
    }
}

#[cfg(test)]
mod explicit_set_text_fence_tests {
    #[test]
    fn explicit_set_text_uncertain_fences_unless_exact_once_ax_no_held_input_evidence() {
        use super::*;
        let mut result = DeviceResult::value(
            json!({"receipt":{"dispatch":"uncertain","replay":"forbidden","goal_verified":false,"dispatch_error":{"code":"set_text_unconfirmed"}}}),
        );
        assert!(needs_execution_input_fence(
            "computer.set_text",
            true,
            true,
            true,
            false,
            &result
        ));
        result.data["receipt"]["dispatch_error"]["diagnostic"] = json!({"version":1,"operation_kind":"set_text","native_setter_attempts":1,"native_dispatch_counter_known":true,"api":"AXUIElementSetAttributeValue","attribute":"AXValue","native_setter_attempted":true,"native_status":-25204,"native_dispatch_count_before":4,"native_dispatch_count_after":5,"held_input_resources":false,"replay_allowed":false});
        assert!(!needs_execution_input_fence(
            "computer.set_text",
            true,
            true,
            true,
            false,
            &result
        ));
        assert!(needs_execution_input_fence(
            "computer.set_text",
            true,
            true,
            true,
            true,
            &result
        ));
        assert!(needs_execution_input_fence(
            "computer.set_text",
            true,
            true,
            false,
            false,
            &result
        ));
        result.data["receipt"]["dispatch_error"]["diagnostic"]["native_dispatch_count_after"] =
            6.into();
        assert!(needs_execution_input_fence(
            "computer.set_text",
            true,
            true,
            true,
            false,
            &result
        ));
    }
}
