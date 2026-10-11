//! Device support, current availability and principal authority are independent.
use super::*;
use crate::config::PlatformPolicy;

impl Runtime {
    pub(super) fn describe(&self, policy: &Principal) -> DeviceResult {
        let native = if policy.observe {
            platform::backend(&self.config, policy, &uuid::Uuid::new_v4().to_string())
        } else {
            Err(rwi_core::fault(
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
        let domain = identity.as_ref().map(rwi_core::Identity::key);
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
                        | "computer.attach_window"
                        | "computer.detach_window"
                        | "computer.lifecycle_preflight"
                        | "computer.launch_application"
                        | "computer.activate_window"
                        | "computer.lifecycle_receipt"
                );
                // Fixture is not an implementation of native window/lifecycle APIs.
                let macos_only = matches!(
                    tool.name,
                    "computer.move_window"
                        | "computer.focus_window"
                        | "computer.hotkey"
                        | "computer.live"
                        | "computer.attach_window"
                        | "computer.detach_window"
                );
                let implemented = control
                    || implementation
                        && if platform == "macos" {
                            macos_tool_implemented(tool.name)
                        } else {
                            !macos_only && (!native_lifecycle || platform == "windows")
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
        // Describe already probes the backend independently of the busy native
        // executor. Keep volatile desktop facts separate from its owner catalog.
        let live_desktop_facts = json!({
            "identity": identity,
            "values": {
                "current_foreground": native_capabilities.get("current_foreground"),
                "user_input_recent_seconds": native_capabilities.get("user_input_recent_seconds")
            },
            "captured_unix_ms": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            "scope": "sampled Describe desktop facts; not atomic target binding or input authority"
        });
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
        // Executor parameter support is not native/window/anchor readiness.
        // Missing metadata on an older peer means unknown, never a downgrade.
        result.data["executor_features"] = json!({
            "resolve_view_red_marker_cluster": {
                "implemented": true, "opt_in_required": true, "fallback": false,
                "unavailable_error": "red_marker_anchor_unavailable",
                "scope": "executor_parameter; native_anchor_is_checked_per_view"
            },
            "move_pointer_intent": {
                "implemented": true, "requires_native_backend_support": true,
                "fallback": false, "scope": "intent_parser; not native hover-effect availability"
            }
        });
        result.data["recent_operations"] = json!(recent_operations(&self.config.state_root, 5));
        result.data["runtime_status"] = json!(runtime_status);
        result.data["live_desktop_facts"] = live_desktop_facts;
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

pub(super) fn refresh_volatile_target_facts(
    targets: &mut serde_json::Value,
    facts: &serde_json::Value,
) {
    let Some(rows) = targets.as_array_mut() else {
        return;
    };
    for row in rows {
        let same_identity =
            !facts["identity"].is_null() && row.get("identity") == facts.get("identity");
        let Some(caps) = row
            .get_mut("capabilities")
            .and_then(serde_json::Value::as_object_mut)
        else {
            continue;
        };
        for name in ["current_foreground", "user_input_recent_seconds"] {
            let value = if same_identity {
                facts["values"]
                    .get(name)
                    .cloned()
                    .unwrap_or(serde_json::Value::Null)
            } else {
                serde_json::Value::Null
            };
            caps.insert(name.into(), value);
        }
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
            | "computer.attach_window"
            | "computer.detach_window"
            | "computer.observe"
            | "computer.capture_visual"
            | "computer.live"
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
            | "computer.move_window"
            | "computer.focus_window"
            | "computer.pointer_feedback"
            | "computer.hotkey"
    )
}

/// Bounded user-facing tail of the per-principal operation journals. It reports
/// what RWI attempted, never that an action took effect.
fn recent_operations(state_root: &std::path::Path, limit: usize) -> Vec<serde_json::Value> {
    let Ok(entries) = std::fs::read_dir(state_root.join("principals")) else {
        return Vec::new();
    };
    let mut rows: Vec<serde_json::Value> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path().join("operations.jsonl");
        rows.extend(rwi_core::operation_journal::recent(&path, limit));
    }
    rows.sort_by_key(|row| std::cmp::Reverse(row["ts_unix_ms"].as_u64().unwrap_or(0)));
    rows.truncate(limit.min(20));
    rows
}

#[cfg(test)]
mod macos_tests;
#[cfg(test)]
mod global_keyboard_tests {
    use super::*;
    #[test]
    fn global_hotkey_capability_is_desktop_scoped_and_keeps_native_permission_prerequisites() {
        assert!(macos_tool_implemented("computer.hotkey"));
        let caps = serde_json::json!({"permissions":{"accessibility":true,"post_event":true,"listen_event":true},
            "foreground_keyboard":{"implemented":true},"window_selected":false});
        assert_eq!(macos_prerequisite("computer.hotkey", &caps), None);
        for (permission, reason) in [
            ("accessibility", "accessibility_permission_missing"),
            ("post_event", "input_permission_missing"),
            ("listen_event", "listen_event_permission_missing"),
        ] {
            let mut missing = caps.clone();
            missing["permissions"][permission] = serde_json::json!(false);
            assert_eq!(
                macos_prerequisite("computer.hotkey", &missing),
                Some(reason)
            );
        }
        let mut attachment = caps;
        attachment["foreground_keyboard"]["implemented"] = serde_json::json!(false);
        assert_eq!(
            macos_prerequisite("computer.hotkey", &attachment),
            Some("desktop_global_keyboard_grant_required")
        );
    }
}
fn macos_prerequisite(tool: &str, caps: &serde_json::Value) -> Option<&'static str> {
    let permissions = &caps["permissions"];
    if matches!(
        tool,
        "computer.observe"
            | "computer.act"
            | "computer.act_checked"
            | "computer.verify"
            | "computer.move_window"
            | "computer.focus_window"
    ) && permissions["accessibility"] != true
    {
        return Some("accessibility_permission_missing");
    }
    if tool == "computer.hotkey" {
        if permissions["accessibility"] != true {
            return Some("accessibility_permission_missing");
        }
        if permissions["post_event"] != true {
            return Some("input_permission_missing");
        }
        if permissions["listen_event"] != true {
            return Some("listen_event_permission_missing");
        }
        if caps["foreground_keyboard"]["implemented"] != true {
            return Some("desktop_global_keyboard_grant_required");
        }
        return None;
    }
    if matches!(tool, "computer.observe" | "computer.capture_visual") {
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
    if tool == "computer.focus_window" && permissions["listen_event"] != true {
        return Some("listen_event_permission_missing");
    }
    if matches!(
        tool,
        "computer.observe"
            | "computer.capture_visual"
            | "computer.act"
            | "computer.act_checked"
            | "computer.verify"
            | "computer.move_window"
            | "computer.focus_window"
    ) {
        // Background operation: the exact window must be discovered and bound
        // first; it does not need to be frontmost.
        return Some("selected_window_required");
    }
    None
}

#[cfg(test)]
mod diagnostic_tests {
    #[test]
    fn volatile_desktop_facts_replace_only_same_identity_and_keep_owner_binding() {
        let identity = serde_json::json!({"host":"own","session":"same"});
        let foreground = serde_json::json!({"pid":9,"window":"cgwindow:99",
            "process_start":"new-process","native_epoch":"new-window"});
        let mut targets = serde_json::json!([
            {"target":"owned","identity":identity,"capabilities":{
                "current_foreground":{"pid":7},"user_input_recent_seconds":99,
                "window_selected":true,"input_opt_in":false,"owner_marker":"unchanged"}},
            {"target":"foreign-session","identity":{"host":"own","session":"other"},
                "capabilities":{"current_foreground":{"pid":7},"owner_marker":"other"}}
        ]);
        super::refresh_volatile_target_facts(
            &mut targets,
            &serde_json::json!({
            "identity":identity,"values":{"current_foreground":foreground,"user_input_recent_seconds":0.5}}),
        );
        assert_eq!(targets[0]["capabilities"]["current_foreground"], foreground);
        assert_eq!(targets[0]["capabilities"]["user_input_recent_seconds"], 0.5);
        assert_eq!(targets[0]["capabilities"]["window_selected"], true);
        assert_eq!(targets[0]["capabilities"]["input_opt_in"], false);
        assert_eq!(targets[0]["capabilities"]["owner_marker"], "unchanged");
        assert!(targets[1]["capabilities"]["current_foreground"].is_null());
        assert_eq!(targets[1]["capabilities"]["owner_marker"], "other");
    }

    #[test]
    fn volatile_desktop_facts_missing_identity_never_preserves_stale_foreground() {
        let mut targets = serde_json::json!([{"identity":{"host":"own"},
            "capabilities":{"current_foreground":{"pid":7},"user_input_recent_seconds":0.0}}]);
        super::refresh_volatile_target_facts(
            &mut targets,
            &serde_json::json!({
            "identity":null,"values":{"current_foreground":{"pid":9},"user_input_recent_seconds":1.0}}),
        );
        assert!(targets[0]["capabilities"]["current_foreground"].is_null());
        assert!(targets[0]["capabilities"]["user_input_recent_seconds"].is_null());
    }
    use super::*;
    #[test]
    fn foreground_permission_scope_is_ax_and_listen_not_cgevent_post() {
        assert!(macos_tool_implemented("computer.focus_window"));
        assert!(!macos_tool_implemented("computer.activate_window"));
        let mut caps =
            json!({"permissions":{"accessibility":true,"post_event":false,"listen_event":false}});
        assert_eq!(
            macos_prerequisite("computer.focus_window", &caps),
            Some("listen_event_permission_missing")
        );
        caps["permissions"]["listen_event"] = json!(true);
        assert_eq!(
            macos_prerequisite("computer.focus_window", &caps),
            Some("selected_window_required")
        );
        caps["permissions"]["accessibility"] = json!(false);
        assert_eq!(
            macos_prerequisite("computer.focus_window", &caps),
            Some("accessibility_permission_missing")
        );
    }
    #[test]
    fn service_diagnostics_neither_require_a_window_nor_bypass_observation_policy() {
        let dir = tempfile::tempdir().unwrap();
        let mut policy = Principal {
            journal_identity: None,
            certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
            observe: true,
            input: false,
        };
        let runtime = Runtime::new(Config {
            version: 1,
            device_id: "diagnostic-test".into(),
            bind: "127.0.0.1:0".parse().unwrap(),
            identity: rwi_link_trust::IdentityPaths::in_profile(dir.path()),
            principals: vec![policy.clone()],
            state_root: dir.path().join("state"),
            platform: PlatformPolicy::Unsupported {
                platform: "offline-test".into(),
            },
        })
        .unwrap();
        let result = runtime.describe(&policy);
        assert_eq!(result.data["service"], "healthy");
        assert_eq!(
            result.data["executor_features"]["resolve_view_red_marker_cluster"],
            json!({
                "implemented":true,"opt_in_required":true,"fallback":false,
                "unavailable_error":"red_marker_anchor_unavailable",
                "scope":"executor_parameter; native_anchor_is_checked_per_view"
            })
        );

        assert_eq!(result.data["availability"], "unavailable");
        assert_eq!(
            result.data["service_process_status"]["native_token_measured"],
            false
        );
        assert!(result.data["service_process_status"].get("pid").is_none());
        assert_eq!(
            result.data["executor_features"]["move_pointer_intent"]["implemented"],
            true
        );
        assert_eq!(
            result.data["executor_features"]["move_pointer_intent"]["requires_native_backend_support"],
            true
        );
        assert_eq!(
            result.data["executor_features"]["move_pointer_intent"]["fallback"],
            false
        );
        policy.observe = false;
        let denied = runtime.describe(&policy);
        assert_eq!(
            denied.data["service_process_status"],
            json!({"status":"not_authorized"})
        );
        assert_eq!(denied.data["service"], "healthy");
    }
}
