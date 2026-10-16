//! Exact engine lifecycle methods, separated by responsibility.
use super::*;

impl Engine {
    pub(super) fn lifecycle_execute<T>(
        &mut self,
        target: &str,
        lease: &str,
        f: impl FnOnce(
            &std::path::Path,
            &lifecycle::LifecycleGrant,
            &mut dyn lifecycle::LifecycleDriver,
        ) -> Result<T>,
    ) -> Result<T> {
        let identity = self.identity(target)?;
        let grant = self.backend(target)?.lifecycle_grant()?;
        grant.validate(&identity, lifecycle::now_ms())?;
        let book = self.leases.shared();
        let held = book
            .lock()
            .map_err(|_| fault("lease_error", "Restart observation session."))?;
        let key = identity.key();
        let deadline = held
            .deadline(lease, target, &key)?
            .min(self.backend(target)?.lifecycle_deadline()?);
        let root = self.state.clone();
        let result = operation::with_dispatch_deadline(deadline, || {
            operation::checkpoint("lifecycle_worker")?;
            let mut driver = self.backend(target)?.lifecycle_driver()?;
            f(&root, &grant, driver.as_mut())
        });
        // Side effects and uncertain dispatch invalidate every alias on this desktop.
        let aliases = self
            .targets
            .iter()
            .filter(|(_, t)| t.identity.key() == key)
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>();
        self.snapshots.retain(|(_, s)| !aliases.contains(&s.target));
        self.refs.retain(|r| !aliases.contains(&r.target));
        drop(held);
        result
    }
    pub fn launch_application(
        &mut self,
        target: &str,
        lease: &str,
        req: lifecycle::LaunchRequest<'_>,
    ) -> Result<lifecycle::LaunchReceipt> {
        self.lifecycle_execute(target, lease, |root, g, d| {
            lifecycle::launch(root, g, d, req)
        })
    }
    pub fn activate_window(
        &mut self,
        target: &str,
        lease: &str,
        req: lifecycle::ActivateRequest<'_>,
    ) -> Result<lifecycle::ActivationReceipt> {
        let grant = self.backend(target)?.lifecycle_grant()?;
        let permission = grant
            .activations
            .iter()
            .find(|p| p.authorization_id == req.authorization_id)
            .ok_or_else(|| {
                fault(
                    "unknown_lifecycle_permission",
                    "Choose an exact preauthorized window.",
                )
            })?;
        self.backend(target)?
            .validate_activation(&permission.window)?;
        self.lifecycle_execute(target, lease, |root, g, d| {
            lifecycle::activate(root, g, d, req)
        })
    }
    pub fn lifecycle_preflight(
        &mut self,
        target: &str,
        authorization: &str,
    ) -> Result<lifecycle::PreflightReceipt> {
        let cleanup = operation::CleanupAudit::begin();
        let identity = self.identity(target)?;
        let grant = self.backend(target)?.lifecycle_grant()?;
        grant.validate(&identity, lifecycle::now_ms())?;
        let deadline = self.backend(target)?.lifecycle_deadline()?;
        let mut result = operation::with_dispatch_deadline(deadline, || {
            operation::checkpoint("lifecycle_preflight_worker")?;
            let mut driver = self.backend(target)?.lifecycle_driver()?;
            lifecycle::preflight(&grant, driver.as_mut(), authorization)
        })?;
        // Include driver construction/destruction signals, not only individual closers.
        if cleanup.uncertain() {
            result.ready_for_launch = false;
            result.resources_released = false;
            result.resource_release_uncertain = true;
            if result.error.is_none() {
                result.error = Some(fault(
                    "native_resource_release_uncertain",
                    "Lifecycle preflight cleanup is unproven; no action was dispatched.",
                ));
            }
        }
        Ok(result)
    }
    pub fn lifecycle_receipt(
        &mut self,
        target: &str,
        kind: &str,
        authorization: &str,
        request: &str,
    ) -> Result<serde_json::Value> {
        let grant = self.backend(target)?.lifecycle_grant()?;
        lifecycle::receipt(&self.state, &grant, kind, authorization, request)
    }
}
