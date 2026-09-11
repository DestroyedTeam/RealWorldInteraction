//! Input uncertainty is isolated to its native resource domain.
use super::*;
impl Runtime {
    pub(super) fn is_fenced(&self, key: &str) -> bool {
        if let Ok(domains) = self.domains.lock()
            && let Some(d) = domains.get(key)
        {
            return d.fenced.load(Ordering::Acquire);
        }
        self.ledger
            .lock()
            .ok()
            .and_then(|l| l.as_ref().ok().map(|l| l.fenced(key)))
            .unwrap_or(true)
    }
    pub(super) fn domain(&self, key: &str) -> Result<Arc<InputDomain>, DeviceResult> {
        let mut domains = self.domains.lock().map_err(|_| {
            DeviceResult::error(
                "domain_registry_unavailable",
                "Reads of service metadata remain available.",
            )
        })?;
        if let Some(d) = domains.get(key) {
            return Ok(d.clone());
        }
        if domains.len() >= 32 {
            domains.retain(|_, d| Arc::strong_count(d) > 1);
        }
        if domains.len() >= 32 {
            return Err(DeviceResult::error(
                "domain_capacity",
                "Native context capacity reached.",
            ));
        }
        let fenced = self
            .ledger
            .lock()
            .ok()
            .and_then(|l| l.as_ref().ok().map(|l| l.fenced(key)))
            .unwrap_or(true);
        let domain = Arc::new(InputDomain {
            key: key.into(),
            gate: Arc::new(tokio::sync::Semaphore::new(1)),
            fenced: AtomicBool::new(fenced),
        });
        domains.insert(key.into(), domain.clone());
        Ok(domain)
    }
    pub(super) async fn recover(&self, policy: &Principal, key: &str) -> DeviceResult {
        if !policy.input {
            return DeviceResult::error(
                "permission_denied",
                "Input-domain reconciliation requires input authorization.",
            );
        }
        let native =
            match platform::backend(&self.config, policy, &uuid::Uuid::new_v4().to_string()) {
                Ok(n) => n,
                Err(e) => return DeviceResult::error(&e.code, &e.next_step),
            };
        if native.identity().key() != key {
            return DeviceResult::error(
                "foreign_resource_domain",
                "Reconcile only the current independently observed native context.",
            );
        }
        let domain = match self.domain(key) {
            Ok(d) => d,
            Err(e) => return e,
        };
        let Ok(_exclusive) = domain.gate.clone().try_acquire_owned() else {
            return DeviceResult::error(
                "native_writer_pending",
                "The original native writer has not released its domain.",
            );
        };
        let quiescent = self
            .contexts
            .lock()
            .map(|c| {
                c.values().filter(|c| c.domain.key == key).all(|c| {
                    c.registry
                        .statuses()
                        .map(|statuses| {
                            statuses
                                .iter()
                                .all(|s| s.terminal && !s.native_call_pending)
                        })
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
        if !quiescent {
            return DeviceResult::error(
                "native_resources_pending",
                "Cancel/query pending native operations before reconciliation.",
            );
        }
        let proof = match platform::input_resources(&self.config) {
            Ok(p) => p,
            Err(e) => return DeviceResult::error(&e.code, &e.next_step),
        };
        let record=self.ledger.lock().map_err(|_|io::Error::other("ledger_lock")).and_then(|l|l.as_ref().map_err(|e|io::Error::other(e.to_string()))?.reconcile(key,json!({"native":proof,"service_instance":self.instance,"exclusive_service_state_owner":true,"native_writers_idle":true})));
        match record {
            Ok(record) => {
                domain.fenced.store(false, Ordering::Release);
                DeviceResult::value(record)
            }
            Err(e) => DeviceResult::error("resource_reconciliation_failed", &e.to_string()),
        }
    }
}
