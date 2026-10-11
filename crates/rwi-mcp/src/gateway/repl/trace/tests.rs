//! Owned loopback mTLS + synthetic status/release receipts, not native GUI input.
use super::*;

#[test]
fn returned_click_then_failed_observation_is_not_an_input_failure_or_rollback() {
    let calls = vec![
        json!({"tool":"computer.act_checked","outcome":"returned","is_error":false,"dispatch":"returned","native_dispatch_entered":true}),
        json!({"tool":"computer.observe","outcome":"returned","is_error":true,"code":"capture_binding_changed","native_dispatch_entered":false}),
        json!({"tool":"computer.release","outcome":"returned","is_error":false}),
    ];
    let summary = execution_summary(&calls, true, false);
    assert_eq!(summary["cell_status"], "failed");
    assert_eq!(summary["dispatch_returned"], 1);
    assert_eq!(summary["mutation_attempts"], 1);
    assert_eq!(summary["last_failed_tool"], "computer.observe");
    assert_eq!(summary["last_failed_code"], "capture_binding_changed");
    assert_eq!(summary["goal_verified"], false);
    assert_eq!(summary["replay_allowed"], false);
}

#[test]
fn uncertain_preparation_missing_dispatch_and_rejected_input_stay_distinct() {
    let calls = vec![
        json!({"tool":"computer.act_checked","outcome":"returned","is_error":true,"dispatch":"uncertain","native_dispatch_entered":true}),
        json!({"tool":"computer.act","outcome":"unconfirmed"}),
        json!({"tool":"computer.act_checked","outcome":"returned","is_error":true,"dispatch":"rejected_before_dispatch","native_dispatch_entered":false}),
        json!({"tool":"computer.observe","outcome":"returned","is_error":false,"native_dispatch_entered":true}),
    ];
    let summary = execution_summary(&calls, true, true);
    assert_eq!(summary["mutation_attempts"], 3);
    assert_eq!(summary["dispatch_returned"], 0);
    assert_eq!(summary["rejected_before_dispatch"], 1);
    assert_eq!(summary["dispatch_unconfirmed"], 2);
    assert_eq!(summary["cleanup_unconfirmed"], true);
}

#[test]
fn local_js_and_read_only_failure_do_not_claim_a_native_mutation() {
    let summary = execution_summary(
        &[json!({"tool":"computer.result","outcome":"returned","is_error":false})],
        true,
        false,
    );
    assert_eq!(summary["mutation_attempts"], 0);
    assert!(summary["last_failed_tool"].is_null());
    assert_eq!(summary["goal_verified"], false);
}
use rwi_link_trust::IdentityPaths;
use rwi_native_link::device::PairedDevice;

