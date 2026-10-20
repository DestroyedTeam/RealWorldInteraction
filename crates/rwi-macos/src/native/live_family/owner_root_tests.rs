use super::*;
#[test]
fn explicit_visual_layers_follow_native_order_and_never_guess_missing_windows() {
    let rect = Rect {
        x: 0.,
        y: 0.,
        w: 10.,
        h: 10.,
    };
    let rows = vec![
        Window {
            id: 7,
            pid: 12,
            rect,
            alpha: 1.,
            title: None,
        },
        Window {
            id: 8,
            pid: 13,
            rect,
            alpha: 1.,
            title: None,
        },
    ];
    assert_eq!(
        visual_layer_order(&["cgwindow:8".into(), "cgwindow:7".into()], &rows).unwrap(),
        vec![0, 1]
    );
    assert_eq!(
        visual_layer_order(&["cgwindow:7".into(), "cgwindow:8".into()], &rows).unwrap(),
        vec![1, 0]
    );
    assert!(visual_layer_order(&["cgwindow:9".into()], &rows).is_err());
    assert!(visual_layer_order(&["cgwindow:7".into(), "cgwindow:7".into()], &rows).is_err());
    for code in [
        "window_missing",
        "process_identity_unknown",
        "session_or_process_changed",
        "window_identity_changed",
    ] {
        assert!(temporary_explicit_source_loss(code));
    }
    for code in [
        "cancelled",
        "screen_capture_permission_missing",
        "window_epoch_changed",
        "storage_or_transport_error",
    ] {
        assert!(!temporary_explicit_source_loss(code));
    }
}
#[test]
fn retained_owner_root_requires_every_positive_native_fact() {
    let rect = Rect {
        x: 1800.,
        y: 24.,
        w: 660.,
        h: 552.,
    };
    let facts = || RootFacts {
        native_type: true,
        pid: Some(17),
        window: Some(23),
        rect: Some(rect),
        role: Some("AXWindow"),
    };
    assert!(root_matches(17, 23, rect, facts()));
    for facts in [
        RootFacts {
            native_type: false,
            ..facts()
        },
        RootFacts {
            pid: None,
            ..facts()
        },
        RootFacts {
            pid: Some(18),
            ..facts()
        },
        RootFacts {
            window: None,
            ..facts()
        },
        RootFacts {
            window: Some(24),
            ..facts()
        },
        RootFacts {
            rect: None,
            ..facts()
        },
        RootFacts {
            rect: Some(Rect { x: 1801., ..rect }),
            ..facts()
        },
        RootFacts {
            role: None,
            ..facts()
        },
        RootFacts {
            role: Some("AXButton"),
            ..facts()
        },
    ] {
        assert!(!root_matches(17, 23, rect, facts));
    }
}
#[test]
fn failed_owner_inventory_has_a_real_bounded_cooldown() {
    let now = Instant::now();
    assert!(relay_ready(now, None));
    assert!(!relay_ready(now, Some(now + RELAY_COOLDOWN)));
    assert!(relay_ready(
        now + RELAY_COOLDOWN,
        Some(now + RELAY_COOLDOWN)
    ));
}
