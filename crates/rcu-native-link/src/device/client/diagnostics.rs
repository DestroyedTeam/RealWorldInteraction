//! Bounded adapter-local reconnect evidence, never native completion evidence.
use super::*;

#[derive(Clone, Debug, Serialize)]
pub struct TransportFailure {
    pub kind: String,
    pub message: String,
    pub message_truncated: bool,
}

impl TransportFailure {
    fn new(error: &io::Error) -> Self {
        let message = error.to_string();
        Self {
            kind: format!("{:?}", error.kind()),
            message: message.chars().take(512).collect(),
            message_truncated: message.chars().count() > 512,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct DescribeReconnect {
    pub request_id: String,
    pub original_failure: TransportFailure,
    pub outcome: &'static str,
    pub reconnect_failure: Option<TransportFailure>,
    /// Describe may touch existing context-idle bookkeeping, never native
    /// authorization, input leases, frames or action history.
    pub context_idle_bookkeeping_possible: bool,
    pub native_input_replayed: bool,
}
#[derive(Default)]
pub(super) struct ReconnectLog {
    pub(super) attempts: u64,
    pub(super) successes: u64,
    latest: Option<(Arc<()>, DescribeReconnect)>,
}
impl ReconnectLog {
    pub(super) fn last(&self) -> Option<DescribeReconnect> {
        self.latest.as_ref().map(|(_, record)| record.clone())
    }
    pub(super) fn begin(&mut self, request: &Request, error: &io::Error) -> Arc<()> {
        self.attempts = self.attempts.saturating_add(1);
        let ticket = Arc::new(());
        self.latest = Some((
            ticket.clone(),
            DescribeReconnect {
                request_id: request.id.clone(),
                original_failure: TransportFailure::new(error),
                // If the future is cancelled, it must not retain a success claim.
                outcome: "completion_unobserved",
                reconnect_failure: None,
                context_idle_bookkeeping_possible: request.context.is_some(),
                native_input_replayed: false,
            },
        ));
        ticket
    }
    pub(super) fn finish(&mut self, ticket: &Arc<()>, result: &io::Result<Reply>) {
        if result.is_ok() {
            self.successes = self.successes.saturating_add(1);
        }
        if let Some((latest, record)) = &mut self.latest
            && Arc::ptr_eq(latest, ticket)
        {
            record.outcome = if result.is_ok() {
                "reply_validated"
            } else {
                "failed"
            };
            record.reconnect_failure = result.as_ref().err().map(TransportFailure::new);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(id: &str) -> Request {
        Request {
            version: PROTOCOL_VERSION,
            id: id.into(),
            device: "device".into(),
            context: Some("private-context-not-in-diagnostics".into()),
            call: Call::Describe,
        }
    }

    #[test]
    fn evidence_is_bounded_and_never_copies_the_context_token() {
        let mut log = ReconnectLog::default();
        log.begin(
            &request("read-only"),
            &io::Error::new(io::ErrorKind::BrokenPipe, "断".repeat(2048)),
        );
        let record = log.last().unwrap();
        assert_eq!(record.original_failure.message.chars().count(), 512);
        assert!(record.original_failure.message_truncated);
        assert_eq!(record.outcome, "completion_unobserved");
        assert!(record.context_idle_bookkeeping_possible);
        assert!(!record.native_input_replayed);
        let encoded = serde_json::to_string(&record).unwrap();
        assert!(encoded.len() < 2048);
        assert!(!encoded.contains("private-context"));
    }

    #[test]
    fn older_completion_cannot_overwrite_a_newer_reconnect_record() {
        let mut log = ReconnectLog::default();
        let first = log.begin(&request("first"), &invalid("first failure"));
        let second = log.begin(&request("second"), &invalid("second failure"));
        let success = Ok(Reply {
            version: PROTOCOL_VERSION,
            id: "first".into(),
            device: "device".into(),
            instance: "agent".into(),
            result: rcu_core::device::DeviceResult::value(serde_json::json!({})),
        });
        log.finish(&first, &success);
        assert_eq!(log.last().unwrap().request_id, "second");
        assert_eq!(log.last().unwrap().outcome, "completion_unobserved");
        log.finish(&second, &Err(invalid("second reconnect failed")));
        assert_eq!(log.attempts, 2);
        assert_eq!(log.successes, 1);
        assert_eq!(log.last().unwrap().outcome, "failed");
        assert_eq!(
            log.last().unwrap().reconnect_failure.unwrap().message,
            "second reconnect failed"
        );
    }
}
