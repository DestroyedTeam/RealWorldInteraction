use super::*;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::ReadBuf;

// Rust-only owned child entry. It returns normally in the parent test suite;
// subprocess cases select it by exact test name and a fixture-only env key.
#[test]
fn owned_child_fixture_entry() {
    let Ok(mode) = std::env::var("RCU_LIFETIME_CHILD_MODE") else {
        return;
    };
    if let Some(marker) = std::env::var_os("RCU_LIFETIME_CHILD_MARKER") {
        std::fs::write(marker, b"owned-child-started").unwrap();
    }
    if mode == "sleep" {
        std::thread::sleep(Duration::from_secs(30));
    } else {
        eprintln!("owned Rust fixture diagnostic");
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn command(mode: &str) -> Command {
    let fixture = format!(
        "{}::owned_child_fixture_entry",
        module_path!().split_once("::").unwrap().1
    );
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .env("RCU_LIFETIME_CHILD_MODE", mode)
        .args(["--exact", &fixture, "--nocapture"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    command
}
async fn child(mode: &str) -> (Registry, Arc<OwnedWorker>) {
    let registry = Registry::default();
    let owned = spawn_registered(
        &mut command(mode),
        &registry,
        json!({"test":"owned rust child"}),
    )
    .unwrap();
    let stderr = owned.child.lock().await.stderr.take().unwrap();
    owned.start_stderr(stderr).unwrap();
    (registry, owned)
}

#[tokio::test]
async fn poisoned_or_busy_registry_refuses_before_process_spawn() {
    let directory = tempfile::tempdir().unwrap();
    let marker = directory.path().join("must-not-exist");
    let registry = Registry::default();
    let mut candidate = command("exit");
    candidate.env("RCU_LIFETIME_CHILD_MARKER", &marker);
    {
        let _held = registry.lock().unwrap();
        assert_eq!(
            spawn_registered(&mut candidate, &registry, json!({}))
                .err()
                .expect("busy registry must fail before spawn")
                .to_string(),
            "owned_registry_busy_before_spawn"
        );
    }
    let poisoned = registry.clone();
    let _ = std::panic::catch_unwind(move || {
        let _held = poisoned.lock().unwrap();
        panic!("offline intentional registry poison");
    });
    assert_eq!(
        spawn_registered(&mut candidate, &registry, json!({}))
            .err()
            .expect("poisoned registry must fail before spawn")
            .to_string(),
        "owned_registry_poisoned_before_spawn"
    );
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(!marker.exists());
    assert!(
        registry
            .lock()
            .err()
            .expect("registry was intentionally poisoned")
            .into_inner()
            .is_empty()
    );
}

#[tokio::test]
async fn registered_child_remains_cleanup_owned_after_setup_failure() {
    let (registry, owned) = child("exit").await;
    let result =
        complete_setup::<()>(&owned, Err(crate::invalid("injected_post_spawn_failure"))).await;
    assert_eq!(
        result.unwrap_err().to_string(),
        "injected_post_spawn_failure"
    );
    assert_eq!(registry.lock().unwrap().len(), 1);
    assert!(!owned.still_owned_alive().await.unwrap());
    let record = owned.record.lock().unwrap();
    assert_eq!(record["setup_failure"]["cleanup_observed"], true);
    assert_eq!(record["setup_failure"]["stderr"]["joined"], true);
    assert_eq!(record["setup_failure"]["stderr"]["eof_observed"], true);
}

#[tokio::test]
async fn liveness_and_stop_child_locks_are_bounded() {
    let (_registry, owned) = child("sleep").await;
    let lock = owned.child.lock().await;
    let started = tokio::time::Instant::now();
    assert_eq!(
        owned.still_owned_alive().await.unwrap_err().to_string(),
        "owned_child_liveness_lock_timeout"
    );
    assert!(started.elapsed() < Duration::from_secs(2));
    let exit = owned
        .stop_with_limits(
            Duration::from_millis(120),
            Duration::from_millis(80),
            Duration::from_millis(40),
        )
        .await;
    assert!(!exit.observed);
    assert_eq!(
        exit.error.as_deref(),
        Some("owned_child_lock_cleanup_timeout")
    );
    assert!(exit.elapsed_ms < 1000);
    drop(lock);
    let final_exit = owned
        .stop_with_limits(
            Duration::from_secs(2),
            Duration::from_millis(10),
            Duration::from_millis(200),
        )
        .await;
    assert!(final_exit.observed);
    assert!(final_exit.forced);
    assert!(!owned.still_owned_alive().await.unwrap());
}

#[tokio::test]
async fn late_lock_release_does_not_restart_full_grace_period() {
    let (_registry, owned) = child("sleep").await;
    let other = owned.clone();
    let (ready, received) = tokio::sync::oneshot::channel();
    let hold = tokio::spawn(async move {
        let lock = other.child.lock().await;
        ready.send(()).unwrap();
        tokio::time::sleep(Duration::from_millis(160)).await;
        drop(lock);
    });
    received.await.unwrap();
    let exit = owned
        .stop_with_limits(
            Duration::from_millis(240),
            Duration::from_millis(220),
            Duration::from_millis(30),
        )
        .await;
    assert!(exit.forced);
    assert!(
        exit.elapsed_ms < 400,
        "absolute deadline was restarted: {exit:?}"
    );
    tokio::time::timeout(Duration::from_secs(1), hold)
        .await
        .unwrap()
        .unwrap();
    let later = owned
        .stop_with_limits(
            Duration::from_secs(2),
            Duration::from_millis(10),
            Duration::from_millis(200),
        )
        .await;
    assert!(later.observed);
    assert!(
        later.forced,
        "later cleanup must not erase an earlier force request"
    );
}

struct PendingReader(Arc<AtomicBool>);
impl AsyncRead for PendingReader {
    fn poll_read(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Poll::Pending
    }
}
impl Drop for PendingReader {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
#[tokio::test]
async fn stderr_timeout_aborts_and_joins_instead_of_detaching() {
    let registry = Registry::default();
    let owned = spawn_registered(&mut command("exit"), &registry, json!({})).unwrap();
    drop(owned.child.lock().await.stderr.take());
    let closed = Arc::new(AtomicBool::new(false));
    owned.start_stderr(PendingReader(closed.clone())).unwrap();
    let exit = owned
        .stop_with_limits(
            Duration::from_millis(600),
            Duration::from_millis(300),
            Duration::from_millis(200),
        )
        .await;
    assert!(exit.observed);
    assert!(exit.stderr.abort_requested);
    assert!(exit.stderr.joined);
    assert!(!exit.stderr.clean());
    assert!(closed.load(Ordering::Acquire));
    assert!(owned.stderr.lock().await.task.is_none());
}

#[tokio::test]
async fn stderr_cap_and_join_failure_are_not_success() {
    let bytes = vec![b'x'; STDERR_CAP + 10];
    let capture = capture_stderr(bytes.as_slice()).await;
    assert!(capture.capped);
    assert_eq!(capture.bytes.len(), STDERR_CAP);
    let capture = capture_stderr(&b"normal"[..]).await;
    assert!(!capture.capped);
    assert!(capture.error.is_none());
    let registry = Registry::default();
    let owned = spawn_registered(&mut command("exit"), &registry, json!({})).unwrap();
    drop(owned.child.lock().await.stderr.take());
    owned.stderr.lock().await.task = Some(tokio::spawn(async {
        panic!("offline stderr collector panic");
        #[allow(unreachable_code)]
        StderrCapture {
            bytes: vec![],
            capped: false,
            error: None,
        }
    }));
    let exit = owned
        .stop_with_limits(
            Duration::from_secs(2),
            Duration::from_millis(500),
            Duration::from_millis(300),
        )
        .await;
    assert!(exit.observed);
    assert!(exit.stderr.joined);
    assert!(!exit.stderr.clean());
    assert!(
        exit.stderr
            .error
            .unwrap()
            .contains("owned_stderr_join_error")
    );
}

#[test]
fn bootstrap_lifecycle_grant_is_metadata_stage_not_model_input() {
    for stage in [
        Stage::Bootstrap,
        Stage::Preflight,
        Stage::Launch,
        Stage::Activate,
    ] {
        assert!(lifecycle_metadata_stage(stage));
    }
    for stage in [Stage::Prepare, Stage::Model, Stage::Oracle, Stage::Cleanup] {
        assert!(!lifecycle_metadata_stage(stage));
    }
    assert_eq!(STOP_BUDGET, Duration::from_secs(8));
}

#[tokio::test]
async fn raw_response_is_preserved_before_ledger_rejection_and_audit_failures_hold() {
    let (_registry, owned) = child("exit").await;
    let temp = tempfile::tempdir().unwrap();
    let directory =
        Directory::create(&temp.path().canonicalize().unwrap().join("responses")).unwrap();
    let raw = b"{ \"jsonrpc\":\"2.0\", \"id\":1, \"result\":{\"wrong_schema\":true} }\r\n";
    let name = format!("wire-{}.ndjson", crate::sha(raw));
    owned.audit_response(directory.path(), raw).unwrap();
    let mut ledger = Ledger::new("fixture", json!({}), false, "request-", "expected");
    ledger.request(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"owned-audit-test","version":"1"}}})).unwrap();
    assert_eq!(
        ledger
            .reply(&serde_json::from_slice(raw).unwrap(), raw)
            .expect_err("mismatched worker version must reject")
            .to_string(),
        "worker_version_or_instructions_mismatch"
    );
    assert_eq!(directory.read(&name, frame::OUTBOUND).unwrap(), raw);
    owned.audit_response(directory.path(), raw).unwrap();
    assert_eq!(
        owned.record.lock().unwrap()["last_wire_response_sha256"],
        crate::sha(raw)
    );
    assert!(owned.wire_recovery.lock().unwrap().is_none());

    let bad = Directory::create(&temp.path().canonicalize().unwrap().join("collision")).unwrap();
    bad.create_new(&name, b"not the same bytes", frame::OUTBOUND)
        .unwrap();
    assert!(owned.audit_response(bad.path(), raw).is_err());
    assert_eq!(
        owned.wire_recovery.lock().unwrap().as_deref(),
        Some(raw.as_slice())
    );
    assert_eq!(
        bad.read(&name, frame::OUTBOUND).unwrap(),
        b"not the same bytes"
    );
    let exit = owned.stop().await;
    assert!(exit.observed);
}

#[tokio::test]
async fn non_json_complete_worker_frame_is_audited_before_parse_rejection() {
    let (_registry, owned) = child("exit").await;
    let temp = tempfile::tempdir().unwrap();
    let directory =
        Directory::create(&temp.path().canonicalize().unwrap().join("responses")).unwrap();
    let raw: &'static [u8] = b"native diagnostic is not JSON\n";
    let now = crate::budget::now_ms();
    let mut worker = Worker {
        input: Box::new(tokio::io::sink()),
        output: BufReader::new(Box::new(raw) as Read),
        ledger: Ledger::new("fixture", json!({}), false, "request-", "expected"),
        owned: owned.clone(),
        budget: Budget::start(crate::budget::BudgetSpec {
            issued_unix_ms: now,
            expires_unix_ms: now + 10000,
            maximum_elapsed_ms: 10000,
            clock_drift_tolerance_ms: 1000,
        })
        .unwrap(),
        audit_root: Some(directory.path().into()),
        model_stage: false,
        ready_written: false,
        #[cfg(windows)]
        ready_binding: None,
    };
    assert!(
        worker
            .rpc(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"owned-invalid-raw-test","version":"1"}}}))
            .await
            .is_err()
    );
    assert_eq!(
        directory
            .read(&format!("wire-{}.ndjson", crate::sha(raw)), frame::OUTBOUND)
            .unwrap(),
        raw
    );
    assert_eq!(
        owned.record.lock().unwrap()["last_wire_response_sha256"],
        crate::sha(raw)
    );
    assert!(worker.ledger.response_digests.is_empty());
    assert!(!worker.ledger.proof().requests_terminal);
    drop(worker);
    assert!(owned.stop().await.observed);
}
