#![cfg(feature = "legacy-workflow")]
#![cfg(all(feature = "fixture", unix))]
//! Pure original-format catalog paging and actual Runtime/TLS admission. Neither
//! fabricated catalog metadata nor fixture workers count as native execution.
use rcu_core::lifecycle::WindowIdentity;
use rcu_native_link::{
    budget::{Budget, BudgetSpec, MODEL_ADMISSION_MS, MODEL_USABLE_MS, ModelAdmission, now_ms},
    control::{CATALOG_TTL_MS, CatalogChain},
    frame, net,
    private::Directory,
    runtime::Runtime,
    server, sha,
    types::*,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};
use tokio::io::BufReader;

fn identity() -> Value {
    json!({"host":"fixture-host","guest":null,"session":"session-1","seat":"seat-0","desktop":"desktop-1"})
}
fn window(index: usize) -> WindowIdentity {
    WindowIdentity {
        window: format!("hwnd:{:x}", index + 1),
        pid: 100 + index as u32,
        process_created: 1000 + index as u64,
        native_epoch: format!("epoch-{index}"),
    }
}
fn pages(count: usize, foreground: usize) -> Vec<Value> {
    let rows = (0..count)
        .map(|index| {
            let key = window(index);
            rcu_windows::WindowRecord {
                window: key.window,
                pid: key.pid,
                process_created: Some(key.process_created),
                native_epoch: Some(key.native_epoch),
                title: format!("topmost-fixture-{index}"),
                title_truncated: false,
                class: "OriginalFormatFixture".into(),
                rectangle: None,
                dwm_visible_frame: None,
                visible: true,
                enabled: true,
                minimized: false,
                cloaked: Some(false),
                foreground: index == foreground,
                window_dpi: Some(96),
                window_dpi_awareness: 2,
                owner_chain: vec![],
                owner_chain_complete: true,
                parent: None,
                z_index: index,
            }
        })
        .collect::<Vec<_>>();
    let header = json!({"identity":identity(),"enumeration_complete":true,"native_windows_enumerated":count,
        "catalog_ttl_ms":10000,"displays":[],"selected":null,"filter":{"cursor":null,"pid":null,"title_contains":null},
        "input_grant_scoped":false,"native_api":"test-original-production-page-formatter","coverage_note":"not native enumeration"});
    (0..count)
        .step_by(8)
        .map(|offset| {
            rcu_windows::catalog_page(header.clone(), &rows, &"a".repeat(64), offset, true).unwrap()
        })
        .collect()
}
fn validate(
    pages: &[Value],
    target: usize,
    elapsed: Duration,
) -> std::io::Result<rcu_native_link::control::CatalogSelection> {
    let mut chain = CatalogChain::new(identity(), window(target))?;
    for page in pages {
        let cursor = chain.next_cursor().map(str::to_owned);
        chain.push_at(page, cursor.as_deref(), elapsed)?;
    }
    chain.finish_at(elapsed)
}
#[test]
fn original_more_than_eight_topmost_rows_keep_offsets_and_verify_the_entire_chain() {
    for foreground in [0, 8, 19] {
        let pages = pages(20, foreground);
        let bytes = serde_json::to_vec(&pages).unwrap();
        let selected = validate(&pages, 18, Duration::from_millis(20)).unwrap();
        assert_eq!(selected.foreground, window(foreground));
        assert_eq!(selected.activation_target, window(18));
        assert_eq!((selected.matched_windows, selected.pages), (20, 3));
        assert_eq!(pages[1]["offset"], 8);
        assert_eq!(pages[2]["offset"], 16);
        assert_eq!(
            pages[2]["complete"], false,
            "original native page semantics preserved"
        );
        assert_eq!(serde_json::to_vec(&pages).unwrap(), bytes);
        assert!(
            validate(&pages[..1], 0, Duration::ZERO).is_err(),
            "even foreground+target in first page do not permit early exit"
        );
    }
}
#[test]
fn catalog_changes_missing_duplicates_ambiguity_and_ttl_fail_closed() {
    let base = pages(20, 10);
    for fault in [
        "identity",
        "catalog",
        "offset",
        "cursor",
        "missing",
        "duplicate",
        "total",
        "empty",
        "row_count",
        "expiry",
        "zero_foreground",
        "ambiguous",
        "target_epoch",
        "enum",
        "filter",
        "header",
        "fake_complete",
        "extra",
    ] {
        let mut changed = base.clone();
        let elapsed = if fault == "expiry" {
            Duration::from_millis(CATALOG_TTL_MS)
        } else {
            Duration::ZERO
        };
        match fault {
            "identity" => changed[1]["identity"]["session"] = json!("other"),
            "catalog" => changed[1]["catalog_id"] = json!("b".repeat(64)),
            "offset" => changed[1]["offset"] = json!(0),
            "cursor" => changed[0]["next_cursor"] = json!(format!("{}/16", "a".repeat(64))),
            "missing" => {
                changed.remove(1);
            }
            "duplicate" => changed[1]["windows"][0] = changed[0]["windows"][0].clone(),
            "total" => changed[1]["matched_windows"] = json!(21),
            "empty" => changed[1]["windows"] = json!([]),
            "row_count" => changed[1]["page_count"] = json!(7),
            "zero_foreground" => changed[1]["windows"][2]["foreground"] = json!(false),
            "ambiguous" => changed[2]["windows"][0]["foreground"] = json!(true),
            "target_epoch" => changed[2]["windows"][2]["native_epoch"] = json!("changed"),
            "enum" => changed[0]["enumeration_complete"] = json!(false),
            "filter" => changed[0]["filter"]["pid"] = json!(100),
            "header" => changed[1]["selected"] = json!({"window":"hwnd:99"}),
            "fake_complete" => changed[2]["complete"] = json!(true),
            "extra" => changed.push(changed[2].clone()),
            "expiry" => {}
            _ => unreachable!(),
        }
        assert!(validate(&changed, 18, elapsed).is_err(), "{fault}");
    }
    let mut chain = CatalogChain::new(identity(), window(18)).unwrap();
    assert!(
        chain
            .push_at(&base[0], Some("fabricated/0"), Duration::ZERO)
            .is_err()
    );
}
fn spec(ms: u64) -> BudgetSpec {
    let now = now_ms();
    BudgetSpec {
        issued_unix_ms: now,
        expires_unix_ms: now + ms,
        maximum_elapsed_ms: ms,
        clock_drift_tolerance_ms: 2000,
    }
}
#[test]
fn full_model_reservation_is_not_the_generic_clipped_stage_helper() {
    for remaining in [1_199_000, 1_200_000, 1_979_000] {
        let budget = Budget::start(spec(remaining)).unwrap();
        assert!(budget.stage_spec(MODEL_USABLE_MS).is_ok());
        assert!(ModelAdmission::reserve(&budget).is_err());
    }
    let budget = Budget::start(spec(MODEL_ADMISSION_MS + 5_000)).unwrap();
    let mut admission = ModelAdmission::reserve(&budget).unwrap();
    assert!(admission.worker_spec().unwrap().maximum_elapsed_ms > MODEL_USABLE_MS);
    std::thread::sleep(Duration::from_millis(25));
    let ready = admission.ready().unwrap();
    assert_eq!(ready.maximum_elapsed_ms, MODEL_USABLE_MS);
    assert_eq!(
        ready.expires_unix_ms - ready.issued_unix_ms,
        MODEL_USABLE_MS
    );
    assert!(admission.ready().is_err());
    admission.end().unwrap();
    assert!(admission.end().is_err());
}

