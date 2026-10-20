use super::*;
use crate::fixture::Fixture;

fn setup() -> (tempfile::TempDir, Engine, Fixture) {
    let dir = tempfile::tempdir().unwrap();
    let f = Fixture::default();
    let mut e = Engine::new(dir.path().into()).unwrap();
    e.register("fixture".into(), Box::new(f.clone())).unwrap();
    (dir, e, f)
}
#[test]
fn typed_drag_is_one_durable_intent_and_changed_path_never_replays() {
    use crate::trajectory::{DragPath, Waypoint};
    let (_dir, mut e, f) = setup();
    let frame = e.observe("fixture").unwrap().frame;
    let lease = e.claim("fixture", 60000).unwrap();
    let mut path = DragPath {
        waypoints: vec![
            Waypoint {
                at_ms: 0,
                offset: [0., 0.],
            },
            Waypoint {
                at_ms: 200,
                offset: [20., 0.],
            },
        ],
    };
    let invoke = |e: &mut Engine, path: DragPath| {
        e.act_checked(CheckedAction {
            target: "fixture",
            frame: &frame,
            element: "Input",
            lease: &lease,
            request: "drag-once",
            intent: Intent::Drag(path),
            value: None,
        })
    };
    assert_eq!(
        invoke(&mut e, path.clone()).unwrap().system_accepted,
        Some(true)
    );
    assert!(invoke(&mut e, path.clone()).unwrap().deduplicated);
    assert_eq!(f.0.lock().unwrap().calls, 1);
    path.waypoints[1].offset = [21., 0.];
    assert_eq!(
        invoke(&mut e, path).unwrap_err().code,
        "idempotency_conflict"
    );
    assert_eq!(f.0.lock().unwrap().calls, 1);
    e.release(&lease).unwrap();
}
fn age(e: &mut Engine, frame: &str, seconds: u64) {
    e.snapshots
        .iter_mut()
        .find(|(id, _)| id == frame)
        .unwrap()
        .1
        .at -= Duration::from_secs(seconds);
}
fn checked(e: &mut Engine, frame: &str, lease: &str, request: &str) -> Result<Receipt> {
    e.act_checked(CheckedAction {
        target: "fixture",
        frame,
        element: "Input",
        lease,
        request,
        intent: Intent::Text,
        value: Some("checked result".into()),
    })
}
#[test]
fn delayed_semantic_proposal_is_revalidated_and_dispatched_once() {
    let (_d, mut e, f) = setup();
    let frame = e.observe("fixture").unwrap().frame;
    let lease = e.claim("fixture", 60000).unwrap();
    age(&mut e, &frame, 30);
    assert_eq!(f.0.lock().unwrap().calls, 0);
    let result = checked(&mut e, &frame, &lease, "checked-once").unwrap();
    assert_eq!(result.system_accepted, Some(true));
    assert_ne!(result.frame, frame);
    assert!(!result.goal_verified);
    assert_eq!(f.0.lock().unwrap().value, "checked result");
    assert_eq!(f.0.lock().unwrap().calls, 1);
    assert!(
        checked(&mut e, &frame, &lease, "checked-once")
            .unwrap()
            .deduplicated
    );
    assert_eq!(f.0.lock().unwrap().calls, 1);
    e.release(&lease).unwrap();
}
#[test]
fn unchanged_document_does_not_turn_old_pixel_reference_into_checked_authority() {
    let (_d, mut e, f) = setup();
    let frame = e.observe("fixture").unwrap().frame;
    let pixel = e
        .resolve("fixture", &frame, Locator::Pixel { x: 30., y: 40. })
        .unwrap();
    let lease = e.claim("fixture", 60000).unwrap();
    age(&mut e, &frame, INPUT_PROPOSAL_MAX_AGE_SECS + 1);
    assert_eq!(
        e.act(
            "fixture",
            &pixel.reference,
            &lease,
            "pixel",
            Intent::Click,
            None
        )
        .unwrap_err()
        .code,
        "stale_reference"
    );
    assert!(
        e.act_checked(CheckedAction {
            target: "fixture",
            frame: &frame,
            element: &pixel.reference,
            lease: &lease,
            request: "pixel-as-element",
            intent: Intent::Click,
            value: None
        })
        .is_err()
    );
    assert_eq!(f.0.lock().unwrap().calls, 0);
}
#[test]
fn proposal_horizon_cancel_unknown_context_and_expired_lease_remain_fail_closed() {
    let (d, mut e, f) = setup();
    let frame = e.observe("fixture").unwrap().frame;
    let lease = e.claim("fixture", 60000).unwrap();
    age(&mut e, &frame, 121);
    assert_eq!(
        checked(&mut e, &frame, &lease, "expired").unwrap_err().code,
        "checked_proposal_expired"
    );
    let frame = e.observe("fixture").unwrap().frame;
    e.snapshots
        .back_mut()
        .unwrap()
        .1
        .scene
        .binding
        .as_mut()
        .unwrap()
        .mutation_context = None;
    assert_eq!(
        checked(&mut e, &frame, &lease, "unknown").unwrap_err().code,
        "checked_context_unavailable"
    );
    let frame = e.observe("fixture").unwrap().frame;
    let op = operation::Registry::default().begin(None).unwrap();
    op.cancel();
    assert_eq!(
        operation::run(&op, || checked(&mut e, &frame, &lease, "cancelled"))
            .unwrap_err()
            .code,
        "operation_cancelled"
    );
    e.release(&lease).unwrap();
    assert!(checked(&mut e, &frame, &lease, "released-lease").is_err());
    assert_eq!(f.0.lock().unwrap().calls, 0);
    assert_eq!(fs::read_dir(d.path().join("receipts")).unwrap().count(), 0);
}
#[test]
fn checked_request_fingerprint_survives_new_engine_and_rejects_altered_intent() {
    let (d, mut e, f) = setup();
    let frame = e.observe("fixture").unwrap().frame;
    let lease = e.claim("fixture", 60000).unwrap();
    checked(&mut e, &frame, &lease, "persist").unwrap();
    e.release(&lease).unwrap();
    drop(e);
    let mut e = Engine::new(d.path().into()).unwrap();
    e.register("fixture".into(), Box::new(f.clone())).unwrap();
    assert!(
        checked(&mut e, &frame, "not-a-reused-token", "persist")
            .unwrap()
            .deduplicated
    );
    assert_eq!(
        e.act_checked(CheckedAction {
            target: "fixture",
            frame: &frame,
            element: "Input",
            lease: "none",
            request: "persist",
            intent: Intent::Text,
            value: Some("different".into())
        })
        .unwrap_err()
        .code,
        "idempotency_conflict"
    );
    assert_eq!(f.0.lock().unwrap().calls, 1);
}
struct NativeLikeEditor(Fixture);
impl Backend for NativeLikeEditor {
    fn identity(&self) -> Identity {
        self.0.identity()
    }
    fn capabilities(&self) -> serde_json::Value {
        self.0.capabilities()
    }
    fn observe(&mut self) -> Result<Scene> {
        let mut s = self.0.observe()?;
        // Deliberately model the previous native revision weakness, not the
        // stronger original Fixture Input revision that already hashes value.
        for e in &mut s.elements {
            e.state_revision = Some("stable-runtime-name-geometry-focus".into());
        }
        Ok(s)
    }
    fn act(
        &mut self,
        b: &Binding,
        r: &Resolved,
        i: &Intent,
        v: Option<&str>,
    ) -> Result<Option<bool>> {
        self.0.act(b, r, i, v)
    }
    fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> Result<Oracle> {
        self.0.inspect(s, p, e)
    }
}
#[test]
fn value_switch_with_identical_runtime_name_geometry_focus_is_rejected_before_input() {
    let d = tempfile::tempdir().unwrap();
    let f = Fixture::default();
    let mut e = Engine::new(d.path().into()).unwrap();
    e.register("fixture".into(), Box::new(NativeLikeEditor(f.clone())))
        .unwrap();
    let frame = e.observe("fixture").unwrap().frame;
    let lease = e.claim("fixture", 60000).unwrap();
    f.0.lock().unwrap().value = "other document same editor".into();
    assert_eq!(
        checked(&mut e, &frame, &lease, "switched-doc")
            .unwrap_err()
            .code,
        "checked_precondition_changed"
    );
    assert_eq!(f.0.lock().unwrap().calls, 0);
    assert_eq!(fs::read_dir(d.path().join("receipts")).unwrap().count(), 0);
}
#[test]
fn changed_title_context_or_layout_is_not_refreshed_into_permission() {
    let (_d, mut e, f) = setup();
    let lease = e.claim("fixture", 60000).unwrap();
    let frame = e.observe("fixture").unwrap().frame;
    e.snapshots
        .back_mut()
        .unwrap()
        .1
        .scene
        .binding
        .as_mut()
        .unwrap()
        .mutation_context
        .as_mut()
        .unwrap()
        .window_title = "different document".into();
    assert_eq!(
        checked(&mut e, &frame, &lease, "title").unwrap_err().code,
        "checked_precondition_changed"
    );
    let frame = e.observe("fixture").unwrap().frame;
    f.0.lock().unwrap().transform = Some([1., 0., 0., 1., 0., 0.]);
    assert_eq!(
        checked(&mut e, &frame, &lease, "layout").unwrap_err().code,
        "checked_precondition_changed"
    );
    assert_eq!(f.0.lock().unwrap().calls, 0);
}
