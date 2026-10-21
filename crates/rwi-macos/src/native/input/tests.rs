use super::*;
#[test]
fn explicit_native_action_rejects_unmarked_or_mismatched_references_before_input() {
    for action in ["AXShowMenu", "AXCancel", "AXPick"] {
        assert_eq!(
            native_action_request(&Intent::Click, Some(action), Some(action)).unwrap(),
            Some(action)
        );
        assert!(native_action_request(&Intent::Click, None, Some(action)).is_err());
        assert!(native_action_request(&Intent::Click, Some(action), Some("AXPress")).is_err());
        assert!(native_action_request(&Intent::DoubleClick, Some(action), Some(action)).is_err());
        assert!(native_action_request(&Intent::KeyChord, Some(action), Some(action)).is_err());
    }
    assert!(
        native_action_request(&Intent::Click, None, None)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        native_action_request(&Intent::Click, None, Some("AXShowMenu"))
            .unwrap_err()
            .code,
        "native_action_mismatch"
    );
    assert!(native_action_request(&Intent::Click, Some("AXShowMenu"), Some("AXPress")).is_err());
    assert!(
        native_action_request(&Intent::DoubleClick, Some("AXShowMenu"), Some("AXShowMenu"))
            .is_err()
    );
}

#[test]
fn semantic_surfaces_cannot_use_pixels_keys_or_implicit_clicks() {
    assert!(require_surface_action(true, &Intent::Click, Some("AXPress")).is_ok());
    assert!(require_surface_action(true, &Intent::Click, None).is_err());
    assert!(require_surface_action(true, &Intent::KeyChord, None).is_err());
    assert!(require_surface_action(true, &Intent::DoubleClick, Some("AXPress")).is_err());
    assert!(require_surface_action(false, &Intent::Click, None).is_ok());
}
fn grounded() -> (Identity, Binding, Resolved, Scene) {
    let identity = Identity {
        host: "mac-synthetic".into(),
        guest: None,
        session: "uid:501:security:1:boot:1".into(),
        seat: "console:1".into(),
        desktop: "quartz-console".into(),
    };
    let binding = Binding {
        scope: Scope {
            window: "cgwindow:10".into(),
            pid: 20,
            element: None,
        },
        focus_window: "cgwindow:10".into(),
        native_epoch: "native-layout-and-focus".into(),
        layout: "one-native-display".into(),
        content: "pixels".into(),
        mutation_context: None,
    };
    let element = Element {
        enabled: None,
        actions: vec![],
        role: None,
        id: "exact-ax-element".into(),
        name: "Text".into(),
        point: [5., 5.],
        visible: Some(true),
        occluded: Some(false),
        state_revision: Some("classified-value-revision".into()),
        selected: None,
        value: None,
        value_truncated: false,
        description: None,
    };
    let reference = Resolved {
        native_action: None,
        reference: "ref".into(),
        target: "target".into(),
        frame: "frame".into(),
        scope: Scope {
            element: Some(element.id.clone()),
            ..binding.scope.clone()
        },
        point: element.point,
        placement_only: false,
        element_revision: Some(element.revision()),
        source_region: None,
        pixel_anchor: None,
        red_marker_anchor: None,
        pixel_anchor_used: false,
    };
    let scene = Scene {
        identity: identity.clone(),
        assurance: "native-attested".into(),
        binding: Some(binding.clone()),
        mutation_ready: true,
        blocked: Some(false),
        image_size: [0, 0],
        pixel_to_desktop: None,
        elements: vec![element],
        captured_unix_ms: 1,
        atomicity: "synthetic".into(),
        image_normalization: "none".into(),
        png: vec![],
    };
    (identity, binding, reference, scene)
}
#[test]
fn native_element_policy_requires_exact_fresh_identity_focus_layout_state_and_hit_visibility() {
    let (identity, binding, reference, scene) = grounded();
    assert!(validate_batch_scene(&identity, &binding, &reference, &scene).is_ok());
    for change in 0..9 {
        let mut now = scene.clone();
        match change {
            0 => now.identity.session = "different-login".into(),
            1 => now.binding.as_mut().unwrap().focus_window = "cgwindow:11".into(),
            2 => now.binding.as_mut().unwrap().layout = "changed-display".into(),
            3 => now.elements[0].id = "replacement".into(),
            4 => now.elements[0].state_revision = Some("changed-value-or-classification".into()),
            5 => now.elements[0].visible = Some(false),
            6 => now.elements[0].occluded = None,
            7 => now.mutation_ready = false,
            _ => now.blocked = None,
        }
        assert_eq!(
            validate_batch_scene(&identity, &binding, &reference, &now)
                .unwrap_err()
                .code,
            "stale_reference"
        );
    }
}
#[test]
fn final_native_batch_diagnostic_distinguishes_content_geometry_and_readiness() {
    let (identity, binding, reference, scene) = grounded();
    for change in 0..5 {
        let mut now = scene.clone();
        match change {
            0 => now.identity.session = "different-session".into(),
            1 => now.binding.as_mut().unwrap().layout = "different-layout".into(),
            2 => now.elements[0].state_revision = Some("different-revision".into()),
            3 => now.mutation_ready = false,
            _ => now.blocked = None,
        }
        let error = validate_batch_scene(&identity, &binding, &reference, &now).unwrap_err();
        let diagnostic = error.diagnostic.unwrap();
        assert_eq!(diagnostic["stage"], "macos_first_input_batch");
        assert_eq!(diagnostic["identity_changed"], change == 0);
        assert_eq!(diagnostic["layout_changed"], change == 1);
        assert_eq!(diagnostic["grounding_matches"], !matches!(change, 1 | 2));
        assert_eq!(diagnostic["mutation_ready"], change != 3);
        assert_eq!(diagnostic["content_changed"], false);
        assert!(
            diagnostic.get("dispatch_evidence").is_none(),
            "a scene comparison alone cannot prove no held input"
        );
    }
    let mut now = scene;
    now.binding = None;
    let error = validate_batch_scene(&identity, &binding, &reference, &now).unwrap_err();
    assert_eq!(error.diagnostic.unwrap()["binding_missing"], true);
}
#[test]
fn keyboard_cannot_substitute_pixels_or_a_different_focused_element() {
    let element = AxElement {
        element: grounded().3.elements.remove(0),
        text_class: TextClass::Plain,
        value: Some("synthetic".into()),
        focus_availability: None,
    };
    for requested in [None, Some("other")] {
        assert!(require_keyboard_target(&element, requested).is_err());
    }
    assert!(require_keyboard_target(&element, Some("exact-ax-element")).is_ok());
    assert!(require_text_target(&element, Some("exact-ax-element")).is_ok());
}
#[test]
fn unsupported_source_region_is_rejected_before_any_native_read_or_capture() {
    let (identity, _, mut reference, _) = grounded();
    let mut native = NativeMac {
        semantic_only: false,
        pid: 20,
        window: 10,
        identity,
        process: "synthetic".into(),
        allow_input: true,
        capturer: Capturer::default(),
        hit_diagnostics: std::sync::Mutex::new(Vec::new()),
        dispatches: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        objects: std::sync::Mutex::new(super::super::objects::Objects::default()),
        last_observation: std::sync::Mutex::new(serde_json::Value::Null),
        text_effect: None,
        pointer_dispatch: None,
    };
    reference.source_region = Some([1, 2, 3, 4]);
    assert_eq!(
        native.revalidate(&reference).unwrap_err().code,
        "capability_unavailable"
    );
    assert_eq!(
        native.observe_region([1, 2, 3, 4]).unwrap_err().code,
        "capability_unavailable"
    );
    assert_eq!(native.input_dispatch_count(), Some(0));
    assert!(native.require_discovered_process("synthetic").is_ok());
    assert_eq!(
        native
            .require_discovered_process("replacement-process")
            .unwrap_err()
            .code,
        "window_epoch_changed"
    );
}