struct Case {
    _temp: tempfile::TempDir,
    runtime: Arc<Runtime>,
    model: ClientConfig,
}
impl Case {
    fn new(ms: u64, actual: bool) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        let install =
            Directory::create(&temp.path().canonicalize().unwrap().join("installation")).unwrap();
        let bins = Directory::create(&install.path().join("bin")).unwrap();
        let path = bins.path().join("rcu-mcp");
        let worker = if actual {
            let source = PathBuf::from(
                std::env::var_os("RCU_NATIVE_TEST_WORKER")
                    .expect("explicit prebuilt actual Rust worker required"),
            );
            let bytes = std::fs::read(source).unwrap();
            bins.create_new("rcu-mcp", &bytes, 128 * 1024 * 1024)
                .unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            Image {
                path,
                sha256: sha(&bytes),
            }
        } else {
            Image {
                path,
                sha256: "0".repeat(64),
            }
        };
        let trust = install.path().join("trust");
        let profiles = rcu_link_trust::provision(&trust).unwrap();
        let task = uuid::Uuid::new_v4().to_string();
        let approval = Approval {
            version: 1,
            task: task.clone(),
            controller_host: "owned-loopback".into(),
            guest_claim: None,
            native_binding: None,
            budget: spec(ms),
            request_prefix: format!("{task}-"),
            files: vec![FileSpec {
                name: "allocated.txt".into(),
                initial_utf8: "initial".into(),
                expected_sha256: sha(b"final"),
            }],
            expected_ui_value: "final".into(),
            stages: [
                Stage::Bootstrap,
                Stage::Preflight,
                Stage::Launch,
                Stage::Activate,
            ]
            .into_iter()
            .map(|stage| {
                (
                    stage,
                    vec![Step {
                        id: "targets".into(),
                        tool: ControlTool::Targets,
                        arguments: json!({}),
                    }],
                )
            })
            .collect(),
            input_grant_template: Value::Null,
            lifecycle_grant_template: Value::Null,
            fixture_control: Value::Null,
        };
        install
            .create_new(
                "approved-task.json",
                &serde_json::to_vec(&approval).unwrap(),
                256 * 1024,
            )
            .unwrap();
        let config = AgentConfig {
            privilege_mode: Some(rcu_windows::agent_contract::PrivilegeMode::Ordinary),
            version: 1,
            install_root: install.path().into(),
            state_root: install.path().join("state"),
            approval: install.path().join("approved-task.json"),
            bind: "127.0.0.1:0".parse().unwrap(),
            server_name: rcu_link_trust::SERVER_DNS_NAME.into(),
            trust: rcu_link_trust::IdentityPaths::in_profile(&trust.join("agent")),
            controller_peer_sha256: profiles.controller.certificate_sha256.as_str().into(),
            model_peer_sha256: profiles.model.certificate_sha256.as_str().into(),
            agent_image: worker.clone(),
            worker_image: worker.clone(),
            backend: BackendMode::Fixture,
        };
        let model = ClientConfig {
            model_output: Default::default(),
            version: 1,
            root: install.path().into(),
            endpoint: "127.0.0.1:1".parse().unwrap(),
            server_name: config.server_name.clone(),
            trust: rcu_link_trust::IdentityPaths::in_profile(&trust.join("model")),
            agent_peer_sha256: profiles.agent.certificate_sha256.as_str().into(),
            task: approval.task.clone(),
            approval_sha256: approval.digest(),
            role: Role::Model,
            relay_image: worker,
            entry_sha256: None,
        };
        let runtime = Runtime::new(config).unwrap();
        // Explicit state fixture only: this case isolates admission, not native
        // stage success. The actual server and model worker are not substituted.
        let files = Directory::create(&runtime.files_path).unwrap();
        files
            .create_new("allocated.txt", b"initial", 65536)
            .unwrap();
        runtime.prepare_fixture().unwrap();
        let mut journal = runtime.journal().unwrap();
        journal.start_task(&approval, false).unwrap();
        journal.state.as_mut().unwrap().next = Stage::Model;
        journal
            .event("test_admission_predecessor_state_not_native", Value::Null)
            .unwrap();
        drop(journal);
        Self {
            _temp: temp,
            runtime,
            model,
        }
    }
    async fn open(
        &mut self,
    ) -> (
        tokio::task::JoinHandle<std::io::Result<()>>,
        BufReader<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>,
        Attach,
    ) {
        let listener = net::bind_fixture_loopback("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        self.model.endpoint = listener.local_addr().unwrap();
        let task = tokio::spawn(server::serve(self.runtime.clone(), listener));
        let mut stream = BufReader::new(net::connect(&self.model).await.unwrap());
        let attach = Attach {
            version: rcu_native_link::TRANSPORT_VERSION,
            task: self.model.task.clone(),
            approval_sha256: self.model.approval_sha256.clone(),
            role: Role::Model,
            nonce: uuid::Uuid::new_v4().to_string(),
        };
        net::write_json(&mut stream, &attach, frame::ATTACH_CAP)
            .await
            .unwrap();
        (task, stream, attach)
    }
}
fn journal_tree(runtime: &Runtime) -> BTreeMap<String, String> {
    std::fs::read_dir(runtime.config.state_root.join("journal"))
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (
                entry.file_name().to_string_lossy().into_owned(),
                sha(&std::fs::read(entry.path()).unwrap()),
            )
        })
        .collect()
}
#[tokio::test]
async fn actual_runtime_and_authenticated_server_reject_short_model_before_intent_or_spawn() {
    for ms in [1_200_000, 1_979_000] {
        let mut case = Case::new(ms, false);
        let before = journal_tree(&case.runtime);
        assert!(
            case.runtime
                .begin(Stage::Model, "")
                .unwrap_err()
                .to_string()
                .contains("full_model")
        );
        assert_eq!(before, journal_tree(&case.runtime));
        let (server, mut stream, attach) = case.open().await;
        let returned = tokio::time::timeout(
            Duration::from_secs(5),
            frame::read_frame(&mut stream, frame::ATTACH_CAP),
        )
        .await
        .unwrap();
        let denial: Value = serde_json::from_slice(&returned.unwrap().unwrap()).unwrap();
        assert_eq!(denial["attached"], false);
        assert_eq!(denial["version"], rcu_native_link::TRANSPORT_VERSION);
        assert_eq!(denial["task"], attach.task);
        assert_eq!(denial["nonce"], attach.nonce);
        assert_eq!(denial["role"], "model");
        assert!(denial["reason"].as_str().unwrap().contains("full_model"));
        assert!(denial["hold_reason"].is_null());
        assert_eq!(denial["next_stage"], "model");
        assert_eq!(before, journal_tree(&case.runtime));
        assert!(case.runtime.live.lock().unwrap().is_empty());
        assert_eq!(
            case.runtime
                .journal()
                .unwrap()
                .state
                .as_ref()
                .unwrap()
                .active,
            None
        );
        assert!(!case.runtime.journal().unwrap().state.as_ref().unwrap().hold);
        server.abort();
        let _ = server.await;
    }
}
#[tokio::test]
async fn actual_worker_schema_handshake_precedes_full_usable_ready_clock() {
    let mut case = Case::new(2_100_000, true);
    let (server, mut stream, attach) = case.open().await;
    let ack = frame::read_frame(&mut stream, frame::ATTACH_CAP)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&ack).unwrap()["attached"],
        true
    );
    let began = now_ms();
    tokio::time::sleep(Duration::from_millis(40)).await;
    net::write_json(&mut stream,&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"actual-ready-test","version":"1"}}}),frame::INBOUND).await.unwrap();
    let initialized = frame::read_frame(&mut stream, frame::OUTBOUND)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&initialized).unwrap()["result"]["serverInfo"]["version"],
        rcu_native_link::mcp::VERSION
    );
    net::write_json(
        &mut stream,
        &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        frame::INBOUND,
    )
    .await
    .unwrap();
    net::write_json(
        &mut stream,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        frame::INBOUND,
    )
    .await
    .unwrap();
    let ready = frame::read_frame(&mut stream, frame::OUTBOUND)
        .await
        .unwrap()
        .unwrap();
    let ready: net::ModelReady = serde_json::from_slice(&ready).unwrap();
    assert_eq!((ready.task, ready.nonce), (attach.task, attach.nonce));
    assert_eq!(ready.usable_ms, MODEL_USABLE_MS);
    let tools = frame::read_frame(&mut stream, frame::OUTBOUND)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&tools).unwrap()["result"]["tools"]
            .as_array()
            .unwrap()
            .len(),
        19
    );
    let mut ready_budget = None;
    for entry in std::fs::read_dir(case.runtime.config.state_root.join("journal")).unwrap() {
        let value: Value =
            serde_json::from_slice(&std::fs::read(entry.unwrap().path()).unwrap()).unwrap();
        if value["kind"] == "actual_model_ready_full_usable_budget" {
            ready_budget = Some(value["detail"]["budget"].clone());
        }
    }
    let budget = ready_budget.unwrap();
    assert!(budget["issued_unix_ms"].as_u64().unwrap() >= began + 35);
    assert_eq!(budget["maximum_elapsed_ms"], MODEL_USABLE_MS);
    assert_eq!(
        budget["expires_unix_ms"].as_u64().unwrap() - budget["issued_unix_ms"].as_u64().unwrap(),
        MODEL_USABLE_MS
    );
    drop(stream); // intentional disconnect, not a goal or normal-completion claim
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let workers = case.runtime.live.lock().unwrap().clone();
            let mut alive = false;
            for worker in workers {
                alive |= worker.still_owned_alive().await.unwrap();
            }
            if !alive && case.runtime.journal().unwrap().state.as_ref().unwrap().hold {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    server.abort();
    let _ = server.await;
}

