//! Bounded receiving-client trace. No event sender, input fallback or OS authority.
use serde::Serialize;

pub const MAX_EVENTS: usize = 256;
pub const MAX_UNITS: usize = 64;
pub const MAX_INPUT_SOURCE_UNITS: usize = 128;

#[derive(Debug, Serialize)]
pub struct Units {
    pub total_utf16: usize,
    pub raw_utf16: Vec<u16>,
    pub truncated: bool,
}
impl Units {
    pub fn new(total_utf16: usize, raw_utf16: Vec<u16>) -> Self {
        let raw_utf16: Vec<_> = raw_utf16.into_iter().take(MAX_UNITS).collect();
        Self {
            truncated: total_utf16 > raw_utf16.len(),
            total_utf16,
            raw_utf16,
        }
    }
}

/// Read-only bracket of the receiving NSTextView's real marked-text state.
/// Ranges are UTF-16 offsets; unavailable/unstable samples are never a zero range.
#[derive(Debug, Serialize)]
pub struct MarkedTextState {
    pub before: bool,
    pub after: bool,
    pub has_marked_text: Option<bool>,
    pub range_utf16: Option<[u64; 2]>,
    pub range_unavailable_reason: Option<&'static str>,
}
impl MarkedTextState {
    pub fn sample(before: bool, range: [u64; 2], after: bool) -> Self {
        // Preserve exact values in JSON consumers without rounding NSNotFound,
        // overflowing ranges, or pretending an unmarked sample starts at zero.
        const JSON_SAFE_INTEGER: u64 = (1u64 << 53) - 1;
        let (has_marked_text, range_utf16, reason) = if before != after {
            (None, None, Some("changed_during_sample"))
        } else if !before {
            (Some(false), None, Some("not_marked"))
        } else if range[0]
            .checked_add(range[1])
            .is_some_and(|end| end <= JSON_SAFE_INTEGER)
        {
            (Some(true), Some(range), None)
        } else {
            (Some(true), None, Some("invalid_or_unrepresentable_range"))
        };
        Self {
            before,
            after,
            has_marked_text,
            range_utf16,
            range_unavailable_reason: reason,
        }
    }
}

/// Own field-editor context metadata, not the system TIS or an IME mode proof.
/// Missing, mismatched or over-budget sources never become an empty identifier.
#[derive(Debug, Serialize)]
pub struct InputContextSource {
    pub context_present: bool,
    pub client_is_editor: Option<bool>,
    pub selected_source: Option<String>,
    pub unavailable_reason: Option<&'static str>,
}
impl InputContextSource {
    pub fn sample(
        context_present: bool,
        client_is_editor: Option<bool>,
        total_utf16: Option<usize>,
        units: &[u16],
    ) -> Self {
        let reason = if !context_present {
            Some("context_unavailable")
        } else if client_is_editor == Some(false) {
            Some("client_mismatch")
        } else if client_is_editor.is_none() {
            Some("client_unavailable")
        } else {
            match total_utf16 {
                None => Some("selected_source_unavailable"),
                Some(0) => Some("empty_identifier"),
                Some(n) if n > MAX_INPUT_SOURCE_UNITS => Some("identifier_over_budget"),
                Some(n) if n != units.len() => Some("incomplete_identifier"),
                Some(_) => None,
            }
        };
        let (selected_source, unavailable_reason) = if let Some(reason) = reason {
            (None, Some(reason))
        } else {
            match String::from_utf16(units) {
                Ok(identifier) => (Some(identifier), None),
                Err(_) => (None, Some("invalid_utf16_identifier")),
            }
        };
        Self {
            context_present,
            client_is_editor: context_present.then_some(client_is_editor).flatten(),
            selected_source,
            unavailable_reason,
        }
    }
}

/// Public per-process currently activated context, sampled read-only. A selected
/// source on an editor's context is not proof that this context is current.
#[derive(Debug, Serialize)]
pub struct CurrentInputContext {
    pub api_available: bool,
    pub context_present: Option<bool>,
    pub client_present: Option<bool>,
    pub client_is_editor: Option<bool>,
    pub unavailable_reason: Option<&'static str>,
}
impl CurrentInputContext {
    pub fn sample(api: bool, present: bool, client: bool, own: bool) -> Self {
        Self {
            api_available: api,
            context_present: api.then_some(present),
            client_present: (api && present).then_some(client),
            client_is_editor: (api && present && client).then_some(own),
            unavailable_reason: if !api {
                Some("current_context_api_unavailable")
            } else if !present {
                Some("no_current_activated_context")
            } else if !client {
                Some("current_context_client_unavailable")
            } else {
                None
            },
        }
    }
}

