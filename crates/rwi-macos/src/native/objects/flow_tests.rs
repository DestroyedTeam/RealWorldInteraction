//! Object registry -> exact-target revalidation -> Engine -> durable receipt.
//! Synthetic only; no native AX calls, applications, capture or OS input.
use super::*;
use rwi_core::{
    Backend, Binding, CheckedAction, Element, Engine, Identity, Intent, Oracle, Predicate,
    Resolved, Scene, Scope, grounding_matches,
};
use std::sync::{Arc, Mutex};

struct State {
    objects: Objects<u64>,
    root: u64,
    control: u64,
    attached: bool,
    enabled: bool,
    action: bool,
    sibling_revision: u64,
    reads: Vec<u64>,
    dispatches: u64,
}
struct TargetFixture(Arc<Mutex<State>>);
fn identity() -> Identity {
    Identity {
        host: "synthetic-retained-ax".into(),
        guest: None,
        session: "fixture".into(),
        seat: "fixture".into(),
        desktop: "fixture".into(),
    }
}
fn element(state: &mut State, object: u64) -> Element {
    let id = state
        .objects
        .identify(state.root, object, "same-public-attributes")
        .unwrap();
    Element {
        id,
        name: "Identical button".into(),
        role: Some("AXButton".into()),
        actions: if state.action {
            vec!["AXPress".into()]
        } else {
            vec![]
        },
        enabled: Some(state.enabled),
        point: [5., 5.],
        visible: None,
        occluded: None,
        state_revision: Some("fixed".into()),
        selected: None,
        description: None,
        value: None,
        value_truncated: false,
    }
}
fn scene(elements: Vec<Element>) -> Scene {
    Scene {
        identity: identity(),
        assurance: "synthetic".into(),
        binding: Some(Binding {
            scope: Scope {
                window: "fixture-window".into(),
                pid: 42,
                element: None,
            },
            focus_window: "fixture-window".into(),
            layout: "fixed".into(),
            content: "fixed".into(),
            native_epoch: "fixed".into(),
            mutation_context: None,
        }),
        mutation_ready: true,
        blocked: Some(false),
        image_size: [0, 0],
        pixel_to_desktop: None,
        elements,
        captured_unix_ms: 0,
        atomicity: "synthetic".into(),
        image_normalization: "no pixels".into(),
        png: vec![],
    }
}
impl TargetFixture {
    fn target(&self, reference: &Resolved) -> Result<Scene> {
        let mut state = self.0.lock().unwrap();
        let id = reference.scope.element.as_deref().unwrap();
        let object = *state.objects.get(&state.root, id)?;
        state.reads.push(object);
        if object != state.control {
            return Err(fault(
                "native_object_invalid",
                "Original synthetic native object was replaced.",
            ));
        }
        require_ancestor(
            object,
            &state.root,
            |_| {
                if state.attached {
                    Ok(state.root)
                } else {
                    Err(fault("parent_unknown", "Detached from bound root."))
                }
            },
            || Ok(()),
        )?;
        Ok(scene(vec![element(&mut state, object)]))
    }
}
impl Backend for TargetFixture {
    fn identity(&self) -> Identity {
        identity()
    }
    fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({"checked_context_policy":"native_element","native_semantic_actions":["AXPress"],"synthetic":true})
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        Some(self.0.lock().unwrap().dispatches)
    }
    fn observe(&mut self) -> Result<Scene> {
        let mut state = self.0.lock().unwrap();
        let control = state.control;
        let target = element(&mut state, control);
        let mut sibling = element(&mut state, 99);
        sibling.state_revision = Some(state.sibling_revision.to_string());
        Ok(scene(vec![target, sibling]))
    }
    fn revalidate(&mut self, reference: &Resolved) -> Result<Scene> {
        self.target(reference)
    }
    fn act(
        &mut self,
        expected: &Binding,
        reference: &Resolved,
        _: &Intent,
        value: Option<&str>,
    ) -> Result<Option<bool>> {
        assert_eq!(value, Some("AXPress"));
        assert_eq!(reference.native_action.as_deref(), Some("AXPress"));
        if !grounding_matches(expected, &self.target(reference)?, reference) {
            return Err(fault("target_changed", "No synthetic input."));
        }
        self.0.lock().unwrap().dispatches += 1;
        Ok(None)
    }
    fn inspect(&mut self, _: &Scope, _: &Predicate, _: &str) -> Result<Oracle> {
        Err(fault("unavailable", "No business oracle."))
    }
}
fn setup() -> (
    tempfile::TempDir,
    Engine,
    Arc<Mutex<State>>,
    String,
    String,
    String,
) {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(Mutex::new(State {
        objects: Objects::default(),
        root: 1,
        control: 10,
        attached: true,
        enabled: true,
        action: true,
        sibling_revision: 0,
        reads: vec![],
        dispatches: 0,
    }));
    let mut engine = Engine::new(dir.path().into()).unwrap();
    engine
        .register("fixture".into(), Box::new(TargetFixture(state.clone())))
        .unwrap();
    let observation = engine.observe("fixture").unwrap();
    let ids = observation
        .scene
        .elements
        .iter()
        .map(|e| e.id.clone())
        .collect::<Vec<_>>();
    assert_ne!(ids[0], ids[1]);
    let lease = engine.claim("fixture", 30000).unwrap();
    (dir, engine, state, observation.frame, ids[0].clone(), lease)
}
fn action(engine: &mut Engine, frame: &str, id: &str, lease: &str) -> Result<rwi_core::Receipt> {
    engine.act_checked(CheckedAction {
        target: "fixture",
        frame,
        element: id,
        lease,
        request: "one-intended-press",
        intent: Intent::Click,
        value: Some("AXPress".into()),
    })
}
#[test]
fn identical_controls_and_unrelated_dynamic_branch_preserve_one_exact_dispatch_and_no_replay() {
    let (_dir, mut engine, state, frame, id, lease) = setup();
    state.lock().unwrap().sibling_revision = 1000;
    let receipt = action(&mut engine, &frame, &id, &lease).unwrap();
    assert_eq!(receipt.dispatch, "returned");
    assert!(!receipt.goal_verified);
    assert_eq!(state.lock().unwrap().dispatches, 1);
    assert!(
        state
            .lock()
            .unwrap()
            .reads
            .iter()
            .all(|object| *object == 10)
    );
    let reads = state.lock().unwrap().reads.len();
    assert!(
        action(&mut engine, &frame, &id, &lease)
            .unwrap()
            .deduplicated
    );
    assert_eq!(state.lock().unwrap().reads.len(), reads);
    assert_eq!(state.lock().unwrap().dispatches, 1);
    engine.release(&lease).unwrap();
    drop(engine);
    state.lock().unwrap().control = 20;
    let mut restarted = Engine::new(_dir.path().into()).unwrap();
    restarted
        .register("fixture".into(), Box::new(TargetFixture(state.clone())))
        .unwrap();
    assert!(
        action(&mut restarted, &frame, &id, &lease)
            .unwrap()
            .deduplicated
    );
    assert_eq!(state.lock().unwrap().reads.len(), reads);
    assert_eq!(state.lock().unwrap().dispatches, 1);
}
#[test]
fn replacement_window_detachment_disabled_and_removed_action_still_dispatch_nothing() {
    for change in 0..5 {
        let (_dir, mut engine, state, frame, id, lease) = setup();
        {
            let mut s = state.lock().unwrap();
            match change {
                0 => s.control = 20,
                1 => s.root = 2,
                2 => s.attached = false,
                3 => s.enabled = false,
                _ => s.action = false,
            }
        }
        assert!(
            action(&mut engine, &frame, &id, &lease).is_err(),
            "negative case {change}"
        );
        assert_eq!(state.lock().unwrap().dispatches, 0);
        engine.release(&lease).unwrap();
    }
}
