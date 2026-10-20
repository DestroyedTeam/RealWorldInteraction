//! Synthetic Core/Backend contract only: no macOS, real application or input post.
use super::*;

#[derive(Default)]
struct BlankState {
    phase: u8,
    dispatches: usize,
    change_on_dispatch: bool,
}
struct BlankCanvas {
    state: Arc<Mutex<BlankState>>,
    opted_in: bool,
}
fn panel_image(phase: u8) -> Vec<u8> {
    let mut rgb = vec![255; 768 * 768 * 3];
    for y in 300..468 {
        for x in 300..468 {
            if !(306..462).contains(&x) || !(306..462).contains(&y) {
                let i = (y * 768 + x) * 3;
                rgb[i..i + 3].fill(20);
            }
        }
    }
    if phase != 0 {
        rgb[..3].fill(0);
    }
    if phase == 2 {
        let i = (384 * 768 + 384) * 3;
        rgb[i..i + 3].fill(0);
    }
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, 768, 768);
        e.set_color(png::ColorType::Rgb);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&rgb).unwrap();
    }
    out
}
impl Backend for BlankCanvas {
    fn local_pixel_anchor(&self) -> bool {
        self.opted_in
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
        serde_json::json!({"adapter":"synthetic-blank-context-not-native"})
    }
    fn observe(&mut self) -> Result<Scene> {
        let phase = self.state.lock().unwrap().phase;
        let png = panel_image(phase);
        Ok(Scene {
            identity: self.identity(),
            assurance: "synthetic".into(),
            binding: Some(Binding {
                scope: Scope {
                    window: "blank-window".into(),
                    pid: if phase == 5 { 43 } else { 42 },
                    element: None,
                },
                focus_window: "blank-window".into(),
                layout: "fixed".into(),
                content: digest(&png),
                native_epoch: if phase == 4 { "changed" } else { "fixed" }.into(),
                mutation_context: None,
            }),
            mutation_ready: phase != 6,
            blocked: Some(phase == 3),
            image_size: [768, 768],
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
        if self.state.lock().unwrap().change_on_dispatch {
            self.state.lock().unwrap().phase = 2;
        }
        if !grounding_matches(expected, &self.observe()?, target) {
            return Err(fault(
                "stale_reference",
                "Synthetic second revalidation refused; never replay.",
            ));
        }
        self.state.lock().unwrap().dispatches += 1;
        Ok(Some(true))
    }
    fn inspect(&mut self, _: &Scope, _: &Predicate, _: &str) -> Result<Oracle> {
        Err(fault("unavailable", "Not an oracle test."))
    }
}
fn setup_blank(opted_in: bool) -> (tempfile::TempDir, Engine, Arc<Mutex<BlankState>>) {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(Mutex::new(BlankState::default()));
    let mut e = Engine::new(dir.path().to_owned()).unwrap();
    e.register(
        "blank".into(),
        Box::new(BlankCanvas {
            state: state.clone(),
            opted_in,
        }),
    )
    .unwrap();
    (dir, e, state)
}
fn reference(e: &mut Engine) -> Resolved {
    let frame = e.observe("blank").unwrap().frame;
    e.resolve("blank", &frame, Locator::Pixel { x: 384., y: 384. })
        .unwrap()
}
#[test]
fn f400_engine_blank_point_remote_change_dispatches_once_not_replay() {
    let (_d, mut e, s) = setup_blank(true);
    let r = reference(&mut e);
    assert!(r.pixel_anchor.is_some());
    s.lock().unwrap().phase = 1;
    let lease = e.claim("blank", 5000).unwrap();
    let a = e
        .act(
            "blank",
            &r.reference,
            &lease,
            "f400-blank-once",
            Intent::Click,
            None,
        )
        .unwrap();
    assert!(!a.deduplicated);
    assert_eq!(s.lock().unwrap().dispatches, 1);
    let b = e
        .act(
            "blank",
            &r.reference,
            &lease,
            "f400-blank-once",
            Intent::Click,
            None,
        )
        .unwrap();
    assert!(b.deduplicated);
    assert_eq!(s.lock().unwrap().dispatches, 1);
    e.release(&lease).unwrap();
    println!(
        "F400_SYNTHETIC_CORE_CONTEXT=remote_change_allowed,backend_dispatches=1,deduplicated=1,native_input=none"
    );
}
#[test]
fn f400_engine_local_replacement_refused_before_reservation() {
    let (d, mut e, s) = setup_blank(true);
    let r = reference(&mut e);
    s.lock().unwrap().phase = 2;
    let l = e.claim("blank", 5000).unwrap();
    let err = e
        .act(
            "blank",
            &r.reference,
            &l,
            "f400-local-refused",
            Intent::Click,
            None,
        )
        .unwrap_err();
    assert_eq!(err.code, "stale_reference");
    assert_eq!(s.lock().unwrap().dispatches, 0);
    assert_eq!(
        std::fs::read_dir(d.path().join("receipts"))
            .unwrap()
            .count(),
        0
    );
    e.release(&l).unwrap();
}
#[test]
fn f400_engine_backend_opt_out_keeps_full_frame_equality() {
    let (_d, mut e, s) = setup_blank(false);
    let r = reference(&mut e);
    assert!(r.pixel_anchor.is_none());
    s.lock().unwrap().phase = 1;
    let l = e.claim("blank", 5000).unwrap();
    assert_eq!(
        e.act(
            "blank",
            &r.reference,
            &l,
            "f400-no-optin",
            Intent::Click,
            None
        )
        .unwrap_err()
        .code,
        "stale_reference"
    );
    assert_eq!(s.lock().unwrap().dispatches, 0);
    e.release(&l).unwrap();
}
#[test]
fn f400_engine_modal_epoch_scope_and_readiness_guards_remain() {
    for phase in [3, 4, 5, 6] {
        let (_d, mut e, s) = setup_blank(true);
        let r = reference(&mut e);
        s.lock().unwrap().phase = phase;
        let l = e.claim("blank", 5000).unwrap();
        assert!(
            e.act(
                "blank",
                &r.reference,
                &l,
                &format!("f400-guard-{phase}"),
                Intent::Click,
                None
            )
            .is_err()
        );
        assert_eq!(s.lock().unwrap().dispatches, 0);
        e.release(&l).unwrap();
    }
}
#[test]
fn f400_engine_second_revalidation_cannot_fall_back_after_context_changes() {
    let (_d, mut e, s) = setup_blank(true);
    let r = reference(&mut e);
    {
        let mut x = s.lock().unwrap();
        x.phase = 1;
        x.change_on_dispatch = true;
    }
    let l = e.claim("blank", 5000).unwrap();
    let receipt = e
        .act(
            "blank",
            &r.reference,
            &l,
            "f400-second-refused",
            Intent::Click,
            None,
        )
        .unwrap();
    // The backend boundary has been entered: preserve its actual uncertain
    // receipt, not a fabricated pre-dispatch Err or a safe-to-retry result.
    assert_eq!(receipt.dispatch, "uncertain");
    assert_eq!(
        receipt.dispatch_error.as_ref().unwrap().code,
        "stale_reference"
    );
    assert_eq!(receipt.replay, "forbidden");
    assert!(!receipt.goal_verified);
    assert!(receipt.system_accepted.is_none());
    assert_eq!(s.lock().unwrap().dispatches, 0);
    e.release(&l).unwrap();
}
