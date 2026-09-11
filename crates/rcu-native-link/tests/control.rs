#![cfg(feature = "legacy-workflow")]
#![cfg(all(feature = "fixture", unix))]
//! Controller/template policy + exact private file tests, plus separately named
//! actual MCP process cases. Simulated native metadata is never native execution.
use rcu_link_trust::IdentityPaths;
use rcu_native_link::{
    budget::{BudgetSpec, now_ms},
    control,
    private::Directory,
    runtime::Runtime,
    sha,
    types::*,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

struct Case {
    _temp: tempfile::TempDir,
    runtime: Arc<Runtime>,
}
impl Case {
    fn new(label: &str, change: impl FnOnce(&mut Approval)) -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = Directory::create(&temp.path().canonicalize().unwrap().join("run")).unwrap();
        let install = Directory::create(&root.path().join("installation")).unwrap();
        let bins = Directory::create(&install.path().join("bin")).unwrap();
        let worker_path = std::env::var_os("RCU_NATIVE_TEST_WORKER").map(PathBuf::from);
        let image = if let Some(path) = worker_path {
            let bytes = std::fs::read(path).unwrap();
            bins.create_new("rcu-mcp", &bytes, 128 * 1024 * 1024)
                .unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                bins.path().join("rcu-mcp"),
                std::fs::Permissions::from_mode(0o700),
            )
            .unwrap();
            Image {
                path: bins.path().join("rcu-mcp"),
                sha256: sha(&bytes),
            }
        } else {
            Image {
                path: bins.path().join("rcu-mcp"),
                sha256: "0".repeat(64),
            }
        };
        let task = format!("control-{label}");
        let now = now_ms();
        let mut approval = Approval {
            version: 1,
            task: task.clone(),
            controller_host: "own-offline-test".into(),
            guest_claim: None,
            native_binding: None,
            budget: BudgetSpec {
                issued_unix_ms: now,
                expires_unix_ms: now + 2_100_000,
                maximum_elapsed_ms: 2_100_000,
                clock_drift_tolerance_ms: 2000,
            },
            request_prefix: format!("{task}-"),
            files: vec![FileSpec {
                name: "allocated.txt".into(),
                initial_utf8: "initial\n".into(),
                expected_sha256: sha(b"final\n"),
            }],
            expected_ui_value: "final\n".into(),
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
        change(&mut approval);
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
            trust: IdentityPaths::in_profile(&root.path().join("not-used-trust")),
            controller_peer_sha256: "a".repeat(64),
            model_peer_sha256: "b".repeat(64),
            agent_image: image.clone(),
            worker_image: image,
            backend: BackendMode::Fixture,
        };
        let runtime = Runtime::new(config).unwrap();
        Self {
            _temp: temp,
            runtime,
        }
    }
    async fn command(&self, operation: ControlOp) -> std::io::Result<Value> {
        control::execute(
            &self.runtime,
            &ControlRequest {
                version: 1,
                request_id: uuid::Uuid::new_v4().to_string(),
                operation,
            },
        )
        .await
    }
    async fn prepare(&self) -> String {
        self.command(ControlOp::Prepare).await.unwrap()["result"]["stage_receipt"]
            .as_str()
            .unwrap()
            .into()
    }
}
fn evidence(name: &str, value: Value) {
    if let Some(directory) = std::env::var_os("RCU_NATIVE_CONTROL_EVIDENCE") {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(PathBuf::from(directory).join(format!("{name}.json")),serde_json::to_vec_pretty(&json!({"schema":"rcu.typed_control_tests.v1","native_runtime":false,"private_keys_included":false,"result":value})).unwrap()).unwrap();
    }
}
fn tree(path: &Path) -> BTreeMap<PathBuf, String> {
    let mut result = BTreeMap::new();
    fn walk(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, String>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                walk(root, &entry.path(), result);
            } else {
                result.insert(
                    entry.path().strip_prefix(root).unwrap().into(),
                    sha(&std::fs::read(entry.path()).unwrap()),
                );
            }
        }
    }
    walk(path, path, &mut result);
    result
}

