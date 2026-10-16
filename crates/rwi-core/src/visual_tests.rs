use super::*;
use crate::{
    Backend, Binding, Intent, Locator, Oracle, Predicate, Resolved, Scene, Scope, fixture::Fixture,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
#[derive(Default)]
struct Driver {
    fixture: Fixture,
    observations: Arc<AtomicUsize>,
    visuals: Arc<AtomicUsize>,
    adjust: Option<fn(&mut VisualCapture)>,
    cleanup_error: bool,
    cancel: Option<operation::Operation>,
}
impl Backend for Driver {
    fn identity(&self) -> Identity {
        self.fixture.identity()
    }
    fn capabilities(&self) -> Value {
        self.fixture.capabilities()
    }
    fn observe(&mut self) -> Result<Scene> {
        self.observations.fetch_add(1, Ordering::Relaxed);
        self.fixture.observe()
    }
    fn capture_visual(&mut self, options: VisualOptions) -> Result<VisualCapture> {
        self.visuals.fetch_add(1, Ordering::Relaxed);
        let mut v = self.fixture.capture_visual(options)?;
        if let Some(adjust) = self.adjust {
            adjust(&mut v);
        }
        if self.cleanup_error {
            operation::resource_release_uncertain();
        }
        if let Some(op) = &self.cancel {
            op.cancel();
        }
        Ok(v)
    }
    fn act(
        &mut self,
        b: &Binding,
        r: &Resolved,
        i: &Intent,
        v: Option<&str>,
    ) -> Result<Option<bool>> {
        self.fixture.act(b, r, i, v)
    }
    fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> Result<Oracle> {
        self.fixture.inspect(s, p, e)
    }
}
fn engine(d: Driver) -> (tempfile::TempDir, Engine) {
    let dir = tempfile::tempdir().unwrap();
    let mut e = Engine::new(dir.path().into()).unwrap();
    e.register("selected".into(), Box::new(d)).unwrap();
    (dir, e)
}
#[test]
fn visual_does_not_capture_scene_or_create_refresh_or_evict_grounding_state() {
    let d = Driver::default();
    let observes = d.observations.clone();
    let visuals = d.visuals.clone();
    let fixture = d.fixture.clone();
    let (_dir, mut e) = engine(d);
    let old = e.observe("selected").unwrap();
    let r = e
        .resolve(
            "selected",
            &old.frame,
            Locator::Element { id: "Input".into() },
        )
        .unwrap();
    let before_at = e.snapshots.front().unwrap().1.at;
    let before = (e.snapshot_count(), e.artifact_bytes(), e.refs.len());
    fixture.0.lock().unwrap().ready = false; // Observation is not input authority.
    for _ in 0..70 {
        let visual = e
            .capture_visual("selected", VisualOptions::default())
            .unwrap();
        assert!(visual.visual_id.starts_with("visual:"));
        assert!(!visual.frame_created && !visual.input_authority_granted && !visual.goal_verified);
        assert!(
            e.resolve(
                "selected",
                &visual.visual_id,
                Locator::Pixel { x: 0., y: 0. }
            )
            .is_err()
        );
        assert!(e.artifact(&visual.visual_id, 0, 64).is_err());
        assert_eq!(visual.capture.source_rect.x, -1920);
    }
    assert_eq!(
        before,
        (e.snapshot_count(), e.artifact_bytes(), e.refs.len())
    );
    assert_eq!(before_at, e.snapshots.front().unwrap().1.at);
    assert_eq!(e.refs.front().unwrap().reference, r.reference);
    assert_eq!(
        (
            observes.load(Ordering::Relaxed),
            visuals.load(Ordering::Relaxed)
        ),
        (1, 70)
    );
    assert_eq!(fixture.0.lock().unwrap().calls, 0);
}
#[test]
fn invalid_options_are_rejected_before_the_backend_and_include_is_not_exclusion() {
    let d = Driver::default();
    let calls = d.visuals.clone();
    let (_dir, mut e) = engine(d);
    for edge in [0, 63, 2049, u32::MAX] {
        assert_eq!(
            e.capture_visual(
                "selected",
                VisualOptions {
                    max_edge: edge,
                    ..Default::default()
                }
            )
            .unwrap_err()
            .code,
            "invalid_visual_limit"
        );
    }
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    let result = e
        .capture_visual(
            "selected",
            VisualOptions {
                feedback: VisualFeedbackMode::Include,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        result.capture.feedback.requested,
        VisualFeedbackMode::Include
    );
    assert_eq!(result.capture.feedback.suspension_ticket, None);
    assert_eq!(result.capture.image_size, [1, 1]);
}
#[test]
fn visual_metadata_limit_includes_the_outer_target_and_result_envelope() {
    let (_dir, mut e) = engine(Driver::default());
    let oversized_target = "t".repeat(MAX_VISUAL_METADATA_BYTES + 1);
    e.register(oversized_target.clone(), Box::new(Driver::default()))
        .unwrap();
    assert_eq!(
        e.capture_visual(&oversized_target, VisualOptions::default())
            .unwrap_err()
            .code,
        "visual_metadata_budget"
    );
    assert_eq!(
        (e.snapshot_count(), e.artifact_bytes(), e.refs.len()),
        (0, 0, 0)
    );
}
#[test]
fn identity_source_output_metadata_png_and_mapping_budgets_fail_closed() {
    type Case = (fn(&mut VisualCapture), &'static str);
    let cases: [Case; 10] = [
        (
            |v| v.identity.session = "foreign".into(),
            "session_mismatch",
        ),
        (|v| v.source_rect.width = 17_000_000, "visual_source_budget"),
        (|v| v.source_pixel_size = [0, 1], "visual_source_budget"),
        (|v| v.source_pixel_size = [2, 2], "visual_output_budget"),
        (|v| v.image_size = [2049, 2049], "visual_output_budget"),
        (|v| v.visual_to_desktop[4] = 0., "visual_mapping_mismatch"),
        (
            |v| v.png = vec![0; MAX_VISUAL_PNG_BYTES + 1],
            "visual_transport_budget",
        ),
        (
            |v| v.png[16..20].copy_from_slice(&2u32.to_be_bytes()),
            "invalid_visual_png",
        ),
        (
            |v| v.details = Value::String("a".repeat(MAX_VISUAL_METADATA_BYTES)),
            "visual_metadata_budget",
        ),
        (
            |v| v.feedback.requested = VisualFeedbackMode::Include,
            "invalid_visual_capture",
        ),
    ];
    for (adjust, expected) in cases {
        let (_dir, mut e) = engine(Driver {
            adjust: Some(adjust),
            ..Default::default()
        });
        assert_eq!(
            e.capture_visual("selected", VisualOptions::default())
                .unwrap_err()
                .code,
            expected
        );
        assert_eq!(
            (e.snapshot_count(), e.artifact_bytes(), e.refs.len()),
            (0, 0, 0)
        );
    }
}
#[test]
fn negative_pixel_center_mapping_and_valid_png_signature_are_required() {
    let mut v = Fixture::default()
        .capture_visual(VisualOptions::default())
        .unwrap();
    v.source_rect = VisualRect {
        x: -3000,
        y: -1200,
        width: 2,
        height: 1,
    };
    v.source_pixel_size = [2, 1];
    v.image_size = [2, 1];
    v.visual_to_desktop = [1., 0., 0., 1., -3000., -1200.];
    v.png[16..20].copy_from_slice(&2u32.to_be_bytes());
    validate_capture(&v, VisualOptions::default()).unwrap();
    v.png.truncate(33);
    assert_eq!(
        validate_capture(&v, VisualOptions::default())
            .unwrap_err()
            .code,
        "invalid_visual_png"
    );
}
#[test]
fn retina_visual_uses_native_pixel_extent_not_logical_point_extent() {
    let mut v = Fixture::default()
        .capture_visual(VisualOptions::default())
        .unwrap();
    v.source_rect = VisualRect {
        x: 1536,
        y: 24,
        width: 1434,
        height: 660,
    };
    v.source_pixel_size = [2868, 1320];
    v.image_size = [2048, 942];
    let sx = 1434.0 / 2048.0;
    let sy = 660.0 / 942.0;
    v.visual_to_desktop = [
        sx,
        0.,
        0.,
        sy,
        1536.0 + (sx - 1.0) / 2.0,
        24.0 + (sy - 1.0) / 2.0,
    ];
    v.png[16..20].copy_from_slice(&2048u32.to_be_bytes());
    v.png[20..24].copy_from_slice(&942u32.to_be_bytes());
    validate_capture(
        &v,
        VisualOptions {
            max_edge: 2048,
            ..Default::default()
        },
    )
    .unwrap();
    // Even on Retina, a response larger than native source pixels is not
    // accepted merely because it fits the public 2048-pixel output budget.
    v.source_pixel_size = [1434, 660];
    assert_eq!(
        validate_capture(
            &v,
            VisualOptions {
                max_edge: 2048,
                ..Default::default()
            }
        )
        .unwrap_err()
        .code,
        "visual_output_budget"
    );
}
#[test]
fn legacy_visual_wire_can_decode_without_claiming_an_unknown_native_pixel_size() {
    let v = Fixture::default()
        .capture_visual(VisualOptions::default())
        .unwrap();
    let mut wire = serde_json::to_value(v).unwrap();
    wire.as_object_mut().unwrap().remove("source_pixel_size");
    let legacy: VisualCapture = serde_json::from_value(wire).unwrap();
    assert_eq!(legacy.source_pixel_size, [0, 0]);
    assert_eq!(
        validate_capture(&legacy, VisualOptions::default())
            .unwrap_err()
            .code,
        "visual_source_budget"
    );
}
#[test]
fn cancellation_and_cleanup_failure_return_no_visual_or_retained_authority() {
    let op = operation::Registry::default().begin(None).unwrap();
    let (_dir, mut e) = engine(Driver {
        cancel: Some(op.clone()),
        ..Default::default()
    });
    assert_eq!(
        operation::run(&op, || e
            .capture_visual("selected", VisualOptions::default()))
        .unwrap_err()
        .code,
        "operation_cancelled"
    );
    assert_eq!(e.snapshot_count(), 0);
    let (_dir, mut e) = engine(Driver {
        cleanup_error: true,
        ..Default::default()
    });
    assert_eq!(
        e.capture_visual("selected", VisualOptions::default())
            .unwrap_err()
            .code,
        "native_resource_release_uncertain"
    );
    assert_eq!(e.artifact_bytes(), 0);
}
#[test]
fn public_catalog_is_native_execution_with_strict_readonly_arguments() {
    use crate::device::{api::CaptureVisualArgs, catalog};
    let args: CaptureVisualArgs =
        serde_json::from_value(serde_json::json!({"target":"selected"})).unwrap();
    assert_eq!(
        (args.max_edge, args.feedback),
        (1024, VisualFeedbackMode::Exclude)
    );
    for bad in [
        serde_json::json!({"target":"selected","feedback":"force_include"}),
        serde_json::json!({"target":"selected","lease":"not-permitted"}),
    ] {
        assert!(catalog::validate("computer.capture_visual", &bad).is_err());
    }
    assert!(catalog::executable("computer.capture_visual"));
    assert!(!catalog::cached_read("computer.capture_visual"));
    let definition = catalog::definitions()
        .into_iter()
        .find(|d| d.name == "computer.capture_visual")
        .unwrap();
    assert!(!definition.writes);
    assert_eq!(
        definition.input_schema["properties"]["max_edge"]["minimum"],
        64
    );
    assert_eq!(
        definition.input_schema["properties"]["max_edge"]["maximum"],
        2048
    );
}

#[test]
fn backend_default_does_not_fallback_to_scene_observation() {
    struct Unsupported;
    impl Backend for Unsupported {
        fn identity(&self) -> Identity {
            Fixture::default().identity()
        }
        fn capabilities(&self) -> Value {
            Value::Null
        }
        fn observe(&mut self) -> Result<Scene> {
            panic!("visual must not call observe")
        }
        fn act(
            &mut self,
            _: &Binding,
            _: &Resolved,
            _: &Intent,
            _: Option<&str>,
        ) -> Result<Option<bool>> {
            unreachable!()
        }
        fn inspect(&mut self, _: &Scope, _: &Predicate, _: &str) -> Result<Oracle> {
            unreachable!()
        }
    }
    assert_eq!(
        Unsupported
            .capture_visual(VisualOptions::default())
            .unwrap_err()
            .code,
        "capability_unavailable"
    );
}