#[test]
fn foreground_template_revalidates_original_page_blobs_and_current_worker_binding() {
    let case = Case::new(2_100_000, false);
    let record =
        json!({"stage":"activate","owned_child":true,"binding_fixture":"one-current-worker"});
    {
        let mut journal = case.runtime.journal().unwrap();
        journal.state.as_mut().unwrap().active = Some(Stage::Activate);
        journal.worker(record.clone()).unwrap();
    }
    let original = pages(20, 10);
    let selection = validate(&original, 18, Duration::ZERO).unwrap();
    let mut page_refs = Vec::new();
    let mut cursor = Value::Null;
    for (index, content) in original.iter().enumerate() {
        let wire = json!({"jsonrpc":"2.0","id":10+index,"result":{"structuredContent":content}});
        let mut raw = serde_json::to_vec_pretty(&wire).unwrap();
        // One exact NDJSON frame, preserving deliberate harmless whitespace.
        raw.retain(|byte| *byte != b'\n');
        raw.push(b'\n');
        case.runtime.save_wire(&raw).unwrap();
        page_refs.push(json!({"request_cursor":cursor,"structured_blob":case.runtime.save_response(content).unwrap(),
            "wire_blob":case.runtime.save_response(&wire).unwrap(),"raw_sha256":sha(&raw)}));
        cursor = content["next_cursor"].clone();
    }
    let aggregate = json!({"kind":"catalog_chain_v2","started_unix_ms":now_ms(),"worker_binding_sha256":sha(&serde_json::to_vec(&record).unwrap()),"selection":selection,"pages":page_refs});
    let template =
        json!({"$foreground_window":{"$result":{"stage":"current","step":"catalog","pointer":""}}});
    let expand = |aggregate: Value| {
        rcu_native_link::control::expand(
            &case.runtime,
            &template,
            &BTreeMap::from([("catalog".into(), aggregate)]),
            now_ms() + 1000,
        )
    };
    assert_eq!(
        expand(aggregate.clone()).unwrap(),
        serde_json::to_value(window(10)).unwrap()
    );
    for fault in [
        "projected_target",
        "projected_foreground",
        "raw_digest",
        "later_as_zero",
        "expired",
        "worker",
        "extra_field",
    ] {
        let mut changed = aggregate.clone();
        match fault {
            "projected_target" => {
                changed["selection"]["activation_target"] =
                    serde_json::to_value(window(99)).unwrap()
            }
            "projected_foreground" => {
                changed["selection"]["foreground"] = serde_json::to_value(window(0)).unwrap()
            }
            "raw_digest" => changed["pages"][1]["raw_sha256"] = json!("b".repeat(64)),
            "later_as_zero" => changed["pages"][1]["request_cursor"] = Value::Null,
            "expired" => changed["started_unix_ms"] = json!(now_ms() - 10_000),
            "worker" => changed["worker_binding_sha256"] = json!("b".repeat(64)),
            "extra_field" => changed["offset"] = json!(0),
            _ => unreachable!(),
        }
        assert!(expand(changed).is_err(), "{fault}");
    }
    assert!(expand(original[0].clone()).is_err());
}