#[tokio::test]
async fn prepare_is_exact_new_only_and_reconcile_reads_held_effect_without_writes() {
    let case = Case::new("prepare", |_| {});
    let receipt = case.prepare().await;
    assert_eq!(
        case.runtime
            .files()
            .unwrap()
            .read("allocated.txt", control::FILE_CAP)
            .unwrap(),
        b"initial\n"
    );
    let original = tree(&case.runtime.config.state_root);
    assert!(case.command(ControlOp::Prepare).await.is_err());
    assert_eq!(
        tree(&case.runtime.config.state_root),
        original,
        "rejected admission must not mutate another task"
    );
    assert!(!case.runtime.journal().unwrap().state.as_ref().unwrap().hold);
    assert!(
        case.command(ControlOp::Oracle {
            previous_receipt: receipt.clone()
        })
        .await
        .is_err()
    );
    case.runtime.hold("simulated_unknown_effect").unwrap();
    // This is an explicitly labeled test file write, not a claim that a native
    // UI action happened. Reconcile must reveal bytes independently of HOLD.
    std::fs::write(case.runtime.files_path.join("allocated.txt"), b"final\n").unwrap();
    let before = tree(&case.runtime.config.state_root);
    let result = case.command(ControlOp::Reconcile).await.unwrap();
    assert_eq!(result["result"]["file_readback"]["all_match"], true);
    assert_eq!(result["result"]["hold_cleared"], false);
    assert_eq!(result["result"]["status"]["state"]["hold"], true);
    assert_eq!(
        before,
        tree(&case.runtime.config.state_root),
        "reconcile cannot append journal/response/marker"
    );
    let cleaned = case
        .command(ControlOp::Cleanup {
            previous_receipt: receipt,
        })
        .await
        .unwrap();
    assert_eq!(cleaned["result"]["complete"], false);
    assert_eq!(cleaned["result"]["hold_cleared"], false);
    evidence(
        "held-file-readback",
        json!({"scope":"exact_private_files_and_policy_only","file_match_after_explicit_test_write":true,"reconcile_writes":0,"hold_preserved":true}),
    );
}

#[tokio::test]
async fn prepare_collision_holds_but_never_overwrites_and_symlink_oracle_read_refuses() {
    use std::os::unix::fs::symlink;
    let collision = Case::new("collision", |_| {});
    let existing = Directory::create(&collision.runtime.files_path).unwrap();
    existing.create_new("allocated.txt", b"keep", 4).unwrap();
    assert!(collision.command(ControlOp::Prepare).await.is_err());
    assert_eq!(existing.read("allocated.txt", 4).unwrap(), b"keep");
    assert!(
        collision
            .runtime
            .journal()
            .unwrap()
            .state
            .as_ref()
            .unwrap()
            .hold
    );
    let case = Case::new("link", |_| {});
    case.prepare().await;
    case.runtime.hold("test_readonly_hold").unwrap();
    let actual = case.runtime.files_path.join("allocated.txt");
    let outside = case.runtime.run_parent.path().join("outside.txt");
    case.runtime
        .run_parent
        .create_new("outside.txt", b"do-not-read-outside", 64)
        .unwrap();
    std::fs::remove_file(&actual).unwrap();
    symlink(outside, &actual).unwrap();
    let result = case.command(ControlOp::Reconcile).await.unwrap();
    assert_eq!(result["result"]["file_readback"]["all_match"], false);
    assert!(result["result"]["file_readback"]["files"][0]["error"].is_string());
    assert!(
        result["result"]["file_readback"]["files"][0]
            .get("bytes_base64")
            .is_none()
    );
}

