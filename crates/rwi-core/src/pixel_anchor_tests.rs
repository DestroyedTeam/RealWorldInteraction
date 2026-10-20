//! Cross-layer tests: a native backend must opt in; input still rechecks the
//! exact binding and a changed local subject is rejected before reservation.
use super::*;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct State {
    phase: u8,
    dispatches: usize,
}
struct Canvas {
    state: Arc<Mutex<State>>,
    tolerant: bool,
}
fn image(phase: u8) -> Vec<u8> {
    let mut rgb = vec![255; 160 * 160 * 3];
    if phase == 5 || phase == 6 {
        // Test-only uniform local patch; a distant corner change affects the
        // full frame without creating texture under the requested point.
        if phase == 6 {
            rgb[..3].copy_from_slice(&[0, 0, 0]);
        }
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, 160, 160);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&rgb)
            .unwrap();
        return out;
    }
    let shift = usize::from(phase == 2) * 2;
    for y in 61..99 {
        for x in 61 + shift..99 + shift {
            let i = (y * 160 + x) * 3;
            rgb[i..i + 3].copy_from_slice(&[20, 50, 90]);
        }
    }
    if phase == 1 {
        for x in 65..81 {
            let i = (55 * 160 + x) * 3;
            rgb[i..i + 3].copy_from_slice(&[0, 0, 0]);
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, 160, 160);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&rgb).unwrap();
    }
    out
}
impl Backend for Canvas {
    fn local_pixel_anchor(&self) -> bool {
        self.tolerant
    }
    fn identity(&self) -> Identity {
        Identity {
            host: "test".into(),
            guest: None,
            session: "test".into(),
            seat: "test".into(),
            desktop: "test".into(),
        }
    }
    fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({"adapter":"synthetic-pixel-anchor-test"})
    }
    fn observe(&mut self) -> Result<Scene> {
        let phase = self.state.lock().unwrap().phase;
        let png = image(phase);
        Ok(Scene {
            identity: self.identity(),
            assurance: "synthetic".into(),
            binding: Some(Binding {
                scope: Scope {
                    window: "test-window".into(),
                    pid: 42,
                    element: None,
                },
                focus_window: "test-window".into(),
                layout: "fixed".into(),
                content: digest(&png),
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
            return Err(fault("stale_reference", "Native second check failed."));
        }
        self.state.lock().unwrap().dispatches += 1;
        Ok(Some(true))
    }
    fn inspect(&mut self, _: &Scope, _: &Predicate, _: &str) -> Result<Oracle> {
        Err(fault("unavailable", "Not an oracle test."))
    }
}

#[derive(Default)]
struct MapState {
    phase: u8,
    dispatches: usize,
}
struct MapCanvas {
    state: Arc<Mutex<MapState>>,
    marker_anchor: bool,
}
fn map_image(phase: u8) -> Vec<u8> {
    let background = if phase == 1 {
        [104, 132, 52]
    } else {
        [72, 118, 60]
    };
    let mut rgb = vec![0; 160 * 160 * 3];
    for pixel in rgb.chunks_exact_mut(3) {
        pixel.copy_from_slice(&background);
    }
    let mut markers = vec![
        (80i32, 80i32),
        (64, 80),
        (96, 80),
        (80, 64),
        (80, 96),
        (69, 69),
        (91, 69),
        (69, 91),
        (91, 91),
    ];
    match phase {
        2 => markers[0] = (84, 80),
        3 => {
            markers.remove(0);
        }
        _ => {}
    }
    for (cx, cy) in markers {
        for dy in -3..=3 {
            for dx in -3..=3 {
                if dx * dx + dy * dy <= 9 {
                    let i = ((cy + dy) as usize * 160 + (cx + dx) as usize) * 3;
                    rgb[i..i + 3].copy_from_slice(&[250, 18, 20]);
                }
            }
        }
    }
    if phase == 1 {
        // Dynamic health/terrain pixels and a separate red damage number do
        // not modify any component in the intended marker cluster.
        for y in 40..56 {
            for x in 100..132 {
                let i = (y * 160 + x) * 3;
                rgb[i..i + 3].copy_from_slice(&[232, 238, 235]);
            }
        }
        for y in 40..45 {
            for x in 38..45 {
                let i = (y * 160 + x) * 3;
                rgb[i..i + 3].copy_from_slice(&[250, 18, 20]);
            }
        }
    } else if phase == 4 {
        // A non-red overlay hides the intended target while leaving neighbors.
        for y in 76..85 {
            for x in 76..85 {
                let i = (y * 160 + x) * 3;
                rgb[i..i + 3].copy_from_slice(&[132, 145, 150]);
            }
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, 160, 160);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&rgb).unwrap();
    }
    out
}
impl Backend for MapCanvas {
    fn local_pixel_anchor(&self) -> bool {
        self.marker_anchor
    }
    fn identity(&self) -> Identity {
        Identity {
            host: "test".into(),
            guest: None,
            session: "map-test".into(),
            seat: "test".into(),
            desktop: "test".into(),
        }
    }
    fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({"adapter":"synthetic-map-marker-test"})
    }
    fn observe(&mut self) -> Result<Scene> {
        let phase = self.state.lock().unwrap().phase;
        let png = map_image(phase);
        Ok(Scene {
            identity: self.identity(),
            assurance: "synthetic".into(),
            binding: Some(Binding {
                scope: Scope {
                    window: "map-window".into(),
                    pid: 43,
                    element: None,
                },
                focus_window: "map-window".into(),
                layout: "fixed".into(),
                content: digest(&png),
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
                "Red marker changed before dispatch.",
            ));
        }
        self.state.lock().unwrap().dispatches += 1;
        Ok(Some(true))
    }
    fn inspect(&mut self, _: &Scope, _: &Predicate, _: &str) -> Result<Oracle> {
        Err(fault("unavailable", "Not an oracle test."))
    }
}
fn setup_map(marker_anchor: bool) -> (tempfile::TempDir, Engine, Arc<Mutex<MapState>>) {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(Mutex::new(MapState::default()));
    let mut engine = Engine::new(dir.path().into()).unwrap();
    engine
        .register(
            "map".into(),
            Box::new(MapCanvas {
                state: state.clone(),
                marker_anchor,
            }),
        )
        .unwrap();
    (dir, engine, state)
}
fn setup(tolerant: bool) -> (tempfile::TempDir, Engine, Arc<Mutex<State>>) {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(Mutex::new(State::default()));
    let mut engine = Engine::new(dir.path().into()).unwrap();
    engine
        .register(
            "canvas".into(),
            Box::new(Canvas {
                state: state.clone(),
                tolerant,
            }),
        )
        .unwrap();
    (dir, engine, state)
}
#[test]
fn opted_in_canvas_ignores_only_small_animation_and_rechecks_at_dispatch() {
    let (_dir, mut engine, state) = setup(true);
    let frame = engine.observe("canvas").unwrap().frame;
    let reference = engine
        .resolve("canvas", &frame, Locator::Pixel { x: 80., y: 80. })
        .unwrap();
    assert!(reference.pixel_anchor.is_some());
    state.lock().unwrap().phase = 1;
    let lease = engine.claim("canvas", 5000).unwrap();
    let receipt = engine
        .act(
            "canvas",
            &reference.reference,
            &lease,
            "animated-canvas",
            Intent::Click,
            None,
        )
        .unwrap();
    assert_eq!(receipt.dispatch, "returned");
    assert_eq!(state.lock().unwrap().dispatches, 1);
    engine.release(&lease).unwrap();
}
#[test]
fn moved_subject_and_non_opted_in_backend_remain_strict() {
    for (tolerant, phase) in [(true, 2), (false, 1)] {
        let (dir, mut engine, state) = setup(tolerant);
        let frame = engine.observe("canvas").unwrap().frame;
        let reference = engine
            .resolve("canvas", &frame, Locator::Pixel { x: 80., y: 80. })
            .unwrap();
        assert_eq!(reference.pixel_anchor.is_some(), tolerant);
        state.lock().unwrap().phase = phase;
        let lease = engine.claim("canvas", 5000).unwrap();
        assert_eq!(
            engine
                .act(
                    "canvas",
                    &reference.reference,
                    &lease,
                    "changed-canvas",
                    Intent::Click,
                    None,
                )
                .unwrap_err()
                .code,
            "stale_reference"
        );
        assert_eq!(state.lock().unwrap().dispatches, 0);
        assert_eq!(
            std::fs::read_dir(dir.path().join("receipts"))
                .unwrap()
                .count(),
            0
        );
        engine.release(&lease).unwrap();
    }
}

#[test]
fn explicit_red_marker_mode_tolerates_unrelated_animation_once() {
    let (_dir, mut engine, state) = setup_map(true);
    let frame = engine.observe("map").unwrap().frame;
    let reference = engine
        .resolve("map", &frame, Locator::RedMarkerCluster { x: 80., y: 80. })
        .unwrap();
    assert!(reference.red_marker_anchor.is_some());
    assert!(reference.pixel_anchor.is_none());
    state.lock().unwrap().phase = 1;
    let lease = engine.claim("map", 5000).unwrap();
    let receipt = engine
        .act(
            "map",
            &reference.reference,
            &lease,
            "red-marker-one-shot",
            Intent::Click,
            None,
        )
        .unwrap();
    assert_eq!(receipt.dispatch, "returned");
    assert_eq!(state.lock().unwrap().dispatches, 1);
    engine.release(&lease).unwrap();
}

#[test]
fn explicit_red_marker_mode_refuses_move_loss_or_occlusion_without_reservation() {
    for (phase, expected_reason) in [
        (2, "target_marker_missing_or_moved"),
        (3, "target_marker_missing_or_moved"),
        (4, "target_marker_missing_or_moved"),
    ] {
        let (dir, mut engine, state) = setup_map(true);
        let frame = engine.observe("map").unwrap().frame;
        let reference = engine
            .resolve("map", &frame, Locator::RedMarkerCluster { x: 80., y: 80. })
            .unwrap();
        state.lock().unwrap().phase = phase;
        let lease = engine.claim("map", 5000).unwrap();
        let error = engine
            .act(
                "map",
                &reference.reference,
                &lease,
                &format!("red-marker-refuse-{phase}"),
                Intent::Click,
                None,
            )
            .unwrap_err();
        assert_eq!(error.code, "stale_reference");
        assert_eq!(
            error.diagnostic.as_ref().unwrap()["red_marker_cluster_anchor"]["reason"],
            expected_reason
        );
        assert_eq!(
            error.diagnostic.as_ref().unwrap()["native_input_dispatched"],
            false
        );
        assert_eq!(state.lock().unwrap().dispatches, 0);
        assert_eq!(
            std::fs::read_dir(dir.path().join("receipts"))
                .unwrap()
                .count(),
            0
        );
        engine.release(&lease).unwrap();
    }
}

#[test]
fn red_marker_mode_is_opt_in_and_unavailable_on_a_strict_backend() {
    let (_dir, mut strict, _) = setup_map(false);
    let frame = strict.observe("map").unwrap().frame;
    assert_eq!(
        strict
            .resolve("map", &frame, Locator::RedMarkerCluster { x: 80., y: 80. })
            .unwrap_err()
            .code,
        "red_marker_anchor_unavailable"
    );

    let (_dir, mut engine, state) = setup_map(true);
    let frame = engine.observe("map").unwrap().frame;
    let reference = engine
        .resolve("map", &frame, Locator::Pixel { x: 80., y: 80. })
        .unwrap();
    state.lock().unwrap().phase = 1;
    let lease = engine.claim("map", 5000).unwrap();
    assert_eq!(
        engine
            .act(
                "map",
                &reference.reference,
                &lease,
                "ordinary-pixel-remains-strict",
                Intent::Click,
                None,
            )
            .unwrap_err()
            .code,
        "stale_reference"
    );
    assert_eq!(state.lock().unwrap().dispatches, 0);
    engine.release(&lease).unwrap();
}

#[test]
fn single_evaluation_refusal_decodes_the_same_scene_once_without_reservation() {
    let (dir, mut engine, state) = setup(true);
    let frame = engine.observe("canvas").unwrap().frame;
    let reference = engine
        .resolve("canvas", &frame, Locator::Pixel { x: 80., y: 80. })
        .unwrap();
    state.lock().unwrap().phase = 2;
    let lease = engine.claim("canvas", 5000).unwrap();
    crate::pixel_anchor::reset_feature_decode_count();
    let error = engine
        .act(
            "canvas",
            &reference.reference,
            &lease,
            "single-evaluation-refusal",
            Intent::Click,
            None,
        )
        .unwrap_err();
    assert_eq!(error.code, "stale_reference");
    assert_eq!(crate::pixel_anchor::feature_decode_count(), 1);
    assert_eq!(
        error.diagnostic.as_ref().unwrap()["native_input_dispatched"],
        false
    );
    assert!(error.next_step.contains("Local pixel anchor refused:"));
    assert!(!error.next_step.contains("Focus/layout/content changed"));
    assert_eq!(state.lock().unwrap().dispatches, 0);
    assert_eq!(
        std::fs::read_dir(dir.path().join("receipts"))
            .unwrap()
            .count(),
        0
    );
    engine.release(&lease).unwrap();
}

#[test]
#[ignore = "operator-provided captured files only; no GUI/input or preflight authority"]
fn operator_recorded_pixel_guard_pairs() {
    let params: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::env::var("RWI_GUARD_PAIR_PARAMS").unwrap()).unwrap(),
    )
    .unwrap();
    let before = std::fs::read(params["before"].as_str().unwrap()).unwrap();
    let after = std::fs::read(params["after"].as_str().unwrap()).unwrap();
    let size = [
        params["size"][0].as_u64().unwrap() as u32,
        params["size"][1].as_u64().unwrap() as u32,
    ];
    let transform: [f64; 6] = serde_json::from_value(params["transform"].clone()).unwrap();
    let mut reports = Vec::new();
    for point in params["points"].as_array().unwrap() {
        let pixel: [f64; 2] = serde_json::from_value(point["pixel"].clone()).unwrap();
        let desktop = [
            transform[0] * pixel[0] + transform[2] * pixel[1] + transform[4],
            transform[1] * pixel[0] + transform[3] * pixel[1] + transform[5],
        ];
        let report = PixelAnchor::from_png(&before, size, pixel, desktop)
            .map(|a| a.diagnostic(&after, size, desktop, Some(transform)));
        reports.push(serde_json::json!({"label":point["label"],"pixel":pixel,"diagnostic":report,"input_authority":false}));
    }
    println!(
        "OPERATOR_CAPTURED_PAIR={}",
        serde_json::to_string(&reports).unwrap()
    );
}

