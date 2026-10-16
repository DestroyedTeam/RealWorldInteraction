#![cfg(feature = "fixture")]
//! Synthetic service contract tests; no native OS success is inferred.
use rwi_agent::{
    config::{Config, PlatformPolicy, Principal},
    runtime::Runtime,
};
use rwi_core::device::*;
use rwi_link_trust::{IdentityPaths, PeerPin};
use serde_json::{Value, json};
use std::sync::Arc;

fn setup() -> (tempfile::TempDir, Arc<Runtime>, PeerPin, PeerPin) {
    let t = tempfile::tempdir().unwrap();
    let p = PeerPin::parse(&"a".repeat(64)).unwrap();
    let q = PeerPin::parse(&"b".repeat(64)).unwrap();
    let runtime = Runtime::new(Config {
        version: 1,
        device_id: "device".into(),
        bind: "127.0.0.1:0".parse().unwrap(),
        identity: IdentityPaths::in_profile(t.path()),
        state_root: t.path().join("state"),
        platform: PlatformPolicy::Fixture,
        principals: vec![p.clone(), q.clone()]
            .into_iter()
            .map(|certificate| Principal {
                journal_identity: None,
                certificate,
                observe: true,
                input: true,
            })
            .collect(),
    })
    .unwrap();
    (t, runtime, p, q)
}
async fn wire(r: &Arc<Runtime>, p: &PeerPin, context: Option<&str>, call: Call) -> DeviceResult {
    r.clone()
        .handle(
            p.clone(),
            Request {
                version: PROTOCOL_VERSION,
                id: uuid::Uuid::new_v4().to_string(),
                device: "device".into(),
                context: context.map(str::to_owned),
                call,
            },
        )
        .await
        .result
}
async fn open(r: &Arc<Runtime>, p: &PeerPin) -> (String, Value) {
    let v = wire(r, p, None, Call::OpenContext).await;
    assert!(!v.is_error, "{:?}", v.data);
    (
        v.data["context"].as_str().unwrap().into(),
        v.data["targets"][0]["target"].clone(),
    )
}
async fn prepare(r: &Arc<Runtime>, p: &PeerPin, c: &str, tool: &str, args: Value) -> OperationRef {
    let result = wire(
        r,
        p,
        Some(c),
        Call::Prepare {
            tool: tool.into(),
            arguments: args,
        },
    )
    .await;
    assert!(!result.is_error, "{:?}", result.data);
    assert_eq!(result.data["native_dispatch_started"], false);
    assert_eq!(result.data["input_resources_acquired"], false);
    serde_json::from_value(result.data["operation_ref"].clone()).unwrap()
}
async fn execute(r: &Arc<Runtime>, p: &PeerPin, reference: &OperationRef) -> DeviceResult {
    wire(
        r,
        p,
        None,
        Call::Execute {
            operation_ref: reference.clone(),
        },
    )
    .await
}
async fn invoke(r: &Arc<Runtime>, p: &PeerPin, c: &str, tool: &str, args: Value) -> DeviceResult {
    execute(r, p, &prepare(r, p, c, tool, args).await).await
}
async fn query(r: &Arc<Runtime>, p: &PeerPin, reference: &OperationRef) -> DeviceResult {
    wire(
        r,
        p,
        None,
        Call::Operation {
            selector: OperationSelector::Execution {
                operation_ref: reference.clone(),
            },
        },
    )
    .await
}

