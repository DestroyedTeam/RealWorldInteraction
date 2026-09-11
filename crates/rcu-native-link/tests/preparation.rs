#![cfg(feature = "legacy-workflow")]
#![cfg(unix)]
//! Offline plan/import validation only. No Windows installation or OS operation.
use base64::{Engine, engine::general_purpose::STANDARD};
use rcu_link_trust::IdentityPaths;
use rcu_native_link::{
    install,
    plan::{self, NotepadSeed},
    sha,
    types::*,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::PathBuf};
fn seed() -> NotepadSeed {
    serde_json::from_value(json!({"task":"offline-native-plan","controller_host":"offline-fixture-controller","route_vm_id":"12345678-1234-1234-1234-123456789abc",
        "native_binding":{"native":{"host":format!("windows-machine:{}","1".repeat(64)),"guest":null,"session":"wts:2:logon:12345:luid:0:6789","seat":"WinSta0","desktop":"Default"},"user_sid":"S-1-5-21-1-2-3-1001","session_id":2,"authentication_id":{"high":0,"low":26505},"logon_time_100ns":12345,"input_desktop":"Default"},
        "application":{"executable":"C:\\Windows\\System32\\notepad.exe","sha256":"a".repeat(64)},"observed_applications":[],"observed_packages":[],"application_startup_effects_authorized":true,"initial_utf8":"","expected_utf8":"fixture plan not a native run"})).unwrap()
}
#[test]
fn generated_notepad_plan_is_bounded_grounded_staged_and_not_a_runtime_claim() {
    let a = plan::notepad(seed()).unwrap();
    a.validate(BackendMode::Windows).unwrap();
    assert_eq!(
        a.budget.expires_unix_ms - a.budget.issued_unix_ms,
        3_600_000
    );
    assert_eq!(a.request_prefix, "offline-native-plan-");
    let v = serde_json::to_value(&a).unwrap();
    assert_eq!(v["stages"]["activate"][0]["tool"], "catalog");
    assert!(
        v["stages"]["activate"][2]["arguments"]["expected_foreground"]
            .get("$foreground_window")
            .is_some()
    );
    assert_eq!(
        v["lifecycle_grant_template"]["bootstrap"]["launches"][0]["allowed_existing_windows"],
        json!([])
    );
    assert!(
        v["lifecycle_grant_template"]["preflight"]["launches"][0]["allowed_existing_windows"]
            .get("$window_keys")
            .is_some()
    );
    assert!(
        v["input_grant_template"]["root"]
            .get("$input_root")
            .is_some()
    );
    let mut missing = seed();
    missing.application_startup_effects_authorized = false;
    assert!(plan::notepad(missing).is_err());
    let mut bad = seed();
    bad.task = "underscored_task".into();
    assert!(plan::notepad(bad).is_err());
}
#[test]
fn private_windows_bundle_can_be_validated_offline_on_mac_without_install_or_key_output() {
    let temp = tempfile::tempdir().unwrap();
    let profile = temp.path().canonicalize().unwrap().join("profiles");
    let provision = rcu_link_trust::provision(&profile).unwrap();
    let a = plan::notepad(seed()).unwrap();
    let root = PathBuf::from("C:\\Users\\Fixture\\AppData\\Local\\RealComputerUse\\native-link-r1");
    let join = |suffix: &str| PathBuf::from(format!("{}\\{}", root.display(), suffix));
    let agent = b"offline fixture image, never executed";
    let worker = b"offline fixture worker, never executed";
    let config = AgentConfig {
        privilege_mode: Some(rcu_windows::agent_contract::PrivilegeMode::Ordinary),
        version: 1,
        install_root: root.clone(),
        state_root: join("state"),
        approval: join("approved-task.json"),
        bind: "192.0.2.9:45121".parse().unwrap(),
        server_name: rcu_link_trust::SERVER_DNS_NAME.into(),
        trust: IdentityPaths {
            ca: join("trust\\agent\\ca.der"),
            certificate: join("trust\\agent\\certificate.der"),
            private_key: join("trust\\agent\\private-key.der"),
        },
        controller_peer_sha256: provision.controller.certificate_sha256.as_str().into(),
        model_peer_sha256: provision.model.certificate_sha256.as_str().into(),
        agent_image: Image {
            path: join("bin\\rcu-native-agent.exe"),
            sha256: sha(agent),
        },
        worker_image: Image {
            path: join("bin\\rcu-mcp.exe"),
            sha256: sha(worker),
        },
        backend: BackendMode::Windows,
    };
    let mut files = BTreeMap::new();
    for (name, bytes) in [
        ("agent.json", serde_json::to_vec(&config).unwrap()),
        ("approved-task.json", serde_json::to_vec(&a).unwrap()),
        ("rcu-native-agent.exe", agent.to_vec()),
        ("rcu-mcp.exe", worker.to_vec()),
    ] {
        files.insert(
            name.to_owned(),
            json!({"sha256":sha(&bytes),"base64":STANDARD.encode(bytes)}),
        );
    }
    for name in ["ca.der", "certificate.der", "private-key.der"] {
        let b = std::fs::read(profile.join("agent").join(name)).unwrap();
        files.insert(
            name.into(),
            json!({"sha256":sha(&b),"base64":STANDARD.encode(b)}),
        );
    }
    let mut bundle = json!({"version":1,"files":files});
    let bytes = zeroize::Zeroizing::new(serde_json::to_vec(&bundle).unwrap());
    let result = install::validate_bundle(&bytes).unwrap();
    assert_eq!(result["valid_bundle"], true);
    assert_eq!(result["installed"], false);
    assert_eq!(result["native_binding_runtime_checked"], false);
    assert!(
        result["files"]["private-key.der"]
            .as_str()
            .is_some_and(|s| s.len() == 64)
    );
    assert!(install::import(&bytes).is_err());
    assert!(install::approve_next(&bytes).is_err());
    bundle["files"]["rcu-mcp.exe"]["sha256"] = json!("0".repeat(64));
    assert!(install::validate_bundle(&serde_json::to_vec(&bundle).unwrap()).is_err());
    for v in bundle["files"].as_object_mut().unwrap().values_mut() {
        if let Some(Value::String(s)) = v.get_mut("base64") {
            use zeroize::Zeroize;
            s.zeroize();
        }
    }
}