#[tokio::test]
async fn local_template_tags_are_bounded_exact_and_do_not_evaluate_result_data() {
    let case = Case::new("templates", |_| {});
    case.prepare().await;
    let expiry = case.runtime.approval.budget.expires_unix_ms;
    let mut local = BTreeMap::new();
    local.insert(
        "opaque".into(),
        json!({"injection":{"$file":"not-approved.txt"},"a/b":{"~x":7},"lease":"lease-one"}),
    );
    assert_eq!(
        control::expand(
            &case.runtime,
            &json!({"$result":{"stage":"current","step":"opaque","pointer":"/injection"}}),
            &local,
            expiry
        )
        .unwrap(),
        json!({"$file":"not-approved.txt"})
    );
    assert_eq!(
        control::expand(
            &case.runtime,
            &json!({"$result":{"stage":"current","step":"opaque","pointer":"/a~1b/~0x"}}),
            &local,
            expiry
        )
        .unwrap(),
        7
    );
    assert_eq!(
        control::expand(&case.runtime, &json!({"$lease":true}), &local, expiry).unwrap(),
        "lease-one"
    );
    let file = control::expand(
        &case.runtime,
        &json!({"$file":"allocated.txt"}),
        &local,
        expiry,
    )
    .unwrap();
    assert_eq!(
        file,
        serde_json::to_value(case.runtime.files_path.join("allocated.txt")).unwrap()
    );
    assert_eq!(
        control::expand(&case.runtime, &json!({"$expires":true}), &local, expiry).unwrap(),
        expiry
    );
    assert!(
        control::expand(
            &case.runtime,
            &json!({"$native_identity":true}),
            &local,
            expiry
        )
        .unwrap()
        .is_object()
    );
    assert_eq!(
        control::expand(&case.runtime, &json!({"$guest_claim":true}), &local, expiry).unwrap(),
        Value::Null
    );
    for value in [
        json!({"$file":"../outside"}),
        json!({"$file":"not-approved.txt"}),
        json!({"$shell":"anything"}),
        json!({"$file":"allocated.txt","extra":1}),
        json!({"$lease":null}),
        json!({"$result":{"stage":"activate","step":"never","pointer":""}}),
        json!({"$result":{"stage":"current","step":"opaque","pointer":"/~2"}}),
        json!({"$result":{"stage":"current","step":"opaque","pointer":"","extra":1}}),
        json!("x".repeat(control::TEMPLATE_STRING + 1)),
    ] {
        assert!(
            control::expand(&case.runtime, &value, &local, expiry).is_err(),
            "{value}"
        );
    }
    let mut deep = Value::Null;
    for _ in 0..40 {
        deep = json!([deep]);
    }
    assert!(control::expand(&case.runtime, &deep, &local, expiry).is_err());
    assert!(
        control::expand(
            &case.runtime,
            &json!(vec![Value::Null; control::TEMPLATE_NODES + 1]),
            &local,
            expiry
        )
        .is_err()
    );
    assert!(control::expand(&case.runtime, &Value::Null, &local, expiry + 1).is_err());
    local.insert("another".into(), json!({"lease":"lease-two"}));
    assert!(control::expand(&case.runtime, &json!({"$lease":true}), &local, expiry).is_err());
    evidence(
        "template-isolation",
        json!({"scope":"pure_template_policy","returned_template_looking_data_opaque":true,"depth":control::TEMPLATE_DEPTH,"nodes":control::TEMPLATE_NODES,"bytes":control::TEMPLATE_BYTES}),
    );
}

