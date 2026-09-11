//! Host cancellation and future-drop propagate one qualified control request.
//! This never resends the original action and never claims native termination.
use super::*;
pub(super) struct CancelDispatch {
    gateway: Gateway,
    device: String,
    context: Option<String>,
    operation: String,
    armed: bool,
}
impl CancelDispatch {
    pub fn new(
        gateway: Gateway,
        device: String,
        context: Option<String>,
        operation: String,
    ) -> Self {
        Self {
            gateway,
            device,
            context,
            operation,
            armed: true,
        }
    }
    pub fn disarm(&mut self) {
        self.armed = false;
    }
    pub async fn request(&mut self) -> DeviceResult {
        let reply = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            self.gateway.request(
                &self.device,
                self.context.clone(),
                Call::Cancel {
                    operation_id: self.operation.clone(),
                },
            ),
        )
        .await
        .unwrap_or_else(|_| {
            Err(DeviceResult::error(
                "cancellation_delivery_unconfirmed",
                "Device cancellation deadline; no native termination claim.",
            ))
        });
        self.armed = false;
        match reply {
            Ok(r) => DeviceResult {
                is_error: true,
                data: json!({"code":"operation_cancel_requested","operation_id":self.operation,"device_id":self.device,"cancellation":r.result,"goal_verified":false,"original_action_replayed":false}),
                images: vec![],
            },
            Err(e) => e,
        }
    }
}
impl Drop for CancelDispatch {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Ok(mut jobs) = self.gateway.cancels.lock() else {
            eprintln!("rcu-mcp: cancellation queue unavailable; delivery unconfirmed");
            return;
        };
        jobs.retain(|j| !j.is_finished());
        if jobs.len() >= 64 {
            eprintln!("rcu-mcp: cancellation queue full; delivery unconfirmed");
            return;
        }
        let gateway = self.gateway.clone();
        let device = self.device.clone();
        let context = self.context.clone();
        let operation = self.operation.clone();
        let job = tokio::spawn(async move {
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                gateway.request(
                    &device,
                    context,
                    Call::Cancel {
                        operation_id: operation,
                    },
                ),
            )
            .await;
            if !matches!(result, Ok(Ok(_))) {
                eprintln!(
                    "rcu-mcp: cancellation delivery unconfirmed; no native termination claim"
                );
            }
        });
        jobs.push(job);
    }
}
