//! Bounded receiving oracle. Only native AppKit callbacks populate this in GUI
//! runs; pure tests are not native transport or cross-application acceptance.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const PASTEBOARD_TYPE: &str = "com.destroyedteam.rwi.fixture.native-dnd-v1";
pub const MAX_PAYLOAD_BYTES: usize = 4096;
const MAX_CONSUMED_SESSIONS: usize = 128;

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
