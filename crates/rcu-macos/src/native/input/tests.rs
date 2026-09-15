use super::*;
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
        role: None,
        id: "exact-ax-element".into(),
        name: "Text".into(),
        point: [5., 5.],
        visible: Some(true),
        occluded: Some(false),
        state_revision: Some("classified-value-revision".into()),
    };
    let reference = Resolved {
        reference: "ref".into(),
        target: "target".into(),
        frame: "frame".into(),
        scope: Scope {
            element: Some(element.id.clone()),
            ..binding.scope.clone()
        },
        point: element.point,
        element_revision: Some(element.revision()),
        source_region: None,
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
fn keyboard_cannot_substitute_pixels_or_a_different_focused_element() {
    let element = AxElement {
        element: grounded().3.elements.remove(0),
        text_class: TextClass::Plain,
        value: Some("synthetic".into()),
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
        pid: 20,
        window: 10,
        identity,
        process: "synthetic".into(),
        allow_input: true,
        capturer: Capturer::default(),
        dispatches: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
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
