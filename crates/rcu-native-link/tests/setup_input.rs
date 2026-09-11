#![cfg(feature = "legacy-workflow")]
#![cfg(unix)]
//! Actual setup process, no GUI or network. File mode must not wait on stdin.
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn private_file_input_reaches_parser_without_waiting_for_stdin() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let path = root.join("bundle.json");
    fs::write(&path, b"{}").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_rcu-native-setup"))
        .args(["--validate-bundle-file", path.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if started.elapsed() > Duration::from_secs(2) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("file mode waited for stdin");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let result = child.wait_with_output().unwrap();
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("missing field `version`"));

    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let unsafe_mode = Command::new(env!("CARGO_BIN_EXE_rcu-native-setup"))
        .args(["--validate-bundle-file", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!unsafe_mode.status.success());
    assert!(
        String::from_utf8_lossy(&unsafe_mode.stderr)
            .contains("private_file_owner_link_mode_or_cap")
    );
}
