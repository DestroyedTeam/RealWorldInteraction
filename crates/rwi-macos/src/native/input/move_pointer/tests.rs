use super::*;
use rwi_core::operation::Registry;
use std::{cell::Cell, sync::atomic::Ordering, time::Duration};

#[test]
fn move_event_is_private_unpressed_and_has_unknown_previous_motion() {
    let move_event = event([-400.25, 200.5]).unwrap();
    let point = move_event.location();
    assert_eq!(move_event.get_type() as u32, CGEventType::MouseMoved as u32);
    assert_eq!([point.x, point.y], [-400.25, 200.5]);
    assert_eq!(move_event.get_flags(), CGEventFlags::empty());
    assert_eq!(
        move_event.get_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE),
        0
    );
    assert_eq!(
        move_event.get_double_value_field(EventField::MOUSE_EVENT_PRESSURE),
        0.0
    );
    assert_eq!(
        move_event.get_integer_value_field(EventField::MOUSE_EVENT_DELTA_X),
        0
    );
    assert_eq!(
        move_event.get_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y),
        0
    );
    // The Private constructor request is -1, but CoreGraphics assigns the
    // actual private state table a unique ID (SDK CGEventSource.h), not -1.
    let state_table = move_event.get_integer_value_field(EventField::EVENT_SOURCE_STATE_ID);
    assert!(!matches!(state_table, 0 | 1)); // not CombinedSession or HIDSystem
    let another = event([-400.25, 200.5]).unwrap();
    assert_ne!(
        move_event.get_integer_value_field(EventField::EVENT_SOURCE_STATE_ID),
        another.get_integer_value_field(EventField::EVENT_SOURCE_STATE_ID)
    );
    println!(
        "{}",
        serde_json::json!({
            "fixture":"move_pointer_construction_only",
            "type":"mouse_moved","point":[point.x,point.y],
            "private_state_table":state_table,
            "flags":move_event.get_flags().bits(),"pressure":0,"click_count":0,
            "delta":[0,0],"motion_history":"unknown","button_sequence":[],
            "event_number":"not_defined_for_move; not_read",
            "post_calls":0,"warp_calls":0,"capture_calls":0,
            "receiver_effect":"unverified"
        })
    );
}

#[test]
fn hover_constructor_public_nsevent_vs_current_cg() {
    // Exercise the actual production constructor, not a copied CG fixture.
    let point = [-400.25, 200.5];
    let current = event(point).unwrap();
    let original_source = current.get_integer_value_field(EventField::EVENT_SOURCE_STATE_ID);
    let mut snapshots = Vec::new();
    for synthetic_window_number in [0, 1_048_573] {
        let snapshot =
            crate::nsevent_probe::probe_move_constructor(&current, synthetic_window_number)
                .expect("public MouseMoved construction/bridge");
        assert_eq!(snapshot["current"]["quartz"]["event_type"], 5);
        assert_eq!(snapshot["public_nsevent"]["constructed"]["event_type"], 5);
        assert_eq!(snapshot["public_nsevent"]["quartz"]["event_type"], 5);
        assert_eq!(snapshot["public_nsevent"]["roundtrip"]["event_type"], 5);
        assert_eq!(
            snapshot["public_nsevent"]["constructed"]["window_number"],
            synthetic_window_number
        );
        assert_eq!(
            snapshot["public_nsevent"]["constructed"]["window_position"],
            serde_json::json!(point)
        );
        for appkit in [
            &snapshot["current"]["appkit"],
            &snapshot["public_nsevent"]["constructed"],
            &snapshot["public_nsevent"]["roundtrip"],
        ] {
            for forbidden in ["event_number", "click_count", "button_number", "pressure"] {
                assert!(appkit.get(forbidden).is_none());
            }
        }
        // No equality is required for constructor-implied source IDs, delta,
        // timestamp conversion precision, roundtrip window or frame mapping.
        snapshots.push(snapshot);
    }
    assert_eq!(current.get_type() as u32, CGEventType::MouseMoved as u32);
    assert_eq!([current.location().x, current.location().y], point);
    assert_eq!(current.get_flags(), CGEventFlags::empty());
    assert_eq!(
        current.get_integer_value_field(EventField::EVENT_SOURCE_STATE_ID),
        original_source
    );
    println!(
        "{}",
        serde_json::json!({
            "probe":"hover_constructor_public_nsevent_vs_current_cg",
            "scope":"construction_only_no_gui_no_input_delivery",
            "variable":"constructor; factory-implied fields measured without normalization",
            "local_global_frame_mapping":"unproven_without_real_window_geometry",
            "window_identity":"synthetic_fixture_only; not_resolved_or_attested",
            "cg_public_window_number":"no_public_accessor_used",
            "private_routing_fields":"not_read_or_set",
            "moved_nsevent_properties_not_read":["eventNumber", "clickCount", "buttonNumber", "pressure"],
            "input_delivery_attempted":false, "receiver_effect":"unverified",
            "snapshots":snapshots
        })
    );
}

