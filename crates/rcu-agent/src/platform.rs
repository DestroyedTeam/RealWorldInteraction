//! Platform construction depends on policy and the current OS context, never a task.
#[cfg(any(windows, feature = "fixture"))]
use crate::config::PlatformPolicy;
use crate::config::{Config, Principal};
use rcu_core::{Backend, Result, fault};

/// Independent of a selected window and backend readiness; does not request rights.
pub fn process_status(config: &Config) -> serde_json::Value {
    #[cfg(windows)]
    if matches!(config.platform, PlatformPolicy::Windows { .. }) {
        return match rcu_windows::agent_binding::current_process_status() {
            Ok(status) => status,
            Err(error) => {
                serde_json::json!({"status":"unavailable","pid":std::process::id(),"error":error})
            }
        };
    }
    let _ = config;
    serde_json::json!({"status":"unsupported_for_this_platform_policy","platform":std::env::consts::OS,"native_token_measured":false})
}
pub fn backend(config: &Config, policy: &Principal, context: &str) -> Result<Box<dyn Backend>> {
    match &config.platform {
        #[cfg(windows)]
        PlatformPolicy::Windows {
            user_sid,
            privilege,
        } => {
            let current = rcu_windows::agent_binding::current_session()?;
            if &current.user_sid != user_sid {
                return Err(fault(
                    "native_user_mismatch",
                    "Agent must run in the allocated OS user context.",
                ));
            }
            let mode = match privilege.as_str() {
                "admin" => rcu_windows::agent_contract::PrivilegeMode::Admin,
                _ => rcu_windows::agent_contract::PrivilegeMode::Ordinary,
            };
            rcu_windows::agent_binding::require_current_privilege(mode)?;
            Ok(Box::new(
                rcu_windows::NativeWindows::new_with_desktop_policy(context, policy.input)?,
            ))
        }
        #[cfg(feature = "fixture")]
        PlatformPolicy::Fixture => Ok(Box::new(rcu_core::fixture::Fixture::default())),
        _ => {
            let _ = (policy, context);
            Err(fault(
                "platform_unavailable",
                "Native capabilities are not implemented/available in this build or OS context.",
            ))
        }
    }
}

pub fn input_resources(config: &Config) -> Result<serde_json::Value> {
    match &config.platform {
        #[cfg(windows)]
        PlatformPolicy::Windows { .. } => rcu_windows::recovery::input_resources(),
        #[cfg(feature = "fixture")]
        PlatformPolicy::Fixture => Ok(
            serde_json::json!({"source":"explicit_synthetic_fixture","native":false,"input_sent":false}),
        ),
        _ => Err(fault(
            "reconciliation_unavailable",
            "This platform has no implemented native input resource check.",
        )),
    }
}

/// Called once by the serving executable, never by Runtime::new or a test query.
pub fn start_device_feedback(config: &Config) -> Result<()> {
    #[cfg(windows)]
    if let PlatformPolicy::Windows {
        user_sid,
        privilege,
    } = &config.platform
    {
        let current = rcu_windows::agent_binding::current_session()?;
        if &current.user_sid != user_sid {
            return Err(fault(
                "native_user_mismatch",
                "Feedback requires the configured native user.",
            ));
        }
        let mode = if privilege == "admin" {
            rcu_windows::agent_contract::PrivilegeMode::Admin
        } else {
            rcu_windows::agent_contract::PrivilegeMode::Ordinary
        };
        rcu_windows::agent_binding::require_current_privilege(mode)?;
        return rcu_windows::start_pointer_feedback();
    }
    let _ = config;
    Ok(())
}

pub fn stop_device_feedback() {
    #[cfg(windows)]
    rcu_windows::stop_pointer_feedback();
}
