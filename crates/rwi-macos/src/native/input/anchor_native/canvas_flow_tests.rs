//! Offline cross-layer admission fixtures. No native AX, screen or input calls.
use super::*;
use rwi_core::{Backend, Engine, Locator, Oracle, Predicate, Scope};
use std::sync::{Arc, Mutex};

struct State {
    phase: u8,
    proof: WindowCanvasProof,
    dispatches: u64,
}
struct EmptyWindowCanvas(Arc<Mutex<State>>);

fn pixels(phase: u8) -> Vec<u8> {
    let mut rgb = vec![255; 160 * 160 * 3];
    let shift = usize::from(phase == 2) * 2;
    for y in 61..99 {
        for x in 61 + shift..99 + shift {
            let offset = (y * 160 + x) * 3;
            rgb[offset..offset + 3].copy_from_slice(&[20, 50, 90]);
        }
    }
    if phase == 1 {
        rgb[0..3].copy_from_slice(&[0, 0, 0]);
    }
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, 160, 160);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&rgb)
        .unwrap();
    png
}
impl Backend for EmptyWindowCanvas {
    fn local_pixel_anchor(&self) -> bool {
        true
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        Some(self.0.lock().unwrap().dispatches)
    }
    fn identity(&self) -> Identity {
        Identity {
            host: "synthetic-empty-window".into(),
            guest: None,
            session: "fixture".into(),
            seat: "fixture".into(),
            desktop: "fixture".into(),
        }
    }
    fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({"adapter":"synthetic-window-canvas"})
    }
    fn observe(&mut self) -> Result<Scene> {
        let phase = self.0.lock().unwrap().phase;
        let png = pixels(phase);
        Ok(Scene {
            identity: self.identity(),
            assurance: "synthetic".into(),
            binding: Some(Binding {
                scope: Scope {
                    window: "fixture-window".into(),
                    pid: 42,
                    element: None,
                },
                focus_window: "fixture-window".into(),
                layout: "fixed".into(),
                content: rwi_core::digest(&png),
                native_epoch: "fixed".into(),
                mutation_context: None,
            }),
            mutation_ready: true,
            blocked: Some(false),
            image_size: [160, 160],
            pixel_to_desktop: Some([1., 0., 0., 1., 0., 0.]),
            elements: vec![],
            captured_unix_ms: 0,
            atomicity: "synthetic".into(),
            image_normalization: "RGB8".into(),
            png,
        })
    }
    fn act(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        _: &Intent,
        _: Option<&str>,
    ) -> Result<Option<bool>> {
        if !grounding_matches(expected, &self.observe()?, target) {
            return Err(fault(
                "stale_reference",
                "Fixture second bitmap check failed.",
            ));
        }
        // This fixture must reach the new native surface gate, not succeed by
        // merely comparing an unchanged full image.
        assert!(target.pixel_anchor_used);
        let mut state = self.0.lock().unwrap();
        if !local_pixel_canvas(Some("AXWindow"), state.proof) {
            return Err(fault(
                "local_pixel_surface_changed",
                "Fixture native canvas proof rejected.",
            ));
        }
        state.dispatches += 1;
        Ok(None)
    }
    fn inspect(&mut self, _: &Scope, _: &Predicate, _: &str) -> Result<Oracle> {
        Err(fault("unavailable", "No synthetic business oracle."))
    }
}
fn setup() -> (tempfile::TempDir, Engine, Arc<Mutex<State>>) {
    let root = tempfile::tempdir().unwrap();
    let state = Arc::new(Mutex::new(State {
        phase: 0,
        dispatches: 0,
        proof: WindowCanvasProof {
            application_hit: true,
            exact_window_object: true,
            same_pid: true,
            exact_window_number: true,
            children_count: Some(0),
            system_window_hit: None,
        },
    }));
    let mut engine = Engine::new(root.path().into()).unwrap();
    engine
        .register(
            "window-canvas".into(),
            Box::new(EmptyWindowCanvas(state.clone())),
        )
        .unwrap();
    (root, engine, state)
}
#[test]
fn zero_controls_with_stable_button_and_background_animation_dispatches_once() {
    let (_root, mut engine, state) = setup();
    let frame = engine.observe("window-canvas").unwrap().frame;
    let reference = engine
        .resolve("window-canvas", &frame, Locator::Pixel { x: 80., y: 80. })
        .unwrap();
    assert!(reference.pixel_anchor.is_some());
    state.lock().unwrap().phase = 1;
    let lease = engine.claim("window-canvas", 5000).unwrap();
    let first = engine
        .act(
            "window-canvas",
            &reference.reference,
            &lease,
            "stable-button",
            Intent::Click,
            None,
        )
        .unwrap();
    assert_eq!(first.dispatch, "returned");
    assert!(!first.goal_verified);
    assert_eq!(state.lock().unwrap().dispatches, 1);
    let retained = engine
        .act(
            "window-canvas",
            &reference.reference,
            &lease,
            "stable-button",
            Intent::Click,
            None,
        )
        .unwrap();
    assert!(retained.deduplicated);
    assert_eq!(state.lock().unwrap().dispatches, 1);
    engine.release(&lease).unwrap();
}
#[test]
fn moved_button_and_unknown_native_children_still_dispatch_nothing() {
    for moved in [true, false] {
        let (_root, mut engine, state) = setup();
        let frame = engine.observe("window-canvas").unwrap().frame;
        let reference = engine
            .resolve("window-canvas", &frame, Locator::Pixel { x: 80., y: 80. })
            .unwrap();
        {
            let mut state = state.lock().unwrap();
            state.phase = if moved { 2 } else { 1 };
            if !moved {
                state.proof.children_count = None;
            }
        }
        let lease = engine.claim("window-canvas", 5000).unwrap();
        let result = engine.act(
            "window-canvas",
            &reference.reference,
            &lease,
            "unproven-button",
            Intent::Click,
            None,
        );
        if moved {
            assert_eq!(result.unwrap_err().code, "stale_reference");
        } else {
            let receipt = result.unwrap();
            assert_eq!(receipt.dispatch, "rejected_before_dispatch");
            assert_eq!(
                receipt.dispatch_error.unwrap().code,
                "local_pixel_surface_changed"
            );
        }
        assert_eq!(state.lock().unwrap().dispatches, 0);
        engine.release(&lease).unwrap();
    }
}
