use super::*;
use crate::fixture::windows::Desktop;
fn setup() -> (tempfile::TempDir, Engine, Desktop, [WindowSelection; 2]) {
    let dir = tempfile::tempdir().unwrap();
    let desktop = Desktop::default();
    let mut engine = Engine::new(dir.path().into()).unwrap();
    engine
        .register("root".into(), Box::new(desktop.clone()))
        .unwrap();
    let catalog = engine.discover("root").unwrap();
    let selections =
        std::array::from_fn(|i| serde_json::from_value(catalog["windows"][i].clone()).unwrap());
    (dir, engine, desktop, selections)
}
fn attach(e: &mut Engine, s: &WindowSelection) -> String {
    e.attach_window("root", s).unwrap()["target"]
        .as_str()
        .unwrap()
        .into()
}
#[test]
fn independent_windows_keep_root_binding_frames_and_exact_target_scope() {
    let (_d, mut e, f, s) = setup();
    let root = e.observe("root").unwrap();
    let first = attach(&mut e, &s[0]);
    let second = attach(&mut e, &s[1]);
    assert_ne!(first, second);
    assert_eq!(e.targets().as_array().unwrap().len(), 3);
    assert_eq!(
        e.observe("root").unwrap().scene.binding.unwrap().scope,
        root.scene.binding.unwrap().scope
    );
    assert!(
        e.resolve("root", &root.frame, Locator::Element { id: "Input".into() })
            .is_ok()
    );
    let a = e.observe(&first).unwrap();
    let b = e.observe(&second).unwrap();
    assert_ne!(
        a.scene.binding.as_ref().unwrap().scope.window,
        b.scene.binding.as_ref().unwrap().scope.window
    );
    assert!(
        e.resolve(&second, &a.frame, Locator::Element { id: "Input".into() })
            .is_err()
    );
    e.bind_window("root", &s[1]).unwrap();
    assert!(
        e.resolve(&first, &a.frame, Locator::Element { id: "Input".into() })
            .is_ok()
    );
    assert_eq!(
        e.bind_window(&first, &s[1]).unwrap_err().code,
        "fixed_window_target"
    );
    assert_eq!(
        e.attach_window(&first, &s[1]).unwrap_err().code,
        "attachment_source_required"
    );
    assert_eq!(
        f.windows
            .iter()
            .map(|w| w.0.lock().unwrap().calls)
            .sum::<u64>(),
        0
    );
}
#[test]
fn physical_lease_is_shared_and_detach_never_releases_it_implicitly() {
    let (_d, mut e, _, s) = setup();
    let a = attach(&mut e, &s[0]);
    let b = attach(&mut e, &s[1]);
    let lease = e.claim(&a, 60000).unwrap();
    assert_eq!(e.claim(&b, 60000).unwrap_err().code, "desktop_busy");
    assert_eq!(e.claim("root", 60000).unwrap_err().code, "desktop_busy");
    assert_eq!(e.detach_window(&a).unwrap_err().code, "target_leased");
    assert_eq!(
        e.detach_window("root").unwrap_err().code,
        "attachment_required"
    );
    assert!(e.detach_window(&b).is_ok());
    assert_eq!(e.claim("root", 60000).unwrap_err().code, "desktop_busy");
    e.release(&lease).unwrap();
    e.detach_window(&a).unwrap();
}
#[test]
fn detached_frames_expire_but_durable_action_is_queryable_without_replay() {
    let (d, mut e, f, s) = setup();
    let a = attach(&mut e, &s[0]);
    let frame = e.observe(&a).unwrap().frame;
    let lease = e.claim(&a, 60000).unwrap();
    let receipt = e
        .act_checked(CheckedAction {
            target: &a,
            frame: &frame,
            element: "Input",
            lease: &lease,
            request: "once",
            intent: Intent::Text,
            value: Some("kept".into()),
        })
        .unwrap();
    let receipt_path = e.journal_path(&f.windows[0].identity().key(), "once");
    let durable = std::fs::read(&receipt_path).unwrap();
    e.release(&lease).unwrap();
    e.detach_window(&a).unwrap();
    assert!(e.observe(&a).is_err());
    assert!(
        e.resolve(&a, &frame, Locator::Element { id: "Input".into() })
            .is_err()
    );
    let replacement = attach(&mut e, &s[1]);
    assert_ne!(a, replacement);
    assert!(
        e.resolve(
            &replacement,
            &frame,
            Locator::Element { id: "Input".into() }
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&receipt_path).unwrap(), durable);
    assert_eq!(e.recent_operations(1)[0]["request_id"], receipt.request_id);
    drop(e);
    let restored = Engine::new(d.path().into()).unwrap();
    assert_eq!(std::fs::read(&receipt_path).unwrap(), durable);
    assert_eq!(
        restored.recent_operations(1)[0]["request_id"],
        receipt.request_id
    );
    assert_eq!(f.windows[0].0.lock().unwrap().calls, 1);
}
#[test]
fn changed_identity_catalog_selection_capacity_and_cancel_register_nothing() {
    let (_d, mut e, f, s) = setup();
    let mut wrong = s[0].clone();
    wrong.pid = 43;
    assert!(e.attach_window("root", &wrong).is_err());
    wrong = s[0].clone();
    wrong.native_epoch = "other".into();
    assert!(e.attach_window("root", &wrong).is_err());
    f.windows[1].0.lock().unwrap().identity.desktop = "other-desktop".into();
    assert_eq!(
        e.attach_window("root", &s[1]).unwrap_err().code,
        "session_mismatch"
    );
    assert_eq!(e.targets().as_array().unwrap().len(), 1);
    let op = operation::Registry::default().begin(None).unwrap();
    op.cancel();
    assert!(operation::run(&op, || e.attach_window("root", &s[0])).is_err());
    for _ in 0..15 {
        attach(&mut e, &s[0]);
    }
    assert_eq!(
        e.attach_window("root", &s[0]).unwrap_err().code,
        "target_capacity"
    );
    assert_eq!(e.targets().as_array().unwrap().len(), 16);
    assert_eq!(
        f.windows
            .iter()
            .map(|w| w.0.lock().unwrap().calls)
            .sum::<u64>(),
        0
    );
}

