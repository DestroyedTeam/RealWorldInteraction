//! Exact native selection and owner-chain checks.
use super::*;
impl NativeWindows {
    pub(super) fn bind_selected(&mut self, s: &WindowSelection) -> Result<Value> {
        if self.desktop_policy.is_some() && matches!(s.relation, rwi_core::WindowRelation::Initial)
        {
            // Explicitly selecting a fresh resource under the same authorized
            // desktop does not renew or replay input on the old resource.
            self.check_discovery_identity()?;
        } else {
            self.check_identity()?;
        }
        let _dpi = DpiScope::enter()?;
        if self
            .catalog_at
            .is_none_or(|t| t.elapsed() > Duration::from_secs(10))
        {
            return Err(fault("catalog_expired", "Discover again before binding."));
        }
        let selected = self
            .catalog
            .iter()
            .find(|w| {
                w.window == s.window
                    && w.pid == s.pid
                    && w.native_epoch.as_deref() == Some(&s.native_epoch)
            })
            .ok_or_else(|| {
                fault(
                    "window_not_discovered",
                    "Use an exact native catalog entry; unknown process epochs cannot bind.",
                )
            })?;
        let current = desktop::record(hwnd(&s.window)?, selected.z_index)?;
        if current.native_epoch != selected.native_epoch
            || current.owner_chain != selected.owner_chain
        {
            return Err(fault(
                "window_epoch_changed",
                "Window or owner relation changed since discovery.",
            ));
        }
        if !(self.desktop_policy.is_some()
            && matches!(s.relation, rwi_core::WindowRelation::Initial))
        {
            check_relation(self.selected.as_ref(), &current, &s.relation)?;
        }
        self.narrow_service_selection(&current, &s.relation)?;
        if self
            .input_grant
            .as_ref()
            .is_some_and(|g| !g.allows(&current))
        {
            return Err(fault(
                "input_window_not_allocated",
                "Input-enabled connection may bind only its allocated root or native-owned dialogs.",
            ));
        }
        if let Some(authority) = &self.agent_input {
            if matches!(s.relation, rwi_core::WindowRelation::Initial) {
                if current.window != authority.root().window
                    || current.pid != authority.root().pid
                    || current.process_created != Some(authority.root().process_created)
                    || current.native_epoch.as_deref() != Some(&authority.root().native_epoch)
                {
                    return Err(fault(
                        "agent_initial_root_bind_required",
                        "The agent's first selected window must be its unchanged allocated root, never an arbitrary owned popup.",
                    ));
                }
                authority.pin_root_runtime()?;
            } else {
                authority.verify_root_runtime()?;
            }
            authority.retain_selection(&current)?;
        }
        self.root_runtime = None;
        self.foreground_anchor = None;
        self.selected = Some(current.clone());
        self.selected_catalog_id = self.catalog_id.clone();
        self.diagnostics = Value::Null;
        Ok(
            json!({"bound":current,"relation":s.relation,"input":self.input_grant.is_some(),"next":"Observe this exact window; prior frames/references invalidated. Keyboard input requires the exact focused UIA element."}),
        )
    }
}
