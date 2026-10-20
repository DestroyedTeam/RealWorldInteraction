//! All tri-state combinations retain the exact positive input predicate.
use super::*;

#[test]
fn f406_visibility_unknown_is_not_an_occlusion_claim_or_an_input_reference() {
    for visible in [None, Some(false), Some(true)] {
        for occluded in [None, Some(false), Some(true)] {
            let dir = tempfile::tempdir().unwrap();
            let fixture = crate::fixture::Fixture::default();
            let mut engine = Engine::new(dir.path().into()).unwrap();
            engine
                .register("fixture".into(), Box::new(fixture.clone()))
                .unwrap();
            let observation = engine.observe("fixture").unwrap();
            let input = engine
                .snapshots
                .back_mut()
                .unwrap()
                .1
                .scene
                .elements
                .iter_mut()
                .find(|e| e.id == "Input")
                .unwrap();
            input.visible = visible;
            input.occluded = occluded;
            let references = engine.refs.len();
            let result = engine.resolve(
                "fixture",
                &observation.frame,
                Locator::Element { id: "Input".into() },
            );
            if visible == Some(true) && occluded == Some(false) {
                assert!(result.is_ok());
                assert_eq!(engine.refs.len(), references + 1);
            } else {
                let error = result.unwrap_err();
                assert_eq!(error.code, "visibility_unknown");
                let diagnostic = error.diagnostic.unwrap();
                let unavailable = visible == Some(false) || occluded == Some(true);
                assert_eq!(
                    diagnostic["reason"],
                    if unavailable {
                        "native_visibility_unavailable"
                    } else {
                        "native_hit_visibility_unproven"
                    }
                );
                assert_eq!(diagnostic["visible"], serde_json::json!(visible));
                assert_eq!(diagnostic["occluded"], serde_json::json!(occluded));
                for key in [
                    "input_authority",
                    "native_input_dispatched",
                    "replay_allowed",
                ] {
                    assert_eq!(diagnostic[key], false);
                }
                assert!(diagnostic.get("result_ref").is_none());
                assert_ne!(error.next_step, "Resolve occlusion before input.");
                assert_eq!(engine.refs.len(), references);
            }
            assert_eq!(fixture.0.lock().unwrap().calls, 0);
        }
    }
}

#[test]
fn f406_unknown_pointer_visibility_does_not_invent_or_remove_native_actions() {
    for (action, disabled, expected) in [
        ("AXPress", false, None),
        ("AXMadeUp", false, Some("unsupported_action")),
        ("AXPress", true, Some("element_disabled")),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().into()).unwrap();
        engine
            .register(
                "fixture".into(),
                Box::new(crate::fixture::Fixture::default()),
            )
            .unwrap();
        let observation = engine.observe("fixture").unwrap();
        let input = engine
            .snapshots
            .back_mut()
            .unwrap()
            .1
            .scene
            .elements
            .iter_mut()
            .find(|e| e.id == "Input")
            .unwrap();
        input.visible = None;
        input.occluded = None;
        input.enabled = Some(!disabled);
        input.actions = vec!["AXPress".into()];
        let result = engine.resolve_action(
            "fixture",
            &observation.frame,
            Locator::Element { id: "Input".into() },
            Some(action),
        );
        assert_eq!(result.err().map(|e| e.code), expected.map(str::to_owned));
    }
}
