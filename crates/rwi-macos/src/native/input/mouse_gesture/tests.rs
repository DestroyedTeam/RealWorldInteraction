use super::*;
use serde_json::json;

fn fields(event: &CGEvent) -> serde_json::Value {
    let point = event.location();
    json!({
        "type":event.get_type() as u32,"point":[point.x,point.y],
        "source_state":event.get_integer_value_field(EventField::EVENT_SOURCE_STATE_ID),
        "button":event.get_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER),
        "event_number":event.get_integer_value_field(EventField::MOUSE_EVENT_NUMBER),
        "click_count":event.get_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE),
        "pressure":event.get_double_value_field(EventField::MOUSE_EVENT_PRESSURE),
        "delta":[event.get_integer_value_field(EventField::MOUSE_EVENT_DELTA_X),
            event.get_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y)],
        "flags":event.get_flags().bits()
    })
}

#[test]
fn cg_create_mouse_event_constructor_probe() {
    // Actual public CGCreateMouseEvent, not NSEvent-derived assumptions. No post.
    let shared = source().unwrap();
    let mut rows = Vec::new();
    for (kind, point, button) in [
        (CGEventType::MouseMoved, [10.25, 20.5], CGMouseButton::Left),
        (
            CGEventType::LeftMouseDown,
            [10.25, 20.5],
            CGMouseButton::Left,
        ),
        (
            CGEventType::LeftMouseDragged,
            [27.25, 11.5],
            CGMouseButton::Left,
        ),
        (CGEventType::LeftMouseUp, [27.25, 11.5], CGMouseButton::Left),
        (
            CGEventType::RightMouseDown,
            [10.25, 20.5],
            CGMouseButton::Right,
        ),
        (
            CGEventType::RightMouseUp,
            [10.25, 20.5],
            CGMouseButton::Right,
        ),
    ] {
        let event = CGEvent::new_mouse_event(
            shared.clone(),
            kind,
            CGPoint::new(point[0], point[1]),
            button,
        )
        .unwrap();
        let unmodified = fields(&event);
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_X, 17);
        event.set_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y, -9);
        event.set_integer_value_field(EventField::MOUSE_EVENT_NUMBER, 73);
        event.set_double_value_field(EventField::MOUSE_EVENT_PRESSURE, 0.625);
        rows.push(json!({"defaults":unmodified,"after_public_setters":fields(&event)}));
        assert_eq!(
            event.get_integer_value_field(EventField::MOUSE_EVENT_DELTA_X),
            17
        );
        assert_eq!(
            event.get_integer_value_field(EventField::MOUSE_EVENT_DELTA_Y),
            -9
        );
    }
    println!(
        "{}",
        json!({"probe":"CGEventCreateMouseEvent","scope":"construction_only","event_post_calls":0,"global_warp_calls":0,"rows":rows})
    );
}

#[test]
fn production_routes_use_one_private_source_and_explicit_delta() {
    // Source-level route wiring gate; the independent native probe above checks
    // Quartz behavior. This gate is not a receiver/GUI acceptance test.
    let click = include_str!("../../input.rs");
    let drag = include_str!("../drag.rs");
    assert!(
        click.contains("MouseGesture::new"),
        "click route still constructs a source per event"
    );
    assert!(
        drag.contains("MouseGesture::new"),
        "drag route still constructs a source per event"
    );
    assert!(
        !drag.contains("CGEvent::new_mouse_event"),
        "drag bypasses the shared gesture builder"
    );
    assert!(
        drag.contains("commit_motion"),
        "drag attempted-point/delta lifecycle not connected"
    );
}

fn integer(event: &CGEvent, field: u32) -> i64 {
    event.get_integer_value_field(field)
}