#[tokio::test]
async fn real_runtime_oracle_and_cleanup_tail_boundaries_do_not_block_emergency_cleanup() {
    for (stage, ms) in [(Stage::Oracle, 659_000), (Stage::Cleanup, 599_000)] {
        let case = Case::new(ms, false);
        {
            let mut journal = case.runtime.journal().unwrap();
            journal.state.as_mut().unwrap().next = stage;
            journal
                .event("test_tail_predecessor_not_native", Value::Null)
                .unwrap();
        }
        let before = journal_tree(&case.runtime);
        assert!(
            case.runtime
                .begin(stage, "")
                .unwrap_err()
                .to_string()
                .contains("full_model_and_fixed_reserves_unavailable")
        );
        assert_eq!(journal_tree(&case.runtime), before);
        assert!(!case.runtime.journal().unwrap().state.as_ref().unwrap().hold);
        // Explicit emergency cleanup may still revoke and reap owned workers;
        // it never claims a normal Oracle/Cleanup stage or clears HOLD.
        let cancelled = rcu_native_link::control::execute(
            &case.runtime,
            &ControlRequest {
                version: 1,
                request_id: uuid::Uuid::new_v4().to_string(),
                operation: ControlOp::Cancel,
            },
        )
        .await
        .unwrap();
        assert_eq!(cancelled["result"]["cancel_requested"], true);
        assert!(
            cancelled["result"]["cleanup"]["owned_workers"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    let case = Case::new(700_000, false);
    case.runtime.journal().unwrap().state.as_mut().unwrap().next = Stage::Oracle;
    case.runtime.begin(Stage::Oracle, "").unwrap();
    case.runtime.completion_budget_check(Stage::Oracle).unwrap();
    assert!(
        case.runtime
            .completion_budget_check(Stage::Cleanup)
            .is_err()
    );
    let mut proof = case.runtime.base_proof();
    proof.file_oracle_verified = true;
    let previous = case
        .runtime
        .finish(Stage::Oracle, proof, BTreeMap::new())
        .unwrap();
    case.runtime.begin(Stage::Cleanup, &previous).unwrap();
    case.runtime
        .completion_budget_check(Stage::Cleanup)
        .unwrap();
    case.runtime
        .finish(Stage::Cleanup, case.runtime.base_proof(), BTreeMap::new())
        .unwrap();
    assert!(
        case.runtime
            .journal()
            .unwrap()
            .state
            .as_ref()
            .unwrap()
            .complete
    );
}

#[test]
fn model_reservation_requires_both_wall_and_monotonic_headroom() {
    let mut wall_short = spec(MODEL_ADMISSION_MS - 1);
    wall_short.maximum_elapsed_ms = 3_600_000;
    assert!(ModelAdmission::reserve(&Budget::start(wall_short).unwrap()).is_err());
    let mut monotonic_short = spec(3_600_000);
    monotonic_short.maximum_elapsed_ms = MODEL_ADMISSION_MS - 1;
    assert!(ModelAdmission::reserve(&Budget::start(monotonic_short).unwrap()).is_err());
}
