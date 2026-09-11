//! EOF is a sticky admission latch, not merely a notification to current calls.
pub(super) struct Stop(pub tokio::sync::watch::Sender<bool>);
impl Drop for Stop {
    fn drop(&mut self) {
        // send() loses the value when there is no subscriber. Buffered calls
        // may subscribe only after EOF, so retain the closed state unconditionally.
        self.0.send_replace(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateway::Gateway;
    use rcu_native_link::device::AdapterProfile;
    use serde_json::json;

    #[tokio::test]
    async fn eof_without_subscriber_rejects_buffered_and_future_invokes() {
        let gateway = Gateway::new(Ok(AdapterProfile::default()));
        assert_eq!(gateway.shutdown.receiver_count(), 0);
        // Construct a queued future but deliberately do not poll/subscribe yet.
        let queued = gateway.invoke("computer.targets", json!({}), None);
        drop(Stop(gateway.shutdown.clone()));
        assert_eq!(gateway.shutdown.receiver_count(), 0);
        assert!(*gateway.shutdown.borrow());
        assert_eq!(queued.await.data["code"], "host_disconnected");
        // Even a syntactically valid action must stop at admission, before
        // resolving its device/lease or forwarding anything to a native agent.
        let action = json!({"target":"unknown","frame":"frame","element":"element","lease":"lease","request_id":"never-forwarded","intent":"click"});
        assert_eq!(
            gateway
                .invoke("computer.act_checked", action, None)
                .await
                .data["code"],
            "host_disconnected"
        );
        assert_eq!(gateway.shutdown.receiver_count(), 0);
        drop(Stop(gateway.shutdown.clone()));
        assert!(*gateway.shutdown.subscribe().borrow());
    }
}
