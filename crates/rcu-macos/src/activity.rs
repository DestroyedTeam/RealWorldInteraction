//! Recent *human* input detection for cooperative yielding. RCU never
//! synthesizes user input and never moves the real cursor; this only observes
//! how long ago the session last saw a real click or key press.
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state: u32, event_type: u32) -> f64;
}

/// kCGEventSourceStateCombinedSessionState: hardware events of the session.
const COMBINED_SESSION: u32 = 0;
/// kCGEventLeftMouseDown, kCGEventRightMouseDown, kCGEventOtherMouseDown,
/// kCGEventKeyDown. Motion alone does not gate: a jiggler must not block work.
const CLICKS_AND_KEYS: [u32; 4] = [1, 3, 25, 10];

/// Seconds since the last human click/key, when the platform can tell.
pub(crate) fn seconds_since_user_input() -> Option<f64> {
    let mut minimum = f64::INFINITY;
    for event_type in CLICKS_AND_KEYS {
        let seconds =
            unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION, event_type) };
        if seconds.is_finite() && seconds >= 0.0 {
            minimum = minimum.min(seconds);
        }
    }
    minimum.is_finite().then_some(minimum)
}

/// Pure policy: yield while a person is actively clicking or typing.
pub(crate) fn gate(seconds: Option<f64>, threshold_seconds: f64) -> Option<f64> {
    let seconds = seconds?;
    (seconds.is_finite() && seconds >= 0.0 && seconds < threshold_seconds).then_some(seconds)
}

/// The yield decision for the current session.
pub(crate) fn user_active(threshold_seconds: f64) -> Option<f64> {
    gate(seconds_since_user_input(), threshold_seconds)
}

#[cfg(test)]
mod tests {
    use super::gate;
    #[test]
    fn the_gate_yields_only_while_a_person_is_actually_active() {
        assert_eq!(gate(Some(0.2), 1.2), Some(0.2));
        assert_eq!(gate(Some(1.19), 1.2), Some(1.19));
        assert_eq!(gate(Some(1.2), 1.2), None);
        assert_eq!(gate(Some(30.0), 1.2), None);
        assert_eq!(gate(None, 1.2), None);
        assert_eq!(gate(Some(f64::NAN), 1.2), None);
        assert_eq!(gate(Some(-1.0), 1.2), None);
    }
}