fn window() -> Value {
    json!({"window":"hwnd:0x1234","pid":7,"process_created":42,"native_epoch":"pure-policy-epoch"})
}
fn inventory(runtime: &Runtime) -> Value {
    let identity = control::expand(
        runtime,
        &json!({"$native_identity":true}),
        &BTreeMap::new(),
        runtime.approval.budget.expires_unix_ms,
    )
    .unwrap();
    json!({"identity":identity,"windows":[{"key":window(),"application":{"executable":"C:\\approved-not-executed.exe","sha256":"c".repeat(64)},"title":"simulated metadata only","foreground":true,"candidate_facts":{"visible":true,"cloaked":false,"owned":false,"owner_chain_complete":true}}],"complete":true,"native_windows_enumerated":1,"captured_unix_ms":now_ms(),"notes":[]})
}
fn launch_value(runtime: &Runtime) -> Value {
    json!({"request_id":"policy-launch","authorization_id":"policy-only","state":"new_window_observed","before":inventory(runtime),"after":inventory(runtime),"native_events":{"events":[],"complete":true,"hooks_released":true},"native_launch":{"api":"SIMULATED_POLICY_NOT_NATIVE","accepted":true,"process_id":7,"process_created":42,"process_handles_released":true,"error":null},"candidates":[window()],"correlation":"policy only","error":null,"replay":"forbidden","resources_released":true,"resource_release_uncertain":false,"goal_verified":false,"input_authority_granted":false})
}
fn activation_value() -> Value {
    json!({"request_id":"policy-activate","authorization_id":"policy-only","window":window(),"state":"foreground_observed","api_accepted":true,"before_foreground":null,"after_foreground":window(),"user_intervened":false,"error":null,"goal_verified":false,"input_authority_granted":false,"replay":"forbidden","resources_released":true,"resource_release_uncertain":false})
}
#[tokio::test]
async fn simulated_native_metadata_projection_never_selects_ambiguous_or_unconfirmed_window() {
    let case = Case::new("projection", |_| {});
    case.prepare().await;
    let expiry = case.runtime.approval.budget.expires_unix_ms;
    let mut local = BTreeMap::from([
        ("inventory".into(), inventory(&case.runtime)),
        ("launch".into(), launch_value(&case.runtime)),
        ("activation".into(), activation_value()),
    ]);
    let reference = |step| json!({"$result":{"stage":"current","step":step,"pointer":""}});
    let keys = json!({"$window_keys":reference("inventory")});
    assert_eq!(
        control::expand(&case.runtime, &keys, &local, expiry).unwrap(),
        json!([window()])
    );
    let candidate = json!({"$one_candidate":reference("launch")});
    assert_eq!(
        control::expand(&case.runtime, &candidate, &local, expiry).unwrap(),
        window()
    );
    let input = json!({"$input_root":reference("activation")});
    assert_eq!(
        control::expand(&case.runtime, &input, &local, expiry).unwrap(),
        json!({"window":"hwnd:0x1234","pid":7,"process_created":42})
    );
    local.get_mut("inventory").unwrap()["complete"] = json!(false);
    assert!(control::expand(&case.runtime, &keys, &local, expiry).is_err());
    local.get_mut("launch").unwrap()["candidates"] = json!([window(), window()]);
    assert!(control::expand(&case.runtime, &candidate, &local, expiry).is_err());
    local.get_mut("activation").unwrap()["user_intervened"] = json!(true);
    assert!(control::expand(&case.runtime, &input, &local, expiry).is_err());
    local.insert("launch".into(), launch_value(&case.runtime));
    local.get_mut("launch").unwrap()["resource_release_uncertain"] = json!(true);
    assert!(control::expand(&case.runtime, &candidate, &local, expiry).is_err());
    evidence(
        "simulated-native-projections",
        json!({"scope":"simulated_native_metadata_policy_only","real_windows_calls":0,"partial_inventory_rejected":true,"ambiguous_candidate_rejected":true,"user_intervention_rejected":true,"resource_unknown_rejected":true}),
    );
}

