//! Producer-side pointer post evidence. Never a receiver or application oracle.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PointerPost {
    pub event_type: u32,
    pub point: [f64; 2],
    pub click_count: i64,
    pub event_number: i64,
    pub source_state_id: i64,
    pub target_pid: i64,
    pub target_window: i64,
    pub flags: u64,
    pub pressure: f64,
    /// Invocation-local wall time after the void post call returns, not CPU time.
    pub post_return_elapsed_us: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiverEffect {
    Unobserved,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PointerDispatch {
    pub route: String,
    pub api: String,
    pub events: Vec<PointerPost>,
    pub receiver_effect: ReceiverEffect,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}
impl PointerDispatch {
    pub fn background_window() -> Self {
        Self {
            route: "background_window".into(),
            api: "CGEventPostToPid".into(),
            events: Vec::with_capacity(4),
            receiver_effect: ReceiverEffect::Unobserved,
            truncated: false,
        }
    }
    /// The ordinary native click route produces at most two down/up pairs.
    /// Instrumentation has no fallible operation or new cancellation boundary.
    pub fn record_returned(&mut self, event: PointerPost) {
        if self.events.len() < 4 {
            self.events.push(event);
        } else {
            self.truncated = true;
        }
    }
}
