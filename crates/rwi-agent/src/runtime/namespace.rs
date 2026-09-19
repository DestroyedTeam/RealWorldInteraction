//! Immutable device namespace: complete staging, atomic publication, pinned repair.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
const FINAL: &str = "device-namespace.json";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NamespaceRepair {
    pub config: PathBuf,
    pub damaged_sha256: String,
    pub committed_backup: PathBuf,
    /// Operator-supplied digest from a previously trusted, committed backup.
    pub committed_backup_sha256: String,
}
fn read(path: &Path) -> io::Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.len() > 1024 {
        return Err(io::Error::other("device_namespace_corrupt"));
    }
    let mut raw = Vec::new();
    fs::File::open(path)?.take(1025).read_to_end(&mut raw)?;
    if raw.len() > 1024 {
        return Err(io::Error::other("device_namespace_corrupt"));
    }
    Ok(raw)
}
fn binding(raw: &[u8]) -> io::Result<String> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Binding {
        version: u16,
        device_id: String,
    }
    let v:Binding=serde_json::from_slice(raw).map_err(|_|io::Error::other("device_namespace_corrupt; preserve bytes; explicit pinned committed-backup repair required"))?;
    if v.version != 1 || !rwi_core::device::valid_id(&v.device_id) {
        return Err(io::Error::other("device_namespace_corrupt"));
    }
    Ok(v.device_id)
}
fn sync_directory(root: &Path) -> io::Result<()> {
    #[cfg(unix)]
    fs::File::open(root)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = root;
    Ok(())
}
fn point(name: &str) {
    #[cfg(test)]
    if std::env::var("RCU_NAMESPACE_CRASH_POINT").as_deref() == Ok(name) {
        std::process::exit(91);
    }
    #[cfg(not(test))]
    let _ = name;
}
fn stage(root: &Path, raw: &[u8]) -> io::Result<PathBuf> {
    if raw.len() > 1024 {
        return Err(io::Error::other("namespace_size_limit"));
    }
    let attempts = fs::read_dir(root)?
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("namespace-attempt-")
        })
        .take(64)
        .count();
    if attempts >= 64 {
        return Err(io::Error::other(
            "namespace_attempt_capacity; preserve and audit previous incomplete attempts",
        ));
    }
    let path = root.join(format!(
        "namespace-attempt-{}.pending",
        uuid::Uuid::new_v4()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    point("created");
    let split = raw.len() / 2;
    file.write_all(&raw[..split])?;
    point("partial");
    file.write_all(&raw[split..])?;
    point("written");
    file.sync_all()?;
    point("file_synced");
    drop(file);
    Ok(path)
}
pub(super) fn bind(root: &Path, device: &str) -> io::Result<()> {
    let path = root.join(FINAL);
    match read(&path) {
        Ok(raw) => {
            if binding(&raw)? != device {
                return Err(io::Error::other(
                    "device_namespace_mismatch; never reuse this state root for another device",
                ));
            }
            return Ok(());
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let raw = serde_json::to_vec(&json!({"version":1,"device_id":device}))?;
    let staged = stage(root, &raw)?;
    // No incomplete bytes are ever published under the authority filename.
    // No overwrite: another complete binding can only be validated, never replaced.
    match fs::hard_link(&staged, &path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return bind(root, device),
        Err(e) => return Err(e),
    }
    point("published");
    sync_directory(root)?;
    point("directory_synced");
    fs::remove_file(staged)?;
    Ok(())
}
pub(super) fn repair(
    root: &Path,
    device: &str,
    spec: &NamespaceRepair,
    backup: &[u8],
) -> io::Result<Value> {
    if rwi_core::digest(backup) != spec.committed_backup_sha256 || binding(backup)? != device {
        return Err(io::Error::other("namespace_backup_proof_mismatch"));
    }
    let path = root.join(FINAL);
    let damaged = read(&path)?;
    if rwi_core::digest(&damaged) != spec.damaged_sha256 {
        return Err(io::Error::other("namespace_damaged_hash_mismatch"));
    }
    // A valid different binding is authoritative. Explicit repair cannot rebind it.
    if binding(&damaged).is_ok() {
        return Err(io::Error::other("namespace_valid_binding_not_repairable"));
    }
    let staged = stage(root, backup)?;
    let quarantine = root.join(format!(
        "namespace-damaged-{}.evidence",
        uuid::Uuid::new_v4()
    ));
    fs::hard_link(&path, &quarantine)?;
    sync_directory(root)?;
    if read(&quarantine)? != damaged || read(&path)? != damaged {
        return Err(io::Error::other("namespace_changed_during_repair"));
    }
    // The service-state owner lock is held by the operator entry point. Rename
    // replaces atomically, while the quarantined old inode remains evidence.
    fs::rename(staged, &path)?;
    sync_directory(root)?;
    Ok(
        json!({"device_id":device,"namespace_repaired":true,"quarantined":quarantine,"restored_sha256":spec.committed_backup_sha256,"ledger_modified":false,"input_sent":false}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn orderly_owner_unlocks_even_if_an_incidental_descriptor_is_still_open() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("owner");
        let file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        file.try_lock().unwrap();
        let incidental = file.try_clone().unwrap();
        let candidate = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        assert!(candidate.try_lock().is_err());
        drop(super::super::StateOwner(file));
        candidate.try_lock().unwrap();
        drop(incidental);
        assert!(
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .unwrap()
                .try_lock()
                .is_err(),
            "closing an old descriptor must not unlock the new owner"
        );
        candidate.unlock().unwrap();
    }
    #[test]
    fn crash_child() {
        if let Ok(root) = std::env::var("RCU_NAMESPACE_CRASH_ROOT") {
            bind(Path::new(&root), "device").unwrap();
        }
    }
    #[test]
    fn initialization_crashes_leave_only_uncommitted_evidence_or_complete_binding() {
        for p in [
            "created",
            "partial",
            "written",
            "file_synced",
            "published",
            "directory_synced",
        ] {
            let t = tempfile::tempdir().unwrap();
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "runtime::namespace::tests::crash_child"])
                .env("RCU_NAMESPACE_CRASH_ROOT", t.path())
                .env("RCU_NAMESPACE_CRASH_POINT", p)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .unwrap();
            assert_eq!(status.code(), Some(91));
            let attempts: Vec<_> = fs::read_dir(t.path())
                .unwrap()
                .flatten()
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with("namespace-attempt-")
                })
                .map(|e| {
                    let bytes = fs::read(e.path()).unwrap();
                    (e.path(), bytes)
                })
                .collect();
            bind(t.path(), "device").unwrap();
            assert_eq!(
                binding(&read(&t.path().join(FINAL)).unwrap()).unwrap(),
                "device"
            );
            assert!(bind(t.path(), "another-device").is_err());
            for (path, bytes) in attempts {
                assert_eq!(
                    fs::read(path).unwrap(),
                    bytes,
                    "incomplete evidence must not be erased"
                );
            }
        }
    }
    #[test]
    fn corrupt_legacy_authority_requires_exact_backup_and_preserves_damaged_bytes() {
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join(FINAL);
        fs::write(&path, b"{partial").unwrap();
        fs::write(
            t.path().join("old-ledger-proof"),
            b"unchanged reservation bytes",
        )
        .unwrap();
        assert!(bind(t.path(), "device").is_err());
        assert!(bind(t.path(), "other").is_err());
        let backup = serde_json::to_vec(&json!({"version":1,"device_id":"device"})).unwrap();
        let mut proof = NamespaceRepair {
            config: "unused-in-unit".into(),
            committed_backup: "trusted-backup".into(),
            committed_backup_sha256: rwi_core::digest(&backup),
            damaged_sha256: "wrong".into(),
        };
        assert!(repair(t.path(), "device", &proof, &backup).is_err());
        proof.damaged_sha256 = rwi_core::digest(b"{partial");
        assert!(repair(t.path(), "other", &proof, &backup).is_err());
        let result = repair(t.path(), "device", &proof, &backup).unwrap();
        assert_eq!(
            fs::read(result["quarantined"].as_str().unwrap()).unwrap(),
            b"{partial"
        );
        bind(t.path(), "device").unwrap();
        assert!(bind(t.path(), "other").is_err());
        assert_eq!(
            fs::read(t.path().join("old-ledger-proof")).unwrap(),
            b"unchanged reservation bytes"
        );
        assert!(repair(t.path(), "device", &proof, &backup).is_err());
    }
}
