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

#[cfg(all(test, feature = "fixture"))]
mod tests {
    use super::*;
    use crate::config::PlatformPolicy;
    #[tokio::test]
    async fn corrupt_commit_requires_new_authorized_idle_resource_check_and_remains_evidence() {
        let temp = tempfile::tempdir().unwrap();
        let policy = Principal {
            certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
            observe: true,
            input: true,
        };
        let config = Config {
            version: 1,
            device_id: "recovery-fixture".into(),
            bind: "127.0.0.1:0".parse().unwrap(),
            identity: rcu_link_trust::IdentityPaths::in_profile(temp.path()),
            principals: vec![policy.clone()],
            state_root: temp.path().join("state"),
            platform: PlatformPolicy::Fixture,
        };
        let runtime = Runtime::new(config.clone()).unwrap();
        let key = platform::backend(&config, &policy, &uuid::Uuid::new_v4().to_string())
            .unwrap()
            .identity()
            .key();
        {
            let mut storage = runtime.ledger.lock().unwrap();
            let ledger = storage.as_mut().unwrap();
            ledger
                .reserve(
                    policy.certificate.as_str(),
                    "uncertain",
                    &json!({}),
                    "monitor",
                    &key,
                )
                .unwrap();
            ledger
                .reconcile(&key, json!({"source":"earlier_synthetic_check"}))
                .unwrap();
        }
        let recovery_dir = config.state_root.join("operations/recoveries");
        let corrupt = std::fs::read_dir(&recovery_dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        std::fs::write(&corrupt, b"{truncated").unwrap();
        drop(runtime);
        let runtime = Runtime::new(config.clone()).unwrap();
        assert!(runtime.is_fenced(&key));
        let mut readonly = policy.clone();
        readonly.input = false;
        assert_eq!(
            runtime.recover(&readonly, &key).await.data["code"],
            "permission_denied"
        );
        let domain = runtime.domain(&key).unwrap();
        let held = domain.gate.clone().try_acquire_owned().unwrap();
        assert_eq!(
            runtime.recover(&policy, &key).await.data["code"],
            "native_writer_pending"
        );
        assert_eq!(std::fs::read_dir(&recovery_dir).unwrap().count(), 1);
        drop(held);
        let recovered = runtime.recover(&policy, &key).await;
        assert!(!recovered.is_error, "{:?}", recovered.data);
        assert_eq!(
            recovered.data["proof"]["native"]["source"],
            "explicit_synthetic_fixture"
        );
        assert_eq!(recovered.data["proof"]["native"]["input_sent"], false);
        assert_eq!(std::fs::read(corrupt).unwrap(), b"{truncated");
        assert!(!runtime.is_fenced(&key));
        let prior = runtime
            .ledger
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .status(
                policy.certificate.as_str(),
                "uncertain",
                OperationIdKind::Request,
            )
            .unwrap()
            .unwrap();
        assert_eq!(prior["replay_allowed"], false);
        drop(runtime);
        assert!(!Runtime::new(config).unwrap().is_fenced(&key));
    }
}