#[test]
fn unavailable_anchor_diagnostic_keeps_full_frame_rule_and_zero_dispatch() {
    let (dir, mut engine, state) = setup(true);
    state.lock().unwrap().phase = 5;
    let frame = engine.observe("canvas").unwrap().frame;
    let reference = engine
        .resolve("canvas", &frame, Locator::Pixel { x: 80., y: 80. })
        .unwrap();
    assert!(reference.pixel_anchor.is_none());
    state.lock().unwrap().phase = 6;
    let lease = engine.claim("canvas", 5000).unwrap();
    let error = engine
        .act(
            "canvas",
            &reference.reference,
            &lease,
            "uniform-anchor-rejection",
            Intent::Click,
            None,
        )
        .unwrap_err();
    assert_eq!(error.code, "stale_reference");
    let diagnostic = error.diagnostic.as_ref().unwrap();
    assert_eq!(diagnostic["content_changed"], true);
    assert_eq!(diagnostic["local_pixel_anchor_available"], false);
    assert_eq!(diagnostic["red_marker_anchor_available"], false);
    assert_eq!(diagnostic["pixel_content_policy"], "full_frame_equality");
    assert!(diagnostic.get("local_pixel_anchor").is_none());
    assert_eq!(diagnostic["input_authority"], false);
    assert_eq!(diagnostic["native_input_dispatched"], false);
    assert!(error.next_step.contains("no local anchor"));
    assert_eq!(state.lock().unwrap().dispatches, 0);
    assert_eq!(
        std::fs::read_dir(dir.path().join("receipts"))
            .unwrap()
            .count(),
        0
    );
    engine.release(&lease).unwrap();
}

mod context;
mod creation;
