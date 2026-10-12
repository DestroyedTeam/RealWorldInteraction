use super::*;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

#[test]
fn endpoint_equality_allows_mapping_roundoff_not_alternate_drop_sites() {
    let origin = [-1783.73, 20.15];
    let destination = [1912.37, 880.19];
    let offset = [destination[0] - origin[0], destination[1] - origin[1]];
    assert!(endpoint_matches(origin, offset, destination));
    assert!(!endpoint_matches(origin, offset, [1912.3701, 880.19]));
    assert!(!endpoint_matches(origin, offset, [f64::NAN, 880.19]));
    assert!(!endpoint_matches([f64::INFINITY, 0.], offset, destination));
}

struct NativeFixture {
    scene: Arc<Mutex<Scene>>,
    calls: Arc<AtomicU64>,
    supported: bool,
    fail: bool,
}
impl Backend for NativeFixture {
    fn identity(&self) -> Identity {
        self.scene.lock().unwrap().identity.clone()
    }
    fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({"paired_pointer_drag":{"implemented":self.supported}})
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        Some(self.calls.load(Ordering::Acquire))
    }
    fn observe(&mut self) -> Result<Scene> {
        Ok(self.scene.lock().unwrap().clone())
    }
    fn act(
        &mut self,
        _: &Binding,
        _: &Resolved,
        _: &Intent,
        _: Option<&str>,
    ) -> Result<Option<bool>> {
        Err(fault("ordinary_route_called", "No paired fallback."))
    }
    fn inspect(&mut self, _: &Scope, _: &Predicate, _: &str) -> Result<Oracle> {
        Err(fault("not_tested", "No business oracle."))
    }
    fn drag_surface(&mut self, e: &Endpoint, _: bool) -> Result<Surface> {
        let scene = self.scene.lock().unwrap();
        let binding = scene.binding.as_ref().unwrap();
        if scene.identity != e.identity || binding != &e.binding {
            return Err(fault("native_changed", "No native dispatch."));
        }
        Ok(Surface {
            identity: scene.identity.clone(),
            scope: binding.scope.clone(),
            process_start: "test-process-birth".into(),
            native_epoch: binding.native_epoch.clone(),
            layout: binding.layout.clone(),
            bounds: [0., 0., 500., 500.],
        })
    }
    fn drag_between(&mut self, p: &Pair, dst: &mut dyn Backend) -> Result<Option<bool>> {
        self.drag_surface(&p.source, true)?;
        dst.drag_surface(&p.destination, true)?;
        self.calls.fetch_add(1, Ordering::AcqRel);
        if self.fail {
            return Err(fault(
                "native_uncertain",
                "Attempted native dispatch; do not replay.",
            ));
        }
        Ok(None)
    }
}
fn scene(id: u32, point: [f64; 2]) -> Scene {
    let identity = Identity {
        host: "host".into(),
        guest: None,
        session: "session".into(),
        seat: "seat".into(),
        desktop: "desktop".into(),
    };
    let scope = Scope {
        window: format!("window:{id}"),
        pid: id,
        element: None,
    };
    Scene {
        identity,
        assurance: "synthetic".into(),
        binding: Some(Binding {
            scope,
            focus_window: format!("window:{id}"),
            layout: "layout".into(),
            content: "bitmap".into(),
            native_epoch: "epoch".into(),
            mutation_context: None,
        }),
        mutation_ready: true,
        blocked: Some(false),
        image_size: [500, 500],
        pixel_to_desktop: Some([1., 0., 0., 1., 0., 0.]),
        elements: vec![Element {
            id: "Endpoint".into(),
            name: "Endpoint".into(),
            role: Some("AXButton".into()),
            actions: vec![],
            enabled: Some(true),
            selected: None,
            description: None,
            value: None,
            value_truncated: false,
            point,
            visible: Some(true),
            occluded: Some(false),
            state_revision: None,
        }],
        captured_unix_ms: 0,
        atomicity: "test".into(),
        image_normalization: "test".into(),
        png: vec![],
    }
}
struct Setup {
    engine: Engine,
    _dir: tempfile::TempDir,
    source: Resolved,
    destination: Resolved,
    state: Arc<Mutex<Scene>>,
    calls: Arc<AtomicU64>,
}
fn setup(supported: bool, fail: bool) -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::new(dir.path().into()).unwrap();
    let calls = Arc::new(AtomicU64::new(0));
    let source = Arc::new(Mutex::new(scene(1, [20., 50.])));
    let destination = Arc::new(Mutex::new(scene(2, [300., 50.])));
    for (name, s) in [("source", source), ("destination", destination.clone())] {
        engine
            .register(
                name.into(),
                Box::new(NativeFixture {
                    scene: s,
                    calls: calls.clone(),
                    supported,
                    fail,
                }),
            )
            .unwrap();
    }
    let mut refs = Vec::new();
    for name in ["source", "destination"] {
        let o = engine.observe(name).unwrap();
        refs.push(
            engine
                .resolve(
                    name,
                    &o.frame,
                    Locator::Element {
                        id: "Endpoint".into(),
                    },
                )
                .unwrap(),
        );
    }
    Setup {
        engine,
        _dir: dir,
        source: refs.remove(0),
        destination: refs.remove(0),
        state: destination,
        calls,
    }
}
fn intent(s: &Setup) -> Intent {
    Intent::DragBetween(Destination {
        target: "destination".into(),
        reference: s.destination.reference.clone(),
        path: trajectory::DragPath {
            waypoints: vec![
                trajectory::Waypoint {
                    at_ms: 0,
                    offset: [0., 0.],
                },
                trajectory::Waypoint {
                    at_ms: 500,
                    offset: [280., 0.],
                },
            ],
        },
    })
}
fn invoke(s: &mut Setup, id: &str, i: Intent) -> Result<Receipt> {
    let lease = s.engine.claim("source", 10000)?;
    let r = s
        .engine
        .act("source", &s.source.reference, &lease, id, i, None);
    s.engine.release(&lease)?;
    r
}

