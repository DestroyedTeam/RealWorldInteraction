//! Bounded receiving-client trace. No event sender, input fallback or OS authority.
use serde::Serialize;

pub const MAX_EVENTS: usize = 256;
pub const MAX_UNITS: usize = 64;

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

/// Separately sampled public AppKit properties, never an atomic focus proof.
#[derive(Debug, Serialize)]
pub struct Context {
    pub application_active: bool,
    pub window_key: bool,
    pub window_main: bool,
    pub first_responder_is_editor: bool,
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
}