#[test]
fn action_error_retains_native_status_without_claiming_no_effect() {
    for status in [-25204, -25206, -25205, -1] {
        let error = native_action_failure("AXPress", status);
        assert_eq!(error.code, "native_action_unconfirmed");
        let d = error.diagnostic.unwrap();
        assert_eq!(d["native_status"], status);
        assert_eq!(d["replay_allowed"], false);
        assert_eq!(d["effect"], "unknown");
        assert_eq!(d["held_input_resources"], false);
    }
}

#[test]
fn keyboard_activation_depends_on_application_not_main_window_state() {
    assert!(needs_background_activation(
        &Intent::Text,
        false,
        false,
        Some(200),
        100
    ));
    assert!(needs_background_activation(
        &Intent::Text,
        false,
        false,
        None,
        100
    ));
    assert!(!needs_background_activation(
        &Intent::Text,
        false,
        false,
        Some(100),
        100
    ));
    assert!(!needs_background_activation(
        &Intent::Text,
        true,
        false,
        Some(200),
        100
    ));
}

#[test]
fn advertised_native_actions_do_not_prepare_a_synthetic_background_receiver() {
    for native_press in [false, true] {
        for foreground in [None, Some(100), Some(200)] {
            assert!(!needs_background_activation(
                &Intent::Click,
                native_press,
                true,
                foreground,
                100
            ));
        }
    }
}