#[tokio::test]
async fn actual_worker_schema_claim_release_and_prior_result_hash_are_enforced() {
    std::env::var_os("RCU_NATIVE_TEST_WORKER")
        .expect("explicit prebuilt actual link-fixture worker required; no hidden compilation");
    let case = Case::new("actual-worker", |approval| {
        approval.stages.insert(
            Stage::Launch,
            vec![
                Step {
                    id: "targets".into(),
                    tool: ControlTool::Targets,
                    arguments: json!({}),
                },
                Step {
                    id: "claim".into(),
                    tool: ControlTool::Claim,
                    arguments: json!({"target":"synthetic-only","ttl_ms":30000}),
                },
                Step {
                    id: "release".into(),
                    tool: ControlTool::Release,
                    arguments: json!({"lease":{"$lease":true}}),
                },
            ],
        );
    });
    let mut previous = case.prepare().await;
    for stage in [
        Stage::Bootstrap,
        Stage::Preflight,
        Stage::Launch,
        Stage::Activate,
    ] {
        let response = case
            .command(ControlOp::RunStage {
                stage,
                previous_receipt: previous,
            })
            .await
            .unwrap();
        previous = response["result"]["stage_receipt"].as_str().unwrap().into();
        let receipt = case
            .runtime
            .journal()
            .unwrap()
            .state
            .as_ref()
            .unwrap()
            .receipts[&stage]
            .clone();
        assert_eq!(
            receipt.proof.evidence_level,
            "actual_macos_owned_fixture_process"
        );
        assert!(receipt.proof.owned_worker_exit_observed);
        assert_eq!(receipt.proof.worker_exit_code, Some(0));
        assert!(
            receipt.proof.requests_terminal
                && receipt.proof.resources_released
                && receipt.proof.leases_released
        );
        assert!(!receipt.proof.model_goal_verified);
        assert!(receipt.response_files.contains_key("@wire:targets"));
    }
    let resolved=control::expand(&case.runtime,&json!({"$result":{"stage":"bootstrap","step":"targets","pointer":"/targets/0/identity/host"}}),&BTreeMap::new(),case.runtime.approval.budget.expires_unix_ms).unwrap();
    assert_eq!(resolved, "fixture-host");
    let reference = case
        .runtime
        .journal()
        .unwrap()
        .state
        .as_ref()
        .unwrap()
        .receipts[&Stage::Bootstrap]
        .response_files["targets"]
        .clone();
    std::fs::write(case.runtime.responses.path().join(reference), b"{} ").unwrap();
    assert!(
        control::expand(
            &case.runtime,
            &json!({"$result":{"stage":"bootstrap","step":"targets","pointer":""}}),
            &BTreeMap::new(),
            case.runtime.approval.budget.expires_unix_ms
        )
        .is_err()
    );
    evidence(
        "actual-worker-stage-sequence",
        json!({"scope":"actual_mcp_fixture_process_not_windows","stages":4,"schema_tools":19,"schema_sha256":rcu_native_link::mcp::schema_sha(),"claim_release_confirmed":true,"tampered_prior_result_rejected":true}),
    );
}

#[tokio::test]
async fn active_other_connection_rejection_does_not_kill_but_explicit_cancel_holds_owned_worker() {
    std::env::var_os("RCU_NATIVE_TEST_WORKER")
        .expect("explicit prebuilt actual link-fixture worker required");
    let case = Case::new("concurrent", |_| {});
    let receipt = case.prepare().await;
    case.runtime.begin(Stage::Bootstrap, &receipt).unwrap();
    let mut worker = case
        .runtime
        .make_worker(Stage::Bootstrap, Value::Null, Value::Null)
        .await
        .unwrap();
    worker.initialize().await.unwrap();
    let before = tree(&case.runtime.config.state_root);
    assert!(
        case.command(ControlOp::RunStage {
            stage: Stage::Preflight,
            previous_receipt: receipt.clone()
        })
        .await
        .is_err()
    );
    assert!(
        case.command(ControlOp::Cleanup {
            previous_receipt: receipt
        })
        .await
        .is_err()
    );
    assert_eq!(before, tree(&case.runtime.config.state_root));
    assert!(worker.owned.still_owned_alive().await.unwrap());
    let cancel = case.command(ControlOp::Cancel).await.unwrap();
    assert_eq!(cancel["result"]["held"], true);
    assert_eq!(
        cancel["result"]["cleanup"]["owned_workers"][0]["exit_observed"],
        true
    );
    assert!(!worker.owned.still_owned_alive().await.unwrap());
    assert!(case.runtime.journal().unwrap().state.as_ref().unwrap().hold);
    // OwnedWorker::stop owns bounded stderr collection in the integrated API.
    drop(worker);
    evidence(
        "admission-and-explicit-cancel",
        json!({"scope":"actual_owned_mcp_fixture_process","rejected_concurrent_call_changed_journal":false,"rejected_call_killed_worker":false,"explicit_cancel_owned_exit_observed":true,"native_resources_inferred_from_cancel_ack":false}),
    );
}

