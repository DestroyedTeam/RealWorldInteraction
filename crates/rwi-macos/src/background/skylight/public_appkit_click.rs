//! Explicit public PID posting of the same plain AppKit-created left pair.
//! Construction/addressing only until counted Sequence::pair calls post().
use super::{Delivery, stamp, window_local};
use crate::ffi;
use core_graphics::{
    event::{CGEvent, CGEventType, CGMouseButton, EventField},
    geometry::CGPoint,
};
use foreign_types::ForeignType;
use rwi_core::{Result, fault};

pub(crate) struct PreparedPublicClickEvent {
    event: CGEvent,
    pid: libc::pid_t,
}
impl PreparedPublicClickEvent {
    #[cfg(test)]
    pub(super) fn number_for_test(&self) -> i64 {
        self.event
            .get_integer_value_field(EventField::MOUSE_EVENT_NUMBER)
    }
    #[cfg(test)]
    pub(super) fn subtype_for_test(&self) -> i64 {
        self.event
            .get_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE)
    }
    // Void public post, not target receipt. No private driver, auth or fallback.
    pub(super) fn post(&self) {
        self.event.post_to_pid(self.pid);
    }
}
pub(super) fn prepare_pair(
    down: &CGEvent,
    up: &CGEvent,
    delivery: &Delivery,
    point: [f64; 2],
) -> Result<(PreparedPublicClickEvent, PreparedPublicClickEvent)> {
    prepare_numbered_pair(down, up, delivery, point, false)
}
pub(super) fn prepare_zero_number_pair(
    down: &CGEvent,
    up: &CGEvent,
    delivery: &Delivery,
    point: [f64; 2],
) -> Result<(PreparedPublicClickEvent, PreparedPublicClickEvent)> {
    prepare_numbered_pair(down, up, delivery, point, true)
}
fn prepare_numbered_pair(
    down: &CGEvent,
    up: &CGEvent,
    delivery: &Delivery,
    point: [f64; 2],
    zero_number: bool,
) -> Result<(PreparedPublicClickEvent, PreparedPublicClickEvent)> {
    let integer = |event: &CGEvent, field| event.get_integer_value_field(field);
    let location = |event: &CGEvent| {
        let p = event.location();
        [p.x, p.y]
    };
    if down.as_ptr() == up.as_ptr()
        || down.get_type() as u32 != CGEventType::LeftMouseDown as u32
        || up.get_type() as u32 != CGEventType::LeftMouseUp as u32
        || integer(down, EventField::MOUSE_EVENT_BUTTON_NUMBER) != CGMouseButton::Left as i64
        || integer(up, EventField::MOUSE_EVENT_BUTTON_NUMBER) != CGMouseButton::Left as i64
        || integer(down, EventField::MOUSE_EVENT_CLICK_STATE) != 1
        || integer(up, EventField::MOUSE_EVENT_CLICK_STATE) != 1
        || if zero_number {
            integer(down, EventField::MOUSE_EVENT_NUMBER) != 0
        } else {
            integer(down, EventField::MOUSE_EVENT_NUMBER) <= 0
        }
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
        || !point.iter().all(|v| v.is_finite())
        || !delivery.frame.contains(point)
    {
        return Err(fault(
            "invalid_pointer_delivery",
            "One exact finite, correlated left pair is required; no public post.",
        ));
    }
    if !crate::background::pointer_delivery_available() {
        return Err(fault(
            "background_delivery_unavailable",
            "Public PID window-local event addressing unavailable; no fallback or input.",
        ));
    }
    let prepare = |original: &CGEvent| -> Result<PreparedPublicClickEvent> {
        // Retain this already-independent factory event; not a new source or an
        // independent copy claim. Both up/down memories exist before any post.
        let event = original.clone();
        stamp(&event, delivery, true);
        let local = window_local(point, delivery.frame);
        if !ffi::set_window_location(event.as_ptr().cast(), CGPoint::new(local[0], local[1])) {
            return Err(fault(
                "background_delivery_unavailable",
                "Exact public PID local addressing unavailable; no input.",
            ));
        }
        Ok(PreparedPublicClickEvent {
            event,
            pid: delivery.pid as libc::pid_t,
        })
    };
    Ok((prepare(down)?, prepare(up)?))
}
pub(crate) fn capabilities(input_enabled: bool, semantic_only: bool) -> serde_json::Value {
    capability(
        input_enabled,
        semantic_only,
        super::appkit_click::public_factory_available(),
        crate::background::pointer_delivery_available(),
    )
}
pub(crate) fn zero_number_capabilities(
    input_enabled: bool,
    semantic_only: bool,
) -> serde_json::Value {
    let mut cap = capabilities(input_enabled, semantic_only);
    cap.as_object_mut()
        .expect("literal capability object")
        .remove("mouse_subtypes");
    cap.as_object_mut()
        .expect("literal capability object")
        .remove("event_sources");
    cap["value"] =
        serde_json::json!({"delivery":"background_public_appkit_zero_number_experimental"});
    cap["event_number"] = serde_json::json!(0);
    cap["event_number_override_requested"] = serde_json::json!(true);
    cap["focus_preservation_verified"] = serde_json::json!(false);
    cap["scope"] = serde_json::json!(
        "explicit ordinary left pair, zero event number only; existing positive default untouched; no focus or app-effect promise"
    );
    cap
}
fn capability(
    input_enabled: bool,
    semantic_only: bool,
    factory: bool,
    window_location: bool,
) -> serde_json::Value {
    serde_json::json!({"implemented":true,"available":input_enabled&&!semantic_only&&factory&&window_location,
        "opt_in_required":true,"value":{"delivery":"background_public_appkit_experimental"},
        "event_sources":{"appkit_factory":{"implemented":true,"available":input_enabled&&!semantic_only&&factory&&window_location,
            "opt_in_required":true,"source_state_id":0,"ordinary_mouse_only":true,
            "physical_user_input":false,"focus_preservation_verified":false,"application_effect_attested":false,"fallback":false}},
        "mouse_subtypes":{"touch":{"implemented":true,"available":input_enabled&&!semantic_only&&factory&&window_location,
            "opt_in_required":true,"event_subtype":3,"name":"NSEventSubtypeTouch",
            "focus_preservation_verified":false,"application_effect_attested":false,
            "command_modifier_added":false,"fallback":false}},
        "route":"public NSEvent mouse factory + explicit CGEventPostToPid/window-addressed plain left pair",
        "private_post_symbol_required":false,"fallback":false,"native_acknowledgement":false,
        "application_effect_attested":false,"target_state_preservation_attested":false,
        "activation_requested":false,"defocus_requested":false,"authentication_requested":false,
        "cursor_warp_requested":false,"command_modifier_added":false,"subtype_override_requested":false,
        "primer_events":0,"timestamp_override":false,"timestamp_preserved_from_quartz_template":true,
        "scope":"explicit selected macOS pointer pair; existing exact binding/lease/permission/custody guards remain",
        "reason":if !input_enabled {"input disabled"} else if semantic_only {"semantic-only target"} else if !factory {"public factory unavailable"} else if !window_location {"window-local addressing unavailable"} else {"ready to attempt; client acceptance unknown"}})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_appkit_capability_does_not_depend_on_private_driver_or_attest_client_effect() {
        let cap = capability(true, false, true, true);
        assert_eq!(cap["available"], true);
        assert_eq!(cap["private_post_symbol_required"], false);
        for c in [
            capability(false, false, true, true),
            capability(true, true, true, true),
            capability(true, false, false, true),
            capability(true, false, true, false),
        ] {
            assert_eq!(c["available"], false);
        }
        for k in [
            "fallback",
            "activation_requested",
            "authentication_requested",
            "command_modifier_added",
            "application_effect_attested",
        ] {
            assert_eq!(cap[k], false);
        }
    }
    #[test]
    fn public_appkit_pair_rejects_alias_invalid_identity_and_nonleft_phases_before_addressing() {
        use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
        let src = CGEventSource::new(CGEventSourceStateID::Private).unwrap();
        let down = CGEvent::new_mouse_event(
            src.clone(),
            CGEventType::LeftMouseDown,
            CGPoint::new(2., 3.),
            CGMouseButton::Left,
        )
        .unwrap();
        let up = CGEvent::new_mouse_event(
            src,
            CGEventType::LeftMouseUp,
            CGPoint::new(2., 3.),
            CGMouseButton::Left,
        )
        .unwrap();
        let delivery = Delivery {
            pid: 1,
            window: 7,
            frame: crate::geometry::Rect {
                x: 0.,
                y: 0.,
                w: 10.,
                h: 10.,
            },
        };
        assert_eq!(
            prepare_pair(&down, &down, &delivery, [2., 3.])
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
        assert_eq!(
            prepare_pair(&down, &up, &Delivery { pid: 0, ..delivery }, [2., 3.])
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
        assert_eq!(
            prepare_pair(&down, &up, &delivery, [f64::NAN, 3.])
                .err()
                .unwrap()
                .code,
            "invalid_pointer_delivery"
        );
        // No .post() in memory tests, regardless of public/private availability.
    }
}
