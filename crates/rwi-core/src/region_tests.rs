//! Synthetic public Engine tests: no screen, native input or performance claims.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

struct Regional {
    fixture: fixture::Fixture,
    calls: Arc<AtomicU64>,
    ignore_source: bool,
}
impl Regional {
    fn scene(&mut self, region: Option<[u32; 4]>) -> Result<Scene> {
        let mut scene = self.fixture.observe()?;
        if let Some([x, y, width, height]) = region {
            let [a, b, c, d, e, f] = scene.pixel_to_desktop.unwrap();
            scene.pixel_to_desktop = Some([
                a,
                b,
                c,
                d,
                e + a * f64::from(x) + c * f64::from(y),
                f + b * f64::from(x) + d * f64::from(y),
            ]);
            scene.image_size = [width, height];
            scene
                .binding
                .as_mut()
                .unwrap()
                .layout
                .push_str(&format!(";roi:{region:?}"));
        }
        Ok(scene)
    }
}
impl Backend for Regional {
    fn identity(&self) -> Identity {
        self.fixture.identity()
    }
    fn capabilities(&self) -> serde_json::Value {
        self.fixture.capabilities()
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        Some(self.calls.load(Ordering::SeqCst))
    }
    fn observe(&mut self) -> Result<Scene> {
        self.scene(None)
    }
    fn observe_region(&mut self, region: [u32; 4]) -> Result<Scene> {
        self.scene(Some(region))
    }
    fn revalidate(&mut self, reference: &Resolved) -> Result<Scene> {
        self.scene(if self.ignore_source {
            None
        } else {
            reference.source_region
        })
    }
    fn act(
        &mut self,
        expected: &Binding,
        reference: &Resolved,
        _: &Intent,
        _: Option<&str>,
    ) -> Result<Option<bool>> {
        let current = self.revalidate(reference)?;
        if !grounding_matches(expected, &current, reference) {
            return Err(fault("stale_reference", "Synthetic source region changed."));
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Some(true))
    }
    fn inspect(&mut self, scope: &Scope, predicate: &Predicate, expected: &str) -> Result<Oracle> {
        self.fixture.inspect(scope, predicate, expected)
    }
}
fn setup(ignore_source: bool) -> (tempfile::TempDir, Engine, fixture::Fixture, Arc<AtomicU64>) {
    let dir = tempfile::tempdir().unwrap();
    let fixture = fixture::Fixture::default();
    let calls = Arc::new(AtomicU64::new(0));
    let mut engine = Engine::new(dir.path().into()).unwrap();
    engine
        .register(
            "region".into(),
            Box::new(Regional {
                fixture: fixture.clone(),
                calls: calls.clone(),
                ignore_source,
            }),
        )
        .unwrap();
    (dir, engine, fixture, calls)
}
#[test]
fn each_reference_keeps_its_own_roi_across_intervening_full_observations() {
    let (_dir, mut engine, _, calls) = setup(false);
    let region = [12, 20, 100, 80];
    let observation = engine.observe_with_region("region", Some(region)).unwrap();
    let r = engine
        .resolve(
            "region",
            &observation.frame,
            Locator::Pixel { x: 50., y: 40. },
        )
        .unwrap();
    assert_eq!(r.source_region, Some(region));
    assert_eq!(r.point, [-1889., 30.]);
    let semantic = engine
        .resolve(
            "region",
            &observation.frame,
            Locator::Element {
                id: observation.scene.elements[0].id.clone(),
            },
        )
        .unwrap();
    assert_eq!(semantic.source_region, Some(region));
    let full = engine.observe("region").unwrap();
    let full_ref = engine
        .resolve("region", &full.frame, Locator::Pixel { x: 50., y: 40. })
        .unwrap();
    assert_eq!(full_ref.source_region, None);
    assert!(
        serde_json::to_value(&full_ref)
            .unwrap()
            .get("source_region")
            .is_none()
    );
    let lease = engine.claim("region", 5000).unwrap();
    let result = engine
        .act(
            "region",
            &r.reference,
            &lease,
            "roi-action",
            Intent::Click,
            None,
        )
        .unwrap();
    assert_eq!(result.system_accepted, Some(true));
    assert!(!result.goal_verified);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    engine.release(&lease).unwrap();
}
#[test]
fn substituting_full_capture_or_changed_content_still_stops_before_input() {
    for ignore_source in [true, false] {
        let (_dir, mut engine, fixture, calls) = setup(ignore_source);
        let o = engine
            .observe_with_region("region", Some([12, 20, 100, 80]))
            .unwrap();
        let r = engine
            .resolve("region", &o.frame, Locator::Pixel { x: 50., y: 40. })
            .unwrap();
        if !ignore_source {
            fixture.0.lock().unwrap().pixel_epoch += 1;
        }
        let lease = engine.claim("region", 5000).unwrap();
        assert!(
            engine
                .act(
                    "region",
                    &r.reference,
                    &lease,
                    "bad-region",
                    Intent::Click,
                    None
                )
                .is_err()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        engine.release(&lease).unwrap();
    }
}
#[test]
fn action_schema_does_not_accept_a_caller_selected_source_region() {
    let mut args = serde_json::json!({"target":"region","reference":"r","lease":"l","request_id":"id","intent":"click","source_region":[0,0,1,1]});
    assert!(serde_json::from_value::<device::api::ActArgs>(args.clone()).is_err());
    args.as_object_mut().unwrap().remove("source_region");
    assert!(serde_json::from_value::<device::api::ActArgs>(args).is_ok());
}

#[test]
fn checked_semantic_refresh_retains_the_original_region_and_default_refs_roundtrip() {
    let (_dir, mut engine, _, calls) = setup(false);
    let o = engine
        .observe_with_region("region", Some([12, 20, 100, 80]))
        .unwrap();
    let element = o.scene.elements[0].id.clone();
    let lease = engine.claim("region", 5000).unwrap();
    let receipt = engine
        .act_checked(CheckedAction {
            target: "region",
            frame: &o.frame,
            element: &element,
            lease: &lease,
            request: "roi-checked",
            intent: Intent::Click,
            value: None,
        })
        .unwrap();
    assert_eq!(receipt.system_accepted, Some(true));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    engine.release(&lease).unwrap();
    let full = engine.observe("region").unwrap();
    let r = engine
        .resolve("region", &full.frame, Locator::Pixel { x: 1., y: 1. })
        .unwrap();
    let json = serde_json::to_value(&r).unwrap();
    assert!(json.get("source_region").is_none());
    assert_eq!(
        serde_json::from_value::<Resolved>(json)
            .unwrap()
            .source_region,
        None
    );
}
