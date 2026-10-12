//! Bounded receiving oracle. Only native AppKit callbacks populate this in GUI
//! runs; pure tests are not native transport or cross-application acceptance.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};

pub const PASTEBOARD_TYPE: &str = "com.destroyedteam.rwi.fixture.native-dnd-v1";
pub const MAX_PAYLOAD_BYTES: usize = 4096;
const MAX_CONSUMED_SESSIONS: usize = 128;
pub const MAX_TIMELINE_EVENTS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    SourceRequest,
    SourceBegan,
    SourceMoved,
    SourceEnded,
    DestinationEntered,
    DestinationUpdated,
    DestinationExited,
    DestinationPrepared,
    DestinationPerformed,
    DestinationConcluded,
    ActorBeforeActivate,
    ActorAfterActivate,
    ActorBeforeOrder,
    ActorAfterOrder,
    ActorApiUnavailable,
}

/// Independent native callback/own-actor snapshots, not an input receipt or a
/// drop goal. FIFO preserves observed callback order even if wall time changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimelineEvent {
    pub phase: Phase,
    pub native_sequence: Option<isize>,
    pub active_sequence: Option<isize>,
    pub operation: Option<usize>,
    pub unix_ms: Option<u64>,
    pub process_monotonic_ns: u64,
    pub app_active: Option<bool>,
    pub window_key: Option<bool>,
    pub frontmost_pid: Option<i32>,
    pub app_frontmost: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payload {
    pub schema_version: u32,
    /// Self-reported test payload metadata, never native PID/input authority.
    pub source_pid: u32,
    pub run_id: String,
    pub offer: u64,
    pub value: String,
}
impl Payload {
    pub fn own(pid: u32, run_id: &str, offer: u64) -> Self {
        Self {
            schema_version: 1,
            source_pid: pid,
            run_id: run_id.into(),
            offer,
            value: format!("RWI native payload {run_id} #{offer}"),
        }
    }
    pub fn decode(s: &str) -> Option<Self> {
        if s.len() > MAX_PAYLOAD_BYTES {
            return None;
        }
        let p: Self = serde_json::from_str(s).ok()?;
        (p.schema_version == 1
            && p.source_pid != 0
            && p.offer != 0
            && !p.run_id.is_empty()
            && p.run_id.len() <= 64
            && p.run_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
            && p.value == format!("RWI native payload {} #{}", p.run_id, p.offer))
        .then_some(p)
    }
}

#[derive(Default, Serialize)]
pub struct Oracle {
    pub enabled: bool,
    pub timeline: VecDeque<TimelineEvent>,
    pub timeline_total: u64,
    pub timeline_dropped: u64,
    pub source_requests: u64,
    pub source_began: u64,
    pub source_moved: u64,
    pub source_ended: u64,
    pub source_cancelled: u64,
    pub active_sequence: Option<isize>,
    pub last_source_operation: Option<usize>,
    pub destination_entered: u64,
    pub destination_updated: u64,
    pub destination_exited: u64,
    pub destination_prepared: u64,
    pub destination_performed: u64,
    pub destination_concluded: u64,
    pub rejected_drops: u64,
    pub consumed: u64,
    pub last_payload: Option<Payload>,
    pub last_drop_sequence: Option<isize>,
    /// NSDraggingInfo.draggingSource == nil; not inferred from payload.source_pid.
    pub last_source_external: Option<bool>,
    pub last_destination_window: Option<isize>,
    pub error: Option<String>,
    pub explicit_close_requests: u64,
    pub active_at_explicit_close: Option<isize>,
    pub application_terminate_requests: u64,
    pub active_at_application_termination: Option<isize>,
    consumed_sessions: BTreeSet<isize>,
}
impl Oracle {
    /// Recording does not update native session counters, active identity,
    /// cancellation or payload consumption. Missing snapshots remain unknown.
    pub fn note_timeline(&mut self, event: TimelineEvent) {
        if !self.enabled {
            return;
        }
        self.timeline_total += 1;
        if self.timeline.len() == MAX_TIMELINE_EVENTS {
            self.timeline.pop_front();
            self.timeline_dropped += 1;
        }
        self.timeline.push_back(event);
    }

