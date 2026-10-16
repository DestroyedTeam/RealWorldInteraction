use super::*;

#[test]
fn anchor_creation_reason_retains_luma_failure_without_dispatch_or_new_authority() {
    let (dir, mut engine, state) = setup(true);
    // Test-only original uniform pixels; this does not diagnose a real Game.
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
            "creation-reason-luma",
            Intent::Click,
            None,
        )
        .unwrap_err();
    assert_eq!(error.code, "stale_reference");
    let diagnostic = error.diagnostic.as_ref().unwrap();
    assert_eq!(diagnostic["pixel_content_policy"], "full_frame_equality");
    assert_eq!(diagnostic["local_pixel_anchor_available"], false);
    assert_eq!(diagnostic["native_input_dispatched"], false);
    assert_eq!(diagnostic["input_authority"], false);
    assert_eq!(state.lock().unwrap().dispatches, 0);
    assert_eq!(
        std::fs::read_dir(dir.path().join("receipts"))
            .unwrap()
            .count(),
        0
    );
    println!("ANCHOR_CREATION_REASON_FIXTURE={diagnostic}");
    assert_eq!(
        diagnostic["local_anchor_creation_reason"],
        "luma_span_below_minimum"
    );
    assert!(
        error
            .next_step
            .contains("no local anchor (luma_span_below_minimum)")
    );
    assert!(error.next_step.contains("no input sent, never replay"));
    engine.release(&lease).unwrap();
}