#[tokio::test]
async fn started_unknown_step_failure_holds_and_readonly_reconcile_remains_available() {
    std::env::var_os("RCU_NATIVE_TEST_WORKER")
        .expect("explicit prebuilt actual link-fixture worker required");
    let case = Case::new("failure", |approval| {
        approval.stages.insert(
            Stage::Bootstrap,
            vec![Step {
                id: "bad-target".into(),
                tool: ControlTool::Targets,
                arguments: json!({"target":"foreign"}),
            }],
        );
    });
    let receipt = case.prepare().await;
    assert!(
        case.command(ControlOp::RunStage {
            stage: Stage::Bootstrap,
            previous_receipt: receipt
        })
        .await
        .is_err()
    );
    let state = case.runtime.journal().unwrap().state.clone().unwrap();
    assert!(state.hold);
    assert!(state.reservations.len() == 1);
    let owned_workers = case.runtime.live.lock().unwrap().clone();
    for owned in owned_workers {
        assert!(!owned.still_owned_alive().await.unwrap());
    }
    assert_eq!(
        case.command(ControlOp::Reconcile).await.unwrap()["result"]["hold_cleared"],
        false
    );
}

#[tokio::test]
async fn native_missing_semantic_plans_and_malformed_stage_maps_fail_closed() {
    let mut case = Case::new("native-policy", |_| {});
    // Policy flag only: required-step rejection occurs before any Windows API.
    Arc::get_mut(&mut case.runtime).unwrap().config.backend = BackendMode::Windows;
    for stage in [
        Stage::Bootstrap,
        Stage::Preflight,
        Stage::Launch,
        Stage::Activate,
    ] {
        assert!(
            case.command(ControlOp::RunStage {
                stage,
                previous_receipt: String::new()
            })
            .await
            .unwrap_err()
            .to_string()
            .contains("required_semantic_step_missing")
        );
    }
    assert!(case.runtime.journal().unwrap().state.is_none());
    let case = Case::new("bad-map", |approval| {
        approval.lifecycle_grant_template = json!({"bootstrap":null,"unknown":{}});
    });
    let receipt = case.prepare().await;
    assert!(
        case.command(ControlOp::RunStage {
            stage: Stage::Bootstrap,
            previous_receipt: receipt
        })
        .await
        .unwrap_err()
        .to_string()
        .contains("unknown_stage_key")
    );
    assert!(case.runtime.live.lock().unwrap().is_empty());
    assert!(case.runtime.journal().unwrap().state.as_ref().unwrap().hold);
}

#[tokio::test]
async fn expired_approval_allows_readonly_held_file_reconciliation_but_no_stage_or_template() {
    let case = Case::new("expired", |approval| {
        let now = now_ms();
        approval.budget = BudgetSpec {
            issued_unix_ms: now - 5000,
            expires_unix_ms: now - 1000,
            maximum_elapsed_ms: 4000,
            clock_drift_tolerance_ms: 100,
        };
    });
    assert!(case.runtime.budget.is_none());
    // Explicit saved-state/file fixture, not a claim of prior native execution.
    let files = Directory::create(&case.runtime.files_path).unwrap();
    files
        .create_new("allocated.txt", b"final\n", control::FILE_CAP)
        .unwrap();
    case.runtime
        .journal()
        .unwrap()
        .start_task(&case.runtime.approval, false)
        .unwrap();
    case.runtime.hold("saved_unknown_policy_fixture").unwrap();
    let before = tree(&case.runtime.config.state_root);
    let result = case.command(ControlOp::Reconcile).await.unwrap();
    assert_eq!(result["result"]["file_readback"]["all_match"], true);
    assert_eq!(result["result"]["status"]["state"]["hold"], true);
    assert_eq!(result["result"]["hold_cleared"], false);
    assert!(case.command(ControlOp::Prepare).await.is_err());
    assert!(
        case.command(ControlOp::Oracle {
            previous_receipt: String::new()
        })
        .await
        .is_err()
    );
    assert!(
        control::expand(
            &case.runtime,
            &Value::Null,
            &BTreeMap::new(),
            now_ms() + 1000
        )
        .is_err()
    );
    assert_eq!(before, tree(&case.runtime.config.state_root));
    evidence(
        "expired-readonly-reconcile",
        json!({"scope":"past_wall_budget_and_saved_state_file_policy","live_budget":false,"file_match":true,"stage_dispatches":0,"reconciliation_writes":0,"hold_preserved":true}),
    );
}