#[test]
fn invalid_move_point_allocates_no_event() {
    for point in [
        [f64::NAN, 1.0],
        [1.0, f64::INFINITY],
        [f64::NEG_INFINITY, 1.0],
    ] {
        assert_eq!(event(point).err().unwrap().code, "invalid_pointer_point");
    }
}

#[test]
fn move_guard_failure_cancellation_or_expired_lease_counts_zero_dispatches() {
    for mode in 0..4 {
        let op = Registry::default().begin(None).unwrap();
        let counter = AtomicU64::new(0);
        let reads = Cell::new(0);
        if mode == 0 {
            op.cancel();
        }
        let execute = || {
            operation::run(&op, || {
                guarded_single(
                    &counter,
                    || {
                        reads.set(reads.get() + 1);
                        if mode == 1 {
                            Err(fault("fixture_boundary_changed", "No native event."))
                        } else {
                            if mode == 2 {
                                op.cancel();
                            }
                            Ok(())
                        }
                    },
                    || panic!("no post closure may run"),
                )
            })
        };
        let result = if mode == 3 {
            operation::with_dispatch_deadline(Instant::now() - Duration::from_millis(1), execute)
        } else {
            execute()
        };
        assert!(result.is_err(), "mode {mode}");
        assert_eq!(counter.load(Ordering::Acquire), 0, "mode {mode}");
        assert_eq!(reads.get(), usize::from(matches!(mode, 1 | 2)));
        assert!(!op.status().native_call_pending);
        op.finish();
        assert!(op.status().resources_released);
    }
}

#[test]
fn move_single_counts_exactly_one_attempt_and_cannot_replay_after_cancellation() {
    let op = Registry::default().begin(None).unwrap();
    let counter = AtomicU64::new(0);
    let reads = Cell::new(0);
    let posts = Cell::new(0);
    operation::run(&op, || {
        guarded_single(
            &counter,
            || {
                reads.set(reads.get() + 1);
                Ok(())
            },
            || {
                posts.set(posts.get() + 1);
                assert!(op.status().native_call_pending);
                assert_eq!(op.status().stage, "macos_move_pointer_event");
                assert_eq!(counter.load(Ordering::Acquire), 1);
                op.cancel();
            },
        )
        .unwrap();
        assert_eq!(
            guarded_single(
                &counter,
                || panic!("no second read"),
                || panic!("no replay")
            )
            .unwrap_err()
            .code,
            "operation_cancelled"
        );
    });
    assert_eq!((reads.get(), posts.get()), (1, 1));
    assert_eq!(counter.load(Ordering::Acquire), 1);
    assert!(!op.status().native_call_pending);
    assert!(!op.status().resource_release_uncertain);
    op.finish();
    assert!(op.status().resources_released);
}

#[test]
fn move_wiring_preserves_exact_grounding_and_excludes_activation_and_global_fallback() {
    let input = include_str!("../../input.rs");
    let branch = input.split("Intent::MovePointer => {").nth(1).unwrap();
    let branch = branch.split("Intent::Drag(path) =>").next().unwrap();
    for call in [
        "move_pointer::event(target.point)",
        "move_pointer::guarded_single",
        "self.read_input_batch(true, expected, target, intent)",
        ".validate(expected, target, Kind::Pointer)",
        "crate::background::deliver_pointer(&event, &delivery, target.point)",
    ] {
        assert!(branch.contains(call), "missing {call}");
    }
    assert!(input.contains("!matches!(intent, Intent::MovePointer)"));
    assert!(input.contains("validate_capture_fringe(delivery.frame, target.point, intent)"));
    assert!(input.contains("pointer_delivery_available()"));
    assert!(!branch.contains("activate_background_window"));
    assert!(!branch.contains("dispatch_native_action"));
    assert!(!branch.contains("click_pair"));
    assert!(branch.contains("if foreground_pointer"));
    assert!(branch.contains("foreground_move::post(&event)"));
    assert!(branch.contains("foreground_move::guard(self, foreground)"));
    assert!(!branch.contains("warp"));
    assert!(!Kind::of(&Intent::MovePointer).needs_focus());
}
