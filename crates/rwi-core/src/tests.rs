use super::*;
use fixture::Fixture;
#[test]
fn cancelled_operation_never_enters_durable_reservation_or_dispatch() {
    let mut h = Harness::new();
    let r = h.resolve("Settings");
    let lease = h.engine.claim("fixture", 60000).unwrap();
    let op = operation::Registry::default().begin(None).unwrap();
    op.cancel();
    let result = operation::run(&op, || {
        h.engine.act(
            "fixture",
            &r.reference,
            &lease,
            "cancel-before-reserve",
            Intent::Click,
            None,
        )
    });
    assert_eq!(result.unwrap_err().code, "operation_cancelled");
    assert_eq!(
        std::fs::read_dir(h.dir.path().join("receipts"))
            .unwrap()
            .count(),
        0
    );
    h.engine.release(&lease).unwrap();
}
#[test]
fn explicit_element_locator_binds_native_id_not_similar_name() {
    let mut h = Harness::new();
    let observation = h.engine.observe("fixture").unwrap();
    let element = observation.scene.elements.first().unwrap();
    let r = h
        .engine
        .resolve(
            "fixture",
            &observation.frame,
            Locator::Element {
                id: element.id.clone(),
            },
        )
        .unwrap();
    assert_eq!(r.scope.element.as_deref(), Some(element.id.as_str()));
    assert!(
        h.engine
            .resolve(
                "fixture",
                &observation.frame,
                Locator::Element {
                    id: "not-in-frame".into()
                }
            )
            .is_err()
    );
}