#[tokio::test]
async fn foreground_projection_rejects_raw_first_page_without_verified_catalog_chain() {
    let case = Case::new("foreground", |_| {});
    case.prepare().await;
    let expiry = case.runtime.approval.budget.expires_unix_ms;
    let mut record = window();
    record["foreground"] = json!(true);
    let discovery = json!({
        "identity": inventory(&case.runtime)["identity"],
        "enumeration_complete": true, "offset":0, "complete":false,
        "windows":[record]
    });
    let template = json!({"$foreground_window":{"$result":{"stage":"current","step":"discovery","pointer":""}}});
    let local = BTreeMap::from([("discovery".into(), discovery.clone())]);
    assert!(
        control::expand(&case.runtime, &template, &local, expiry).is_err(),
        "r2 requires a same-worker verified aggregate even when first page has foreground"
    );
    for fault in [
        "foreign_identity",
        "incomplete_enumeration",
        "later_page",
        "empty",
        "none_foreground",
        "ambiguous",
        "too_many",
        "epoch_missing",
        "pid_zero",
    ] {
        let mut changed = discovery.clone();
        match fault {
            "foreign_identity" => changed["identity"]["session"] = json!("another-session"),
            "incomplete_enumeration" => changed["enumeration_complete"] = json!(false),
            "later_page" => changed["offset"] = json!(8),
            "empty" => changed["windows"] = json!([]),
            "none_foreground" => changed["windows"][0]["foreground"] = json!(false),
            "ambiguous" => changed["windows"] = json!([record.clone(), record.clone()]),
            "too_many" => changed["windows"] = json!(vec![record.clone(); 9]),
            "epoch_missing" => changed["windows"][0]["native_epoch"] = Value::Null,
            "pid_zero" => changed["windows"][0]["pid"] = json!(0),
            _ => unreachable!(),
        }
        let local = BTreeMap::from([("discovery".into(), changed)]);
        assert!(
            control::expand(&case.runtime, &template, &local, expiry).is_err(),
            "{fault}"
        );
    }
    evidence(
        "foreground-projection",
        json!({"scope":"simulated_native_discovery_policy_only","windows_native_calls":0,"raw_partial_first_page_refused":true,"absence_never_maps_to_null":true,"negative_variants":9}),
    );
}

#[tokio::test]
async fn actual_worker_error_response_is_saved_before_control_semantic_rejection() {
    std::env::var_os("RCU_NATIVE_TEST_WORKER")
        .expect("explicit prebuilt actual link-fixture worker required");
    let case = Case::new("wire-error", |approval| {
        approval.stages.insert(Stage::Bootstrap, vec![Step {
            id:"unsupported-preflight".into(), tool:ControlTool::LifecyclePreflight,
            arguments:json!({"target":"synthetic-only","authorization_id":"fixture-does-not-grant-native-lifecycle"}),
        }]);
    });
    let receipt = case.prepare().await;
    assert!(
        case.command(ControlOp::RunStage {
            stage: Stage::Bootstrap,
            previous_receipt: receipt
        })
        .await
        .is_err()
    );
    assert!(case.runtime.journal().unwrap().state.as_ref().unwrap().hold);
    let mut observed = None;
    let mut references = None;
    for event in std::fs::read_dir(case.runtime.config.state_root.join("journal")).unwrap() {
        let value: Value =
            serde_json::from_slice(&std::fs::read(event.unwrap().path()).unwrap()).unwrap();
        if value["kind"] == "typed_step_raw_reply_observed" {
            observed = Some(value["detail"].clone());
        }
        if value["kind"] == "typed_step_response_blobs" {
            references = Some(value["detail"].clone());
        }
    }
    let observed = observed.expect("actual raw response digest retained");
    assert!(digest_shape(observed["raw_sha256"].as_str().unwrap()));
    let references = references.expect("failed wire/structured blobs retained");
    let wire = case
        .runtime
        .response(references["wire_blob"].as_str().unwrap())
        .unwrap();
    assert!(wire["result"]["isError"] == true || wire.get("error").is_some());
    assert!(
        case.runtime
            .response(references["structured_blob"].as_str().unwrap())
            .is_ok()
    );
    assert!(
        !case
            .runtime
            .journal()
            .unwrap()
            .state
            .as_ref()
            .unwrap()
            .receipts
            .contains_key(&Stage::Bootstrap)
    );
    evidence(
        "actual-worker-error-evidence",
        json!({"scope":"actual_mcp_fixture_process","raw_reply_sha256":observed["raw_sha256"],"failed_wire_and_structured_saved":true,"failed_result_became_template_authority":false}),
    );
}

