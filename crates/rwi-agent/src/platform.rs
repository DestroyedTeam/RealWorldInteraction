//! Platform construction depends on policy and the current OS context, never a task.
#[cfg(any(windows, target_os = "macos", feature = "fixture"))]
use crate::config::PlatformPolicy;
use crate::config::{Config, Principal};
use rwi_core::{Backend, Result, fault};

/// Independent of a selected window and backend readiness; does not request rights.
pub fn process_status(config: &Config) -> serde_json::Value {
    #[cfg(target_os = "macos")]
    if let PlatformPolicy::MacOs { user_uid, .. } = config.platform {
        return rwi_macos::process_status(user_uid);
    }
    #[cfg(windows)]
    if matches!(config.platform, PlatformPolicy::Windows { .. }) {
        return match rwi_windows::agent_binding::current_process_status() {
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
        #[cfg(target_os = "macos")]
        PlatformPolicy::MacOs {
            user_uid,
            authorized_display,
        } => Ok(Box::new(rwi_macos::NativeMacDesktop::new(
            *user_uid,
            policy.input,
            authorized_display
                .as_ref()
                .map(|display| rwi_macos::AuthorizedDisplay {
                    built_in: display.built_in,
                    id: display.id,
                    place_on_bind: display.place_on_bind,
                }),
        )?)),
        #[cfg(windows)]
        PlatformPolicy::Windows {
            user_sid,
            privilege,
        } => {
            let current = rwi_windows::agent_binding::current_session()?;
            if &current.user_sid != user_sid {
                return Err(fault(
                    "native_user_mismatch",
                    "Agent must run in the allocated OS user context.",
                ));
            }
            let mode = match privilege.as_str() {
                "admin" => rwi_windows::agent_contract::PrivilegeMode::Admin,
                _ => rwi_windows::agent_contract::PrivilegeMode::Ordinary,
            };
            rwi_windows::agent_binding::require_current_privilege(mode)?;
            Ok(Box::new(
                rwi_windows::NativeWindows::new_with_desktop_policy(context, policy.input)?,
            ))
        }
        #[cfg(feature = "fixture")]
        PlatformPolicy::Fixture => Ok(Box::new(rwi_core::fixture::Fixture::default())),
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
        #[cfg(target_os = "macos")]
        PlatformPolicy::MacOs { user_uid, .. } => rwi_macos::input_resources(*user_uid),
        #[cfg(windows)]
        PlatformPolicy::Windows { .. } => rwi_windows::recovery::input_resources(),
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
        let current = rwi_windows::agent_binding::current_session()?;
        if &current.user_sid != user_sid {
            return Err(fault(
                "native_user_mismatch",
                "Feedback requires the configured native user.",
            ));
        }
        let mode = if privilege == "admin" {
            rwi_windows::agent_contract::PrivilegeMode::Admin
        } else {
            rwi_windows::agent_contract::PrivilegeMode::Ordinary
        };
        rwi_windows::agent_binding::require_current_privilege(mode)?;
        return rwi_windows::start_pointer_feedback();
    }
    let _ = config;
    Ok(())
}

pub fn stop_device_feedback() {
    #[cfg(windows)]
    rwi_windows::stop_pointer_feedback();
}
