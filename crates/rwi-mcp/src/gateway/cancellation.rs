//! Host cancellation and future-drop propagate one qualified control request.
//! This never resends the original action and never claims native termination.
use super::*;
pub(super) struct CancelDispatch {
    gateway: Gateway,
    reference: device::OperationRef,
    armed: bool,
}
impl CancelDispatch {
    pub fn new(gateway: Gateway, reference: device::OperationRef) -> Self {
        Self {
            gateway,
            reference,
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
                &self.reference.device_id,
                None,
                Call::Cancel {
                    operation_ref: self.reference.clone(),
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
                data: json!({"code":"operation_cancel_requested","operation_ref":self.reference,"device_id":self.reference.device_id,"cancellation_delivery_confirmed":true,"cancellation_accepted":!r.result.is_error,"cancellation":r.result,"goal_verified":false,"original_action_replayed":false}),
                images: vec![],
            },
            Err(mut e) => {
                // Read-only work has no durable request ID fallback. Preserve its
                // only public locator even when cancellation delivery is unknown.
                e.data["operation_ref"] = json!(self.reference);
                e.data["device_id"] = json!(self.reference.device_id);
                e.data["cancellation_delivery_confirmed"] = json!(false);
                e.data["cancellation_accepted"] = Value::Null;
                e.data["original_action_replayed"] = json!(false);
                e
            }
        }
    }
}
impl Drop for CancelDispatch {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Ok(mut jobs) = self.gateway.cancels.lock() else {
            eprintln!("rwi-mcp: cancellation queue unavailable; delivery unconfirmed");
            return;
        };
        jobs.retain(|j| !j.is_finished());
        if jobs.len() >= 64 {
            eprintln!("rwi-mcp: cancellation queue full; delivery unconfirmed");
            return;
        }
        let gateway = self.gateway.clone();
        let reference = self.reference.clone();
        let job = tokio::spawn(async move {
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                gateway.request(
                    &reference.device_id.clone(),
                    None,
                    Call::Cancel {
                        operation_ref: reference,
                    },
                ),
            )
            .await;
            if !matches!(result, Ok(Ok(_))) {
                eprintln!(
                    "rwi-mcp: cancellation delivery unconfirmed; no native termination claim"
                );
            }
        });
        jobs.push(job);
    }
}
