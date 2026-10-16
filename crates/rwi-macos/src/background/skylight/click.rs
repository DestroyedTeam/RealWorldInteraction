//! Local allocation for one explicit left pair. No post, activation or fallback
//! occurs during preparation. The caller owns both events through Sequence::pair.
use super::*;
use core_graphics::event::{CGMouseButton, EventField};

pub(crate) struct PreparedClickEvent {
    driver: &'static Driver,
    event: CGEvent,
    pid: libc::pid_t,
}
impl PreparedClickEvent {
    /// Only the counted pair closure invokes this void SPI. This type is not
    /// Clone; Sequence consumes its down/up values and owns release custody.
    pub(crate) fn post(&self) {
        unsafe { (self.driver.post)(self.pid, self.event.as_ptr().cast()) }
    }
}

pub(crate) fn prepare_click_pair(
    down: &CGEvent,
    up: &CGEvent,
    delivery: &Delivery,
    point: [f64; 2],
) -> Result<(PreparedClickEvent, PreparedClickEvent)> {
    let integer = |event: &CGEvent, field| event.get_integer_value_field(field);
    let location = |event: &CGEvent| {
        let point = event.location();
        [point.x, point.y]
    };
    if down.get_type() as u32 != CGEventType::LeftMouseDown as u32
        || up.get_type() as u32 != CGEventType::LeftMouseUp as u32
        || integer(down, EventField::MOUSE_EVENT_BUTTON_NUMBER) != CGMouseButton::Left as i64
        || integer(up, EventField::MOUSE_EVENT_BUTTON_NUMBER) != CGMouseButton::Left as i64
        || integer(down, EventField::MOUSE_EVENT_CLICK_STATE) != 1
        || integer(up, EventField::MOUSE_EVENT_CLICK_STATE) != 1
        || integer(down, EventField::MOUSE_EVENT_NUMBER) <= 0
        || integer(down, EventField::MOUSE_EVENT_NUMBER)
            != integer(up, EventField::MOUSE_EVENT_NUMBER)
        || integer(down, EventField::EVENT_SOURCE_STATE_ID)
            != integer(up, EventField::EVENT_SOURCE_STATE_ID)
        || location(down) != point
        || location(up) != point
        || delivery.pid == 0
        || delivery.pid > i32::MAX as u32
        || delivery.window == 0
        || !delivery.frame.valid()
        || !point.iter().all(|value| value.is_finite())
        || !delivery.frame.contains(point)
    {
        return Err(fault(
            "invalid_pointer_delivery",
            "One exact finite window-addressed, correlated left down/up pair is required.",
        ));
    }
    let driver = driver().ok_or_else(|| {
        fault(
            "background_delivery_unavailable",
            "Experimental SLEventPostToPid is unavailable; no fallback or input.",
        )
    })?;
    let prepare = |original: &CGEvent| -> Result<PreparedClickEvent> {
        let event = original.clone();
        stamp(&event, delivery, true);
        let local = window_local(point, delivery.frame);
        if !ffi::set_window_location(event.as_ptr().cast(), CGPoint::new(local[0], local[1])) {
            return Err(fault(
                "background_delivery_unavailable",
                "Exact window event addressing is unavailable; no input.",
            ));
        }
        Ok(PreparedClickEvent {
            driver,
            event,
            pid: delivery.pid as libc::pid_t,
        })
    };
    // Release memory and addressing are complete before a guarded down exists.
    Ok((prepare(down)?, prepare(up)?))
}

