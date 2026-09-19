//! D1 final-entry validation. Never launch a different hand-written command when
//! the configured entry is stale. This module performs no network/GUI activity.
use crate::{
    frame::ATTACH_CAP,
    invalid, sha,
    types::{ClientConfig, Role, digest_shape, identifier},
};
use serde::Deserialize;
#[cfg(unix)]
use std::path::PathBuf;
use std::{collections::BTreeMap, io, path::Path};

pub const SERVER_KEY: &str = "real_world_interaction";
pub const IMAGE_CAP: usize = 64 * 1024 * 1024;
#[cfg(not(windows))]
pub const RELAY_BASENAME: &str = "rcu-native-relay";
#[cfg(windows)]
pub const RELAY_BASENAME: &str = "rcu-native-relay.exe";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    #[serde(rename = "mcpServers")]
    servers: BTreeMap<String, EntryServer>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryServer {
    command: String,
    args: Vec<String>,
}

fn canonical_literal(path: &Path) -> io::Result<()> {
    if !path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return Err(invalid("entry_absolute_literal_path_required"));
    }
    #[cfg(unix)]
    {
        if path.canonicalize()? != path {
            return Err(invalid("entry_noncanonical_or_linked_path"));
        }
        let mut prefix = PathBuf::new();
        for part in path.components() {
            prefix.push(part);
            if std::fs::symlink_metadata(&prefix)?.file_type().is_symlink() {
                return Err(invalid("entry_symlink_component"));
            }
        }
    }
    // Windows canonical spelling / no-reparse checks are performed by the
    // native HANDLE reader; std::canonicalize's verbatim prefix is not stripped.
    Ok(())
}

#[cfg(unix)]
fn read_checked(path: &Path, cap: usize, private: bool) -> io::Result<Vec<u8>> {
    use std::{
        fs::OpenOptions,
        io::Read,
        os::unix::fs::{MetadataExt, OpenOptionsExt},
    };
    canonical_literal(path)?;
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)?;
    let before = f.metadata()?;
    // SAFETY: geteuid is a no-argument process identity query.
    let uid = unsafe { libc::geteuid() };
    if !before.is_file()
        || before.uid() != uid
        || before.len() == 0
        || before.len() > cap as u64
        || (private && (before.mode() & 0o077 != 0 || before.nlink() != 1))
        || (!private && before.mode() & 0o022 != 0)
    {
        return Err(invalid("entry_file_owner_mode_identity_or_cap"));
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    (&f).take(cap as u64 + 1).read_to_end(&mut bytes)?;
    let after = f.metadata()?;
    if bytes.len() > cap
        || bytes.len() as u64 != before.len()
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(invalid("entry_file_changed_during_read"));
    }
    Ok(bytes)
}
#[cfg(windows)]
fn read_checked(path: &Path, cap: usize, _private: bool) -> io::Result<Vec<u8>> {
    canonical_literal(path)?;
    rcu_windows::agent_binding::read_private_file_no_reparse(path, cap)
        .map_err(crate::private::core_error)
}

/// Validate the exact final entry and its local client.json. current_exe is a
/// separate check so an independent parent verifier can inspect an entry without
/// pretending that its test executable is the relay.
pub fn load(path: &Path) -> io::Result<ClientConfig> {
    canonical_literal(path)?;
    if path.file_name().and_then(|s| s.to_str()) != Some(".mcp.json") {
        return Err(invalid("entry_basename_must_be_mcp_json"));
    }
    let root = path
        .parent()
        .ok_or_else(|| invalid("entry_parent_missing"))?;
    crate::private::Directory::open(root)?;
    let raw_entry = read_checked(path, ATTACH_CAP, true)?;
    let entry: Entry = serde_json::from_slice(&raw_entry)?;
    if entry.servers.len() != 1 || !entry.servers.contains_key(SERVER_KEY) {
        return Err(invalid("entry_requires_exact_one_native_link_server"));
    }
    let raw_config = read_checked(&root.join("client.json"), ATTACH_CAP, true)?;
    let config: ClientConfig = serde_json::from_slice(&raw_config)?;
    if config.version != 1
        || config.root != root
        || config.role != Role::Model
        || !identifier(&config.task, 64)
        || !digest_shape(&config.approval_sha256)
        || !digest_shape(&config.agent_peer_sha256)
        || !digest_shape(&config.relay_image.sha256)
        || config.entry_sha256.as_deref() != Some(sha(&raw_entry).as_str())
    {
        return Err(invalid("entry_client_root_role_version_or_digest_binding"));
    }
    rcu_link_trust::server_name(&config.server_name)?;
    if config.endpoint.port() == 0
        || config.endpoint.ip().is_unspecified()
        || config.endpoint.ip().is_multicast()
        || config.endpoint.ip() == std::net::IpAddr::V4(std::net::Ipv4Addr::BROADCAST)
    {
        return Err(invalid("entry_endpoint_invalid"));
    }
    canonical_literal(&config.relay_image.path)?;
    if config.relay_image.path.file_name().and_then(|s| s.to_str()) != Some(RELAY_BASENAME) {
        return Err(invalid("entry_relay_platform_basename_mismatch"));
    }
    let relay_text = config
        .relay_image
        .path
        .to_str()
        .ok_or_else(|| invalid("entry_relay_path_utf8"))?;
    let entry_text = path.to_str().ok_or_else(|| invalid("entry_path_utf8"))?;
    let server = entry.servers.get(SERVER_KEY).unwrap();
    if server.command != relay_text || server.args != ["--entry", entry_text] {
        return Err(invalid("entry_command_or_exact_args_mismatch"));
    }
    if sha(&read_checked(&config.relay_image.path, IMAGE_CAP, false)?) != config.relay_image.sha256
    {
        return Err(invalid("entry_relay_image_sha_mismatch"));
    }
    for trust in [
        &config.trust.ca,
        &config.trust.certificate,
        &config.trust.private_key,
    ] {
        canonical_literal(trust)?;
    }
    Ok(config)
}

/// The running relay must be the very image selected by the final entry.
pub fn load_current(path: &Path) -> io::Result<ClientConfig> {
    let config = load(path)?;
    let actual = std::env::current_exe()?;
    canonical_literal(&actual)?;
    if actual != config.relay_image.path {
        return Err(invalid("entry_current_executable_not_configured_relay"));
    }
    if sha(&read_checked(&actual, IMAGE_CAP, false)?) != config.relay_image.sha256 {
        return Err(invalid("entry_current_image_sha_changed"));
    }
    Ok(config)
}