struct Harness {
    dir: tempfile::TempDir,
    engine: Engine,
    app: Fixture,
}
impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let app = Fixture::default();
        let mut engine = Engine::new(dir.path().into()).unwrap();
        engine
            .register("fixture".into(), Box::new(app.clone()))
            .unwrap();
        engine
            .register("alias".into(), Box::new(app.clone()))
            .unwrap();
        Self { dir, engine, app }
    }
    fn resolve(&mut self, name: &str) -> Resolved {
        let shot = self.engine.observe("fixture").unwrap();
        self.engine
            .resolve(
                "fixture",
                &shot.frame,
                Locator::Semantic { name: name.into() },
            )
            .unwrap()
    }
    fn action(&mut self, name: &str, intent: Intent, value: Option<&str>) -> Receipt {
        let r = self.resolve(name);
        let l = self.engine.claim("fixture", 60000).unwrap();
        let receipt = self
            .engine
            .act(
                "fixture",
                &r.reference,
                &l,
                &id(),
                intent,
                value.map(str::to_string),
            )
            .unwrap();
        self.engine.release(&l).unwrap();
        receipt
    }
    fn verify(&mut self, p: Predicate, expected: &str) -> Result<serde_json::Value> {
        let shot = self.engine.observe("fixture")?;
        let el = if matches!(p, Predicate::ElementValue) {
            Some("Input".into())
        } else {
            None
        };
        self.engine.verify(Verification {
            target: "fixture",
            frame: &shot.frame,
            element: el,
            request: None,
            predicate: p,
            expected,
            timeout_ms: 40,
            stable_samples: 2,
        })
    }
}
#[test]
fn settings_change_verify_restore_workflow() {
    let mut h = Harness::new();
    assert!(!h.action("Settings", Intent::Click, None).goal_verified);
    assert_eq!(
        h.verify(Predicate::Modal, "Settings").unwrap()["goal_verified"],
        true
    );
    h.action("Toggle test option", Intent::Click, None);
    assert_eq!(
        h.verify(Predicate::ElementValue, "test-option:on").unwrap()["goal_verified"],
        true
    );
    h.action("Apply", Intent::Click, None);
    assert_eq!(
        h.verify(Predicate::Modal, "none").unwrap()["goal_verified"],
        true
    );
    h.action("Settings", Intent::Click, None);
    h.action("Toggle test option", Intent::Click, None);
    h.action("Apply", Intent::Click, None);
    assert_eq!(
        h.verify(Predicate::ElementValue, "test-option:off")
            .unwrap()["goal_verified"],
        true
    );
    assert!(!h.app.0.lock().unwrap().dialog);
}
#[test]
fn unicode_and_whitespace_use_exact_independent_readback() {
    let mut h = Harness::new();
    let s = " 中文\t🙂\n  print(\"a b\")\n";
    h.action("Input", Intent::Text, Some(s));
    assert_eq!(
        h.verify(Predicate::ElementValue, s).unwrap()["goal_verified"],
        true
    );
    assert_eq!(
        h.verify(Predicate::ElementValue, s.trim()).unwrap()["goal_verified"],
        false
    );
}
#[test]
fn ack_without_effect_is_not_goal_success() {
    let mut h = Harness::new();
    h.app.0.lock().unwrap().ack_without_effect = true;
    let r = h.action("Settings", Intent::Click, None);
    assert_eq!(r.system_accepted, Some(true));
    assert!(!r.goal_verified);
    assert_eq!(
        h.verify(Predicate::Modal, "Settings").unwrap()["goal_verified"],
        false
    );
}
#[test]
fn stale_after_scroll_does_not_dispatch() {
    let mut h = Harness::new();
    let r = h.resolve("Input");
    h.app.0.lock().unwrap().epoch += 1;
    let l = h.engine.claim("fixture", 60000).unwrap();
    assert_eq!(
        h.engine
            .act("fixture", &r.reference, &l, "clear", Intent::Click, None)
            .unwrap_err()
            .code,
        "stale_reference"
    );
    assert_eq!(h.app.0.lock().unwrap().calls, 0);
}
#[test]
fn negative_origin_scale_rotation() {
    let mut h = Harness::new();
    for (matrix, expected) in [
        ([0.5, 0.0, 0.0, 0.5, -1920.0, 0.0], [-1820.0, 50.0]),
        ([0.0, 0.5, -0.5, 0.0, 600.0, -100.0], [550.0, 0.0]),
    ] {
        h.app.0.lock().unwrap().transform = Some(matrix);
        let s = h.engine.observe("fixture").unwrap();
        let r = h
            .engine
            .resolve("fixture", &s.frame, Locator::Pixel { x: 200.0, y: 100.0 })
            .unwrap();
        assert_eq!(r.point, expected);
    }
}
#[test]
fn window_placement_reference_needs_no_pixel_mapping_but_cannot_send_input() {
    let mut h = Harness::new();
    h.app.0.lock().unwrap().transform = None;
    let shot = h.engine.observe("fixture").unwrap();
    let window = h
        .engine
        .resolve("fixture", &shot.frame, Locator::Window)
        .unwrap();
    assert!(window.placement_only);
    assert_eq!(window.point, [0.0, 0.0]);
    assert!(window.scope.element.is_none());
    assert_eq!(
        h.engine
            .resolve("fixture", &shot.frame, Locator::Pixel { x: 1.0, y: 1.0 })
            .unwrap_err()
            .code,
        "coordinate_mapping_unknown"
    );
    let lease = h.engine.claim("fixture", 60000).unwrap();
    assert_eq!(
        h.engine
            .act(
                "fixture",
                &window.reference,
                &lease,
                "window-reference-cannot-click",
                Intent::Click,
                None,
            )
            .unwrap_err()
            .code,
        "invalid_reference_class"
    );
    assert_eq!(h.app.0.lock().unwrap().calls, 0);
    h.engine.release(&lease).unwrap();
}
#[test]
fn layout_change_invalidates_reference() {
    let mut h = Harness::new();
    let r = h.resolve("Input");
    h.app.0.lock().unwrap().transform = Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    let l = h.engine.claim("fixture", 60000).unwrap();
    assert_eq!(
        h.engine
            .act("fixture", &r.reference, &l, "layout", Intent::Click, None)
            .unwrap_err()
            .code,
        "stale_reference"
    );
}
#[test]
fn changed_guest_rejected_against_registered_identity() {
    let mut h = Harness::new();
    h.app.0.lock().unwrap().identity.guest = Some("other-guest".into());
    assert_eq!(
        h.engine.observe("fixture").unwrap_err().code,
        "session_mismatch"
    );
}
#[test]
fn aliases_share_desktop_lease() {
    let mut h = Harness::new();
    let l = h.engine.claim("fixture", 60000).unwrap();
    assert_eq!(
        h.engine.claim("alias", 60000).unwrap_err().code,
        "desktop_busy"
    );
    h.engine.release(&l).unwrap();
    let l = h.engine.claim("alias", 60000).unwrap();
    h.engine.release(&l).unwrap();
}
#[test]
#[ignore]
fn lease_child() {
    let Some(root) = std::env::var_os("RCU_TEST_LOCK_ROOT") else {
        return;
    };
    let mut book = lease::LeaseBook::new(root.into());
    let key = Fixture::default().identity().key();
    if std::env::var("RCU_TEST_LOCK_MODE").as_deref() == Ok("crash") {
        book.claim("child", &key, 60000).unwrap();
        std::process::exit(17); // No Rust Drop: the kernel must release the lock.
    }
    assert_eq!(
        book.claim("child", &key, 60000).unwrap_err().code,
        "desktop_busy"
    );
}
#[test]
fn actual_other_process_cannot_claim_desktop() {
    let mut h = Harness::new();
    let l = h.engine.claim("fixture", 60000).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["tests::lease_child", "--exact", "--ignored"])
        .env("RCU_TEST_LOCK_ROOT", h.dir.path())
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let mut other = lease::LeaseBook::new(h.dir.path().into());
    assert_eq!(
        other
            .claim("other", &h.app.identity().key(), 60000)
            .unwrap_err()
            .code,
        "desktop_busy"
    );
    h.engine.release(&l).unwrap();
    let token = other
        .claim("other", &h.app.identity().key(), 60000)
        .unwrap();
    other.release(&token).unwrap();
}
#[test]
fn process_abnormal_exit_and_engine_drop_release_leases() {
    let h = Harness::new();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["tests::lease_child", "--exact", "--ignored"])
        .env("RCU_TEST_LOCK_ROOT", h.dir.path())
        .env("RCU_TEST_LOCK_MODE", "crash")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(17));
    let mut book = lease::LeaseBook::new(h.dir.path().into());
    book.claim("holder", &h.app.identity().key(), 60000)
        .unwrap();
    drop(book);
    let mut next = lease::LeaseBook::new(h.dir.path().into());
    assert!(next.claim("next", &h.app.identity().key(), 60000).is_ok());
}
#[test]
fn semantic_target_tolerates_caret_or_unrelated_pixel_changes_but_pixel_reference_does_not() {
    let mut h = Harness::new();
    let r = h.resolve("Input");
    let l = h.engine.claim("fixture", 60000).unwrap();
    h.app.0.lock().unwrap().pixel_epoch += 1;
    assert_eq!(
        h.engine
            .act(
                "fixture",
                &r.reference,
                &l,
                "caret",
                Intent::Text,
                Some("still the same grounded input".into())
            )
            .unwrap()
            .dispatch,
        "returned"
    );
    let s = h.engine.observe("fixture").unwrap();
    let r = h
        .engine
        .resolve("fixture", &s.frame, Locator::Pixel { x: 200.0, y: 100.0 })
        .unwrap();
    h.app.0.lock().unwrap().pixel_epoch += 1;
    let error = h
        .engine
        .act("fixture", &r.reference, &l, "pixel", Intent::Click, None)
        .unwrap_err();
    assert_eq!(error.code, "stale_reference");
    let diagnostic = error.diagnostic.unwrap();
    assert_eq!(diagnostic["content_changed"], true);
    assert_eq!(diagnostic["layout_changed"], false);
    assert_eq!(diagnostic["scope_changed"], false);
    assert_eq!(diagnostic["reference_kind"], "pixel");
    assert_eq!(diagnostic["native_input_dispatched"], false);
}
#[test]
fn semantic_revalidation_keeps_element_identity_position_focus_modal_and_layout_guards() {
    let mut h = Harness::new();
    let r = h.resolve("Input");
    let scene = h.app.observe().unwrap();
    let expected = scene.binding.as_ref().unwrap();
    let mut harmless = scene.clone();
    harmless.binding.as_mut().unwrap().content = "caret changed".into();
    harmless.elements[0].name = "unrelated spinner".into();
    assert!(grounding_matches(expected, &harmless, &r));
    for change in 0..7 {
        let mut changed = scene.clone();
        match change {
            0 => changed.elements[3].id = "replaced".into(),
            1 => changed.elements[3].point[0] += 10.0,
            2 => changed.binding.as_mut().unwrap().focus_window = "other".into(),
            3 => changed.binding.as_mut().unwrap().native_epoch = "modal changed".into(),
            4 => changed.binding.as_mut().unwrap().layout = "display moved".into(),
            5 => changed.elements[3].occluded = Some(true),
            _ => changed.elements[3].state_revision = Some("changed target value".into()),
        };
        assert!(
            !grounding_matches(expected, &changed, &r),
            "change {change}"
        );
    }
}
#[test]
fn expired_lease_is_released_without_another_request() {
    let mut h = Harness::new();
    h.engine.claim("fixture", 100).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let mut other = lease::LeaseBook::new(h.dir.path().into());
    let token = other.claim("other", &h.app.identity().key(), 1000).unwrap();
    other.release(&token).unwrap();
}
#[test]
fn uncertain_effect_never_replayed_after_restart() {
    let mut h = Harness::new();
    h.app.0.lock().unwrap().disconnect = true;
    let r = h.resolve("Settings");
    let l = h.engine.claim("fixture", 60000).unwrap();
    let first = h
        .engine
        .act("fixture", &r.reference, &l, "one-shot", Intent::Click, None)
        .unwrap();
    assert_eq!(first.dispatch, "uncertain");
    assert_eq!(first.dispatch_error.unwrap().code, "transport_closed");
    assert!(h.app.0.lock().unwrap().dialog);
    drop(h.engine);
    h.engine = Engine::new(h.dir.path().into()).unwrap();
    h.engine
        .register("fixture".into(), Box::new(h.app.clone()))
        .unwrap();
    let second = h
        .engine
        .act("fixture", &r.reference, &l, "one-shot", Intent::Click, None)
        .unwrap();
    assert!(second.deduplicated);
    assert_eq!(h.app.0.lock().unwrap().calls, 1);
}
#[test]
fn idempotency_conflict_is_not_replayed() {
    let mut h = Harness::new();
    let r = h.resolve("Input");
    let l = h.engine.claim("fixture", 60000).unwrap();
    h.engine
        .act("fixture", &r.reference, &l, "same", Intent::Click, None)
        .unwrap();
    assert_eq!(
        h.engine
            .act(
                "fixture",
                &r.reference,
                &l,
                "same",
                Intent::Text,
                Some("different".into())
            )
            .unwrap_err()
            .code,
        "idempotency_conflict"
    );
    assert_eq!(h.app.0.lock().unwrap().calls, 1);
}
#[test]
fn interrupted_empty_reservation_is_not_replayed() {
    let mut h = Harness::new();
    let r = h.resolve("Input");
    let path = h.engine.journal_path(&h.app.identity().key(), "partial");
    File::create(path).unwrap();
    assert_eq!(
        h.engine
            .act(
                "fixture",
                &r.reference,
                "unused",
                "partial",
                Intent::Click,
                None
            )
            .unwrap_err()
            .code,
        "uncertain_journal"
    );
    assert_eq!(h.app.0.lock().unwrap().calls, 0);
}
#[test]
fn blocked_and_expired_references_do_not_dispatch() {
    let mut h = Harness::new();
    let r = h.resolve("Input");
    let l = h.engine.claim("fixture", 60000).unwrap();
    h.app.0.lock().unwrap().blocked = Some(true);
    assert_eq!(
        h.engine
            .act("fixture", &r.reference, &l, "blocked", Intent::Click, None)
            .unwrap_err()
            .code,
        "target_blocked"
    );
    h.app.0.lock().unwrap().blocked = Some(false);
    h.engine.snapshots.back_mut().unwrap().1.at -=
        Duration::from_secs(INPUT_PROPOSAL_MAX_AGE_SECS + 1);
    assert_eq!(
        h.engine
            .act("fixture", &r.reference, &l, "old", Intent::Click, None)
            .unwrap_err()
            .code,
        "stale_reference"
    );
    assert_eq!(h.app.0.lock().unwrap().calls, 0);
}
#[test]
fn unknown_input_mapping_cannot_be_enabled_by_lease() {
    let mut h = Harness::new();
    h.app.0.lock().unwrap().ready = false;
    h.app.0.lock().unwrap().transform = None;
    let s = h.engine.observe("fixture").unwrap();
    assert_eq!(
        h.engine
            .resolve("fixture", &s.frame, Locator::Pixel { x: 0.0, y: 0.0 })
            .unwrap_err()
            .code,
        "coordinate_mapping_unknown"
    );
    let r = h.resolve("Input");
    let l = h.engine.claim("fixture", 60000).unwrap();
    assert_eq!(
        h.engine
            .act("fixture", &r.reference, &l, "unknown", Intent::Click, None)
            .unwrap_err()
            .code,
        "capability_unavailable"
    );
}
#[test]
fn invalid_pixels_rejected() {
    let mut h = Harness::new();
    let s = h.engine.observe("fixture").unwrap();
    for x in [-1.0, f64::NAN, 800.0, f64::INFINITY] {
        assert_eq!(
            h.engine
                .resolve("fixture", &s.frame, Locator::Pixel { x, y: 0.0 })
                .unwrap_err()
                .code,
            "outside_frame"
        );
    }
    assert!(serde_json::from_str::<Locator>(r#"{"kind":"pixel","x":true,"y":0}"#).is_err());
}
#[test]
fn other_window_element_or_process_cannot_verify_same_value() {
    let mut h = Harness::new();
    for scope in [
        Scope {
            window: "other".into(),
            pid: 42,
            element: Some("Input".into()),
        },
        Scope {
            window: "fixture-window".into(),
            pid: 999,
            element: Some("Input".into()),
        },
        Scope {
            window: "fixture-window".into(),
            pid: 42,
            element: Some("Other".into()),
        },
    ] {
        h.app.0.lock().unwrap().wrong_oracle = Some(scope);
        assert_eq!(
            h.verify(Predicate::ElementValue, "test-option:off")
                .unwrap_err()
                .code,
            "verification_scope_mismatch"
        );
    }
}
#[test]
fn provenance_does_not_verify_other_element() {
    let mut h = Harness::new();
    let r = h.action("Settings", Intent::Click, None);
    let s = h.engine.observe("fixture").unwrap();
    assert_eq!(
        h.engine
            .verify(Verification {
                target: "fixture",
                frame: &s.frame,
                element: Some("Input".into()),
                request: Some(r.request_id),
                predicate: Predicate::ElementValue,
                expected: "test-option:off",
                timeout_ms: 0,
                stable_samples: 1
            })
            .unwrap_err()
            .code,
        "verification_scope_mismatch"
    );
}
#[test]
fn untrusted_oracle_match_is_not_attested_success() {
    let mut h = Harness::new();
    h.app.0.lock().unwrap().assurance = "unknown".into();
    let v = h
        .verify(Predicate::ElementValue, "test-option:off")
        .unwrap();
    assert_eq!(v["predicate_matched"], true);
    assert_eq!(v["goal_verified"], false);
}
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
