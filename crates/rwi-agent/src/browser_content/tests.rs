//! Offline driver tests: no Chrome process, GUI, Native backend or network.
use super::*;
use policy::Grant;
use rwi_link_trust::PeerPin;
use std::sync::atomic::{AtomicUsize, Ordering};
struct Fake {
    closes: Arc<AtomicUsize>,
    closed: bool,
}
impl ChildHandle for Fake {
    fn facts(&self) -> Value {
        json!({"source":"offline_fake_not_native_evidence"})
    }
    fn close(&mut self) -> Value {
        if !self.closed {
            self.closes.fetch_add(1, Ordering::SeqCst);
            self.closed = true;
        }
        json!({"process_exit_observed":true,"source":"offline_fake"})
    }
}
pub(super) fn principal(native: bool) -> Principal {
    Principal {
        certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
        journal_identity: None,
        observe: native,
        input: native,
    }
}
pub(super) fn policy() -> Policy {
    Policy {
        version: 1,
        user_uid: 501,
        chrome_executable: "/Applications/Owned Chrome".into(),
        chrome_sha256: "c".repeat(64),
        initial_urls: vec!["about:blank".into()],
        principals: vec![Grant {
            certificate: principal(false).certificate,
            observe: true,
            create: true,
            close: true,
            snapshot: false,
            input: false,
        }],
    }
}
pub(super) fn runtime(
    path: &Path,
    p: Option<Policy>,
    starts: Arc<AtomicUsize>,
    closes: Arc<AtomicUsize>,
) -> Arc<BrowserRuntime> {
    Arc::new(BrowserRuntime {
        device: "mac-test".into(),
        instance: uuid::Uuid::new_v4().to_string(),
        root: path.into(),
        policy: p.ok_or_else(|| "absent".into()),
        ledger: Mutex::new(Ledger::open(path.join("browser-operations"))),
        sessions: Mutex::new(BTreeMap::new()),
        workers: Arc::new(tokio::sync::Semaphore::new(4)),
        control_workers: Arc::new(tokio::sync::Semaphore::new(2)),
        factory: Arc::new(move |_, _, _| {
            starts.fetch_add(1, Ordering::SeqCst);
            Ok(Box::new(Fake {
                closes: closes.clone(),
                closed: false,
            }))
        }),
    })
}
pub(super) fn create(request: &str) -> SessionArgs {
    SessionArgs::Create {
        device_id: "mac-test".into(),
        request_id: request.into(),
        url: "about:blank".into(),
    }
}
#[tokio::test]
async fn f14_browser_permission_is_separate_from_native_and_default_off() {
    let t = tempfile::tempdir().unwrap();
    let starts = Arc::new(AtomicUsize::new(0));
    let closes = Arc::new(AtomicUsize::new(0));
    let off = runtime(&t.path().join("off"), None, starts.clone(), closes.clone());
    assert!(off.call(&principal(true), create("off")).await.is_error);
    let r = runtime(&t.path().join("on"), Some(policy()), starts.clone(), closes);
    let success = r.clone().call(&principal(false), create("yes")).await;
    assert!(!success.is_error, "{success:?}");
    assert_eq!(success.data["native_binding_required"], false);
    let mut other = principal(true);
    other.certificate = PeerPin::parse(&"b".repeat(64)).unwrap();
    assert!(r.call(&other, create("no")).await.is_error);
    assert_eq!(starts.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn f14_browser_same_request_restart_conflict_and_query_never_replay() {
    let t = tempfile::tempdir().unwrap();
    let starts = Arc::new(AtomicUsize::new(0));
    let closes = Arc::new(AtomicUsize::new(0));
    let r = runtime(t.path(), Some(policy()), starts.clone(), closes.clone());
    let p = principal(false);
    let first = r.clone().call(&p, create("one")).await;
    assert!(!first.is_error);
    let same = r.clone().call(&p, create("one")).await;
    assert_eq!(same.data["dispatched_again"], false);
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    let changed = SessionArgs::Close {
        device_id: "mac-test".into(),
        request_id: "one".into(),
        session_ref: first.data["session_ref"].as_str().unwrap().into(),
    };
    assert_eq!(
        r.clone().call(&p, changed).await.data["code"],
        "operation_payload_conflict"
    );
    let query = r
        .clone()
        .call(
            &p,
            SessionArgs::Query {
                device_id: "mac-test".into(),
                request_id: "one".into(),
            },
        )
        .await;
    assert_eq!(
        query.data["result"]["data"]["session_ref"],
        first.data["session_ref"]
    );
    drop(r);
    let restarted = runtime(t.path(), Some(policy()), starts.clone(), closes);
    let same = restarted.clone().call(&p, create("one")).await;
    assert_eq!(same.data["dispatched_again"], false);
    assert_eq!(starts.load(Ordering::SeqCst), 1);
    let old = SessionArgs::Close {
        device_id: "mac-test".into(),
        request_id: "close-old".into(),
        session_ref: first.data["session_ref"].as_str().unwrap().into(),
    };
    assert_eq!(
        restarted.call(&p, old).await.data["code"],
        "browser_session_unknown"
    );
}
#[tokio::test]
async fn f14_browser_close_invalidates_exact_ref_no_second_close_and_wrong_peer() {
    let t = tempfile::tempdir().unwrap();
    let starts = Arc::new(AtomicUsize::new(0));
    let closes = Arc::new(AtomicUsize::new(0));
    let r = runtime(t.path(), Some(policy()), starts, closes.clone());
    let p = principal(false);
    let a = r.clone().call(&p, create("open")).await;
    let reference = a.data["session_ref"].as_str().unwrap();
    let close = SessionArgs::Close {
        device_id: "mac-test".into(),
        request_id: "close".into(),
        session_ref: reference.into(),
    };
    let done = r.clone().call(&p, close.clone()).await;
    assert_eq!(done.data["references_invalidated"], true);
    let duplicate = r.clone().call(&p, close).await;
    assert_eq!(duplicate.data["dispatched_again"], false);
    let again = SessionArgs::Close {
        device_id: "mac-test".into(),
        request_id: "close-again".into(),
        session_ref: reference.into(),
    };
    assert_eq!(
        r.clone().call(&p, again).await.data["code"],
        "browser_session_unknown"
    );
    assert_eq!(closes.load(Ordering::SeqCst), 1);
    let mut other = p;
    other.certificate = PeerPin::parse(&"b".repeat(64)).unwrap();
    assert!(
        r.call(
            &other,
            SessionArgs::Query {
                device_id: "mac-test".into(),
                request_id: "open".into()
            }
        )
        .await
        .is_error
    );
}
#[tokio::test]
async fn f14_browser_reserved_unknown_durable_id_is_not_dispatched_after_restart() {
    let t = tempfile::tempdir().unwrap();
    let starts = Arc::new(AtomicUsize::new(0));
    let closes = Arc::new(AtomicUsize::new(0));
    let r = runtime(t.path(), Some(policy()), starts.clone(), closes.clone());
    let p = principal(false);
    let args = create("lost");
    r.reserve(
        p.certificate.as_str(),
        args.request_id(),
        &serde_json::to_value(&args).unwrap(),
        "nonce-one",
    )
    .unwrap();
    drop(r);
    let r = runtime(t.path(), Some(policy()), starts.clone(), closes);
    let result = r.call(&p, args).await;
    assert_eq!(result.data["state"], "reserved_or_effect_unknown");
    assert_eq!(result.data["dispatched_again"], false);
    assert_eq!(starts.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn f14_browser_capacity_origin_and_device_do_not_spawn_extra_children() {
    let t = tempfile::tempdir().unwrap();
    let starts = Arc::new(AtomicUsize::new(0));
    let closes = Arc::new(AtomicUsize::new(0));
    let r = runtime(t.path(), Some(policy()), starts.clone(), closes);
    let p = principal(false);
    for id in ["one", "two"] {
        assert!(!r.clone().call(&p, create(id)).await.is_error);
    }
    assert_eq!(
        r.clone().call(&p, create("three")).await.data["code"],
        "browser_session_capacity"
    );
    assert_eq!(
        r.clone()
            .call(
                &p,
                SessionArgs::Create {
                    device_id: "mac-test".into(),
                    request_id: "badurl".into(),
                    url: "https://not-granted.example".into()
                }
            )
            .await
            .data["code"],
        "browser_permission_denied"
    );
    assert_eq!(
        r.call(
            &p,
            SessionArgs::Create {
                device_id: "other".into(),
                request_id: "cross".into(),
                url: "about:blank".into()
            }
        )
        .await
        .data["code"],
        "browser_device_mismatch"
    );
    assert_eq!(starts.load(Ordering::SeqCst), 2);
}
#[test]
fn f14_browser_native_journal_is_not_a_browser_query_source() {
    let t = tempfile::tempdir().unwrap();
    let mut native = Ledger::open(t.path().join("native")).unwrap();
    native
        .reserve_resource("owner", "one", &json!({}), "nonce", "native-resource")
        .unwrap();
    let browser = Ledger::open(t.path().join("browser")).unwrap();
    assert!(browser.request_status("owner", "one").is_none());
}

#[tokio::test]
async fn f14_browser_two_granted_principals_cannot_close_each_others_session() {
    let t = tempfile::tempdir().unwrap();
    let starts = Arc::new(AtomicUsize::new(0));
    let closes = Arc::new(AtomicUsize::new(0));
    let p = principal(false);
    let mut other = p.clone();
    other.certificate = PeerPin::parse(&"b".repeat(64)).unwrap();
    // A Native journal continuity alias is not browser authority.
    other.journal_identity = Some(p.certificate.clone());
    let mut grants = policy();
    assert!(grants.grant(&other).is_none());
    grants.principals.push(Grant {
        certificate: other.certificate.clone(),
        observe: true,
        create: true,
        close: true,
        snapshot: false,
        input: false,
    });
    let r = runtime(t.path(), Some(grants), starts, closes.clone());
    let a = r.clone().call(&p, create("open-a")).await;
    let close = SessionArgs::Close {
        device_id: "mac-test".into(),
        request_id: "close-foreign".into(),
        session_ref: a.data["session_ref"].as_str().unwrap().into(),
    };
    assert_eq!(
        r.clone().call(&other, close).await.data["code"],
        "browser_session_unknown"
    );
    assert_eq!(closes.load(Ordering::SeqCst), 0);
    assert!(
        r.call(
            &other,
            SessionArgs::Query {
                device_id: "mac-test".into(),
                request_id: "open-a".into()
            }
        )
        .await
        .is_error
    );
}

#[tokio::test]
async fn f14_browser_close_query_keep_capacity_when_creation_workers_are_busy() {
    let t = tempfile::tempdir().unwrap();
    let r = runtime(
        t.path(),
        Some(policy()),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
    );
    let p = principal(false);
    let a = r.clone().call(&p, create("open")).await;
    let _busy = r.workers.clone().try_acquire_many_owned(4).unwrap();
    let q = r
        .clone()
        .call(
            &p,
            SessionArgs::Query {
                device_id: "mac-test".into(),
                request_id: "open".into(),
            },
        )
        .await;
    assert!(!q.is_error);
    let c = r
        .call(
            &p,
            SessionArgs::Close {
                device_id: "mac-test".into(),
                request_id: "close".into(),
                session_ref: a.data["session_ref"].as_str().unwrap().into(),
            },
        )
        .await;
    assert_eq!(c.data["references_invalidated"], true);
}

#[tokio::test]
async fn f14_browser_unknown_startup_cleanup_retains_capacity_and_never_replays() {
    struct Pending;
    impl ChildHandle for Pending {
        fn facts(&self) -> Value {
            json!({"source":"offline_unknown_cleanup"})
        }
        fn close(&mut self) -> Value {
            json!({"process_exit_observed":false,"os_exit":"unknown"})
        }
    }
    let t = tempfile::tempdir().unwrap();
    let starts = Arc::new(AtomicUsize::new(0));
    let mut r = runtime(
        t.path(),
        Some(policy()),
        starts.clone(),
        Arc::new(AtomicUsize::new(0)),
    );
    let n = starts.clone();
    Arc::get_mut(&mut r).unwrap().factory = Arc::new(move |_, _, _| {
        n.fetch_add(1, Ordering::SeqCst);
        Err(StartFailure {
            detail: "injected failure AFTER owned startup, cleanup unknown".into(),
            retained: Some(Box::new(Pending)),
        })
    });
    let p = principal(false);
    let first = r.clone().call(&p, create("one")).await;
    assert!(first.is_error);
    assert_eq!(first.data["owned_cleanup_pending"], true);
    let duplicate = r.clone().call(&p, create("one")).await;
    assert_eq!(duplicate.data["dispatched_again"], false);
    assert!(r.clone().call(&p, create("two")).await.is_error);
    assert_eq!(
        r.clone().call(&p, create("three")).await.data["code"],
        "browser_session_capacity"
    );
    assert_eq!(r.sessions.lock().unwrap().len(), 2);
    assert_eq!(starts.load(Ordering::SeqCst), 2);
}