#[tokio::test]
async fn complete_cancel_is_readonly_only_for_matching_clean_complete_task() {
    let case = Case::new("complete-cancel", |_| {});
    case.prepare().await;
    {
        let mut journal = case.runtime.journal().unwrap();
        let state = journal.state.as_mut().unwrap();
        // State-guard fixture only; parent full_link exercises real Complete.
        state.complete = true;
        state.active = None;
        state.next = Stage::Cleanup;
        journal
            .event(
                "complete_cancel_policy_state_fixture",
                json!({"native_execution":false}),
            )
            .unwrap();
    }
    let before = tree(&case.runtime.config.state_root);
    let before_status = case.runtime.journal().unwrap().public_status();
    let result = case.command(ControlOp::Cancel).await.unwrap();
    assert_eq!(result["result"]["cancel_requested"], false);
    assert_eq!(result["result"]["already_complete"], true);
    assert_eq!(result["result"]["complete"], true);
    assert_eq!(result["result"]["hold_cleared"], false);
    assert_eq!(
        case.runtime.journal().unwrap().public_status(),
        before_status
    );
    assert_eq!(tree(&case.runtime.config.state_root), before);
    assert!(
        !case
            .runtime
            .cancelled
            .load(std::sync::atomic::Ordering::SeqCst)
    );
    assert!(case.runtime.live.lock().unwrap().is_empty());

    for mismatch in ["task", "approval"] {
        {
            let mut journal = case.runtime.journal().unwrap();
            let state = journal.state.as_mut().unwrap();
            state.task = case.runtime.approval.task.clone();
            state.approval_sha256 = case.runtime.approval.digest();
            if mismatch == "task" {
                state.task = "another-completed-task".into();
            } else {
                state.approval_sha256 = "0".repeat(64);
            }
        }
        let before = tree(&case.runtime.config.state_root);
        let before_status = case.runtime.journal().unwrap().public_status();
        assert!(case.command(ControlOp::Cancel).await.is_err());
        assert_eq!(
            case.runtime.journal().unwrap().public_status(),
            before_status
        );
        assert_eq!(tree(&case.runtime.config.state_root), before);
        assert!(
            !case
                .runtime
                .cancelled
                .load(std::sync::atomic::Ordering::SeqCst)
        );
    }
    {
        let mut journal = case.runtime.journal().unwrap();
        let state = journal.state.as_mut().unwrap();
        state.task = case.runtime.approval.task.clone();
        state.approval_sha256 = case.runtime.approval.digest();
        state.hold = true;
    }
    assert_eq!(
        case.command(ControlOp::Cancel).await.unwrap()["result"]["cancel_requested"],
        true,
        "an existing HOLD must retain the explicit cancellation path"
    );
    evidence(
        "complete-cancel-noop",
        json!({"scope":"completed_state_policy_fixture_not_native","clean_matching_complete_cancel_writes":0,"worker_restarts":0,"task_and_approval_mismatch_refused":true,"existing_hold_cancel_path_preserved":true}),
    );
}