#[test]
fn gesture_fields_share_private_source_and_press_correlation() {
    let mut gesture = MouseGesture::new([10.25, 20.5], CGMouseButton::Left).unwrap();
    let tracking = gesture.tracking_event().unwrap();
    let down = gesture.down_event().unwrap();
    let (moved, motion) = gesture.motion_event([27.25, 11.5]).unwrap();
    gesture.commit_motion(motion);
    let up = gesture.release_event().unwrap();
    let state = integer(&down, EventField::EVENT_SOURCE_STATE_ID);
    let number = integer(&down, EventField::MOUSE_EVENT_NUMBER);
    assert!(number > 0);
    for e in [&tracking, &down, &moved, &up] {
        assert_eq!(integer(e, EventField::EVENT_SOURCE_STATE_ID), state);
        assert_eq!(integer(e, EventField::MOUSE_EVENT_NUMBER), number);
        assert_eq!(integer(e, EventField::MOUSE_EVENT_BUTTON_NUMBER), 0);
        assert_eq!(e.get_flags(), CGEventFlags::empty());
    }
    assert_eq!(
        moved.get_type() as u32,
        CGEventType::LeftMouseDragged as u32
    );
    assert_eq!(integer(&moved, EventField::MOUSE_EVENT_DELTA_X), 17);
    assert_eq!(integer(&moved, EventField::MOUSE_EVENT_DELTA_Y), -9);
    assert_eq!(
        down.get_double_value_field(EventField::MOUSE_EVENT_PRESSURE),
        1.0
    );
    assert_eq!(
        moved.get_double_value_field(EventField::MOUSE_EVENT_PRESSURE),
        1.0
    );
    assert_eq!(
        up.get_double_value_field(EventField::MOUSE_EVENT_PRESSURE),
        0.0
    );
    assert_eq!(integer(&up, EventField::MOUSE_EVENT_CLICK_STATE), 1);
    assert_eq!(integer(&up, EventField::MOUSE_EVENT_DELTA_X), 0);
    assert_eq!(integer(&up, EventField::MOUSE_EVENT_DELTA_Y), 0);
    let next = MouseGesture::new([0.0, 0.0], CGMouseButton::Left)
        .unwrap()
        .down_event()
        .unwrap();
    assert_ne!(integer(&next, EventField::EVENT_SOURCE_STATE_ID), state);
    println!(
        "{}",
        json!({"fixture":"shared_private_gesture","tracking":fields(&tracking),"down":fields(&down),"dragged":fields(&moved),"up":fields(&up),"post_calls":0,"global_warp_calls":0})
    );
}

#[test]
fn click_pairs_share_source_but_each_press_has_a_matching_number() {
    for button in [CGMouseButton::Left, CGMouseButton::Right] {
        let mut gesture = MouseGesture::new([10.0, 20.0], button).unwrap();
        let (first_down, first_up) = gesture.click_pair(1).unwrap();
        let (second_down, second_up) = gesture.click_pair(2).unwrap();
        let state = integer(&first_down, EventField::EVENT_SOURCE_STATE_ID);
        for (count, down, up) in [(1, &first_down, &first_up), (2, &second_down, &second_up)] {
            assert_eq!(
                integer(down, EventField::MOUSE_EVENT_NUMBER),
                integer(up, EventField::MOUSE_EVENT_NUMBER)
            );
            for e in [down, up] {
                assert_eq!(integer(e, EventField::EVENT_SOURCE_STATE_ID), state);
                assert_eq!(
                    integer(e, EventField::MOUSE_EVENT_BUTTON_NUMBER),
                    button as i64
                );
                assert_eq!(integer(e, EventField::MOUSE_EVENT_CLICK_STATE), count);
                assert_eq!(integer(e, EventField::MOUSE_EVENT_DELTA_X), 0);
                assert_eq!(integer(e, EventField::MOUSE_EVENT_DELTA_Y), 0);
            }
            assert_eq!(
                down.get_double_value_field(EventField::MOUSE_EVENT_PRESSURE),
                1.0
            );
            assert_eq!(
                up.get_double_value_field(EventField::MOUSE_EVENT_PRESSURE),
                0.0
            );
        }
        assert_ne!(
            integer(&first_down, EventField::MOUSE_EVENT_NUMBER),
            integer(&second_down, EventField::MOUSE_EVENT_NUMBER)
        );
        assert_eq!(
            first_down.get_type() as u32,
            if matches!(button, CGMouseButton::Right) {
                CGEventType::RightMouseDown
            } else {
                CGEventType::LeftMouseDown
            } as u32
        );
        println!(
            "{}",
            json!({"fixture":"bounded_click_pairs","first_down":fields(&first_down),"first_up":fields(&first_up),"second_down":fields(&second_down),"second_up":fields(&second_up)})
        );
    }
}