#[test]
fn keyboard_payload_validation_rejects_before_activation_preparation() {
    assert!(validate_keyboard_payload(&Intent::Text, None).is_err());
    assert!(validate_keyboard_payload(&Intent::KeyChord, None).is_err());
    assert!(validate_keyboard_payload(&Intent::KeyChord, Some("not-a-supported-key")).is_err());
    assert!(validate_keyboard_payload(&Intent::KeyChord, Some("escape")).is_ok());
    assert!(validate_keyboard_payload(&Intent::Text, Some("验收")).is_ok());
}

#[test]
fn excluded_compositor_border_cannot_receive_pointer_or_drag_endpoint() {
    use rwi_core::trajectory::{DragPath, Waypoint};
    let frame = Rect {
        x: 2946.0,
        y: -100.0,
        w: 1434.0,
        h: 660.0,
    };
    assert!(validate_capture_fringe(frame, [3000.0, 558.9], &Intent::Click).is_ok());
    for intent in [
        Intent::Click,
        Intent::DoubleClick,
        Intent::Scroll,
        Intent::ContextMenu,
    ] {
        assert!(validate_capture_fringe(frame, [3000.0, 559.0], &intent).is_err());
    }
    let path = Intent::Drag(DragPath {
        waypoints: vec![
            Waypoint {
                at_ms: 0,
                offset: [0.0, 0.0],
            },
            Waypoint {
                at_ms: 500,
                offset: [0.0, 459.0],
            },
        ],
    });
    assert!(validate_capture_fringe(frame, [3000.0, 100.0], &path).is_err());
    assert!(validate_capture_fringe(frame, [3000.0, 99.0], &path).is_ok());
}

// Pure, allocated raster fixture. Engine creates the same pixel anchor as the
// production resolver; no native window, screen, input, lease or GUI is used.
struct GroundingRaster(Scene);
impl Backend for GroundingRaster {
    fn local_pixel_anchor(&self) -> bool {
        true
    }
    fn identity(&self) -> Identity {
        self.0.identity.clone()
    }
    fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({"fixture":"native-single-grounding-raster"})
    }
    fn observe(&mut self) -> Result<Scene> {
        Ok(self.0.clone())
    }
    fn act(
        &mut self,
        _: &Binding,
        _: &Resolved,
        _: &Intent,
        _: Option<&str>,
    ) -> Result<Option<bool>> {
        panic!("grounding fixture never dispatches input")
    }
    fn inspect(&mut self, _: &Scope, _: &Predicate, _: &str) -> Result<Oracle> {
        panic!("grounding fixture has no business oracle")
    }
}

fn grounding_raster_png(changed_center: bool) -> Vec<u8> {
    let mut rgb = vec![255; 160 * 160 * 3];
    for y in 61..99 {
        for x in 61..99 {
            let offset = (y * 160 + x) * 3;
            let color = if changed_center && (76..84).contains(&x) && (76..84).contains(&y) {
                [230, 10, 10]
            } else {
                [20, 50, 90]
            };
            rgb[offset..offset + 3].copy_from_slice(&color);
        }
    }
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 160, 160);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&rgb)
        .unwrap();
    bytes
}

