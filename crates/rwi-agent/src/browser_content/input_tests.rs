//! Offline page authority/no-replay tests, not native browser effect evidence.
use super::*;
use rwi_core::device::browser::{ActArgs, Action, SnapshotArgs};
use std::sync::atomic::{AtomicUsize, Ordering};
struct Driver {
    actions: Arc<AtomicUsize>,
    fail: bool,
}
fn viewport() -> Value {
    json!({"pageX":0,"pageY":0,"offsetX":0,"offsetY":0,"clientWidth":960,"clientHeight":633,"scale":1})
}
impl ChildHandle for Driver {
    fn facts(&self) -> Value {
        json!({"source":"offline_fake"})
    }
    fn close(&mut self) -> Value {
        json!({"source":"offline_fake","process_exit_observed":true})
    }
    fn snapshot(&mut self, _: &SnapshotArgs) -> DeviceResult {
        DeviceResult::value(
            json!({"source":"offline_fake","document":{"frame_id":"f","loader_id":"l","url":"about:blank","backend_document_node":1},"viewport_css":viewport(),"viewport_legacy":viewport(),"nodes":[{"backend_node_id":7,"role":"button","bounds_css_document":[10,10,100,40]}]}),
        )
    }
    fn act(&mut self, _: &ActArgs, g: &input::Grounding, node: u64) -> DeviceResult {
        assert_eq!(node, 7);
        assert_eq!(g.document["backend_document_node"], 1);
        self.actions.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            error("browser_input_unknown", "offline lost ACK, do not replay")
        } else {
            DeviceResult::value(
                json!({"state":"input_sent","source":"offline_fake","goal_verified":false}),
            )
        }
    }
}
fn setup(path: &Path, input: bool, fail: bool) -> (Arc<BrowserRuntime>, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let mut p = tests::policy();
    p.principals[0].snapshot = true;
    p.principals[0].input = input;
    let mut r = tests::runtime(
        path,
        Some(p),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
    );
    let c = count.clone();
    Arc::get_mut(&mut r).unwrap().factory = Arc::new(move |_, _, _| {
        Ok(Box::new(Driver {
            actions: c.clone(),
            fail,
        }))
    });
    (r, count)
}
async fn opened(r: &Arc<BrowserRuntime>, p: &Principal) -> ActArgs {
    let created = r.clone().call(p, tests::create("new")).await;
    assert!(!created.is_error);
    let reference = created.data["session_ref"].as_str().unwrap().to_owned();
    fresh(r, p, &reference, "act").await
}
async fn fresh(r: &Arc<BrowserRuntime>, p: &Principal, s: &str, request: &str) -> ActArgs {
    let read = r
        .clone()
        .snapshot(
            p,
            SnapshotArgs {
                device_id: "mac-test".into(),
                session_ref: s.into(),
                max_nodes: 128,
            },
        )
        .await;
    assert!(!read.is_error, "{read:?}");
    assert_eq!(read.data["content_input_reference_ready"], true);
    assert_eq!(read.data["nodes"][0]["element_ref"], "e1");
    assert_eq!(read.data["input_authority"], false);
    ActArgs {
        device_id: "mac-test".into(),
        session_ref: s.into(),
        request_id: request.into(),
        observation_ref: read.data["observation_ref"].as_str().unwrap().into(),
        element_ref: "e1".into(),
        action: Action::Hover,
    }
}
fn assert_retained(receipt: &DeviceResult, original: &DeviceResult) {
    assert_eq!(receipt.data["state"], "completed");
    assert_eq!(receipt.data["dispatched_again"], false);
    assert_eq!(receipt.data["goal_verified"], false);
    assert_eq!(
        receipt.data["result"],
        serde_json::to_value(original).unwrap()
    );
}
#[tokio::test]
async fn f16_browser_content_input_default_off_and_native_permission_never_upgrades_it() {
    let mut v = serde_json::to_value(tests::policy()).unwrap();
    v["principals"][0].as_object_mut().unwrap().remove("input");
    let policy: Policy = serde_json::from_value(v).unwrap();
    assert!(!policy.principals[0].input);
    let t = tempfile::tempdir().unwrap();
    let (r, n) = setup(t.path(), false, false);
    let p = tests::principal(true);
    assert!(!r.input_capability(&p).authorized);
    let a = opened(&r, &p).await;
    assert_eq!(
        r.act(&p, a).await.data["code"],
        "browser_input_permission_denied"
    );
    assert_eq!(n.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn f16_browser_content_input_durable_duplicate_conflict_close_and_restart_never_replay() {
    let t = tempfile::tempdir().unwrap();
    let (r, n) = setup(t.path(), true, false);
    let p = tests::principal(false);
    assert!(r.input_capability(&p).authorized);
    let a = opened(&r, &p).await;
    let first = r.clone().act(&p, a.clone()).await;
    assert!(!first.is_error);
    assert_eq!(first.data["goal_verified"], false);
    assert_retained(&r.clone().act(&p, a.clone()).await, &first);
    assert_eq!(n.load(Ordering::SeqCst), 1);
    let mut conflict = a.clone();
    conflict.action = Action::Click;
    assert!(r.clone().act(&p, conflict).await.is_error);
    assert_eq!(n.load(Ordering::SeqCst), 1);
    let mut stale = a.clone();
    stale.request_id = "new-id-old-observation".into();
    assert_eq!(
        r.clone().act(&p, stale).await.data["code"],
        "browser_observation_unknown_or_expired"
    );
    let q = r
        .clone()
        .call(
            &p,
            SessionArgs::Query {
                device_id: "mac-test".into(),
                request_id: a.request_id.clone(),
            },
        )
        .await;
    assert!(!q.is_error);
    assert_eq!(q.data["result"], serde_json::to_value(&first).unwrap());
    r.clone()
        .call(
            &p,
            SessionArgs::Close {
                device_id: "mac-test".into(),
                request_id: "close".into(),
                session_ref: a.session_ref.clone(),
            },
        )
        .await;
    assert_retained(&r.clone().act(&p, a.clone()).await, &first);
    assert_eq!(n.load(Ordering::SeqCst), 1);
    drop(r);
    let (r, n) = setup(t.path(), true, false);
    assert_retained(&r.act(&p, a).await, &first);
    assert_eq!(n.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn f16_browser_content_input_unknown_receipt_and_unfinished_reservation_are_not_reexecuted() {
    let t = tempfile::tempdir().unwrap();
    let (r, n) = setup(t.path(), true, true);
    let p = tests::principal(false);
    let a = opened(&r, &p).await;
    let first = r.clone().act(&p, a.clone()).await;
    assert!(first.is_error);
    assert_retained(&r.clone().act(&p, a.clone()).await, &first);
    assert_eq!(n.load(Ordering::SeqCst), 1);
    let pending = fresh(&r, &p, &a.session_ref, "reserved-no-result").await;
    assert!(matches!(
        r.reserve(
            p.certificate.as_str(),
            &pending.request_id,
            &serde_json::to_value(&pending).unwrap(),
            "reserved"
        ),
        Ok(Reservation::New(_))
    ));
    drop(r);
    let (r, n) = setup(t.path(), true, false);
    let retained = r.act(&p, pending).await;
    assert_eq!(retained.data["state"], "reserved_or_effect_unknown");
    assert_eq!(retained.data["dispatched_again"], false);
    assert!(retained.data["result"].is_null());
    assert_eq!(n.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn f16_browser_content_input_owner_alias_stale_reference_and_expiry_do_not_retarget() {
    let t = tempfile::tempdir().unwrap();
    let (mut r, n) = setup(t.path(), true, false);
    let p = tests::principal(false);
    let mut other = p.clone();
    other.certificate = rwi_link_trust::PeerPin::parse(&"b".repeat(64)).unwrap();
    other.journal_identity = Some(p.certificate.clone());
    Arc::get_mut(&mut r)
        .unwrap()
        .policy
        .as_mut()
        .unwrap()
        .principals
        .push(policy::Grant {
            certificate: other.certificate.clone(),
            observe: true,
            create: false,
            close: false,
            snapshot: true,
            input: true,
        });
    let a = opened(&r, &p).await;
    assert_eq!(
        r.clone().act(&other, a.clone()).await.data["code"],
        "browser_session_unknown"
    );
    let mut wrong = a.clone();
    wrong.request_id = "bad-ref".into();
    wrong.observation_ref = "browser-observation:forged".into();
    assert_eq!(
        r.clone().act(&p, wrong).await.data["code"],
        "browser_observation_unknown_or_expired"
    );
    let s = r
        .sessions
        .lock()
        .unwrap()
        .get(&a.session_ref)
        .unwrap()
        .clone();
    s.inner
        .lock()
        .unwrap()
        .observation
        .as_mut()
        .unwrap()
        .expires = Instant::now() - Duration::from_millis(1);
    assert_eq!(
        r.clone().act(&p, a).await.data["code"],
        "browser_observation_unknown_or_expired"
    );
    assert_eq!(n.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn f16_browser_content_input_work_budget_and_same_page_lock_leave_queries_independent() {
    let t = tempfile::tempdir().unwrap();
    let (r, n) = setup(t.path(), true, false);
    let p = tests::principal(false);
    let a = opened(&r, &p).await;
    let permits = r.workers.clone().try_acquire_many_owned(4).unwrap();
    assert_eq!(
        r.clone().act(&p, a.clone()).await.data["code"],
        "browser_busy_not_dispatched"
    );
    assert!(
        !r.clone()
            .call(
                &p,
                SessionArgs::Query {
                    device_id: "mac-test".into(),
                    request_id: "new".into()
                }
            )
            .await
            .is_error
    );
    drop(permits);
    let s = r
        .sessions
        .lock()
        .unwrap()
        .get(&a.session_ref)
        .unwrap()
        .clone();
    let (ready, rx) = std::sync::mpsc::channel();
    let (release, done) = std::sync::mpsc::channel();
    let h = std::thread::spawn(move || {
        let _guard = s.inner.lock().unwrap();
        ready.send(()).unwrap();
        done.recv().unwrap();
    });
    rx.recv().unwrap();
    let result = r.clone().act(&p, a).await;
    release.send(()).unwrap();
    h.join().unwrap();
    assert_eq!(result.data["code"], "browser_session_busy_not_dispatched");
    assert_eq!(n.load(Ordering::SeqCst), 0);
}