/// Separately sampled public AppKit properties, never an atomic focus proof.
#[derive(Debug, Serialize)]
pub struct Context {
    pub application_active: bool,
    pub window_key: bool,
    pub window_main: bool,
    pub first_responder_is_editor: bool,
    pub marked_text: MarkedTextState,
    /// Only sampled after keyDown's superclass returns. None means not sampled.
    pub input_context_source_after_key: Option<InputContextSource>,
    /// Sampled only while interpreting a real delivered key or after keyDown.
    /// None means unsampled. No inputContext getter, activation or mode proof.
    pub current_input_context: Option<CurrentInputContext>,
}

#[derive(Debug, Serialize)]
pub struct Event {
    pub sequence: u64,
    pub elapsed_ns: u64,
    pub phase: &'static str,
    pub characters: Option<Units>,
    pub native_event_type: Option<usize>,
    pub key_code: Option<u16>,
    pub modifier_flags: Option<usize>,
    pub command: Option<String>,
    pub replacement_range: Option<[usize; 2]>,
    pub selected_range: Option<[usize; 2]>,
    pub context: Context,
}
impl Event {
    pub fn new(phase: &'static str, elapsed_ns: u64, context: Context) -> Self {
        Self {
            sequence: 0,
            elapsed_ns,
            phase,
            characters: None,
            native_event_type: None,
            key_code: None,
            modifier_flags: None,
            command: None,
            replacement_range: None,
            selected_range: None,
            context,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Trace {
    pub scope: &'static str,
    pub pid: u32,
    pub window: u32,
    pub field_identifier: &'static str,
    pub field_editor_requests: u64,
    pub total_callbacks: u64,
    pub dropped_events: u64,
    pub ignored_out_of_scope_callbacks: u64,
    pub observer_panics: u64,
    pub events: Vec<Event>,
}
impl Trace {
    pub fn new(pid: u32, window: u32) -> Self {
        Self {
            scope: "own_opt_in_NSTextView_field_editor_public_super_callbacks_not_Chrome",
            pid,
            window,
            field_identifier: "rwi-fixture-input",
            field_editor_requests: 0,
            total_callbacks: 0,
            dropped_events: 0,
            ignored_out_of_scope_callbacks: 0,
            observer_panics: 0,
            events: Vec::new(),
        }
    }
    pub fn push(&mut self, mut event: Event) {
        self.total_callbacks = self.total_callbacks.saturating_add(1);
        event.sequence = self.total_callbacks;
        if self.events.len() < MAX_EVENTS {
            self.events.push(event);
        } else {
            self.dropped_events = self.dropped_events.saturating_add(1);
        }
    }
}

#[cfg(target_os = "macos")]
#[path = "text_client_native.rs"]
pub mod native;

#[cfg(test)]
mod f288_tests {
    use super::CurrentInputContext;
    #[test]
    fn f288_api_unavailable_does_not_fabricate_absence() {
        let x = CurrentInputContext::sample(false, true, true, true);
        assert!(!x.api_available);
        assert_eq!(x.context_present, None);
        assert_eq!(x.client_present, None);
        assert_eq!(x.client_is_editor, None);
        assert_eq!(
            x.unavailable_reason,
            Some("current_context_api_unavailable")
        );
    }
    #[test]
    fn f288_nil_context_is_known_absence_not_client_mismatch() {
        let x = CurrentInputContext::sample(true, false, true, true);
        assert_eq!(x.context_present, Some(false));
        assert_eq!(x.client_present, None);
        assert_eq!(x.client_is_editor, None);
        assert_eq!(x.unavailable_reason, Some("no_current_activated_context"));
    }
    #[test]
    fn f288_nil_client_keeps_match_unknown() {
        let x = CurrentInputContext::sample(true, true, false, true);
        assert_eq!(x.context_present, Some(true));
        assert_eq!(x.client_present, Some(false));
        assert_eq!(x.client_is_editor, None);
        assert_eq!(
            x.unavailable_reason,
            Some("current_context_client_unavailable")
        );
    }
    #[test]
    fn f288_other_client_is_observed_mismatch_not_nil() {
        let x = CurrentInputContext::sample(true, true, true, false);
        assert_eq!(x.client_present, Some(true));
        assert_eq!(x.client_is_editor, Some(false));
        assert_eq!(x.unavailable_reason, None);
    }
    #[test]
    fn f288_own_current_client_is_identity_only() {
        let x = serde_json::to_value(CurrentInputContext::sample(true, true, true, true)).unwrap();
        assert_eq!(x["client_is_editor"], true);
        for key in [
            "goal_verified",
            "ime_effect",
            "source_selected",
            "input_sent",
            "os_foreground",
        ] {
            assert!(x.get(key).is_none());
        }
    }
    #[test]
    fn f288_unknown_and_known_false_serialize_differently() {
        let unknown =
            serde_json::to_value(CurrentInputContext::sample(false, false, false, false)).unwrap();
        let absent =
            serde_json::to_value(CurrentInputContext::sample(true, false, false, false)).unwrap();
        assert!(unknown["context_present"].is_null());
        assert_eq!(absent["context_present"], false);
        assert!(absent["client_is_editor"].is_null());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Options;

    fn parse(extra: &[&str]) -> Result<Option<Options>, String> {
        Options::parse(
            [
                "--open-window",
                "--run-id",
                "trace",
                "--state-dir",
                "/tmp/rwi-trace-check",
                "--display-id",
                "4",
            ]
            .into_iter()
            .chain(extra.iter().copied())
            .map(str::to_owned),
        )
    }
    #[test]
    fn trace_default_is_off_and_duplicate_opt_in_is_rejected() {
        assert!(!parse(&[]).unwrap().unwrap().text_client_trace);
        assert!(
            parse(&[
                "--regular-app",
                "--environment-sample-ms",
                "25",
                "--text-client-trace"
            ])
            .unwrap()
            .unwrap()
            .text_client_trace
        );
        assert!(
            parse(&[
                "--regular-app",
                "--environment-sample-ms",
                "25",
                "--text-client-trace",
                "--text-client-trace"
            ])
            .is_err()
        );
    }
    #[test]
    fn trace_requires_regular_samples_and_no_input_actor_or_other_surface() {
        for extra in [
            vec!["--text-client-trace"],
            vec!["--regular-app", "--text-client-trace"],
            vec!["--environment-sample-ms", "25", "--text-client-trace"],
        ] {
            assert!(parse(&extra).is_err());
        }
        for flag in [
            "--modal-controls",
            "--system-panel-controls",
            "--native-dnd-controls",
            "--cover-controls",
            "--semantic-alternate-ui",
            "--live-freshness-ticks",
        ] {
            assert!(
                parse(&[
                    "--regular-app",
                    "--environment-sample-ms",
                    "25",
                    "--text-client-trace",
                    flag
                ])
                .is_err(),
                "{flag}"
            );
        }
        assert!(
            parse(&[
                "--regular-app",
                "--environment-sample-ms",
                "25",
                "--text-client-trace",
                "--self-foreground-after-ms",
                "1000"
            ])
            .is_err()
        );
    }
    #[test]
    fn raw_units_keep_non_bmp_combining_zwj_and_unpaired_units_without_normalizing() {
        for units in [
            "后🙂𝄞 é👩‍💻".encode_utf16().collect::<Vec<_>>(),
            vec![0xd800],
            vec![0xdc00],
            vec![0, 65],
        ] {
            let value = Units::new(units.len(), units.clone());
            assert_eq!(value.raw_utf16, units);
            assert!(!value.truncated);
        }
    }
    #[test]
    fn unit_truncation_and_empty_text_remain_explicit() {
        let value = Units::new(100, vec![65; 100]);
        assert_eq!(value.raw_utf16.len(), MAX_UNITS);
        assert!(value.truncated);
        let empty = Units::new(0, vec![]);
        assert!(empty.raw_utf16.is_empty() && !empty.truncated);
    }
    #[test]
    fn callback_overflow_is_bounded_counted_and_never_relabelled_complete() {
        let mut trace = Trace::new(7, 9);
        for _ in 0..MAX_EVENTS + 3 {
            trace.push(Event::new(
                "key_down_enter",
                1,
                Context {
                    application_active: false,
                    window_key: false,
                    window_main: false,
                    first_responder_is_editor: true,
                    marked_text: MarkedTextState::sample(false, [u64::MAX, 0], false),
                    input_context_source_after_key: None,
                    current_input_context: None,
                },
            ));
        }
        assert_eq!(trace.events.len(), MAX_EVENTS);
        assert_eq!(trace.total_callbacks, (MAX_EVENTS + 3) as u64);
        assert_eq!(trace.dropped_events, 3);
        assert_eq!(trace.events[0].sequence, 1);
        assert_eq!(trace.events.last().unwrap().sequence, MAX_EVENTS as u64);
        assert!(
            !serde_json::to_string(&trace)
                .unwrap()
                .contains("verified_goal")
        );
    }

    #[test]
    fn marked_state_unmarked_has_no_fabricated_zero_range() {
        let sample = MarkedTextState::sample(false, [isize::MAX as u64, 0], false);
        assert_eq!(sample.has_marked_text, Some(false));
        assert_eq!(sample.range_utf16, None);
        assert_eq!(sample.range_unavailable_reason, Some("not_marked"));
    }
    #[test]
    fn marked_state_keeps_exact_utf16_range() {
        let sample = MarkedTextState::sample(true, [1, 2], true);
        assert_eq!(sample.has_marked_text, Some(true));
        assert_eq!(sample.range_utf16, Some([1, 2]));
        assert_eq!(sample.range_unavailable_reason, None);
    }
    #[test]
    fn marked_state_drift_is_unknown_and_keeps_both_native_samples() {
        for (before, after) in [(false, true), (true, false)] {
            let sample = MarkedTextState::sample(before, [0, 1], after);
            assert_eq!((sample.before, sample.after), (before, after));
            assert_eq!(sample.has_marked_text, None);
            assert_eq!(sample.range_utf16, None);
            assert_eq!(
                sample.range_unavailable_reason,
                Some("changed_during_sample")
            );
        }
    }
    #[test]
    fn marked_state_rejects_not_found_overflow_and_json_rounding() {
        for range in [
            [isize::MAX as u64, 0],
            [u64::MAX, 1],
            [1u64 << 53, 0],
            [(1u64 << 53) - 1, 1],
        ] {
            let sample = MarkedTextState::sample(true, range, true);
            assert_eq!(sample.has_marked_text, Some(true));
            assert_eq!(sample.range_utf16, None);
            assert_eq!(
                sample.range_unavailable_reason,
                Some("invalid_or_unrepresentable_range")
            );
        }
    }
    #[test]
    fn marked_state_serialization_distinguishes_unknown_without_claiming_effect() {
        let sample = MarkedTextState::sample(false, [0, 1], true);
        let value = serde_json::to_value(sample).unwrap();
        assert!(value["has_marked_text"].is_null());
        assert!(value["range_utf16"].is_null());
        assert_eq!(value["before"], false);
        assert_eq!(value["after"], true);
        assert!(value.get("goal_verified").is_none());
        let edge = MarkedTextState::sample(true, [(1u64 << 53) - 1, 0], true);
        assert_eq!(edge.range_utf16, Some([(1u64 << 53) - 1, 0]));
    }

    #[test]
    fn input_context_source_requires_present_context_and_exact_own_client() {
        let units: Vec<_> = "com.apple.keylayout.ABC".encode_utf16().collect();
        for (present, own, reason) in [
            (false, Some(true), "context_unavailable"),
            (true, Some(false), "client_mismatch"),
            (true, None, "client_unavailable"),
        ] {
            let value = InputContextSource::sample(present, own, Some(units.len()), &units);
            assert!(value.selected_source.is_none());
            assert_eq!(value.unavailable_reason, Some(reason));
            if !present {
                assert_eq!(value.client_is_editor, None);
            }
        }
    }

    #[test]
    fn input_context_source_nil_and_empty_are_distinct_unknowns() {
        for (length, reason) in [
            (None, "selected_source_unavailable"),
            (Some(0), "empty_identifier"),
        ] {
            let value = InputContextSource::sample(true, Some(true), length, &[]);
            assert_eq!(value.unavailable_reason, Some(reason));
            assert_eq!(value.selected_source, None);
        }
    }

    #[test]
    fn input_context_source_keeps_exact_identifier_without_interpreting_mode() {
        for source in ["com.bytedance.inputmethod.doubaoime.pinyin", "exact-🙂-é"] {
            let units: Vec<_> = source.encode_utf16().collect();
            let value = InputContextSource::sample(true, Some(true), Some(units.len()), &units);
            assert_eq!(value.selected_source.as_deref(), Some(source));
            assert_eq!(value.unavailable_reason, None);
        }
    }

    #[test]
    fn input_context_source_does_not_publish_an_over_budget_prefix() {
        let units = vec![65; MAX_INPUT_SOURCE_UNITS];
        let value = InputContextSource::sample(true, Some(true), Some(units.len() + 1), &units);
        assert_eq!(value.selected_source, None);
        assert_eq!(value.unavailable_reason, Some("identifier_over_budget"));
        let exact = InputContextSource::sample(true, Some(true), Some(units.len()), &units);
        assert_eq!(exact.selected_source.unwrap().len(), MAX_INPUT_SOURCE_UNITS);
    }

    #[test]
    fn input_context_source_invalid_and_incomplete_utf16_are_unknown() {
        for (length, units, reason) in [
            (1, vec![0xd800], "invalid_utf16_identifier"),
            (2, vec![65], "incomplete_identifier"),
            (1, vec![65, 66], "incomplete_identifier"),
        ] {
            let value = InputContextSource::sample(true, Some(true), Some(length), &units);
            assert_eq!(value.selected_source, None);
            assert_eq!(value.unavailable_reason, Some(reason));
        }
    }

    #[test]
    fn input_context_source_json_never_claims_ime_or_insertion_effect() {
        let value = InputContextSource::sample(true, Some(true), None, &[]);
        let json = serde_json::to_value(value).unwrap();
        assert!(json["selected_source"].is_null());
        assert!(json.get("ime_mode").is_none());
        assert!(json.get("goal_verified").is_none());
        assert!(json.get("input_sent").is_none());
    }
}
