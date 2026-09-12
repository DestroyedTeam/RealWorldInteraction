//! Device-side desktop authorization narrowed to a freshly selected window.
//! Selecting a window does not create permission; the local service policy does.
use super::*;
pub(super) struct DesktopPolicy {
    pub context: String,
    pub input: bool,
}
impl NativeWindows {
    pub fn new_with_desktop_policy(context: &str, input: bool) -> Result<Self> {
        if uuid_shape(context).is_none() {
            return Err(fault(
                "invalid_client_context",
                "Use a server-issued context.",
            ));
        }
        let mut native = Self::new()?;
        native.desktop_policy = Some(DesktopPolicy {
            context: context.into(),
            input,
        });
        Ok(native)
    }
    pub(super) fn narrow_service_selection(
        &mut self,
        window: &WindowRecord,
        relation: &rcu_core::WindowRelation,
    ) -> Result<()> {
        let Some(policy) = &self.desktop_policy else {
            return Ok(());
        };
        if matches!(relation, rcu_core::WindowRelation::Initial) {
            self.check_discovery_identity()?;
        } else {
            self.check_identity()?;
        }
        if !policy.input {
            return Ok(());
        };
        let root = if matches!(relation, rcu_core::WindowRelation::Initial) {
            window.clone()
        } else {
            self.allocated_root
                .clone()
                .ok_or_else(|| fault("root_not_selected", "Select the authorized root first."))?
        };
        let expires = input::now_ms() + 3_600_000;
        let grant = InputGrant {
            run_id: policy.context.clone(),
            route_vm_id: String::new(),
            expected_identity: self.identity.clone(),
            root: root.key().ok_or_else(|| {
                fault(
                    "window_identity_unknown",
                    "Fresh native birth time is required.",
                )
            })?,
            expires_unix_ms: expires,
        };
        grant.validate(&self.identity, input::now_ms())?;
        if !grant.allows(window) {
            return Err(fault(
                "window_scope_mismatch",
                "Native owner chain must match the selected root.",
            ));
        }
        self.input_grant = Some(grant);
        self.allocated_root = Some(root);
        self.grant_deadline = Some(Instant::now() + Duration::from_secs(3600));
        let key = rcu_core::lifecycle::WindowIdentity {
            window: window.window.clone(),
            pid: window.pid,
            process_created: window.process_created.ok_or_else(|| {
                fault("window_identity_unknown", "Native process birth required.")
            })?,
            native_epoch: window
                .native_epoch
                .clone()
                .ok_or_else(|| fault("window_epoch_unknown", "Fresh native epoch required."))?,
        };
        self.lifecycle_grant = Some(rcu_core::lifecycle::LifecycleGrant {
            run_id: policy.context.clone(),
            route_vm_id: String::new(),
            expected_identity: self.identity.clone(),
            expires_unix_ms: expires,
            launches: vec![],
            activations: vec![rcu_core::lifecycle::ActivationPermission {
                authorization_id: "selected-window".into(),
                window: key,
            }],
        });
        self.lifecycle_deadline = Some(Instant::now() + Duration::from_secs(3600));
        Ok(())
    }
}
fn uuid_shape(s: &str) -> Option<()> {
    (s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        }))
    .then_some(())
}