#[test]
fn pair_dispatches_once_and_durable_same_request_does_not_replay() {
    let mut s = setup(true, false);
    let i = intent(&s);
    let r = invoke(&mut s, "pair-once", i.clone()).unwrap();
    assert_eq!(r.dispatch, "returned");
    assert!(!r.goal_verified);
    let again = invoke(&mut s, "pair-once", i).unwrap();
    assert!(again.deduplicated);
    assert_eq!(s.calls.load(Ordering::Acquire), 1);
}
#[test]
fn changed_destination_blocked_unready_or_other_native_domain_refuses_before_source_down() {
    for k in 0..6 {
        let mut s = setup(true, false);
        let i = intent(&s);
        {
            let mut p = s.state.lock().unwrap();
            match k {
                0 => p.binding.as_mut().unwrap().native_epoch = "changed".into(),
                1 => p.blocked = None,
                2 => p.mutation_ready = false,
                3 => p.identity.session = "other".into(),
                4 => p.elements[0].point = [301., 50.],
                _ => p.elements[0].occluded = Some(true),
            }
        }
        assert!(invoke(&mut s, "pair-refuse", i).is_err());
        assert_eq!(s.calls.load(Ordering::Acquire), 0, "{k}");
    }
}
#[test]
fn wrong_endpoint_path_reference_or_source_capability_never_dispatches() {
    for k in 0..4 {
        let mut s = setup(k != 3, false);
        let mut i = intent(&s);
        if let Intent::DragBetween(ref mut d) = i {
            match k {
                0 => d.target = "source".into(),
                1 => d.reference = "foreign-ref".into(),
                2 => d.path.waypoints[1].offset = [279., 0.],
                _ => {}
            }
        }
        assert!(invoke(&mut s, "bad-pair", i).is_err());
        assert_eq!(s.calls.load(Ordering::Acquire), 0);
    }
}
#[test]
fn uncertain_pair_result_is_terminal_non_replayed_and_altered_intent_conflicts() {
    let mut s = setup(true, true);
    let i = intent(&s);
    let r = invoke(&mut s, "uncertain-pair", i.clone()).unwrap();
    assert_eq!(r.dispatch, "uncertain");
    let r = invoke(&mut s, "uncertain-pair", i.clone()).unwrap();
    assert!(r.deduplicated);
    assert_eq!(s.calls.load(Ordering::Acquire), 1);
    let mut altered = i;
    if let Intent::DragBetween(ref mut d) = altered {
        d.path.waypoints[1].at_ms = 700;
    }
    assert_eq!(
        invoke(&mut s, "uncertain-pair", altered).unwrap_err().code,
        "idempotency_conflict"
    );
}
#[test]
fn peer_alias_of_same_window_and_placement_reference_are_not_two_input_endpoints() {
    for k in 0..2 {
        let mut s = setup(true, false);
        if k == 0 {
            let binding = s
                .engine
                .snapshot("source", &s.source.frame)
                .unwrap()
                .scene
                .binding
                .clone();
            s.state.lock().unwrap().binding = binding;
        } else {
            let r = s
                .engine
                .refs
                .iter_mut()
                .find(|r| r.reference == s.destination.reference)
                .unwrap();
            r.placement_only = true;
        }
        let i = intent(&s);
        assert!(invoke(&mut s, "pair-alias", i).is_err());
        assert_eq!(s.calls.load(Ordering::Acquire), 0);
    }
}
