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
        let runtime_status = native.as_ref().ok().map(|b| b.runtime_status());
        let native_capabilities = native
            .as_ref()
            .ok()
            .map(|b| b.capabilities())
            .unwrap_or_default();
        let domain = identity.as_ref().map(rcu_core::Identity::key);
        let native_ready = native.is_ok();
        let reason = native.err().map(|e| e.code);
        let (platform, implementation) = match &self.config.platform {
            PlatformPolicy::MacOs { .. } => ("macos", cfg!(target_os = "macos")),
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
                let implemented = control
                    || implementation
                        && if platform == "macos" {
                            macos_tool_implemented(tool.name)
                        } else {
                            !native_lifecycle || platform == "windows"
                        };
                let prerequisite = match tool.name {
                    _ if platform == "macos" => macos_prerequisite(tool.name, &native_capabilities),
                    "computer.capture_visual" if platform == "windows" => {
                        Some("selected_window_required")
                    }
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
        result.data["runtime_status"] = json!(runtime_status);
        // Diagnostic process facts remain available when desktop construction is
        // unavailable. They do not turn service health into GUI/input authority.
        result.data["service_process_status"] = if policy.observe {
            platform::process_status(&self.config)
        } else {
            json!({"status":"not_authorized"})
        };
        result
    }
}

fn macos_tool_implemented(tool: &str) -> bool {
    matches!(
        tool,
        "computer.targets"
            | "computer.operation"
            | "computer.cancel"
            | "computer.discover"
            | "computer.bind_window"
            | "computer.observe"
            | "computer.resolve"
            | "computer.claim"
            | "computer.act"
            | "computer.act_checked"
            | "computer.verify"
            | "computer.release"
            | "computer.artifact"
            | "computer.image"
            | "computer.resolve_view"
            | "computer.recover_input"
    )
}

#[cfg(test)]
mod macos_tests;
fn macos_prerequisite(tool: &str, caps: &serde_json::Value) -> Option<&'static str> {
    let permissions = &caps["permissions"];
    if matches!(
        tool,
        "computer.observe" | "computer.act" | "computer.act_checked" | "computer.verify"
    ) && permissions["accessibility"] != true
    {
        return Some("accessibility_permission_missing");
    }
    if tool == "computer.observe" {
        if permissions["screen_capture"] != true {
            return Some("screen_capture_permission_missing");
        }
        if permissions["screenshot_api"] != true {
            return Some("capture_api_unavailable");
        }
    }
    if matches!(tool, "computer.act" | "computer.act_checked") && permissions["post_event"] != true
    {
        return Some("input_permission_missing");
    }
    if tool == "computer.recover_input" && permissions["listen_event"] != true {
        return Some("reconciliation_unavailable");
    }
    if matches!(
        tool,
        "computer.observe" | "computer.act" | "computer.act_checked" | "computer.verify"
    ) {
        // Background operation: the exact window must be discovered and bound
        // first; it does not need to be frontmost.
        return Some("selected_window_required");
    }
    None
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    #[test]
    fn service_diagnostics_neither_require_a_window_nor_bypass_observation_policy() {
        let dir = tempfile::tempdir().unwrap();
        let mut policy = Principal {
            certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
            observe: true,
            input: false,
        };
        let runtime = Runtime::new(Config {
            version: 1,
            device_id: "diagnostic-test".into(),
            bind: "127.0.0.1:0".parse().unwrap(),
            identity: rcu_link_trust::IdentityPaths::in_profile(dir.path()),
            principals: vec![policy.clone()],
            state_root: dir.path().join("state"),
            platform: PlatformPolicy::Unsupported {
                platform: "offline-test".into(),
            },
        })
        .unwrap();
        let result = runtime.describe(&policy);
        assert_eq!(result.data["service"], "healthy");
        assert_eq!(result.data["availability"], "unavailable");
        assert_eq!(
            result.data["service_process_status"]["native_token_measured"],
            false
        );
        assert!(result.data["service_process_status"].get("pid").is_none());
        policy.observe = false;
        let denied = runtime.describe(&policy);
        assert_eq!(
            denied.data["service_process_status"],
            json!({"status":"not_authorized"})
        );
        assert_eq!(denied.data["service"], "healthy");
    }
}
