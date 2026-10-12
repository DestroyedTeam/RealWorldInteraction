//! Offscreen content enumeration is allowed only for response-only visuals.
use rwi_core::{Result, fault};
pub(super) fn onscreen_only(max_edge: Option<u32>, include_offscreen: bool) -> Result<bool> {
    if include_offscreen && max_edge.is_none() {
        return Err(fault(
            "invalid_capture_policy",
            "Input-grounding captures retain the on-screen content filter.",
        ));
    }
    Ok(!include_offscreen)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_capture_never_enables_offscreen() {
        assert!(onscreen_only(None, false).unwrap());
        assert_eq!(
            onscreen_only(None, true).unwrap_err().code,
            "invalid_capture_policy"
        );
    }
    #[test]
    fn ordinary_visual_keeps_fast_on_screen_filter() {
        assert!(onscreen_only(Some(2048), false).unwrap());
    }
    #[test]
    fn exact_visual_may_enumerate_offscreen_without_input() {
        assert!(!onscreen_only(Some(2048), true).unwrap());
    }
}