#[test]
fn fractional_delta_is_carried_only_after_attempt_commit() {
    let mut gesture = MouseGesture::new([0.0, 0.0], CGMouseButton::Left).unwrap();
    let (uncommitted, _) = gesture.motion_event([0.4, 0.4]).unwrap();
    assert_eq!(integer(&uncommitted, EventField::MOUSE_EVENT_DELTA_X), 0);
    assert_eq!(gesture.last_point(), [0.0, 0.0]);
    let mut sum = [0; 2];
    for point in [[0.4, 0.4], [0.8, 0.8], [1.2, 1.2], [0.0, 0.0]] {
        let (e, motion) = gesture.motion_event(point).unwrap();
        sum[0] += integer(&e, EventField::MOUSE_EVENT_DELTA_X);
        sum[1] += integer(&e, EventField::MOUSE_EVENT_DELTA_Y);
        gesture.commit_motion(motion);
    }
    assert_eq!(sum, [0, 0]);
    assert_eq!(gesture.last_point(), [0.0, 0.0]);
}

#[test]
fn invalid_motion_preserves_last_attempted_release_point() {
    let gesture = MouseGesture::new([10.0, 20.0], CGMouseButton::Left).unwrap();
    assert!(gesture.motion_event([f64::NAN, 21.0]).is_err());
    assert!(
        gesture
            .motion_event([f64::from(i32::MAX) + 100.0, 21.0])
            .is_err()
    );
    let up = gesture.release_event().unwrap();
    assert_eq!([up.location().x, up.location().y], [10.0, 20.0]);
    assert_eq!(up.get_type() as u32, CGEventType::LeftMouseUp as u32);
    assert_eq!(
        up.get_double_value_field(EventField::MOUSE_EVENT_PRESSURE),
        0.0
    );
}

#[test]
fn owned_events_retain_their_private_source_after_gesture_drop() {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventCreateSourceFromEvent(
            event: *const std::ffi::c_void,
        ) -> *mut core_graphics::sys::CGEventSource;
    }
    let gesture = MouseGesture::new([0.0, 0.0], CGMouseButton::Left).unwrap();
    let event = gesture.down_event().unwrap();
    let state = integer(&event, EventField::EVENT_SOURCE_STATE_ID);
    drop(gesture);
    // Public create-rule source extraction is RAII released after this probe.
    let raw = unsafe { CGEventCreateSourceFromEvent(event.as_ptr().cast()) };
    assert!(!raw.is_null());
    let retained = unsafe { CGEventSource::from_ptr(raw) };
    let next = CGEvent::new_mouse_event(
        retained,
        CGEventType::LeftMouseUp,
        CGPoint::new(0.0, 0.0),
        CGMouseButton::Left,
    )
    .unwrap();
    assert_eq!(integer(&next, EventField::EVENT_SOURCE_STATE_ID), state);
}