#[test]
fn same_pid_element_id_cannot_cross_window_frame_reference_or_lease() {
    let (_d, mut e, f, s) = setup();
    let a = attach(&mut e, &s[0]);
    let b = attach(&mut e, &s[1]);
    let aframe = e.observe(&a).unwrap().frame;
    let bframe = e.observe(&b).unwrap().frame;
    let reference = e
        .resolve(&a, &aframe, Locator::Element { id: "Input".into() })
        .unwrap();
    let alease = e.claim(&a, 60000).unwrap();
    assert!(
        e.act_checked(CheckedAction {
            target: &b,
            frame: &bframe,
            element: "Input",
            lease: &alease,
            request: "foreign-lease",
            intent: Intent::Click,
            value: None
        })
        .is_err()
    );
    e.release(&alease).unwrap();
    let blease = e.claim(&b, 60000).unwrap();
    assert!(
        e.act_checked(CheckedAction {
            target: &b,
            frame: &aframe,
            element: "Input",
            lease: &blease,
            request: "foreign-frame",
            intent: Intent::Click,
            value: None
        })
        .is_err()
    );
    assert!(
        e.act(
            &b,
            &reference.reference,
            &blease,
            "foreign-ref",
            Intent::Click,
            None
        )
        .is_err()
    );
    e.release(&blease).unwrap();
    assert!(e.recent_operations(5).is_empty());
    assert_eq!(
        f.windows
            .iter()
            .map(|w| w.0.lock().unwrap().calls)
            .sum::<u64>(),
        0
    );
}

#[test]
fn attachments_in_separate_context_engines_share_one_physical_lease_root() {
    let dir = tempfile::tempdir().unwrap();
    let desktop = Desktop::default();
    let shared = dir.path().join("shared");
    let mut first = Engine::with_lease_root(dir.path().join("first"), shared.clone()).unwrap();
    let mut second = Engine::with_lease_root(dir.path().join("second"), shared).unwrap();
    first
        .register("root".into(), Box::new(desktop.clone()))
        .unwrap();
    second.register("root".into(), Box::new(desktop)).unwrap();
    let selection: WindowSelection =
        serde_json::from_value(first.discover("root").unwrap()["windows"][0].clone()).unwrap();
    second.discover("root").unwrap();
    let a = attach(&mut first, &selection);
    let b = attach(&mut second, &selection);
    assert_ne!(a, b);
    let lease = first.claim(&a, 60000).unwrap();
    assert_eq!(second.claim(&b, 60000).unwrap_err().code, "desktop_busy");
    first.release(&lease).unwrap();
    let lease = second.claim(&b, 60000).unwrap();
    second.release(&lease).unwrap();
}
