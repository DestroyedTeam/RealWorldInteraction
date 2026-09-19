#![cfg(unix)]
use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn invoke(args: &[&std::ffi::OsStr]) -> (bool, Vec<u8>, Vec<u8>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rwi-link-provision"))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("own provisioning binary");
    let until = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("owned child status") {
            break status;
        }
        if Instant::now() >= until {
            let kill = child.kill();
            let reap_until = Instant::now() + Duration::from_secs(2);
            let mut reaped = false;
            while Instant::now() < reap_until {
                if child.try_wait().is_ok_and(|s| s.is_some()) {
                    reaped = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!(
                "provisioning deadline; kill_accepted={}, reaped={reaped}",
                kill.is_ok()
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .stdout
        .take()
        .unwrap()
        .take(16_384)
        .read_to_end(&mut stdout)
        .unwrap();
    child
        .stderr
        .take()
        .unwrap()
        .take(16_384)
        .read_to_end(&mut stderr)
        .unwrap();
    (status.success(), stdout, stderr)
}

#[test]
fn actual_provision_cli_emits_only_public_metadata_and_refuses_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp
        .path()
        .canonicalize()
        .unwrap()
        .join("new-private-profiles");
    let args = [std::ffi::OsStr::new("--profile-dir"), root.as_os_str()];
    let (success, stdout, stderr) = invoke(&args);
    assert!(
        success,
        "provisioning failed: {}",
        String::from_utf8_lossy(&stderr)
    );
    assert!(stderr.is_empty());
    let public: rwi_link_trust::ProvisionManifest = serde_json::from_slice(&stdout).unwrap();
    assert_eq!(public.schema, "rcu.private_pairing_provision.v1");
    assert!(!public.ca_private_key_persisted);
    for role in ["agent", "controller", "model"] {
        let paths = rwi_link_trust::IdentityPaths::in_profile(&root.join(role));
        let identity = paths.load().unwrap();
        assert_eq!(identity.ca_fingerprint(), public.ca_sha256);
        let key = zeroize::Zeroizing::new(std::fs::read(paths.private_key).unwrap());
        assert!(
            !stdout
                .windows(key.len())
                .any(|window| window == key.as_slice())
        );
    }
    let manifest_before = std::fs::read(root.join("public-manifest.json")).unwrap();
    let (again, _, _) = invoke(&args);
    assert!(!again);
    assert_eq!(
        manifest_before,
        std::fs::read(root.join("public-manifest.json")).unwrap()
    );
    let (bad, _, _) = invoke(&[
        std::ffi::OsStr::new("--profile-dir"),
        Path::new("relative").as_os_str(),
    ]);
    assert!(!bad);
}
