//! Windows per-user installation. Bundle/config describe a service, never a task.
/// One-shot configuration diagnosis. Creates only the explicitly named report
/// under an already validated private directory, never a service or state root.
#[cfg(windows)]
pub fn check_config(path: &std::path::Path, report: &std::path::Path) -> std::io::Result<()> {
    use rwi_windows::agent_binding::{PrivateDirectory, read_private_file_no_reparse};
    use std::io;
    let directory = PrivateDirectory::open(
        report
            .parent()
            .ok_or_else(|| io::Error::other("report_parent_required"))?,
    )
    .map_err(|e| io::Error::other(e.to_string()))?;
    let name = report
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| io::Error::other("literal_report_name_required"))?;
    let result = (|| -> io::Result<()> {
        let bytes = read_private_file_no_reparse(path, 65536)
            .map_err(|e| io::Error::other(e.to_string()))?;
        let config: crate::config::Config = serde_json::from_slice(&bytes)?;
        config.validate()
    })();
    let facts = serde_json::json!({
        "check":"private_config_read_and_schema_only",
        "configuration_valid":result.is_ok(),
        "error":result.as_ref().err().map(ToString::to_string),
        "checking_process":rwi_windows::agent_binding::current_process_status().ok(),
        "service_started":false,"state_directory_touched":false,
        "tls_loaded":false,"gui_validated":false,"input_sent":false
    });
    directory
        .create_new(name, &serde_json::to_vec(&facts)?, 65536)
        .map_err(|e| io::Error::other(e.to_string()))?;
    result
}

#[cfg(windows)]
pub fn import(path: &std::path::Path) -> std::io::Result<serde_json::Value> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use rwi_windows::agent_binding::{PrivateDirectory, read_private_file_no_reparse};
    use std::{collections::BTreeMap, io};
    use zeroize::{Zeroize, Zeroizing};
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct File {
        sha256: String,
        base64: String,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Bundle {
        version: u16,
        files: BTreeMap<String, File>,
    }
    impl Drop for File {
        fn drop(&mut self) {
            self.base64.zeroize();
        }
    }
    fn err(e: rwi_core::Fault) -> io::Error {
        io::Error::other(e.to_string())
    }
    let raw = Zeroizing::new(read_private_file_no_reparse(path, 32 * 1024 * 1024).map_err(err)?);
    let bundle: Bundle = serde_json::from_slice(&raw)?;
    let expected = [
        "agent.json",
        "ca.der",
        "certificate.der",
        "private-key.der",
        "rwi-agent.exe",
    ];
    if bundle.version != 1
        || bundle.files.len() != expected.len()
        || expected.iter().any(|n| !bundle.files.contains_key(*n))
    {
        return Err(io::Error::other("invalid_service_bundle"));
    }
    let mut files = BTreeMap::new();
    for (name, file) in bundle.files {
        let bytes = STANDARD
            .decode(file.base64.as_bytes())
            .map_err(io::Error::other)?;
        if bytes.len() > 16 * 1024 * 1024 || rwi_core::digest(&bytes) != file.sha256 {
            return Err(io::Error::other("bundle_digest_mismatch"));
        }
        files.insert(name, Zeroizing::new(bytes));
    }
    let config: crate::config::Config = serde_json::from_slice(&files["agent.json"])?;
    config.validate()?;
    let legacy_root = rwi_windows::agent_binding::fixed_agent_installation_root().map_err(err)?;
    let parent = legacy_root
        .parent()
        .ok_or_else(|| io::Error::other("installation_parent"))?;
    let root = parent.join("agent-v1");
    let trust = root.join("trust");
    if config.state_root != root.join("state")
        || config.identity.ca != trust.join("ca.der")
        || config.identity.certificate != trust.join("certificate.der")
        || config.identity.private_key != trust.join("private-key.der")
    {
        return Err(io::Error::other("service_installation_path_mismatch"));
    }
    match &config.platform {
        crate::config::PlatformPolicy::Windows {
            user_sid,
            privilege,
        } => {
            let session = rwi_windows::agent_binding::current_session().map_err(err)?;
            if &session.user_sid != user_sid {
                return Err(io::Error::other("installation_user_mismatch"));
            }
            let mode = if privilege == "admin" {
                rwi_windows::agent_contract::PrivilegeMode::Admin
            } else {
                rwi_windows::agent_contract::PrivilegeMode::Ordinary
            };
            rwi_windows::agent_binding::require_current_privilege(mode).map_err(err)?;
        }
        _ => return Err(io::Error::other("windows_policy_required")),
    }
    // Validate crypto before creating the installation. The native reader has
    // already established the bundle's private owner/ACL and no-reparse chain.
    let identity = config.identity.load_with(|p, _| {
        files
            .get(p.file_name().and_then(|n| n.to_str()).unwrap_or_default())
            .map(|b| b.to_vec())
            .ok_or_else(|| io::Error::other("identity_file_missing"))
    })?;
    identity.server_config(
        &config
            .principals
            .iter()
            .map(|p| p.certificate.clone())
            .collect::<Vec<_>>(),
    )?;
    if !parent.exists() {
        PrivateDirectory::create(parent).map_err(err)?;
    }
    let directory = PrivateDirectory::create(&root).map_err(err)?;
    let private_trust = PrivateDirectory::create(&trust).map_err(err)?;
    for (name, bytes) in files {
        let base = if name.ends_with(".der") {
            &private_trust
        } else {
            &directory
        };
        base.create_new(&name, &bytes, 16 * 1024 * 1024)
            .map_err(err)?;
    }
    Ok(
        serde_json::json!({"installed":true,"root":root,"device_id":config.device_id,"workflow_required":false,"started":false,"global_services_changed":false}),
    )
}