#[tokio::test]
async fn closed_context_and_restarted_agent_refs_never_alias_reused_labels() {
    let (_t, r, p, q) = setup();
    let (ca, ta) = open(&r, &p).await;
    let old = prepare(
        &r,
        &p,
        &ca,
        "computer.observe",
        json!({"target":ta,"correlation_id":"shared","inline_image":false}),
    )
    .await;
    assert!(!execute(&r, &p, &old).await.is_error);
    assert_eq!(
        query(&r, &q, &old).await.data["code"],
        "execution_ref_expired"
    );
    assert!(!wire(&r, &p, Some(&ca), Call::CloseContext).await.is_error);
    let (cb, tb) = open(&r, &p).await;
    let new = prepare(
        &r,
        &p,
        &cb,
        "computer.observe",
        json!({"target":tb,"correlation_id":"shared","inline_image":false}),
    )
    .await;
    assert_ne!(old, new);
    assert_eq!(
        query(&r, &p, &old).await.data["code"],
        "execution_ref_expired"
    );
    assert_eq!(
        wire(
            &r,
            &p,
            Some(&cb),
            Call::Cancel {
                operation_ref: old.clone()
            }
        )
        .await
        .data["code"],
        "execution_ref_expired"
    );
    assert_eq!(query(&r, &p, &new).await.data["phase"], "accepted");
    assert!(!execute(&r, &p, &new).await.is_error);
    let config = r.config.clone();
    drop(r);
    let r = Runtime::new(config).unwrap();
    let (cc, tc) = open(&r, &p).await;
    let after_restart = prepare(
        &r,
        &p,
        &cc,
        "computer.observe",
        json!({"target":tc,"correlation_id":"shared","inline_image":false}),
    )
    .await;
    assert_ne!(new.agent_instance, after_restart.agent_instance);
    for old in [old, new] {
        assert_eq!(
            query(&r, &p, &old).await.data["code"],
            "execution_ref_expired"
        );
        assert_eq!(
            wire(&r, &p, None, Call::Cancel { operation_ref: old })
                .await
                .data["code"],
            "execution_ref_expired"
        );
    }
    assert!(!execute(&r, &p, &after_restart).await.is_error);
    assert_eq!(
        execute(&r, &p, &after_restart).await.data["code"],
        "execution_already_consumed"
    );
}

#[tokio::test]
async fn acceptance_is_resource_free_and_forged_or_foreign_references_do_not_cancel() {
    let (_t, r, p, q) = setup();
    let (c, target) = open(&r, &p).await;
    let reference = prepare(
        &r,
        &p,
        &c,
        "computer.claim",
        json!({"target":target,"ttl_ms":60000}),
    )
    .await;
    let (other, other_target) = open(&r, &q).await;
    let lease = invoke(
        &r,
        &q,
        &other,
        "computer.claim",
        json!({"target":other_target,"ttl_ms":60000}),
    )
    .await;
    assert!(!lease.is_error, "Prepare must not acquire an input lease");
    assert!(
        !invoke(
            &r,
            &q,
            &other,
            "computer.release",
            json!({"lease":lease.data["lease"]})
        )
        .await
        .is_error
    );
    for field in [
        "device_id",
        "agent_instance",
        "context_generation",
        "execution_nonce",
    ] {
        let mut altered = json!(reference);
        altered[field] = json!("different");
        let altered: OperationRef = serde_json::from_value(altered).unwrap();
        assert_eq!(
            wire(
                &r,
                &p,
                None,
                Call::Cancel {
                    operation_ref: altered.clone()
                }
            )
            .await
            .data["code"],
            "execution_ref_expired"
        );
        assert_eq!(
            execute(&r, &p, &altered).await.data["code"],
            "execution_ref_expired"
        );
    }
    assert_eq!(
        wire(
            &r,
            &q,
            None,
            Call::Cancel {
                operation_ref: reference.clone()
            }
        )
        .await
        .data["code"],
        "execution_ref_expired"
    );
    assert_eq!(query(&r, &p, &reference).await.data["phase"], "accepted");
    let cancelled = wire(
        &r,
        &p,
        None,
        Call::Cancel {
            operation_ref: reference.clone(),
        },
    )
    .await;
    assert_eq!(cancelled.data["resources_released"], true);
    assert_eq!(
        execute(&r, &p, &reference).await.data["code"],
        "execution_cancelled_before_dispatch"
    );
}