#[derive(Clone, Copy)]
enum Mode {
    Settles,
    Pending,
    Expired,
    ReadFailure,
    StalledQuery,
    WrongInstance,
    WrongRef,
    Malformed,
    BusyOnce,
    BusyAlways,
    HoldRelease,
    HoldRetry,
    Normal,
}
struct Counts {
    queries: AtomicUsize,
    prepares: AtomicUsize,
    releases: AtomicUsize,
    unexpected: AtomicUsize,
    order: Mutex<Vec<&'static str>>,
    executed: Mutex<BTreeSet<String>>,
    release_entered: tokio::sync::Notify,
    release_reply: tokio::sync::Notify,
}
struct Fixture {
    _dir: tempfile::TempDir,
    _stop: tokio::sync::watch::Sender<bool>,
    server: tokio::task::JoinHandle<io::Result<()>>,
    gateway: Gateway,
    counts: Arc<Counts>,
}
fn operation(nonce: &str) -> device::OperationRef {
    device::OperationRef {
        ref_version: 1,
        device_id: "device".into(),
        agent_instance: "agent".into(),
        context_generation: "context".into(),
        execution_nonce: nonce.into(),
    }
}
fn limits() -> Limits {
    Limits {
        total: Duration::from_millis(400),
        settle: Duration::from_millis(180),
        poll: Duration::from_millis(10),
        queries: 8,
    }
}
impl Fixture {
    async fn new(mode: Mode) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap().join("trust");
        let pairing = rwi_link_trust::provision(&root).unwrap();
        let tls = IdentityPaths::in_profile(&root.join("agent"))
            .load()
            .unwrap()
            .server_config(std::slice::from_ref(&pairing.model.certificate_sha256))
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = listener.local_addr().unwrap();
        let counts = Arc::new(Counts {
            queries: AtomicUsize::new(0),
            prepares: AtomicUsize::new(0),
            releases: AtomicUsize::new(0),
            unexpected: AtomicUsize::new(0),
            order: Mutex::new(vec![]),
            executed: Mutex::new(BTreeSet::new()),
            release_entered: tokio::sync::Notify::new(),
            release_reply: tokio::sync::Notify::new(),
        });
        let stats = counts.clone();
        let (stop, rx) = tokio::sync::watch::channel(false);
        let server = tokio::spawn(rwi_native_link::device::serve(
            listener,
            tls,
            move |_, r: Request| {
                let counts = stats.clone();
                async move {
                    let mut instance = "agent";
                    let result = match r.call {
                        Call::Operation {
                            selector: device::OperationSelector::Execution { operation_ref },
                        } => {
                            assert!(
                                r.context.is_none(),
                                "control status must not enter the context gate"
                            );
                            assert_eq!(operation_ref, operation("cancelled"));
                            let n = counts.queries.fetch_add(1, Ordering::Relaxed) + 1;
                            counts.order.lock().unwrap().push("query");
                            match mode {
                                Mode::Expired => DeviceResult::error(
                                    "execution_ref_expired",
                                    "Not a termination proof.",
                                ),
                                Mode::ReadFailure => {
                                    DeviceResult::error("status_read_failed", "Unknown.")
                                }
                                Mode::StalledQuery => std::future::pending::<DeviceResult>().await,
                                _ => {
                                    let quiet = !matches!(mode, Mode::Pending)
                                        && (n >= 2 || matches!(mode, Mode::Normal));
                                    let mut data = json!({"operation_ref":operation_ref,"terminal":quiet,"native_call_pending":!quiet,"resources_released":quiet,"input_domain_fenced":true});
                                    if matches!(mode, Mode::WrongInstance) {
                                        instance = "restarted-agent";
                                    }
                                    if matches!(mode, Mode::WrongRef) {
                                        data["operation_ref"] = json!(operation("different"));
                                    }
                                    if matches!(mode, Mode::Malformed) {
                                        data.as_object_mut().unwrap().remove("resources_released");
                                    }
                                    DeviceResult::value(data)
                                }
                            }
                        }
                        Call::Prepare { tool, arguments } if tool == "computer.release" => {
                            assert!(arguments["lease"].as_str().unwrap().starts_with("owned-"));
                            assert_eq!(r.context.as_deref(), Some("context"));
                            DeviceResult::value(
                                json!({"accepted":true,"operation_ref":operation(&format!("release-{}", counts.prepares.fetch_add(1, Ordering::Relaxed) + 1))}),
                            )
                        }
                        Call::Execute { operation_ref }
                            if operation_ref.execution_nonce.starts_with("release-") =>
                        {
                            assert!(same_context(&operation_ref, &operation("release")));
                            assert!(
                                counts
                                    .executed
                                    .lock()
                                    .unwrap()
                                    .insert(operation_ref.execution_nonce.clone()),
                                "Execute was replayed"
                            );
                            let n = counts.releases.fetch_add(1, Ordering::Relaxed) + 1;
                            counts.order.lock().unwrap().push("release");
                            if matches!(mode, Mode::HoldRelease) && n == 1
                                || matches!(mode, Mode::HoldRetry) && n == 2
                            {
                                counts.release_entered.notify_one();
                                counts.release_reply.notified().await;
                            }
                            let busy = matches!(
                                mode,
                                Mode::Pending
                                    | Mode::Expired
                                    | Mode::ReadFailure
                                    | Mode::StalledQuery
                                    | Mode::BusyAlways
                            ) || matches!(mode, Mode::BusyOnce | Mode::HoldRetry)
                                && n == 1;
                            if busy {
                                let mut e = DeviceResult::error(
                                    "context_busy",
                                    "Only this context is busy; not dispatched.",
                                );
                                e.data["operation_ref"] = json!(operation_ref);
                                e.data["native_dispatch_entered"] = json!(false);
                                e
                            } else {
                                DeviceResult::value(
                                    json!({"released":true,"operation_ref":operation_ref,"input_domain_fenced":true}),
                                )
                            }
                        }
                        _ => {
                            counts.unexpected.fetch_add(1, Ordering::Relaxed);
                            DeviceResult::error(
                                "unexpected_call",
                                "No input, Cancel, OpenContext, recovery or durable query is allowed by this fixture.",
                            )
                        }
                    };
                    device::Reply {
                        version: device::PROTOCOL_VERSION,
                        id: r.id,
                        device: r.device,
                        instance: instance.into(),
                        result,
                    }
                }
            },
            rx,
        ));
        let gateway = Gateway::new(Ok(AdapterProfile {
            devices: vec![PairedDevice {
                device_id: "device".into(),
                endpoint,
                identity: IdentityPaths::in_profile(&root.join("model")),
                agent_pin: pairing.agent.certificate_sha256,
            }],
        }));
        *gateway.routes["device"].context.lock().await = Some(Connection {
            token: "context".into(),
            instance: "agent".into(),
            targets: Value::Null,
        });
        Self {
            _dir: dir,
            _stop: stop,
            server,
            gateway,
            counts,
        }
    }
    fn trace(&self, cancelled: bool) -> Trace {
        let mut trace = Trace::new(self.gateway.clone());
        self.claim(&mut trace, "owned-lease");
        let args = json!({"request_id":"original-action"})
            .as_object()
            .unwrap()
            .clone();
        let index = trace.begin("computer.act", &args);
        let result = if cancelled {
            CallToolResult::structured_error(
                json!({"code":"operation_cancel_requested","operation_ref":operation("cancelled"),"cancellation_delivery_confirmed":true,"cancellation_accepted":true,"goal_verified":false,"original_action_replayed":false}),
            )
        } else {
            CallToolResult::structured(
                json!({"operation_ref":operation("successful-action"),"system_accepted":true,"goal_verified":false}),
            )
        };
        trace.finish(index, "computer.act", &args, &result);
        trace
    }
    fn claim(&self, trace: &mut Trace, lease: &str) {
        self.gateway.remember("device", &json!({"lease":lease}));
        let args = serde_json::Map::new();
        let index = trace.begin("computer.claim", &args);
        trace.finish(
            index,
            "computer.claim",
            &args,
            &CallToolResult::structured(json!({"lease":lease,"operation_ref":operation("claim")})),
        );
    }
    fn assert_no_input_or_fallback(&self) {
        assert_eq!(self.counts.unexpected.load(Ordering::Relaxed), 0);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
    }
}

