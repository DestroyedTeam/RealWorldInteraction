//! One durable device namespace per state root. Epochs never enter replay keys.
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::Path,
};
pub(super) fn bind(root: &Path, device: &str) -> io::Result<()> {
    let path = root.join("device-namespace.json");
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(&serde_json::to_vec(
                &serde_json::json!({"version":1,"device_id":device}),
            )?)?;
            file.sync_all()?;
            #[cfg(unix)]
            fs::File::open(root)?.sync_all()?;
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            let meta = fs::symlink_metadata(&path)?;
            if !meta.is_file() || meta.len() > 1024 {
                return Err(io::Error::other("device_namespace_corrupt"));
            }
            let mut raw = Vec::new();
            fs::File::open(path)?.take(1025).read_to_end(&mut raw)?;
            let stored: serde_json::Value = serde_json::from_slice(&raw)?;
            if stored != serde_json::json!({"version":1,"device_id":device}) {
                return Err(io::Error::other(
                    "device_namespace_mismatch; preserve old reservations; do not reuse this state root for another device",
                ));
            }
        }
        Err(e) => return Err(e),
    }
    Ok(())
}