pub(crate) fn click_capabilities(input_enabled: bool, semantic_only: bool) -> serde_json::Value {
    let spi = available();
    let ready = input_enabled && !semantic_only && spi;
    let reason = if !input_enabled {
        "native input is not enabled"
    } else if semantic_only {
        "semantic-only dialogs have no synthetic-window authority"
    } else if !spi {
        "SLEventPostToPid or window-local addressing SPI is unavailable"
    } else {
        "selected native window supports an opt-in attempted pair; receiving/effect must be independently verified"
    };
    serde_json::json!({
        "implemented":true,"available":ready,"reason":reason,"opt_in_required":true,
        "value":{"delivery":"background_skylight_experimental"},
        "route":"private SLEventPostToPid/window-addressed single left down/up",
        "events":["left_mouse_down","left_mouse_up"],"fallback":false,
        "activation_requested":false,"defocus_requested":false,"primer_events":0,
        "authentication_requested":false,"command_modifier_added":false,
        "timestamp_override":false,"cursor_warp_requested":false,
        "native_acknowledgement":false,"application_effect_attested":false,
        "target_state_preservation_attested":false,
        "scope":"exact selected native binding/geometry/context/lease/permissions/visibility/semantic guards unchanged"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    fn pair() -> (CGEvent, CGEvent) {
        let source = CGEventSource::new(CGEventSourceStateID::Private).unwrap();
        let down = CGEvent::new_mouse_event(
            source.clone(),
            CGEventType::LeftMouseDown,
            CGPoint::new(12., 23.),
            CGMouseButton::Left,
        )
        .unwrap();
        let up = CGEvent::new_mouse_event(
            source,
            CGEventType::LeftMouseUp,
            CGPoint::new(12., 23.),
            CGMouseButton::Left,
        )
        .unwrap();
        for event in [&down, &up] {
            event.set_integer_value_field(EventField::MOUSE_EVENT_NUMBER, 73);
            event.set_integer_value_field(EventField::MOUSE_EVENT_BUTTON_NUMBER, 0);
            event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 1);
        }
        (down, up)
    }
    fn delivery() -> Delivery {
        Delivery {
            pid: 1,
            window: 7,
            frame: crate::geometry::Rect {
                x: 10.,
                y: 20.,
                w: 100.,
                h: 100.,
            },
        }
    }
    #[test]
    fn r92_invalid_click_pair_fails_before_spi_or_post() {
        let (down, up) = pair();
        let target = delivery();
        for bad in [
            Delivery { pid: 0, ..target },
            Delivery {
                pid: u32::MAX,
                ..target
            },
            Delivery {
                window: 0,
                ..target
            },
        ] {
            assert_eq!(
                prepare_click_pair(&down, &up, &bad, [12., 23.])
                    .err()
                    .unwrap()
                    .code,
                "invalid_pointer_delivery"
            );
        }
        for point in [[f64::NAN, 23.], [9., 23.], [110., 23.]] {
            assert_eq!(
                prepare_click_pair(&down, &up, &target, point)
                    .err()
                    .unwrap()
                    .code,
                "invalid_pointer_delivery"
            );
        }
        assert_eq!(
            prepare_click_pair(&up, &down, &target, [12., 23.])
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
        up.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 2);
        assert_eq!(
            prepare_click_pair(&down, &up, &target, [12., 23.])
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
        up.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, 1);
        up.set_integer_value_field(EventField::MOUSE_EVENT_NUMBER, 74);
        assert_eq!(
            prepare_click_pair(&down, &up, &target, [12., 23.])
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
        let (down, up) = pair();
        assert_eq!(
            prepare_click_pair(&down, &up, &target, [13., 23.])
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
    }
    #[test]
    fn r92_prepared_pair_is_memory_only_and_keeps_original_timestamps() {
        use core_graphics::event::CGEventFlags;
        let (down, up) = pair();
        let before = [
            down.get_integer_value_field(58),
            up.get_integer_value_field(58),
        ];
        down.set_flags(CGEventFlags::CGEventFlagCommand);
        up.set_flags(CGEventFlags::CGEventFlagAlternate);
        if !available() {
            assert_eq!(
                prepare_click_pair(&down, &up, &delivery(), [12., 23.])
                    .err()
                    .unwrap()
                    .code,
                "background_delivery_unavailable"
            );
            return;
        }
        let (prepared_down, prepared_up) =
            prepare_click_pair(&down, &up, &delivery(), [12., 23.]).unwrap();
        for (event, timestamp) in [
            (&prepared_down.event, before[0]),
            (&prepared_up.event, before[1]),
        ] {
            assert_eq!(event.get_flags(), CGEventFlags::empty());
            assert_eq!(event.get_integer_value_field(58), timestamp);
            assert_eq!(
                event.get_integer_value_field(EventField::EVENT_TARGET_UNIX_PROCESS_ID),
                1
            );
            assert_eq!(event.get_integer_value_field(51), 7);
        }
        assert_eq!(
            prepared_down.event.get_type() as u32,
            CGEventType::LeftMouseDown as u32
        );
        assert_eq!(
            prepared_up.event.get_type() as u32,
            CGEventType::LeftMouseUp as u32
        );
        // Dropping prepared memory never calls post or changes a receiver.
        drop((prepared_down, prepared_up));
    }
    #[test]
    fn r92_capability_refuses_disabled_input_and_semantic_only_without_claiming_effect() {
        for (enabled, semantic) in [(false, false), (true, true)] {
            let caps = click_capabilities(enabled, semantic);
            assert_eq!(caps["implemented"], true);
            assert_eq!(caps["available"], false);
            assert_eq!(caps["fallback"], false);
            assert_eq!(caps["application_effect_attested"], false);
            assert_eq!(caps["target_state_preservation_attested"], false);
        }
    }
}
