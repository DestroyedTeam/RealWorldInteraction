//! Read already-owned CG fields after the ordinary post returns. No GUI read,
//! event tap, receiver acknowledgement, retry, extra post or cancellation gate.
use super::*;
use rwi_core::pointer_dispatch::PointerPost;

pub(super) fn sample(event: &CGEvent, started: Instant) -> PointerPost {
    let point = event.location();
    PointerPost {
        event_type: event.get_type() as u32,
        point: [point.x, point.y],
        click_count: event.get_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE),
        event_number: event.get_integer_value_field(EventField::MOUSE_EVENT_NUMBER),
        source_state_id: event.get_integer_value_field(EventField::EVENT_SOURCE_STATE_ID),
        target_pid: event.get_integer_value_field(EventField::EVENT_TARGET_UNIX_PROCESS_ID),
        target_window: event.get_integer_value_field(crate::background::TARGET_WINDOW_NUMBER_FIELD),
        flags: event.get_flags().bits(),
        pressure: event.get_double_value_field(EventField::MOUSE_EVENT_PRESSURE),
        post_return_elapsed_us: started.elapsed().as_micros().min(u128::from(u64::MAX)) as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rwi_core::pointer_dispatch::{PointerDispatch, ReceiverEffect};

    #[test]
    fn f405_double_click_constructor_transcript_is_not_receiver_evidence() {
        let mut gesture = MouseGesture::new([180.25, 245.5], CGMouseButton::Left).unwrap();
        let mut trace = PointerDispatch::background_window();
        let start = Instant::now();
        for n in 1..=2 {
            let (down, up) = gesture.click_pair(n).unwrap();
            // Construction-only samples. This fixture deliberately NEVER calls
            // deliver_pointer or any Post API, and does not claim real posts.
            trace.record_returned(sample(&down, start));
            trace.record_returned(sample(&up, start));
        }
        let events = &trace.events;
        assert_eq!(
            events.iter().map(|e| e.event_type).collect::<Vec<_>>(),
            [1, 2, 1, 2]
        );
        assert_eq!(
            events.iter().map(|e| e.click_count).collect::<Vec<_>>(),
            [1, 1, 2, 2]
        );
        assert_eq!(events[0].event_number, events[1].event_number);
        assert_eq!(events[2].event_number, events[3].event_number);
        assert_ne!(events[0].event_number, events[2].event_number);
        assert!(
            events
                .iter()
                .all(|e| e.point == [180.25, 245.5] && e.flags == 0)
        );
        assert!(
            events
                .iter()
                .all(|e| e.source_state_id == events[0].source_state_id)
        );
        assert!(!matches!(events[0].source_state_id, 0 | 1));
        assert_eq!(trace.receiver_effect, ReceiverEffect::Unobserved);
        println!(
            "F405_POINTER constructor_only=true; real_post_calls=0; types=1,2,1,2; click_counts=1,1,2,2; receiver_effect=unobserved"
        );
    }
}
