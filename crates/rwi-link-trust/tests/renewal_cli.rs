#![cfg(unix)]
use std::process::Command;

#[test]
fn operator_cli_is_explicit_create_only_and_emits_public_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap().join("pairing");
    let binary = env!("CARGO_BIN_EXE_rwi-link-renew");
    let result = Command::new(binary)
        .arg("provision")
        .arg(&root)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let data: rwi_link_trust::RenewalManifest = serde_json::from_slice(&result.stdout).unwrap();
    assert!(data.ca_private_key_persisted);
    assert!(
        data.peers
            .iter()
            .all(|peer| peer.pin.as_str().starts_with("spki:"))
    );
    let key = zeroize::Zeroizing::new(std::fs::read(root.join("ca-private-key.der")).unwrap());
    assert!(
        !result
            .stdout
            .windows(key.len())
            .any(|w| w == key.as_slice())
    );
    let unchanged = std::fs::read(root.join("renewal.json")).unwrap();
    assert!(
        !Command::new(binary)
            .arg("provision")
            .arg(&root)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(unchanged, std::fs::read(root.join("renewal.json")).unwrap());
    for action in ["renew", "status"] {
        let result = Command::new(binary)
            .arg(action)
            .arg(&root)
            .output()
            .unwrap();
        assert!(result.status.success());
        let public: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(public["renewed"], serde_json::json!([]));
        assert_eq!(public["leaves"].as_array().unwrap().len(), 3);
    }
    for args in [
        vec!["provision", "relative"],
        vec!["renew"],
        vec!["migrate", "relative", "/unused"],
        vec!["--force", "/unused"],
    ] {
        assert!(
            !Command::new(binary)
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
}
