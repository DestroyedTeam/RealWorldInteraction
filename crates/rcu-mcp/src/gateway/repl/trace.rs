//! Parent-owned receipts and lease cleanup survive JS exceptions/process death.
use super::*;
use std::collections::BTreeSet;

pub(super) struct Trace {
    gateway: Gateway,
    pub calls: Vec<Value>,
    pub outputs: Vec<Value>,
    pub images: Vec<Content>,
    pub output_bytes: usize,
    pub image_bytes: usize,
    leases: BTreeSet<String>,
    cleanup: Vec<Value>,
    started: std::time::Instant,
    call_started: std::time::Instant,
}
impl Trace {
    pub fn new(gateway: Gateway) -> Self {
        Self {
            gateway,
            calls: vec![],
            outputs: vec![],
            images: vec![],
            output_bytes: 0,
            image_bytes: 0,
            leases: BTreeSet::new(),
            cleanup: vec![],
            started: std::time::Instant::now(),
            call_started: std::time::Instant::now(),
        }
    }
    pub fn begin(&mut self, tool: &str, args: &serde_json::Map<String, Value>) -> usize {
        self.call_started = std::time::Instant::now();
        let mut record = json!({"tool":tool,"outcome":"unconfirmed"});
        if let Some(id) = args.get("request_id") {
            record["request_id"] = id.clone();
        }
        self.calls.push(record);
        self.calls.len() - 1
    }
    pub fn finish(
        &mut self,
        index: usize,
        tool: &str,
        args: &serde_json::Map<String, Value>,
        reply: &CallToolResult,
    ) {
        let data = reply.structured_content.clone().unwrap_or(Value::Null);
        let reference = data["presentation"]["result_ref"]
            .as_str()
            .map(str::to_owned)
            .or_else(|| {
                self.gateway
                    .presentation
                    .lock()
                    .ok()
                    .and_then(|mut cache| cache.retain(tool, data.clone()))
            });
        let call = &mut self.calls[index];
        call["elapsed_ms"] = json!(self.call_started.elapsed().as_secs_f64() * 1000.0);
        call["outcome"] = json!("returned");
        call["is_error"] = json!(reply.is_error.unwrap_or(false));
        call["result_ref"] = json!(reference);
        // Preserve exact native execution identity inline, independent of the
        // shorter-lived metadata cache. Never substitute the JS generation.
        for key in [
            "operation_ref",
            "code",
            "system_accepted",
            "goal_verified",
            "cancellation_delivery_confirmed",
            "cancellation_accepted",
            "original_action_replayed",
            "dispatch_error",
            "dispatch",
            "native_dispatch_entered",
        ] {
            if let Some(v) = data.get(key) {
                call[key] = v.clone();
            }
        }
        if tool == "computer.claim"
            && let Some(lease) = data["lease"].as_str()
        {
            self.leases.insert(lease.into());
        }
        if tool == "computer.release"
            && reply.is_error != Some(true)
            && let Some(lease) = args.get("lease").and_then(Value::as_str)
        {
            self.leases.remove(lease);
        }
    }
    pub async fn cleanup(&mut self) {
        // Keep ownership until each cleanup response arrives, so future-drop
        // during this await still has tokens for bounded best-effort cleanup.
        let leases = self.leases.clone();
        let mut jobs = tokio::task::JoinSet::new();
        for lease in leases {
            let gateway = self.gateway.clone();
            jobs.spawn(async move {
                let result = tokio::time::timeout(
                    Duration::from_secs(2),
                    gateway.invoke("computer.release", json!({"lease":lease}), None),
                )
                .await;
                match result {
                    Ok(result) => {
                        json!({"lease":lease,"is_error":result.is_error,"result":result.data})
                    }
                    Err(_) => {
                        json!({"lease":lease,"is_error":true,"code":"lease_release_unconfirmed"})
                    }
                }
            });
        }
        while let Some(result) = jobs.join_next().await {
            if let Ok(value) = &result
                && let Some(lease) = value["lease"].as_str()
            {
                self.leases.remove(lease);
            }
            self.cleanup
                .push(result.unwrap_or_else(|e| json!({"is_error":true,"message":e.to_string()})));
        }
        self.leases.clear();
    }
    pub fn cleanup_failed(&self) -> bool {
        self.cleanup.iter().any(|c| c["is_error"] == true)
    }
    pub fn result(
        &mut self,
        generation: String,
        state_retained: bool,
        error: Option<String>,
    ) -> CallToolResult {
        let data = json!({"generation":generation,"state_retained":state_retained,"error":error,"outputs":self.outputs,"calls":self.calls,"cleanup":self.cleanup,"original_action_replayed":false,"adapter_elapsed_ms":self.started.elapsed().as_secs_f64()*1000.0});
        let mut result = if error.is_some() {
            CallToolResult::structured_error(data)
        } else {
            CallToolResult::structured(data)
        };
        result.content.append(&mut self.images);
        result
    }
}
impl Drop for Trace {
    fn drop(&mut self) {
        // Future-drop is not a proof of lease release. Best-effort cleanup is
        // tracked by the same gateway shutdown queue; native expiry is authority.
        if self.leases.is_empty() {
            return;
        }
        let Ok(mut jobs) = self.gateway.cancels.lock() else {
            return;
        };
        jobs.retain(|j| !j.is_finished());
        let leases = std::mem::take(&mut self.leases);
        if jobs.len() >= 64 {
            return;
        }
        let gateway = self.gateway.clone();
        jobs.push(tokio::spawn(async move {
            let mut pending = tokio::task::JoinSet::new();
            for lease in leases {
                let gateway = gateway.clone();
                pending.spawn(async move {
                    let _ = tokio::time::timeout(
                        Duration::from_secs(2),
                        gateway.invoke("computer.release", json!({"lease":lease}), None),
                    )
                    .await;
                });
            }
            while pending.join_next().await.is_some() {}
        }));
    }
}