#[tokio::test]
async fn cancelled_pending_execution_is_confirmed_before_release_without_touching_original_receipt()
{
    let f = Fixture::new(Mode::Settles).await;
    let mut trace = f.trace(true);
    let calls = trace.calls.clone();
    trace.cleanup_with(limits()).await;
    assert_eq!(trace.calls, calls);
    assert!(!trace.cleanup_failed());
    assert_eq!(trace.cleanup[0]["native_quiescence_confirmed"], true);
    assert_eq!(trace.cleanup[0]["result"]["released"], true);
    assert_eq!(trace.cleanup[0]["result"]["input_domain_fenced"], true);
    assert_eq!(
        *f.counts.order.lock().unwrap(),
        ["query", "query", "release"]
    );
    f.assert_no_input_or_fallback();
}

#[tokio::test]
async fn normal_success_never_polls_an_operation() {
    let f = Fixture::new(Mode::Normal).await;
    let mut trace = f.trace(false);
    trace.cleanup_with(limits()).await;
    assert!(!trace.cleanup_failed());
    assert_eq!(f.counts.queries.load(Ordering::Relaxed), 0);
    assert_eq!(f.counts.releases.load(Ordering::Relaxed), 1);
    assert!(trace.cleanup[0]["native_quiescence_confirmed"].is_null());
    f.assert_no_input_or_fallback();
}