    /// Preserve the missing native end callback. Own-fixture shutdown is not
    /// drag cancellation or a successfully consumed second payload.
    pub fn note_pending_native_shutdown(&mut self) -> bool {
        if self.enabled
            && let Some(seq) = self.active_sequence
        {
            self.application_terminate_requests += 1;
            self.active_at_application_termination = Some(seq);
            return true;
        }
        false
    }
    pub fn consume(&mut self, seq: isize, text: &str, external: bool, window: isize) -> bool {
        self.destination_performed += 1;
        let Some(payload) = Payload::decode(text) else {
            self.rejected_drops += 1;
            return false;
        };
        if !self.enabled
            || seq <= 0
            || window <= 0
            || self.consumed_sessions.contains(&seq)
            || self.consumed_sessions.len() >= MAX_CONSUMED_SESSIONS
        {
            self.rejected_drops += 1;
            return false;
        }
        self.consumed_sessions.insert(seq);
        self.consumed += 1;
        self.last_payload = Some(payload);
        self.last_drop_sequence = Some(seq);
        self.last_source_external = Some(external);
        self.last_destination_window = Some(window);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn payload() -> String {
        serde_json::to_string(&Payload::own(41, "own-dnd", 1)).unwrap()
    }
    fn enabled() -> Oracle {
        Oracle {
            enabled: true,
            ..Default::default()
        }
    }
    fn timeline_event(phase: Phase, native_sequence: Option<isize>) -> TimelineEvent {
        TimelineEvent {
            phase,
            native_sequence,
            active_sequence: native_sequence,
            operation: None,
            unix_ms: None,
            process_monotonic_ns: 0,
            app_active: None,
            window_key: None,
            frontmost_pid: None,
            app_frontmost: None,
        }
    }
    #[test]
    fn disabled_oracle_does_not_record_timeline_or_claim_native_observation() {
        let mut s = Oracle::default();
        s.note_timeline(timeline_event(Phase::SourceBegan, Some(10)));
        assert!(s.timeline.is_empty());
        assert_eq!(s.timeline_total, 0);
        assert_eq!(s.timeline_dropped, 0);
        assert_eq!(s.source_began, 0);
        assert_eq!(s.active_sequence, None);
        assert_eq!(s.consumed, 0);
    }
    #[test]
    fn timeline_preserves_callback_order_and_same_native_sequence() {
        let mut s = enabled();
        let mut began = timeline_event(Phase::SourceBegan, Some(10));
        began.unix_ms = Some(100);
        began.process_monotonic_ns = 1;
        let mut moved = timeline_event(Phase::SourceMoved, Some(10));
        moved.unix_ms = Some(90);
        moved.process_monotonic_ns = 2;
        let mut ended = timeline_event(Phase::SourceEnded, Some(10));
        ended.unix_ms = Some(90);
        ended.process_monotonic_ns = 3;
        ended.active_sequence = None;
        ended.operation = Some(0);
        for event in [began.clone(), moved.clone(), ended.clone()] {
            s.note_timeline(event);
        }
        assert_eq!(s.timeline, VecDeque::from([began, moved, ended]));
        assert_eq!(s.timeline_total, 3);
        assert_eq!(s.timeline_dropped, 0);
        assert_eq!(s.source_began, 0);
        assert_eq!(s.source_moved, 0);
        assert_eq!(s.source_ended, 0);
        assert_eq!(s.source_cancelled, 0);
        assert_eq!(s.active_sequence, None);
        assert_eq!(s.last_source_operation, None);
        assert_eq!(s.destination_performed, 0);
        assert_eq!(s.consumed, 0);
    }
    #[test]
    fn timeline_capacity_is_fifo_and_overflow_is_explicit() {
        let mut s = enabled();
        for i in 0..MAX_TIMELINE_EVENTS {
            let mut event = timeline_event(Phase::SourceMoved, Some(10));
            event.process_monotonic_ns = i as u64;
            s.note_timeline(event);
        }
        assert_eq!(s.timeline.len(), MAX_TIMELINE_EVENTS);
        assert_eq!(s.timeline_total, MAX_TIMELINE_EVENTS as u64);
        assert_eq!(s.timeline_dropped, 0);
        for i in MAX_TIMELINE_EVENTS..MAX_TIMELINE_EVENTS + 3 {
            let mut event = timeline_event(Phase::SourceMoved, Some(10));
            event.process_monotonic_ns = i as u64;
            s.note_timeline(event);
        }
        assert_eq!(s.timeline.len(), MAX_TIMELINE_EVENTS);
        assert_eq!(s.timeline_total, MAX_TIMELINE_EVENTS as u64 + 3);
        assert_eq!(s.timeline_dropped, 3);
        assert_eq!(s.timeline.front().unwrap().process_monotonic_ns, 3);
        assert_eq!(
            s.timeline.back().unwrap().process_monotonic_ns,
            MAX_TIMELINE_EVENTS as u64 + 2
        );
        assert_eq!(
            s.timeline_total,
            s.timeline.len() as u64 + s.timeline_dropped
        );
    }
    #[test]
    fn timeline_serialization_preserves_unknowns_and_explicit_false_separately() {
        let unknown = timeline_event(Phase::ActorApiUnavailable, None);
        let value = serde_json::to_value(&unknown).unwrap();
        assert_eq!(value["phase"], "actor_api_unavailable");
        for field in [
            "native_sequence",
            "active_sequence",
            "operation",
            "unix_ms",
            "app_active",
            "window_key",
            "frontmost_pid",
            "app_frontmost",
        ] {
            assert!(value[field].is_null(), "{field}");
        }
        assert_eq!(
            serde_json::from_value::<TimelineEvent>(value).unwrap(),
            unknown
        );
        let mut observed = timeline_event(Phase::ActorBeforeActivate, Some(10));
        observed.app_active = Some(false);
        observed.window_key = Some(false);
        observed.frontmost_pid = Some(41);
        observed.app_frontmost = Some(false);
        let observed_value = serde_json::to_value(&observed).unwrap();
        assert_eq!(observed_value["app_active"], false);
        assert_eq!(observed_value["window_key"], false);
        assert_eq!(observed_value["frontmost_pid"], 41);
        assert_eq!(observed_value["app_frontmost"], false);
    }
    #[test]
    fn default_is_unobserved_not_drop_success() {
        let s = serde_json::to_value(Oracle::default()).unwrap();
        assert_eq!(s["enabled"], false);
        assert_eq!(s["consumed"], 0);
        assert!(s["last_payload"].is_null());
        assert!(s["last_source_external"].is_null());
    }
    #[test]
    fn typed_payload_is_bounded_and_rejects_unknown_or_altered_fields() {
        assert_eq!(
            Payload::decode(&payload()),
            Some(Payload::own(41, "own-dnd", 1))
        );
        for value in [
            "x".repeat(MAX_PAYLOAD_BYTES + 1),
            "{}".into(),
            payload().replace("own-dnd", "../foreign"),
            payload().replace("\"schema_version\":1", "\"schema_version\":2"),
            payload().replace("\"source_pid\":41", "\"source_pid\":0"),
            payload().replace("\"offer\":1", "\"offer\":0"),
            payload().replace("\"value\":", "\"extra\":true,\"value\":"),
        ] {
            assert!(Payload::decode(&value).is_none(), "{value}");
        }
    }
    #[test]
    fn actual_consume_is_separate_from_source_receipts_and_same_app_is_not_cross_app() {
        let mut s = enabled();
        s.source_requests = 1;
        s.source_began = 1;
        s.source_moved = 8;
        assert_eq!(s.consumed, 0);
        assert!(s.consume(10, &payload(), false, 100));
        assert_eq!(s.last_source_external, Some(false));
        assert_eq!(s.source_ended, 0);
    }
    #[test]
    fn duplicate_invalid_and_unenabled_drops_do_not_replace_consumed_payload() {
        let mut s = enabled();
        assert!(s.consume(10, &payload(), true, 100));
        assert!(!s.consume(10, &payload(), true, 100));
        assert!(!s.consume(11, "bad", true, 100));
        assert!(!s.consume(0, &payload(), true, 100));
        assert!(!s.consume(11, &payload(), true, 0));
        assert_eq!(s.consumed, 1);
        assert_eq!(s.last_drop_sequence, Some(10));
        assert_eq!(s.last_source_external, Some(true));
        assert!(!Oracle::default().consume(10, &payload(), true, 100));
    }
    #[test]
    fn bounded_dedup_refuses_capacity_instead_of_forgetting_old_sessions() {
        let mut s = enabled();
        for i in 1..=MAX_CONSUMED_SESSIONS {
            assert!(s.consume(i as isize, &payload(), true, 100));
        }
        assert!(!s.consume(1, &payload(), true, 100));
        assert!(!s.consume(129, &payload(), true, 100));
        assert_eq!(s.consumed, 128);
    }
    #[test]
    fn own_shutdown_preserves_unended_native_session_without_forging_cancel_or_drop() {
        let mut s = enabled();
        assert!(!s.note_pending_native_shutdown());
        s.active_sequence = Some(9);
        assert!(s.note_pending_native_shutdown());
        assert_eq!(s.active_sequence, Some(9));
        assert_eq!(s.source_ended, 0);
        assert_eq!(s.source_cancelled, 0);
        assert_eq!(s.consumed, 0);
        assert_eq!(s.active_at_application_termination, Some(9));
        assert_eq!(s.application_terminate_requests, 1);
    }
}
