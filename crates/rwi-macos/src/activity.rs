//! Recent session activity for cooperative yielding. A CoreGraphics state
//! table is not a physical-human attestation and carries no receiving window.
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state: u32, event_type: u32) -> f64;
}

/// kCGEventSourceStateCombinedSessionState: *all* sources in this session.
const COMBINED_SESSION: u32 = 0;
const HID_SYSTEM: u32 = 1;
/// kCGEventLeftMouseDown, kCGEventRightMouseDown, kCGEventOtherMouseDown,
/// kCGEventKeyDown. Motion alone does not gate: a jiggler must not block work.
const CLICKS_AND_KEYS: [u32; 4] = [1, 3, 25, 10];

/// Seconds since the last session click/key; source and receiver are unknown.
pub(crate) fn seconds_since_user_input() -> Option<f64> {
    seconds_since_input(COMBINED_SESSION)
}

pub(crate) fn seconds_since_hid_input() -> Option<f64> {
    seconds_since_input(HID_SYSTEM)
}

fn seconds_since_input(state: u32) -> Option<f64> {
    let mut minimum = f64::INFINITY;
    for event_type in CLICKS_AND_KEYS {
        let seconds = unsafe { CGEventSourceSecondsSinceLastEventType(state, event_type) };
        if seconds.is_finite() && seconds >= 0.0 {
            minimum = minimum.min(seconds);
        }
    }
    minimum.is_finite().then_some(minimum)
}

/// Pure policy: recent activity in the selected resource still yields.
pub(crate) fn gate(seconds: Option<f64>, threshold_seconds: f64) -> Option<f64> {
    let seconds = seconds?;
    (seconds.is_finite() && seconds >= 0.0 && seconds < threshold_seconds).then_some(seconds)
}

/// The yield decision for the current session.
pub(crate) fn user_active(threshold_seconds: f64) -> Option<f64> {
    gate(seconds_since_user_input(), threshold_seconds)
}

/// Another exact window of the same application is not this input resource.
/// Unknown window identity retains the previous conservative PID-level yield.
pub(crate) fn same_input_resource(
    selected_pid: u32,
    selected_window: u32,
    foreground_pid: Option<u32>,
    focused_window: Option<u32>,
    native_window_isolated: bool,
) -> bool {
    foreground_pid == Some(selected_pid)
        && (!native_window_isolated
            || focused_window.is_none_or(|window| window == selected_window))
}

#[cfg(test)]
mod tests {
    use super::{gate, same_input_resource};
    #[test]
    fn the_gate_yields_only_for_finite_recent_activity() {
        assert_eq!(gate(Some(0.2), 1.2), Some(0.2));
        assert_eq!(gate(Some(1.19), 1.2), Some(1.19));
        assert_eq!(gate(Some(1.2), 1.2), None);
        assert_eq!(gate(Some(30.0), 1.2), None);
        assert_eq!(gate(None, 1.2), None);
        assert_eq!(gate(Some(f64::NAN), 1.2), None);
        assert_eq!(gate(Some(-1.0), 1.2), None);
    }

    #[test]
    fn exact_sibling_window_does_not_inherit_the_selected_windows_activity() {
        assert!(same_input_resource(7, 10, Some(7), Some(10), true));
        assert!(!same_input_resource(7, 10, Some(7), Some(11), true));
        assert!(!same_input_resource(7, 10, Some(8), Some(10), true));
        assert!(same_input_resource(7, 10, Some(7), None, true));
        assert!(!same_input_resource(7, 10, None, None, true));
        // Synthetic delivery may prepare this application's keyboard receiver;
        // do not waive a possible same-app conflict without isolation evidence.
        assert!(same_input_resource(7, 10, Some(7), Some(11), false));
    }
}