#[tokio::test]
async fn durable_requests_survive_epochs_labels_and_restart_without_changing_old_bytes() {
    let (t, r, p, _q) = setup();
    let (c, target) = open(&r, &p).await;
    let old_read = invoke(
        &r,
        &p,
        &c,
        "computer.observe",
        json!({"target":target,"correlation_id":"request-b","inline_image":false}),
    )
    .await;
    let lease = invoke(
        &r,
        &p,
        &c,
        "computer.claim",
        json!({"target":target,"ttl_ms":60000}),
    )
    .await;
    let args = json!({"target":target,"frame":old_read.data["frame"],"element":"Input","lease":lease.data["lease"],"request_id":"request-b","correlation_id":"same-label","intent":"text","value":"durable exactly once"});
    let mutation = invoke(&r, &p, &c, "computer.act_checked", args.clone()).await;
    assert_eq!(mutation.data["dispatch"], "returned", "{:?}", mutation.data);
    let mut renamed = args;
    renamed["correlation_id"] = json!("other-label");
    let repeated = invoke(&r, &p, &c, "computer.act_checked", renamed).await;
    assert_eq!(repeated.data["dispatched_again"], false);
    let repeated_read = invoke(
        &r,
        &p,
        &c,
        "computer.observe",
        json!({"target":target,"correlation_id":"same-label","inline_image":false}),
    )
    .await;
    assert!(!repeated_read.is_error);
    assert_ne!(
        repeated_read.data["operation_ref"],
        mutation.data["operation_ref"]
    );
    let query_request = || Call::Operation {
        selector: OperationSelector::Request {
            device_id: "device".into(),
            request_id: "request-b".into(),
        },
    };
    let before = wire(&r, &p, None, query_request()).await;
    assert_eq!(before.data["result"]["data"]["dispatch"], "returned");
    invoke(
        &r,
        &p,
        &c,
        "computer.release",
        json!({"lease":lease.data["lease"]}),
    )
    .await;
    let key = rwi_core::digest(&serde_json::to_vec(&(p.as_str(), "request-b")).unwrap());
    let paths = [
        t.path().join(format!("state/operations/{key}.json")),
        t.path()
            .join(format!("state/operations/results/{key}.json")),
    ];
    let bytes = paths
        .iter()
        .map(std::fs::read)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let reference: OperationRef =
        serde_json::from_value(mutation.data["operation_ref"].clone()).unwrap();
    let config = r.config.clone();
    drop(r);
    let r = Runtime::new(config).unwrap();
    assert_eq!(wire(&r, &p, None, query_request()).await.data, before.data);
    assert_eq!(
        query(&r, &p, &reference).await.data["code"],
        "execution_ref_expired"
    );
    for (path, old) in paths.iter().zip(bytes) {
        assert_eq!(std::fs::read(path).unwrap(), old);
    }
    // Request selectors do not accept a read's label unless it is truly a durable key.
    assert_eq!(
        wire(
            &r,
            &p,
            None,
            Call::Operation {
                selector: OperationSelector::Request {
                    device_id: "device".into(),
                    request_id: "same-label".into()
                }
            }
        )
        .await
        .data["code"],
        "request_unknown"
    );
}

#[test]
fn public_selectors_are_strictly_tagged_not_heuristic() {
    let reference = json!({"ref_version":1,"device_id":"device","agent_instance":"agent","context_generation":"context","execution_nonce":"nonce"});
    for args in [
        json!({"operation_id":"legacy"}),
        json!({"selector":{"kind":"request","device_id":"device","request_id":"request","operation_ref":reference}}),
        json!({"selector":{"kind":"execution","operation_ref":reference,"request_id":"request"}}),
    ] {
        assert!(catalog::validate("computer.operation", &args).is_err());
    }
    assert!(catalog::validate("computer.cancel", &json!({"request_id":"request"})).is_err());
    assert!(catalog::validate("computer.cancel", &json!({"operation_ref":reference})).is_ok());
}

#[tokio::test]
async fn state_root_cannot_be_reused_under_another_device_identity() {
    let (_t, r, _p, _q) = setup();
    let mut config = r.config.clone();
    drop(r);
    config.device_id = "another-device".into();
    assert!(
        Runtime::new(config)
            .err()
            .unwrap()
            .to_string()
            .contains("device_namespace_mismatch")
    );
}

#[tokio::test]
async fn namespace_repair_requires_offline_state_owner_and_does_not_reset_ledger() {
    let (t, r, _p, _q) = setup();
    let config = r.config.clone();
    let namespace = config.state_root.join("device-namespace.json");
    let backup = std::fs::read(&namespace).unwrap();
    let backup_path = t.path().join("committed-backup.json");
    std::fs::write(&backup_path, &backup).unwrap();
    std::fs::write(&namespace, b"{truncated").unwrap();
    let proof = rwi_agent::runtime::NamespaceRepair {
        config: t.path().join("config.json"),
        committed_backup: backup_path,
        committed_backup_sha256: rwi_core::digest(&backup),
        damaged_sha256: rwi_core::digest(b"{truncated"),
    };
    assert!(
        Runtime::repair_namespace(&config, &proof, &backup)
            .unwrap_err()
            .to_string()
            .contains("service_state_already_owned")
    );
    assert_eq!(std::fs::read(&namespace).unwrap(), b"{truncated");
    drop(r);
    let repaired = Runtime::repair_namespace(&config, &proof, &backup).unwrap();
    assert_eq!(repaired["ledger_modified"], false);
    assert_eq!(repaired["input_sent"], false);
    assert_eq!(std::fs::read(&namespace).unwrap(), backup);
    assert!(Runtime::new(config).is_ok());
}
