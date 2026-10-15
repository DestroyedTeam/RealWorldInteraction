//! Diagnostic monotonic wall durations only; not CPU, Host queue or speedup evidence.
use serde_json::{Value, json};
use std::time::{Duration, Instant};
#[derive(Default)]
pub(crate) struct CaptureTimings {
    pub(crate) shareable_request_to_callback_ms: Option<f64>,
    pub(crate) screenshot_request_to_callback_ms: Option<f64>,
    pub(crate) bitmap_conversion_ms: Option<f64>,
    pub(crate) unpremultiply_ms: Option<f64>,
    pub(crate) content_digest_ms: Option<f64>,
    pub(crate) png_encode_ms: Option<f64>,
    pub(crate) caller_total_ms: Option<f64>,
}
pub(super) fn elapsed_ms(started: Instant) -> f64 {
    duration_ms(started.elapsed())
}
fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
impl CaptureTimings {
    pub(crate) fn brief(&self) -> Value {
        json!({"scope":"native_capture_wall", "units":"ms",
            "shareable_request_to_callback_ms":self.shareable_request_to_callback_ms,
            "screenshot_request_to_callback_ms":self.screenshot_request_to_callback_ms,
            "bitmap_conversion_ms":self.bitmap_conversion_ms,
            "unpremultiply_ms":self.unpremultiply_ms,
            "content_digest_ms":self.content_digest_ms,
            "png_encode_ms":self.png_encode_ms,"caller_total_ms":self.caller_total_ms})
    }
    pub(crate) fn details(&self) -> Value {
        json!({"scope":"native_capture_wall", "units":"ms", "cpu_time":false,
            "request_intervals_include_callback_setup":true,
            "phases_do_not_account_for_entire_caller_total":true,
            "shareable_request_to_callback_ms":self.shareable_request_to_callback_ms,
            "screenshot_request_to_callback_ms":self.screenshot_request_to_callback_ms,
            "bitmap_conversion_ms":self.bitmap_conversion_ms,
            "unpremultiply_ms":self.unpremultiply_ms,
            "content_digest_ms":self.content_digest_ms,
            "png_encode_ms":self.png_encode_ms,"caller_total_ms":self.caller_total_ms})
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wall_duration_units_are_exact_and_unknown_is_not_zero() {
        assert_eq!(duration_ms(Duration::from_micros(1250)), 1.25);
        let details = CaptureTimings::default().details();
        for key in [
            "shareable_request_to_callback_ms",
            "screenshot_request_to_callback_ms",
            "bitmap_conversion_ms",
            "unpremultiply_ms",
            "content_digest_ms",
            "png_encode_ms",
            "caller_total_ms",
        ] {
            assert!(details[key].is_null());
        }
        assert_eq!(details["cpu_time"], false);
    }
    #[test]
    fn wall_phase_data_never_claims_complete_accounting_or_cpu() {
        let timing = CaptureTimings {
            bitmap_conversion_ms: Some(0.0),
            png_encode_ms: Some(1.25),
            ..Default::default()
        };
        let details = timing.details();
        assert_eq!(details["bitmap_conversion_ms"], 0.0);
        assert_eq!(details["png_encode_ms"], 1.25);
        assert!(details["caller_total_ms"].is_null());
        assert_eq!(
            details["phases_do_not_account_for_entire_caller_total"],
            true
        );
    }
}
