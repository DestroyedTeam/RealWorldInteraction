//! Independent window bindings, never independent physical input authority.
use crate::*;
impl Engine {
    pub fn attach_window(
        &mut self,
        source: &str,
        selection: &WindowSelection,
    ) -> Result<serde_json::Value> {
        operation::checkpoint("attach_window")?;
        let identity = self.identity(source)?;
        if self.targets.get(source).is_some_and(|t| t.parent.is_some()) {
            return Err(fault(
                "attachment_source_required",
                "Discover and attach through the original desktop target.",
            ));
        }
        if self.targets.len() >= 16 {
            return Err(fault(
                "target_capacity",
                "Detach an idle attached window; at most 16 targets per context.",
            ));
        }
        let attachment = self.backend(source)?.attach_window(selection)?;
        if attachment.backend.identity() != identity {
            return Err(fault(
                "session_mismatch",
                "Attachment changed the physical desktop identity; no target registered.",
            ));
        }
        operation::checkpoint("attach_window_register")?;
        let target = format!("{source}:window:{}", id());
        self.targets.insert(
            target.clone(),
            RegisteredTarget {
                identity: identity.clone(),
                backend: attachment.backend,
                parent: Some(source.into()),
            },
        );
        Ok(
            serde_json::json!({"target":target,"parent_target":source,"identity":identity,
            "selection":attachment.details,"attached":true,"input_sent":false,"activated":false,
            "source_binding_changed":false,"physical_lease_shared":true,"input_authority_granted":false}),
        )
    }
    pub fn detach_window(&mut self, target: &str) -> Result<serde_json::Value> {
        operation::checkpoint("detach_window")?;
        let entry = self
            .targets
            .get(target)
            .ok_or_else(|| fault("unknown_target", "Use a current context target."))?;
        if entry.parent.is_none() {
            return Err(fault(
                "attachment_required",
                "Only attached window targets can be detached; close the context to retire its root.",
            ));
        }
        // Keep the lease mutex until removal. No concurrent expiry/claim can
        // cause a leased target to disappear or implicitly release authority.
        let book = self.leases.shared();
        let mut leases = book
            .lock()
            .map_err(|_| fault("lease_error", "No target detached."))?;
        leases.require_idle_target(target)?;
        self.targets.remove(target);
        self.snapshots.retain(|(_, s)| s.target != target);
        self.refs.retain(|r| r.target != target);
        Ok(
            serde_json::json!({"target":target,"detached":true,"input_sent":false,
            "durable_receipts_preserved":true,"leases_released":false}),
        )
    }
}

#[cfg(test)]
mod tests;
