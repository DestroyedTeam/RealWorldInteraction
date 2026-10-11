//! Optional MCP progress notifications, independent of stdio/HTTP transport.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

pub(super) struct Progress {
    context: RequestContext<RoleServer>,
    sequence: AtomicU64,
}
impl Progress {
    pub fn new(context: RequestContext<RoleServer>) -> Self {
        Self {
            context,
            sequence: AtomicU64::new(0),
        }
    }
    pub async fn emit(&self, message: &'static str) {
        let Some(token) = self.context.meta.get_progress_token() else {
            return;
        };
        if self.context.ct.is_cancelled() {
            return;
        }
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let notification =
            ProgressNotificationParam::new(token, sequence as f64).with_message(message);
        // Status delivery is advisory, not input authority. Never delay cleanup
        // indefinitely, infer completion from a notification, or retry an action.
        let _ = tokio::time::timeout(
            Duration::from_millis(25),
            self.context.peer.notify_progress(notification),
        )
        .await;
    }
}
pub(super) fn stage(tool: &str) -> &'static str {
    match tool {
        "computer.observe" => "Reading window state",
        "computer.image" => "Preparing mapped preview",
        "computer.resolve_view" => "Validating screenshot coordinates",
        "computer.claim" => "Acquiring short input lease",
        "computer.act" | "computer.act_checked" => "Dispatching one input operation",
        _ => "Processing request",
    }
}
