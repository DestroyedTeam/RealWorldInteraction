//! Read-only browser content uses explicit authority and one exact owned session.
use super::*;
use rwi_core::device::browser::SnapshotArgs;

impl BrowserRuntime {
    pub fn snapshot_capability(&self, principal: &Principal) -> Capability {
        let authorized = self
            .policy
            .as_ref()
            .ok()
            .and_then(|p| p.grant(principal))
            .is_some_and(|g| g.snapshot);
        let ready = self.ledger.lock().map(|l| l.is_ok()).unwrap_or(false);
        let implemented = cfg!(target_os = "macos");
        Capability {
            id: "browser.snapshot".into(),
            implemented,
            authorized,
            available: implemented && authorized && ready,
            reason: if !implemented {
                Some("browser_platform_unsupported".into())
            } else if let Err(e) = &self.policy {
                Some(format!("browser_policy_disabled:{e}"))
            } else if !authorized {
                Some("browser_snapshot_permission_denied".into())
            } else if !ready {
                Some("browser_journal_unavailable".into())
            } else {
                None
            },
        }
    }
    pub async fn snapshot(
        self: Arc<Self>,
        principal: &Principal,
        args: SnapshotArgs,
    ) -> DeviceResult {
        if let Err(e) = args.validate() {
            return error("invalid_arguments", e);
        }
        if args.device_id != self.device {
            return error("browser_device_mismatch", "No content request was sent.");
        }
        if !self
            .policy
            .as_ref()
            .ok()
            .and_then(|p| p.grant(principal))
            .is_some_and(|g| g.snapshot)
        {
            return error(
                "browser_snapshot_permission_denied",
                "An explicit snapshot grant is required; lifecycle observation and Native grants do not authorize page content.",
            );
        }
        let owner = principal.certificate.as_str();
        let session = self
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&args.session_ref)
            .filter(|s| s.owner == owner)
            .cloned();
        let Some(session) = session else {
            return error(
                "browser_session_unknown",
                "No other principal, instance, profile or native window is searched.",
            );
        };
        let permit = match self.workers.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                return error(
                    "browser_busy_not_dispatched",
                    "Content worker capacity is bounded; close/query capacity is separate.",
                );
            }
        };
        let device = self.device.clone();
        let instance = self.instance.clone();
        match tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let Ok(mut state) = session.inner.try_lock() else {
                return error(
                    "browser_session_busy_not_dispatched",
                    "Only this owned resource is busy.",
                );
            };
            if !state.valid || Instant::now() >= session.expires {
                return error(
                    "browser_session_closed_or_expired",
                    "No content request was sent.",
                );
            }
            let Some(driver) = state.driver.as_mut() else {
                return error("browser_session_not_ready", "No content driver is ready.");
            };
            let mut r = driver.snapshot(&args);
            if Instant::now() >= session.expires {
                return error(
                    "browser_session_expired_during_snapshot",
                    "Observation discarded after session expiry; no alternate target or input.",
                );
            }
            r.data["device_id"] = json!(device);
            r.data["agent_instance"] = json!(instance);
            r.data["session_ref"] = json!(args.session_ref);
            r.data["input_authority"] = json!(false);
            r.data["automatic_replay"] = json!(false);
            if !r.is_error {
                r.data["observation_ref"] = json!(format!(
                    "browser-observation:{}",
                    uuid::Uuid::new_v4().simple()
                ));
            }
            r
        })
        .await
        {
            Ok(r) => r,
            Err(e) => error(
                "browser_snapshot_unknown",
                format!("Observation worker terminated: {e}; no input or automatic replay."),
            ),
        }
    }
}
