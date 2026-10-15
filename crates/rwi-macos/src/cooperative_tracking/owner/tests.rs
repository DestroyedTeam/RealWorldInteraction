use super::*;
#[test]
fn f05_cooperative_tracking_history_never_evicts_or_reexecutes_nonce() {
    let mut h = History(BTreeMap::new());
    let r = Request::Execute {
        version: 1,
        nonce: "once".into(),
        registration_id: "r".into(),
        generation: "g".into(),
        window: 1,
        frame: [0., 0., 4., 4.],
        title: "own".into(),
        phase: TrackingPhase::Entered,
        window_point: [1., 2.],
        deadline_ns: "1".into(),
    };
    assert!(h.reserve("once", &r).is_ok());
    assert!(h.reserve("once", &r).is_err());
    let mut changed = r.clone();
    if let Request::Execute { phase, .. } = &mut changed {
        *phase = TrackingPhase::Exited;
    }
    assert_eq!(
        h.reserve("once", &changed).unwrap_err(),
        reject("nonce_payload_conflict")
    );
    for n in 1..HISTORY_LIMIT {
        assert!(h.reserve(&format!("n{n}"), &r).is_ok());
    }
    assert_eq!(
        h.reserve("never-evict", &r).unwrap_err(),
        reject("history_full_no_eviction")
    );
}
#[test]
fn f05_cooperative_tracking_safe_role_rejects_arbitrary_objc_without_window() {
    let text = NSString::from_str("not a view/window/app");
    assert!(require_class(&text, c"NSView").is_err());
    assert!(require_class(&text, c"NSWindow").is_err());
    assert!(require_class(&text, c"NSApplication").is_err());
}
#[test]
fn f05_cooperative_tracking_staged_cleanup_only_original_private_inodes() {
    let parent = tempfile::Builder::new()
        .prefix("rwi-coop-clean-")
        .tempdir_in("/tmp")
        .unwrap();
    let directory = parent.path().join("new");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let endpoint = directory.join("owner.sock");
    let listener = UnixListener::bind(&endpoint).unwrap();
    let guard = CreatedPaths {
        directory: directory.clone(),
        directory_inode: std::fs::symlink_metadata(&directory).unwrap().ino(),
        endpoint: endpoint.clone(),
        socket_inode: Some(std::fs::symlink_metadata(&endpoint).unwrap().ino()),
        armed: true,
    };
    drop(listener);
    drop(guard);
    assert!(!endpoint.exists());
    assert!(!directory.exists());
    let foreign = parent.path().join("keep");
    std::fs::create_dir(&foreign).unwrap();
    cleanup_owned(&foreign, 0, &foreign.join("absent.sock"), None);
    assert!(foreign.exists());
}
