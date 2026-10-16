//! User-local import, no service/global registration or permission repair.
//! Private material enters stdin, never argv, diagnostics, or output evidence.
use crate::{
    invalid, sha,
    types::{AgentConfig, Approval, BackendMode, digest_shape},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, io, path::Path};
use zeroize::{Zeroize, Zeroizing};
pub const BUNDLE_CAP: usize = 128 * 1024 * 1024;
const NAMES: [&str; 7] = [
    "agent.json",
    "approved-task.json",
    "ca.der",
    "certificate.der",
    "private-key.der",
    "rwi-native-agent.exe",
    "rwi-mcp.exe",
];
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Blob {
    sha256: String,
    base64: String,
}
impl Drop for Blob {
    fn drop(&mut self) {
        self.base64.zeroize();
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bundle {
    version: u8,
    files: BTreeMap<String, Blob>,
}
struct Prepared {
    config: AgentConfig,
    approval: Approval,
    files: BTreeMap<String, Zeroizing<Vec<u8>>>,
}
fn same_dos(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .replace('/', "\\")
        .eq_ignore_ascii_case(&b.to_string_lossy().replace('/', "\\"))
}
fn parse(bytes: &[u8]) -> io::Result<Prepared> {
    if bytes.len() > BUNDLE_CAP {
        return Err(invalid("private_import_bundle_cap"));
    }
    let bundle: Bundle = serde_json::from_slice(bytes)?;
    if bundle.version != 1
        || bundle.files.len() != NAMES.len()
        || NAMES.iter().any(|n| !bundle.files.contains_key(*n))
    {
        return Err(invalid("exact_private_import_bundle_names"));
    }
    let mut files = BTreeMap::new();
    for (name, blob) in &bundle.files {
        let cap = if name.ends_with(".exe") {
            48 * 1024 * 1024
        } else if name == "approved-task.json" {
            256 * 1024
        } else {
            65536
        };
        if blob.base64.len() > cap * 4 / 3 + 8 || !digest_shape(&blob.sha256) {
            return Err(invalid("bundle_file_encoded_cap_or_digest"));
        }
        let decoded = Zeroizing::new(
            STANDARD
                .decode(&blob.base64)
                .map_err(|_| invalid("bundle_base64_invalid"))?,
        );
        if decoded.is_empty() || decoded.len() > cap || sha(&decoded) != blob.sha256 {
            return Err(invalid("bundle_file_digest_or_cap"));
        }
        files.insert(name.clone(), decoded);
    }
    let config: AgentConfig = serde_json::from_slice(&files["agent.json"])?;
    let approval: Approval = serde_json::from_slice(&files["approved-task.json"])?;
    approval.validate(BackendMode::Windows)?;
    let root = &config.install_root;
    rwi_windows::agent_contract::validate_local_dos_path(&root.to_string_lossy())
        .map_err(crate::private::core_error)?;
    let trust = rwi_link_trust::IdentityPaths::in_profile(&root.join("trust/agent"));
    if config.version != 1
        || config.privilege_mode.is_none()
        || !config.backend.is_windows()
        || !same_dos(&config.state_root, &root.join("state"))
        || !same_dos(&config.approval, &root.join("approved-task.json"))
        || !same_dos(
            &config.agent_image.path,
            &root.join("bin/rwi-native-agent.exe"),
        )
        || !same_dos(&config.worker_image.path, &root.join("bin/rwi-mcp.exe"))
        || config.agent_image.sha256 != sha(&files["rwi-native-agent.exe"])
        || config.worker_image.sha256 != sha(&files["rwi-mcp.exe"])
        || !same_dos(&config.trust.ca, &trust.ca)
        || !same_dos(&config.trust.certificate, &trust.certificate)
        || !same_dos(&config.trust.private_key, &trust.private_key)
        || config.controller_peer_sha256 == config.model_peer_sha256
        || config.bind.port() == 0
        || config.bind.ip().is_unspecified()
        || config.bind.ip().is_multicast()
    {
        return Err(invalid("bundle_fixed_native_configuration_boundary"));
    }
    rwi_link_trust::server_name(&config.server_name)?;
    crate::deployment::plan_validation::bootstrap(&approval, &root.to_string_lossy())?;
    // The DOS configuration was checked above. Offline Mac validation uses
    // memory-backed DER blobs, not an attempt to open DOS paths on this host.
    #[cfg(unix)]
    let memory_paths =
        rwi_link_trust::IdentityPaths::in_profile(Path::new("/rcu-memory-only-validation/agent"));
    #[cfg(windows)]
    let memory_paths = config.trust.clone();
    let identity = memory_paths.load_with(|path, cap| {
        let path_text = path.to_string_lossy();
        let name = path_text
            .rsplit(['\\', '/'])
            .next()
            .ok_or_else(|| invalid("bundle_trust_name"))?;
        let b = files
            .get(name)
            .ok_or_else(|| invalid("bundle_trust_missing"))?;
        if b.len() > cap {
            return Err(invalid("bundle_trust_cap"));
        }
        Ok(b.to_vec())
    })?;
    identity.server_config(&[
        rwi_link_trust::PeerPin::parse(&config.controller_peer_sha256)?,
        rwi_link_trust::PeerPin::parse(&config.model_peer_sha256)?,
    ])?;
    Ok(Prepared {
        config,
        approval,
        files,
    })
}
pub fn validate_bundle(bytes: &[u8]) -> io::Result<Value> {
    let prepared = parse(bytes)?;
    Ok(
        json!({"valid_bundle":true,"native_binding_runtime_checked":false,"installed":false,"install_root":prepared.config.install_root,"task":prepared.approval.task,"approval_sha256":prepared.approval.digest(),"files":prepared.files.iter().map(|(name,bytes)|(name,sha(bytes))).collect::<BTreeMap<_,_>>()}),
    )
}
/// Offline preparation only. Inputs are exact private local files; callers must
/// redirect returned bytes to a private file/channel, never an evidence logger.
pub fn make_bundle(bytes: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Sources {
        files: BTreeMap<String, std::path::PathBuf>,
    }
    if bytes.len() > 65536 {
        return Err(invalid("bundle_sources_manifest_cap"));
    }
    let sources: Sources = serde_json::from_slice(bytes)?;
    if sources.files.len() != NAMES.len() || NAMES.iter().any(|n| !sources.files.contains_key(*n)) {
        return Err(invalid("exact_bundle_sources_required"));
    }
    let mut files = BTreeMap::new();
    for (name, path) in sources.files {
        let file = Zeroizing::new(crate::private::read_private(&path, 48 * 1024 * 1024)?);
        files.insert(
            name,
            json!({"sha256":sha(&file),"base64":STANDARD.encode(&file)}),
        );
    }
    let mut document = json!({"version":1,"files":files});
    let output = Zeroizing::new(serde_json::to_vec(&document)?);
    if let Some(files) = document["files"].as_object_mut() {
        for v in files.values_mut() {
            if let Some(Value::String(s)) = v.get_mut("base64") {
                s.zeroize();
            }
        }
    }
    validate_bundle(&output)?;
    Ok(output)
}
pub fn import(bytes: &[u8]) -> io::Result<Value> {
    #[cfg(not(windows))]
    {
        let _ = bytes;
        Err(invalid("native_private_import_requires_windows"))
    }
    #[cfg(windows)]
    {
        use crate::{
            installation::Lease,
            private::{Directory, core_error},
        };
        let prepared = parse(bytes)?;
        rwi_windows::agent_binding::require_current_privilege(
            prepared.config.native_privilege_mode()?,
        )
        .map_err(core_error)?;
        let root =
            rwi_windows::agent_binding::fixed_agent_installation_root().map_err(core_error)?;
        if prepared.config.install_root != root || root.exists() {
            return Err(invalid(
                "fixed_installation_exists_or_root_override_no_overwrite",
            ));
        }
        let current = rwi_windows::agent_binding::current_session().map_err(core_error)?;
        prepared
            .approval
            .native_binding
            .as_ref()
            .ok_or_else(|| invalid("missing_native_binding"))?
            .require_same(&current)
            .map_err(core_error)?;
        prepared.approval.budget.validate(crate::budget::now_ms())?;
        let parent = root
            .parent()
            .ok_or_else(|| invalid("fixed_installation_parent"))?;
        if parent.exists() {
            Directory::open(parent)?;
        } else {
            Directory::create(parent)?;
        }
        let directory = Directory::create(&root)?;
        let _exclusive = Lease::acquire(&directory)?;
        let bins = Directory::create(&root.join("bin"))?;
        let trust = Directory::create(&root.join("trust"))?;
        let role = Directory::create(&trust.path().join("agent"))?;
        for (name, bytes) in &prepared.files {
            let target = if name.ends_with(".exe") {
                &bins
            } else if name.ends_with(".der") {
                &role
            } else {
                &directory
            };
            target.create_new(name, bytes, 48 * 1024 * 1024)?;
            if sha(&target.read(name, 48 * 1024 * 1024)?) != sha(bytes) {
                return Err(invalid("native_import_roundtrip_digest_mismatch"));
            }
        }
        crate::net::server_tls(&prepared.config)?;
        Ok(
            json!({"installed":true,"started":false,"services_or_global_settings_changed":false,"root":root,"task":prepared.approval.task,"approval_sha256":prepared.approval.digest(),"private_keys_emitted":false}),
        )
    }
}
/// The agent must be stopped and the previous installation journal Complete.
/// Changes the local approval document, never deletes/resets an event or HOLD.
pub fn approve_next(bytes: &[u8]) -> io::Result<Value> {
    #[cfg(not(windows))]
    {
        let _ = bytes;
        Err(invalid("next_native_approval_requires_windows"))
    }
    #[cfg(windows)]
    {
        use crate::{
            installation::Lease,
            private::{Directory, core_error},
            store::Store,
        };
        if bytes.len() > 256 * 1024 {
            return Err(invalid("next_approval_cap"));
        }
        let approval: Approval = serde_json::from_slice(bytes)?;
        approval.validate(BackendMode::Windows)?;
        approval.budget.validate(crate::budget::now_ms())?;
        let current = rwi_windows::agent_binding::current_session().map_err(core_error)?;
        approval
            .native_binding
            .as_ref()
            .ok_or_else(|| invalid("native_binding_missing"))?
            .require_same(&current)
            .map_err(core_error)?;
        let root =
            rwi_windows::agent_binding::fixed_agent_installation_root().map_err(core_error)?;
        let directory = Directory::open(&root)?;
        let _exclusive = Lease::acquire(&directory)?;
        let store = Store::open(&root.join("state/journal"))?;
        let prior = store
            .state
            .as_ref()
            .ok_or_else(|| invalid("next_approval_requires_completed_prior_task"))?;
        if !prior.complete || prior.hold || prior.task == approval.task {
            return Err(invalid(
                "unfinished_held_or_repeated_task_cannot_be_replaced",
            ));
        }
        let old = directory.read("approved-task.json", 256 * 1024)?;
        let old_approval: Approval = serde_json::from_slice(&old)?;
        if old_approval.digest() != prior.approval_sha256 {
            return Err(invalid("prior_approval_journal_mismatch"));
        }
        let archive = format!("completed-{}.json", old_approval.digest());
        directory.create_new(&archive, &old, 256 * 1024)?;
        let staged = format!("approved-{}.json", approval.digest());
        directory.create_new(&staged, bytes, 256 * 1024)?;
        // New approval is fully durable before switching the fixed pointer.
        // A failed replacement leaves startup fail-closed, not an empty journal.
        std::fs::remove_file(root.join("approved-task.json"))?;
        std::fs::rename(root.join(staged), root.join("approved-task.json"))?;
        if directory.read("approved-task.json", 256 * 1024)? != bytes {
            return Err(invalid("next_approval_roundtrip"));
        }
        Ok(
            json!({"approved_next_task":approval.task,"approval_sha256":approval.digest(),"prior_task":prior.task,"journal_preserved":true,"started":false}),
        )
    }
}
