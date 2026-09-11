//! Device support, current availability and principal authority are independent.
use super::*;
use crate::config::PlatformPolicy;

impl Runtime {
    pub(super) fn describe(&self, policy: &Principal) -> DeviceResult {
        let native = if policy.observe {
            platform::backend(&self.config, policy, &uuid::Uuid::new_v4().to_string())
        } else {
            Err(rcu_core::fault(
                "authorization_required",
                "No observation permission.",
            ))
        };
        let identity = native.as_ref().ok().map(|b| b.identity());
        let domain = identity.as_ref().map(rcu_core::Identity::key);
        let native_ready = native.is_ok();
        let reason = native.err().map(|e| e.code);
        let (platform, implementation) = match &self.config.platform {
            PlatformPolicy::Windows { .. } => ("windows", cfg!(windows)),
            PlatformPolicy::Unsupported { platform } => (platform.as_str(), false),
            #[cfg(feature = "fixture")]
            PlatformPolicy::Fixture => ("synthetic_fixture", true),
        };
        let capabilities = catalog::definitions()
            .into_iter()
            .map(|tool| {
                let control = matches!(
                    tool.name,
                    "computer.targets" | "computer.operation" | "computer.cancel"
                );
                let needs_input =
                    tool.writes || matches!(tool.name, "computer.claim" | "computer.recover_input");
                let authorized = if needs_input {
                    policy.input
                } else {
                    policy.observe
                };
                let native_lifecycle = matches!(
                    tool.name,
                    "computer.discover"
                        | "computer.bind_window"
                        | "computer.lifecycle_preflight"
                        | "computer.launch_application"
                        | "computer.activate_window"
                        | "computer.lifecycle_receipt"
                );
                // Fixture is not an implementation of native window/lifecycle APIs.
                let implemented =
                    control || implementation && (!native_lifecycle || platform == "windows");
                let prerequisite = match tool.name {
                    "computer.launch_application" => Some("application_policy_not_configured"),
                    "computer.lifecycle_preflight"
                    | "computer.activate_window"
                    | "computer.lifecycle_receipt" => {
                        Some("selected_window_authorization_required")
                    }
                    _ => None,
                };
                let available = implemented
                    && authorized
                    && (control || native_ready)
                    && prerequisite.is_none();
                Capability {
                    id: tool.name.into(),
                    implemented,
                    available,
                    authorized,
                    reason: if !implemented {
                        Some("not_implemented_on_this_platform".into())
                    } else if !authorized {
                        Some("principal_permission_denied".into())
                    } else if !control && !native_ready {
                        reason.clone()
                    } else {
                        prerequisite.map(str::to_owned)
                    },
                }
            })
            .collect();
        let mut result = DeviceResult::value(DeviceInfo {
            device_id: self.config.device_id.clone(),
            instance: self.instance.clone(),
            platform: platform.into(),
            availability: if native_ready {
                "available"
            } else {
                "unavailable"
            }
            .into(),
            native_context: identity,
            capabilities,
        });
        result.data["service"] = json!("healthy");
        result.data["device_protocol_version"] = json!(PROTOCOL_VERSION);
        result.data["execution_ref_version"] = json!(EXECUTION_REF_VERSION);
        result.data["execution_identity"] =
            json!("service_reference; correlation_labels_are_not_lookup_keys");
        result.data["capability_scope"] = json!(
            "device_support_and_policy; frame, binding, focus and lease prerequisites are checked per operation"
        );
        result.data["input_domain_fenced"] =
            json!(domain.as_deref().is_some_and(|key| self.is_fenced(key)));
        result.data["resource_domain"] = json!(domain);
        result
    }
}