#[tokio::test]
async fn pending_expired_read_error_and_identity_changes_stay_unconfirmed_and_bounded() {
    for mode in [
        Mode::Pending,
        Mode::Expired,
        Mode::ReadFailure,
        Mode::StalledQuery,
        Mode::WrongInstance,
        Mode::WrongRef,
        Mode::Malformed,
    ] {
        let f = Fixture::new(mode).await;
        let mut trace = f.trace(true);
        let original = trace.calls.clone();
        let start = Instant::now();
        trace.cleanup_with(limits()).await;
        assert!(start.elapsed() < Duration::from_secs(2));
        assert!(trace.cleanup_failed());
        assert_eq!(trace.calls, original);
        assert_eq!(trace.cleanup[0]["native_quiescence_confirmed"], false);
        assert_eq!(trace.cleanup[0]["code"], "lease_release_unconfirmed");
        assert_eq!(
            trace.cleanup[0]["release_attempts"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(f.counts.queries.load(Ordering::Relaxed) <= limits().queries);
        if matches!(
            mode,
            Mode::Pending | Mode::Expired | Mode::ReadFailure | Mode::StalledQuery
        ) {
            assert_eq!(trace.cleanup[0]["result"]["code"], "context_busy");
        }
        f.assert_no_input_or_fallback();
    }
}

#[tokio::test]
async fn context_busy_gets_only_one_qualified_retry_and_retains_the_first_error() {
    for mode in [Mode::BusyOnce, Mode::BusyAlways] {
        let f = Fixture::new(mode).await;
        let mut trace = f.trace(true);
        trace.cleanup_with(limits()).await;
        let attempts = trace.cleanup[0]["release_attempts"].as_array().unwrap();
        assert_eq!(attempts.len(), 2);
        assert_eq!(attempts[0]["result"]["code"], "context_busy");
        assert_eq!(attempts[0]["result"]["native_dispatch_entered"], false);
        assert_eq!(f.counts.queries.load(Ordering::Relaxed), 3);
        assert_eq!(f.counts.releases.load(Ordering::Relaxed), 2);
        assert_eq!(trace.cleanup_failed(), matches!(mode, Mode::BusyAlways));
        f.assert_no_input_or_fallback();
    }
}

#[tokio::test]
async fn earlier_release_error_and_unqualified_cancellation_never_become_stop_proof() {
    let f = Fixture::new(Mode::Normal).await;
    let mut trace = f.trace(true);
    trace.calls[1]["operation_ref"] = json!({"execution_nonce":"bare"});
    let args = json!({"lease":"owned-lease"}).as_object().unwrap().clone();
    let index = trace.begin("computer.release", &args);
    let original_error = json!({"code":"context_busy","marker":"preserve-original-release"});
    trace.finish(
        index,
        "computer.release",
        &args,
        &CallToolResult::structured_error(original_error.clone()),
    );
    trace.cleanup_with(limits()).await;
    assert_eq!(f.counts.queries.load(Ordering::Relaxed), 0);
    assert_eq!(f.counts.releases.load(Ordering::Relaxed), 1);
    assert!(trace.cleanup_failed());
    assert_eq!(trace.cleanup[0]["original_release_error"], original_error);
    assert_eq!(trace.cleanup[0]["native_quiescence_confirmed"], false);
    f.assert_no_input_or_fallback();
}

#[tokio::test]
async fn unrelated_context_is_not_polled_and_new_adapter_context_cannot_redirect_old_release() {
    let f = Fixture::new(Mode::Normal).await;
    let mut trace = f.trace(true);
    trace.calls[1]["operation_ref"]["context_generation"] = json!("another-context");
    trace.cleanup_with(limits()).await;
    assert!(!trace.cleanup_failed());
    assert_eq!(f.counts.queries.load(Ordering::Relaxed), 0);
    let mut trace = f.trace(true);
    f.gateway.routes["device"]
        .context
        .lock()
        .await
        .as_mut()
        .unwrap()
        .token = "replacement".into();
    trace.cleanup_with(limits()).await;
    assert!(!trace.cleanup_failed());
    assert_eq!(
        trace.cleanup[0]["result"]["operation_ref"]["context_generation"],
        "context"
    );
    assert_eq!(f.counts.releases.load(Ordering::Relaxed), 2);
    f.assert_no_input_or_fallback();
}

#[tokio::test]
async fn all_leases_share_one_cleanup_deadline_and_query_cap() {
    let f = Fixture::new(Mode::Pending).await;
    let mut trace = f.trace(true);
    for n in 0..3 {
        f.claim(&mut trace, &format!("owned-extra-{n}"));
    }
    let start = Instant::now();
    trace.cleanup_with(limits()).await;
    assert!(start.elapsed() < Duration::from_secs(2));
    assert_eq!(trace.cleanup.len(), 4);
    assert!(trace.cleanup.iter().all(|row| row["is_error"] == true));
    assert_eq!(f.counts.queries.load(Ordering::Relaxed), limits().queries);
    assert_eq!(f.counts.releases.load(Ordering::Relaxed), 4);
    f.assert_no_input_or_fallback();
}

#[tokio::test]
async fn future_drop_uses_the_same_owned_cleanup_rules_and_budget() {
    let f = Fixture::new(Mode::Settles).await;
    let mut trace = f.trace(true);
    trace.budget = Some(Budget::new(limits()));
    let budget = trace.budget.clone().unwrap();
    drop(trace);
    let jobs = std::mem::take(&mut *f.gateway.cancels.lock().unwrap());
    for job in jobs {
        job.await.unwrap();
    }
    assert_eq!(budget.queries.load(Ordering::Relaxed), 2);
    assert_eq!(
        *f.counts.order.lock().unwrap(),
        ["query", "query", "release"]
    );
    f.assert_no_input_or_fallback();
}

#[tokio::test]
async fn expired_cleanup_budget_cannot_restart_rpc_work_on_drop() {
    let f = Fixture::new(Mode::Settles).await;
    let mut trace = f.trace(true);
    trace.budget = Some(Budget {
        end: Instant::now() - Duration::from_millis(1),
        settle: Instant::now() - Duration::from_millis(1),
        poll: limits().poll,
        queries: Arc::new(AtomicUsize::new(limits().queries)),
        limit: limits().queries,
    });
    drop(trace);
    let jobs = std::mem::take(&mut *f.gateway.cancels.lock().unwrap());
    for job in jobs {
        job.await.unwrap();
    }
    assert_eq!(f.counts.queries.load(Ordering::Relaxed), 0);
    assert_eq!(f.counts.prepares.load(Ordering::Relaxed), 0);
    assert_eq!(f.counts.releases.load(Ordering::Relaxed), 0);
    f.assert_no_input_or_fallback();
}

async fn start_then_cancel_cleanup_await(trace: &mut Trace, f: &Fixture) {
    let future = trace.cleanup_with(limits());
    tokio::pin!(future);
    tokio::select! {
        _ = f.counts.release_entered.notified() => {},
        _ = &mut future => panic!("cleanup completed before held Execute response"),
    }
    // Dropping ONLY this borrowed future must leave its accepted job in Trace.
}

#[tokio::test]
async fn drop_during_first_or_second_release_execute_transfers_jobs_without_replay() {
    for (mode, attempts) in [(Mode::HoldRelease, 1), (Mode::HoldRetry, 2)] {
        let f = Fixture::new(mode).await;
        let mut trace = f.trace(true);
        start_then_cancel_cleanup_await(&mut trace, &f).await;
        assert!(Instant::now() < trace.budget.as_ref().unwrap().end);
        assert_eq!(f.counts.prepares.load(Ordering::Relaxed), attempts);
        assert_eq!(f.counts.releases.load(Ordering::Relaxed), attempts);
        drop(trace);
        f.counts.release_reply.notify_one();
        let jobs = std::mem::take(&mut *f.gateway.cancels.lock().unwrap());
        for job in jobs {
            job.await.unwrap();
        }
        assert_eq!(f.counts.prepares.load(Ordering::Relaxed), attempts);
        assert_eq!(f.counts.releases.load(Ordering::Relaxed), attempts);
        assert_eq!(f.counts.executed.lock().unwrap().len(), attempts);
        f.assert_no_input_or_fallback();
    }
}

#[tokio::test]
async fn cancelled_cleanup_await_resumes_the_same_accepted_ref_error_history_and_budget() {
    let f = Fixture::new(Mode::HoldRetry).await;
    let mut trace = f.trace(true);
    let calls = trace.calls.clone();
    start_then_cancel_cleanup_await(&mut trace, &f).await;
    let budget = trace.budget.clone().unwrap();
    assert!(Instant::now() < budget.end);
    f.counts.release_reply.notify_one();
    trace.cleanup_with(LIMITS).await; // Deliberately different: must not reset it.
    assert_eq!(trace.budget.as_ref().unwrap().end, budget.end);
    assert!(Arc::ptr_eq(
        &trace.budget.as_ref().unwrap().queries,
        &budget.queries
    ));
    assert_eq!(trace.calls, calls);
    assert!(!trace.cleanup_failed());
    let attempts = trace.cleanup[0]["release_attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0]["result"]["code"], "context_busy");
    assert_eq!(
        attempts[0]["result"]["operation_ref"],
        json!(operation("release-1"))
    );
    assert_eq!(
        attempts[1]["result"]["operation_ref"],
        json!(operation("release-2"))
    );
    assert_eq!(f.counts.prepares.load(Ordering::Relaxed), 2);
    assert_eq!(f.counts.releases.load(Ordering::Relaxed), 2);
    assert_eq!(budget.queries.load(Ordering::Relaxed), 3);
    f.assert_no_input_or_fallback();
}

#[tokio::test]
async fn an_existing_empty_job_set_never_rebuilds_unconfirmed_lease_bookkeeping() {
    for resume in [false, true] {
        let f = Fixture::new(Mode::Normal).await;
        let mut trace = f.trace(false);
        trace.pending_cleanup = Some(tokio::task::JoinSet::new());
        trace.budget = Some(Budget::new(limits()));
        if resume {
            trace.cleanup_with(limits()).await;
            assert!(trace.cleanup_failed());
            assert_eq!(trace.cleanup[0]["reason"], "cleanup_job_result_unconfirmed");
        }
        drop(trace);
        let jobs = std::mem::take(&mut *f.gateway.cancels.lock().unwrap());
        for job in jobs {
            job.await.unwrap();
        }
        assert_eq!(f.counts.queries.load(Ordering::Relaxed), 0);
        assert_eq!(f.counts.prepares.load(Ordering::Relaxed), 0);
        assert_eq!(f.counts.releases.load(Ordering::Relaxed), 0);
    }
}
