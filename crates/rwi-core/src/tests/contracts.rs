use super::*;

#[test]
fn metadata_snapshots_and_raw_artifacts_are_separately_bounded() {
    let mut h = Harness::new();
    h.app.0.lock().unwrap().png_bytes = 512 * 1024;
    let first = h.engine.observe("fixture").unwrap();
    for _ in 0..70 {
        h.engine.observe("fixture").unwrap();
    }
    assert_eq!(h.engine.snapshot_count(), 64);
    assert!(h.engine.artifact_bytes() <= ARTIFACT_BUDGET);
    assert!(
        h.engine
            .snapshots
            .iter()
            .all(|(_, s)| s.scene.png.is_empty())
    );
    assert_eq!(
        h.engine
            .artifact(first.artifact_id.as_deref().unwrap(), 0, 1)
            .unwrap_err()
            .code,
        "artifact_expired"
    );
}
#[test]
fn screenshot_size_and_chunk_limits_are_typed() {
    let mut h = Harness::new();
    h.app.0.lock().unwrap().png_bytes = MAX_FRAME_BYTES + 1;
    assert_eq!(
        h.engine.observe("fixture").unwrap_err().code,
        "screenshot_size_limit"
    );
    h.app.0.lock().unwrap().png_bytes = 32;
    let s = h.engine.observe("fixture").unwrap();
    let aid = s.artifact_id.unwrap();
    let (bytes, hash, total) = h.engine.artifact(&aid, 8, 8).unwrap();
    assert_eq!(bytes.len(), 8);
    assert_eq!(total, 32);
    assert_eq!(hash, s.png_sha256.unwrap());
    assert_eq!(
        h.engine
            .artifact(&aid, 0, INLINE_BYTES + 1)
            .unwrap_err()
            .code,
        "invalid_range"
    );
}
#[test]
fn action_invalidates_all_target_aliases_even_when_ack_has_no_effect() {
    let mut h = Harness::new();
    h.app.0.lock().unwrap().ack_without_effect = true;
    let s = h.engine.observe("alias").unwrap();
    let alias = h
        .engine
        .resolve(
            "alias",
            &s.frame,
            Locator::Semantic {
                name: "Settings".into(),
            },
        )
        .unwrap();
    h.action("Settings", Intent::Click, None);
    let l = h.engine.claim("alias", 60000).unwrap();
    assert_eq!(
        h.engine
            .act("alias", &alias.reference, &l, "alias", Intent::Click, None)
            .unwrap_err()
            .code,
        "unknown_reference"
    );
}
#[test]
fn lease_expiry_does_not_overlap_an_in_flight_dispatch_guard() {
    let mut h = Harness::new();
    h.engine.claim("fixture", 100).unwrap();
    let book = h.engine.leases.shared();
    let guard = book.lock().unwrap();
    std::thread::sleep(Duration::from_millis(150));
    let mut other = lease::LeaseBook::new(h.dir.path().into());
    assert_eq!(
        other
            .claim("other", &h.app.identity().key(), 1000)
            .unwrap_err()
            .code,
        "desktop_busy"
    );
    drop(guard);
    std::thread::sleep(Duration::from_millis(30));
    assert!(other.claim("other", &h.app.identity().key(), 1000).is_ok());
}

#[test]
fn aged_proposals_still_require_exact_native_revalidation() {
    for changed in [false, true] {
        let mut h = Harness::new();
        let r = h.resolve("Input");
        h.engine.snapshots.back_mut().unwrap().1.at -= Duration::from_secs(30);
        if changed {
            h.app.0.lock().unwrap().epoch += 1;
        }
        let lease = h.engine.claim("fixture", 60000).unwrap();
        let result = h.engine.act(
            "fixture",
            &r.reference,
            &lease,
            "aged-proposal",
            Intent::Click,
            None,
        );
        if changed {
            assert_eq!(result.unwrap_err().code, "stale_reference");
            assert_eq!(h.app.0.lock().unwrap().calls, 0);
        } else {
            assert!(result.is_ok());
            assert_eq!(h.app.0.lock().unwrap().calls, 1);
        }
        h.engine.release(&lease).unwrap();
    }
}

#[test]
fn display_uuid_target_requires_canonical_identity_and_all_selectors_match() {
    let uuid = "13E4E9B0-47F2-4043-9CDC-878D5441BD41";
    let target = DisplayTarget {
        built_in: Some(false),
        id: Some(7),
        uuid: Some(uuid.to_owned()),
    };
    target.validate().unwrap();
    assert!(target.matches(false, 7, Some(&uuid.to_ascii_lowercase())));
    assert!(!target.matches(false, 8, Some(uuid)));
    assert!(!target.matches(true, 7, Some(uuid)));
    assert!(!target.matches(false, 7, None));
    assert_eq!(
        DisplayTarget {
            built_in: None,
            id: None,
            uuid: Some("not-a-uuid".into()),
        }
        .validate()
        .unwrap_err()
        .code,
        "invalid_display_target"
    );
}

#[test]
fn native_action_revision_ignores_only_pointer_hints_and_preserves_semantic_identity() {
    let mut h = Harness::new();
    let scene = h.app.observe().unwrap();
    let base = scene.elements[3].clone();
    let mut moved_hit = base.clone();
    moved_hit.visible = Some(false);
    moved_hit.occluded = Some(true);
    assert_ne!(base.revision(), moved_hit.revision());
    assert_eq!(
        base.action_revision(Some("AXPress")),
        moved_hit.action_revision(Some("AXPress"))
    );
    for change in 0..8 {
        let mut altered = base.clone();
        match change {
            0 => altered.id = "replacement".into(),
            1 => altered.point[0] += 1.,
            2 => altered.enabled = Some(false),
            3 => altered.actions.push("withdrawn".into()),
            4 => altered.selected = Some(true),
            5 => altered.value = Some("new-value".into()),
            6 => altered.state_revision = Some("changed".into()),
            _ => altered.name = "different target".into(),
        }
        assert_ne!(
            base.action_revision(Some("AXPress")),
            altered.action_revision(Some("AXPress"))
        );
    }
}

#[test]
fn hover_contract_move_pointer_has_no_value_and_no_click_semantics() {
    let intent: Intent = serde_json::from_str("\"move_pointer\"")
        .expect("hover requires distinct move_pointer intent");
    assert!(intent.validate(None).is_ok());
    for value in ["", "AXPress", "100"] {
        assert_eq!(
            intent.validate(Some(value)).unwrap_err().code,
            "invalid_input_value"
        );
    }
    let mut h = Harness::new();
    let receipt = h.action("Settings", intent, None);
    assert!(!receipt.goal_verified);
    assert!(
        !h.app.0.lock().unwrap().dialog,
        "hover must not click Settings"
    );
}
