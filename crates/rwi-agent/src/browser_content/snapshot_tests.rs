//! Offline authority/resource tests; these fakes prove no GUI or page effect.
use super::*;
use rwi_core::device::{DeviceImage, browser::SnapshotArgs};
use std::sync::atomic::{AtomicUsize, Ordering};
struct Reader(Arc<AtomicUsize>);
impl ChildHandle for Reader {
    fn facts(&self) -> Value {
        json!({"source":"offline_content_fake"})
    }
    fn close(&mut self) -> Value {
        json!({"process_exit_observed":true,"source":"offline_fake"})
    }
    fn snapshot(&mut self, _: &SnapshotArgs) -> DeviceResult {
        self.0.fetch_add(1, Ordering::SeqCst);
        let mut r = DeviceResult::value(
            json!({"source":"offline_content_fake","nodes":[{"role":"button","name":"test"}],"goal_verified":false}),
        );
        r.images.push(DeviceImage {
            mime: "image/png".into(),
            base64: "offline_fake_not_image_evidence".into(),
        });
        r
    }
}
fn setup(path: &Path, snapshot: bool) -> (Arc<BrowserRuntime>, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let mut p = tests::policy();
    p.principals[0].snapshot = snapshot;
    let mut r = tests::runtime(
        path,
        Some(p),
        Arc::new(AtomicUsize::new(0)),
        Arc::new(AtomicUsize::new(0)),
    );
    let c = count.clone();
    Arc::get_mut(&mut r).unwrap().factory =
        Arc::new(move |_, _, _| Ok(Box::new(Reader(c.clone()))));
    (r, count)
}
fn args(reference: &str) -> SnapshotArgs {
    SnapshotArgs {
        device_id: "mac-test".into(),
        session_ref: reference.into(),
        max_nodes: 128,
    }
}
#[test]
fn f15_browser_content_snapshot_old_grant_defaults_off_and_capability_is_distinct() {
    let mut v = serde_json::to_value(tests::policy()).unwrap();
    assert_eq!(v["principals"][0]["snapshot"], false);
    v["principals"][0]
        .as_object_mut()
        .unwrap()
        .remove("snapshot");
    let p: Policy = serde_json::from_value(v).unwrap();
    assert!(!p.principals[0].snapshot);
    let t = tempfile::tempdir().unwrap();
    let (r, _) = setup(t.path(), false);
    assert!(!r.snapshot_capability(&tests::principal(true)).authorized);
    assert!(r.capability(&tests::principal(false)).authorized);
}
#[tokio::test]
async fn f15_browser_content_snapshot_explicit_grant_without_native_permission_keeps_images() {
    let t = tempfile::tempdir().unwrap();
    let (r, n) = setup(t.path(), true);
    let p = tests::principal(false);
    let a = r.clone().call(&p, tests::create("new")).await;
    let reference = a.data["session_ref"].as_str().unwrap();
    let read = r.clone().snapshot(&p, args(reference)).await;
    assert!(!read.is_error, "{read:?}");
    assert_eq!(read.images.len(), 1);
    assert_eq!(read.data["input_authority"], false);
    assert_eq!(read.data["session_ref"], reference);
    assert_eq!(n.load(Ordering::SeqCst), 1);
    assert!(
        read.data["observation_ref"]
            .as_str()
            .unwrap()
            .starts_with("browser-observation:")
    );
    let mut wrong = args(reference);
    wrong.device_id = "other".into();
    assert_eq!(
        r.snapshot(&p, wrong).await.data["code"],
        "browser_device_mismatch"
    );
    assert_eq!(n.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn f15_browser_content_snapshot_lifecycle_grant_never_reads_content() {
    let t = tempfile::tempdir().unwrap();
    let (r, n) = setup(t.path(), false);
    let p = tests::principal(true);
    let a = r.clone().call(&p, tests::create("new")).await;
    let read = r
        .snapshot(&p, args(a.data["session_ref"].as_str().unwrap()))
        .await;
    assert_eq!(read.data["code"], "browser_snapshot_permission_denied");
    assert!(read.images.is_empty());
    assert_eq!(n.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn f15_browser_content_snapshot_exact_owner_closed_expired_and_busy_do_not_retarget() {
    let t = tempfile::tempdir().unwrap();
    let (mut r, n) = setup(t.path(), true);
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
            input: false,
        });
    let a = r.clone().call(&p, tests::create("new")).await;
    let reference = a.data["session_ref"].as_str().unwrap();
    assert_eq!(
        r.clone().snapshot(&other, args(reference)).await.data["code"],
        "browser_session_unknown"
    );
    let s = r.sessions.lock().unwrap().get(reference).unwrap().clone();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let locked = s.clone();
    let holder = std::thread::spawn(move || {
        let _guard = locked.inner.lock().unwrap();
        ready_tx.send(()).unwrap();
        release_rx.recv().unwrap();
    });
    ready_rx.recv().unwrap();
    let read = r.clone().snapshot(&p, args(reference)).await;
    release_tx.send(()).unwrap();
    holder.join().unwrap();
    assert_eq!(read.data["code"], "browser_session_busy_not_dispatched");
    r.clone()
        .call(
            &p,
            SessionArgs::Close {
                device_id: "mac-test".into(),
                request_id: "close".into(),
                session_ref: reference.into(),
            },
        )
        .await;
    assert_eq!(
        r.clone().snapshot(&p, args(reference)).await.data["code"],
        "browser_session_unknown"
    );
    drop(s);
    let a = r.clone().call(&p, tests::create("expiry")).await;
    let reference = a.data["session_ref"].as_str().unwrap();
    {
        let mut map = r.sessions.lock().unwrap();
        Arc::get_mut(map.get_mut(reference).unwrap())
            .unwrap()
            .expires = Instant::now() - Duration::from_millis(1);
    }
    assert_eq!(
        r.snapshot(&p, args(reference)).await.data["code"],
        "browser_session_closed_or_expired"
    );
    assert_eq!(n.load(Ordering::SeqCst), 0);
}
