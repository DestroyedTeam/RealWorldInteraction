use super::*;

#[tokio::test]
async fn device_call_timing_executor_is_measured_and_duplicate_not_retimed() {
    let (_temp, runtime, principal, preview) = setup();
    let (token, target) = binding(&runtime, &principal, preview).await;
    let accepted = runtime.prepare(
        principal.identity(),
        &principal,
        Some(&token),
        "computer.observe".into(),
        json!({"target":target}),
    );
    assert!(!accepted.is_error, "{}", accepted.data);
    let reference = serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
    let first = runtime
        .execute(principal.identity(), &principal, &reference)
        .await;
    assert!(!first.is_error, "{}", first.data);
    let timing = &first.data["device_call_wall"];
    assert_eq!(timing["scope"], "executor_call");
    assert!(timing["elapsed_us"].as_u64().is_some());
    assert_eq!(timing["cpu_measured"], false);
    let duplicate = runtime
        .execute(principal.identity(), &principal, &reference)
        .await;
    assert!(duplicate.is_error);
    assert_eq!(duplicate.data["code"], "execution_already_consumed");
    assert!(duplicate.data.get("device_call_wall").is_none());
}
#[tokio::test]
async fn device_call_timing_live_success_and_controller_error_are_measured() {
    let (_temp, runtime, principal, preview) = setup();
    let (token, target) = binding(&runtime, &principal, preview.clone()).await;
    let status = invoke(
        &runtime,
        &principal,
        &token,
        "computer.live",
        json!({"target":target,"action":"status"}),
    )
    .await;
    assert!(!status.is_error, "{}", status.data);
    assert_eq!(
        status.data["device_call_wall"]["scope"],
        "live_controller_call"
    );
    assert_eq!(preview.0.lock().unwrap().calls, 1);
    let error = invoke(&runtime, &principal, &token, "computer.live",
        json!({"target":target,"action":"status","view":{"view_id":"b".repeat(32),"sequence":1,"visible":true}})).await;
    assert!(error.is_error);
    assert_eq!(error.data["code"], "live_view_mismatch");
    assert_eq!(
        error.data["device_call_wall"]["scope"],
        "live_controller_call"
    );
    assert!(
        error.data["device_call_wall"]["elapsed_us"]
            .as_u64()
            .is_some()
    );
    assert_eq!(preview.0.lock().unwrap().calls, 1);
}
#[tokio::test]
async fn device_call_timing_pre_dispatch_cancel_has_no_fabricated_zero() {
    let (_temp, runtime, principal, preview) = setup();
    let (token, target) = binding(&runtime, &principal, preview.clone()).await;
    let accepted = runtime.prepare(
        principal.identity(),
        &principal,
        Some(&token),
        "computer.live".into(),
        view(&target, 1, true),
    );
    let reference = serde_json::from_value(accepted.data["operation_ref"].clone()).unwrap();
    runtime.cancel(principal.identity(), &reference);
    let result = runtime
        .execute(principal.identity(), &principal, &reference)
        .await;
    assert_eq!(result.data["code"], "execution_cancelled_before_dispatch");
    assert!(result.data.get("device_call_wall").is_none());
    assert_eq!(preview.0.lock().unwrap().calls, 0);
}

#[tokio::test]
async fn device_call_timing_durable_input_retains_original_sample_without_replay() {
    let (_temp, runtime, principal, preview) = setup();
    let (token, target) = binding(&runtime, &principal, preview).await;
    let lease = invoke(
        &runtime,
        &principal,
        &token,
        "computer.claim",
        json!({"target":target,"ttl_ms":60000}),
    )
    .await;
    let observation = invoke(
        &runtime,
        &principal,
        &token,
        "computer.observe",
        json!({"target":target,"inline_image":false}),
    )
    .await;
    let args = json!({"target":target,"frame":observation.data["frame"],"lease":lease.data["lease"],
        "request_id":"timed-input","element":"Input","intent":"text","value":"timing fixture"});
    let first = invoke(
        &runtime,
        &principal,
        &token,
        "computer.act_checked",
        args.clone(),
    )
    .await;
    assert!(!first.is_error, "{}", first.data);
    assert_eq!(first.data["device_call_wall"]["scope"], "executor_call");
    let duplicate = invoke(&runtime, &principal, &token, "computer.act_checked", args).await;
    assert!(!duplicate.is_error, "{}", duplicate.data);
    assert_eq!(duplicate.data["dispatched_again"], false);
    assert_eq!(duplicate.data["automatic_replay"], false);
    assert!(duplicate.data.get("device_call_wall").is_none());
    assert_eq!(
        duplicate.data["result"]["data"]["device_call_wall"],
        first.data["device_call_wall"]
    );
    let retained = wire(
        &runtime,
        &principal,
        None,
        Call::Operation {
            selector: OperationSelector::Request {
                device_id: "device".into(),
                request_id: "timed-input".into(),
            },
        },
    )
    .await;
    assert_eq!(retained.data["replay_allowed"], false);
    assert_eq!(
        retained.data["result"]["data"]["device_call_wall"],
        first.data["device_call_wall"]
    );
    let released = invoke(
        &runtime,
        &principal,
        &token,
        "computer.release",
        json!({"lease":lease.data["lease"]}),
    )
    .await;
    assert!(!released.is_error);
}