#[test]
fn anchor_creation_reason_distinguishes_backend_and_missing_retained_source() {
    for (case, expected) in [
        (0, "backend_not_opted_in"),
        (1, "source_artifact_absent"),
        (2, "source_artifact_evicted"),
    ] {
        let (dir, mut engine, state) = setup(case != 0);
        let frame = engine.observe("canvas").unwrap().frame;
        if case == 1 {
            // Controlled test-only source absence. Do not invent this reason
            // for a historical reference whose construction was not recorded.
            engine
                .snapshots
                .iter_mut()
                .find(|(id, _)| id == &frame)
                .unwrap()
                .1
                .artifact_id = None;
        } else if case == 2 {
            engine.artifacts.clear();
        }
        let reference = engine
            .resolve("canvas", &frame, Locator::Pixel { x: 80., y: 80. })
            .unwrap();
        assert!(reference.pixel_anchor.is_none());
        assert!(
            serde_json::to_value(&reference)
                .unwrap()
                .get("pixel_anchor_creation_reason")
                .is_none()
        );
        state.lock().unwrap().phase = 1;
        let lease = engine.claim("canvas", 5000).unwrap();
        let error = engine
            .act(
                "canvas",
                &reference.reference,
                &lease,
                &format!("creation-reason-source-{case}"),
                Intent::Click,
                None,
            )
            .unwrap_err();
        let diagnostic = error.diagnostic.as_ref().unwrap();
        assert_eq!(error.code, "stale_reference");
        assert_eq!(diagnostic["local_anchor_creation_reason"], expected);
        assert_eq!(diagnostic["pixel_content_policy"], "full_frame_equality");
        assert_eq!(diagnostic["native_input_dispatched"], false);
        assert_eq!(diagnostic["input_authority"], false);
        assert!(error.next_step.contains(expected));
        assert!(error.next_step.contains("Inspect the source snapshot or"));
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
fn anchor_creation_reason_is_not_fabricated_for_legacy_nonpixel_or_success_refs() {
    let (_dir, mut engine, _) = setup(true);
    let frame = engine.observe("canvas").unwrap().frame;
    let window = engine.resolve("canvas", &frame, Locator::Window).unwrap();
    let pixel = engine
        .resolve("canvas", &frame, Locator::Pixel { x: 80., y: 80. })
        .unwrap();
    assert!(pixel.pixel_anchor.is_some());
    for reference in [&window.reference, &pixel.reference] {
        assert!(
            engine
                .refs
                .iter()
                .find(|entry| &entry.reference == reference)
                .unwrap()
                .pixel_anchor_creation_reason
                .is_none()
        );
    }
    let (_dir, mut marker_engine, _) = setup_map(true);
    let frame = marker_engine.observe("map").unwrap().frame;
    marker_engine
        .resolve("map", &frame, Locator::RedMarkerCluster { x: 80., y: 80. })
        .unwrap();
    assert!(
        marker_engine
            .refs
            .back()
            .unwrap()
            .pixel_anchor_creation_reason
            .is_none()
    );

    let dir = tempfile::tempdir().unwrap();
    let mut semantic_engine = Engine::new(dir.path().into()).unwrap();
    semantic_engine
        .register(
            "semantic".into(),
            Box::new(crate::fixture::Fixture::default()),
        )
        .unwrap();
    let frame = semantic_engine.observe("semantic").unwrap().frame;
    semantic_engine
        .resolve("semantic", &frame, Locator::Element { id: "Input".into() })
        .unwrap();
    assert!(
        semantic_engine
            .refs
            .back()
            .unwrap()
            .pixel_anchor_creation_reason
            .is_none()
    );

    let (_dir, mut legacy, state) = setup(true);
    state.lock().unwrap().phase = 5;
    let frame = legacy.observe("canvas").unwrap().frame;
    let reference = legacy
        .resolve("canvas", &frame, Locator::Pixel { x: 80., y: 80. })
        .unwrap();
    // Simulate an old unknown record only; matching and authority are unchanged.
    legacy.refs.back_mut().unwrap().pixel_anchor_creation_reason = None;
    state.lock().unwrap().phase = 6;
    let lease = legacy.claim("canvas", 5000).unwrap();
    let error = legacy
        .act(
            "canvas",
            &reference.reference,
            &lease,
            "creation-reason-legacy-unknown",
            Intent::Click,
            None,
        )
        .unwrap_err();
    assert_eq!(
        error.diagnostic.unwrap()["local_anchor_creation_reason"],
        "not_recorded"
    );
    assert_eq!(
        error.next_step,
        "Pixel reference has no local anchor; full-frame content changed. Inspect a fresh stable textured surface; no input sent."
    );
    assert_eq!(state.lock().unwrap().dispatches, 0);
    legacy.release(&lease).unwrap();
}

#[test]
fn anchor_creation_reason_retention_is_bounded_and_retires_with_reference() {
    let (_dir, mut engine, state) = setup(true);
    state.lock().unwrap().phase = 5;
    let frame = engine.observe("canvas").unwrap().frame;
    let first = engine
        .resolve("canvas", &frame, Locator::Pixel { x: 80., y: 80. })
        .unwrap();
    for _ in 0..300 {
        engine
            .resolve("canvas", &frame, Locator::Pixel { x: 80., y: 80. })
            .unwrap();
    }
    assert_eq!(engine.refs.len(), 256);
    assert!(
        !engine
            .refs
            .iter()
            .any(|entry| entry.reference == first.reference)
    );
    assert!(
        engine
            .refs
            .iter()
            .all(|entry| entry.pixel_anchor_creation_reason == Some("luma_span_below_minimum"))
    );
    let reference = engine.refs.back().unwrap().reference.clone();
    let lease = engine.claim("canvas", 5000).unwrap();
    // The unchanged synthetic full frame remains valid, even without an anchor.
    // This is fixture input only; a recorded reason creates no new authority.
    let receipt = engine
        .act(
            "canvas",
            &reference,
            &lease,
            "creation-reason-retire",
            Intent::Click,
            None,
        )
        .unwrap();
    assert_eq!(receipt.dispatch, "returned");
    assert_eq!(state.lock().unwrap().dispatches, 1);
    assert!(engine.refs.is_empty());
    engine.release(&lease).unwrap();
}