fn grounded_pixel_raster() -> (Identity, Binding, Resolved, Scene) {
    let (identity, mut binding, _, mut scene) = grounded();
    scene.png = grounding_raster_png(false);
    binding.content = rwi_core::digest(&scene.png);
    scene.binding = Some(binding.clone());
    scene.elements.clear();
    scene.image_size = [160, 160];
    scene.pixel_to_desktop = Some([1., 0., 0., 1., 0., 0.]);
    let directory = tempfile::tempdir().unwrap();
    let mut engine = rwi_core::Engine::new(directory.path().into()).unwrap();
    engine
        .register(
            "single-grounding".into(),
            Box::new(GroundingRaster(scene.clone())),
        )
        .unwrap();
    let frame = engine.observe("single-grounding").unwrap().frame;
    let reference = engine
        .resolve(
            "single-grounding",
            &frame,
            rwi_core::Locator::Pixel { x: 80., y: 80. },
        )
        .unwrap();
    assert!(reference.pixel_anchor.is_some());
    (identity, binding, reference, scene)
}

#[test]
fn final_native_batch_structure_failure_skips_pixel_comparison_diagnostics() {
    let (identity, binding, reference, scene) = grounded_pixel_raster();
    for change in 0..6 {
        let mut now = scene.clone();
        now.binding.as_mut().unwrap().content = "external-pixel-change".into();
        // Corrupting the fixture PNG is safe: a structure rejection must not
        // ask the comparator to decode it just for an error diagnostic.
        now.png = b"not-a-png".to_vec();
        match change {
            0 => now.binding.as_mut().unwrap().scope.window = "replacement-window".into(),
            1 => now.binding.as_mut().unwrap().focus_window = "replacement-focus".into(),
            2 => now.binding.as_mut().unwrap().layout = "replacement-layout".into(),
            3 => now.binding.as_mut().unwrap().native_epoch = "replacement-epoch".into(),
            4 => {
                now.binding.as_mut().unwrap().mutation_context = Some(rwi_core::MutationContext {
                    window_title: "changed-context".into(),
                    document_state: None,
                })
            }
            _ => now.binding = None,
        }
        let error = validate_batch_scene(&identity, &binding, &reference, &now).unwrap_err();
        assert_eq!(error.code, "stale_reference");
        let diagnostic = error.diagnostic.unwrap();
        assert_eq!(diagnostic["grounding_matches"], false);
        assert!(
            diagnostic.get("local_pixel_anchor").is_none(),
            "structure case {change} must not compare/decode pixels for diagnostic"
        );
        assert!(diagnostic.get("red_marker_cluster_anchor").is_none());
    }
}

#[test]
fn final_native_batch_pixel_failure_preserves_comparator_reason_and_limits() {
    let (identity, binding, reference, mut now) = grounded_pixel_raster();
    now.png = grounding_raster_png(true);
    now.binding.as_mut().unwrap().content = rwi_core::digest(&now.png);
    let expected_diagnostic = reference.pixel_anchor.as_ref().unwrap().diagnostic(
        &now.png,
        now.image_size,
        reference.point,
        now.pixel_to_desktop,
    );
    assert_eq!(expected_diagnostic["matched"], false);
    assert_eq!(expected_diagnostic["reason"], "central_patch_changed");
    let error = validate_batch_scene(&identity, &binding, &reference, &now).unwrap_err();
    assert_eq!(error.code, "stale_reference");
    let diagnostic = error.diagnostic.unwrap();
    assert_eq!(diagnostic["local_pixel_anchor"], expected_diagnostic);
    assert_eq!(diagnostic["local_pixel_anchor"]["input_authority"], false);
    assert_eq!(diagnostic["grounding_matches"], false);
    assert_eq!(diagnostic["content_changed"], true);
    assert_eq!(diagnostic["stage"], "macos_first_input_batch");
    assert!(diagnostic.get("dispatch_evidence").is_none());
}

#[test]
fn final_native_batch_pixel_match_does_not_bypass_readiness_or_blocked_guards() {
    let (identity, binding, reference, scene) = grounded_pixel_raster();
    for (ready, blocked) in [(false, Some(false)), (true, Some(true)), (true, None)] {
        let mut now = scene.clone();
        now.mutation_ready = ready;
        now.blocked = blocked;
        let error = validate_batch_scene(&identity, &binding, &reference, &now).unwrap_err();
        let diagnostic = error.diagnostic.unwrap();
        assert_eq!(diagnostic["grounding_matches"], true);
        assert_eq!(diagnostic["mutation_ready"], ready);
        assert_eq!(diagnostic["blocked"], serde_json::json!(blocked));
        assert!(diagnostic.get("local_pixel_anchor").is_none());
    }
}