#[test]
fn cancelled_or_uncertain_trajectory_keeps_one_source_and_release_only_cleanup() {
    use rwi_core::{
        input_trajectory::{self, TrajectorySink},
        operation::{self, Operation, Registry},
        trajectory::{DragPath, Waypoint},
    };
    struct ProbeSink {
        gesture: MouseGesture,
        events: Vec<CGEvent>,
        now: u64,
        cancel: Option<Operation>,
        fail_move: bool,
        fail_release: bool,
    }
    impl TrajectorySink for ProbeSink {
        fn now_ms(&self) -> u64 {
            self.now
        }
        fn sleep_ms(&mut self, ms: u64) {
            self.now += ms;
        }
        fn validate_point(&mut self, _: [f64; 2]) -> Result<()> {
            Ok(())
        }
        fn guard_point(&mut self, _: [f64; 2], _: bool) -> Result<()> {
            Ok(())
        }
        fn press_left(&mut self, _: [f64; 2]) -> Result<()> {
            self.events.push(self.gesture.down_event()?);
            if let Some(op) = &self.cancel {
                op.cancel();
            }
            Ok(())
        }
        fn move_held(&mut self, point: [f64; 2]) -> Result<()> {
            let (event, motion) = self.gesture.motion_event(point)?;
            self.events.push(event);
            self.gesture.commit_motion(motion);
            if self.fail_move {
                Err(fault(
                    "fixture_move_uncertain",
                    "attempt recorded; no replay",
                ))
            } else {
                Ok(())
            }
        }
        fn release_left(&mut self) -> Result<()> {
            self.events.push(self.gesture.release_event()?);
            if self.fail_release {
                Err(fault("fixture_release_uncertain", "release attempted once"))
            } else {
                Ok(())
            }
        }
    }
    let path = DragPath {
        waypoints: vec![
            Waypoint {
                at_ms: 0,
                offset: [0.0, 0.0],
            },
            Waypoint {
                at_ms: 32,
                offset: [17.0, -9.0],
            },
        ],
    };
    for mode in 0..3 {
        let op = Registry::default().begin(None).unwrap();
        let mut sink = ProbeSink {
            gesture: MouseGesture::new([10.0, 20.0], CGMouseButton::Left).unwrap(),
            events: vec![],
            now: 0,
            cancel: (mode == 0).then(|| op.clone()),
            fail_move: mode == 1,
            fail_release: mode == 2,
        };
        let result = operation::run(&op, || {
            input_trajectory::execute(&path, [10.0, 20.0], &mut sink)
        });
        assert!(result.is_err());
        let kinds = sink
            .events
            .iter()
            .map(|e| e.get_type() as u32)
            .collect::<Vec<_>>();
        assert_eq!(kinds.iter().filter(|kind| **kind == 1).count(), 1);
        assert_eq!(kinds.iter().filter(|kind| **kind == 2).count(), 1);
        assert_eq!(kinds.last(), Some(&2));
        let state = integer(&sink.events[0], EventField::EVENT_SOURCE_STATE_ID);
        let number = integer(&sink.events[0], EventField::MOUSE_EVENT_NUMBER);
        for event in &sink.events {
            assert_eq!(integer(event, EventField::EVENT_SOURCE_STATE_ID), state);
            assert_eq!(integer(event, EventField::MOUSE_EVENT_NUMBER), number);
        }
        let up = sink.events.last().unwrap();
        assert_eq!(
            up.get_double_value_field(EventField::MOUSE_EVENT_PRESSURE),
            0.0
        );
        assert_eq!(
            [up.location().x, up.location().y],
            sink.gesture.last_point()
        );
        if mode == 0 {
            assert_eq!(kinds, [1, 2]);
        }
        op.finish();
        if mode == 2 {
            assert!(op.status().resource_release_uncertain);
            assert!(!op.status().resources_released);
        }
        println!(
            "{}",
            json!({"fixture":"release_only_cleanup","mode":mode,"types":kinds,"source_state":state,"event_number":number,"release":fields(up),"actual_post_calls":0,"actual_warp_calls":0})
        );
    }
}

#[test]
fn production_move_pointer_route_is_nonactivating_and_guarded() {
    // Wiring regression only, not a receiver or hover-goal assertion.
    let input = include_str!("../../input.rs");
    assert!(
        input.contains("Intent::MovePointer =>"),
        "MovePointer native route missing"
    );
    assert!(
        input.contains("move_pointer::guarded_single"),
        "MovePointer bypasses guarded single delivery"
    );
    assert!(
        input.contains("!matches!(intent, Intent::MovePointer)"),
        "MovePointer can prepare/activate an app receiver"
    );
}