#[test]
fn final_native_batch_pixel_identity_change_skips_comparison_without_new_authority() {
    let (identity, binding, reference, mut now) = grounded_pixel_raster();
    now.identity.session = "replacement-login".into();
    now.binding.as_mut().unwrap().content = "changed-image".into();
    now.png = b"not-a-png".to_vec();
    let error = validate_batch_scene(&identity, &binding, &reference, &now).unwrap_err();
    assert_eq!(error.code, "stale_reference");
    let diagnostic = error.diagnostic.unwrap();
    assert_eq!(diagnostic["identity_changed"], true);
    assert_eq!(diagnostic["grounding_matches"], false);
    assert_eq!(diagnostic["pixel_comparison_skipped"], "identity_changed");
    assert!(diagnostic.get("local_pixel_anchor").is_none());
    assert!(diagnostic.get("red_marker_cluster_anchor").is_none());
    assert!(diagnostic.get("dispatch_evidence").is_none());
}

#[test]
fn pointer_delivery_policy_never_prepares_application_receiver() {
    use rwi_core::trajectory::{DragPath, Waypoint};
    let pointer_intents = [
        Intent::Click,
        Intent::MovePointer,
        Intent::DoubleClick,
        Intent::ContextMenu,
        Intent::Scroll,
        Intent::Drag(DragPath {
            waypoints: vec![
                Waypoint {
                    at_ms: 0,
                    offset: [0., 0.],
                },
                Waypoint {
                    at_ms: 5000,
                    offset: [220., 90.],
                },
            ],
        }),
    ];
    for intent in pointer_intents {
        for foreground in [None, Some(100), Some(200)] {
            assert!(!needs_background_activation(
                &intent, false, false, foreground, 100
            ));
        }
    }
    for intent in [Intent::Text, Intent::KeyChord] {
        assert!(needs_background_activation(
            &intent,
            false,
            false,
            Some(200),
            100
        ));
        assert!(!needs_background_activation(
            &intent,
            false,
            false,
            Some(100),
            100
        ));
        assert!(!needs_background_activation(
            &intent,
            false,
            true,
            Some(200),
            100
        ));
    }
}

#[test]
fn ordinary_standard_window_keeps_pointer_click_independent_of_main_window() {
    for _is_main in [true, false] {
        assert!(
            !native_pointer_requires_axpress(
                Some("AXWindow"),
                Some("AXStandardWindow"),
                Some(false)
            )
            .unwrap()
        );
    }
}
#[test]
fn actual_sheet_dialog_or_raw_modal_retains_native_press_protection() {
    for (role, subrole, modal) in [
        (Some("AXSheet"), None, None),
        (Some("AXSheet"), Some("AXSheet"), Some(false)),
        (Some("AXWindow"), Some("AXDialog"), None),
        (Some("AXWindow"), Some("AXSystemDialog"), Some(false)),
        (Some("AXWindow"), Some("AXStandardWindow"), Some(true)),
        (None, None, Some(true)),
    ] {
        assert!(native_pointer_requires_axpress(role, subrole, modal).unwrap());
    }
}
#[test]
fn unknown_pointer_surface_is_not_a_standard_window_or_implicit_axpress() {
    for (role, subrole, modal) in [
        (None, None, None),
        (Some("AXWindow"), Some("AXStandardWindow"), None),
        (Some("AXWindow"), None, Some(false)),
        (Some("AXWindow"), Some("AXUnknown"), Some(false)),
        (Some("AXPopover"), None, Some(false)),
        (Some("AXGroup"), Some("AXStandardWindow"), Some(false)),
    ] {
        let error = native_pointer_requires_axpress(role, subrole, modal).unwrap_err();
        assert_eq!(error.code, "native_click_surface_unknown");
        let d = error.diagnostic.unwrap();
        assert_eq!(d["role"], serde_json::json!(role));
        assert_eq!(d["subrole"], serde_json::json!(subrole));
        assert_eq!(d["raw_modal"], serde_json::json!(modal));
        assert_eq!(d["native_input_dispatched"], false);
        assert_eq!(d["replay_allowed"], false);
    }
    // Existing semantic-only policy is not weakened by the ordinary-window fix.
    assert_eq!(
        require_surface_action(true, &Intent::Click, None)
            .unwrap_err()
            .code,
        "semantic_action_required"
    );
    assert!(require_surface_action(true, &Intent::Click, Some("AXPress")).is_ok());
}
