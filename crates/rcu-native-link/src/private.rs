//! Application-local private storage. No arbitrary pathname is accepted over MCP.
use crate::{invalid, types::file_name};
#[cfg(unix)]
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
};
use std::{
    io,
    path::{Path, PathBuf},
};
pub struct Directory {
    path: PathBuf,
    #[cfg(windows)]
    inner: rcu_windows::agent_binding::PrivateDirectory,
    #[cfg(unix)]
    _handle: File,
}
pub fn core_error(e: rcu_core::Fault) -> io::Error {
    invalid(&e.to_string())
}
#[cfg(unix)]
fn no_links(path: &Path) -> io::Result<()> {
    if !path.is_absolute() {
        return Err(invalid("absolute_private_path_required"));
    }
    let mut p = PathBuf::new();
    for part in path.components() {
        p.push(part);
        if std::fs::symlink_metadata(&p)?.file_type().is_symlink() {
            return Err(invalid("private_path_symlink"));
        }
    }
    Ok(())
}
impl Directory {
    pub fn create(path: &Path) -> io::Result<Self> {
        #[cfg(windows)]
        {
            let inner =
                rcu_windows::agent_binding::PrivateDirectory::create(path).map_err(core_error)?;
            Ok(Self {
                path: path.to_owned(),
                inner,
            })
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new().mode(0o700).create(path)?;
            Self::open(path)
        }
    }
    pub fn open(path: &Path) -> io::Result<Self> {
        #[cfg(windows)]
        {
            let inner =
                rcu_windows::agent_binding::PrivateDirectory::open(path).map_err(core_error)?;
            Ok(Self {
                path: path.to_owned(),
                inner,
            })
        }
        #[cfg(unix)]
        {
            no_links(path)?;
            let f = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
                .open(path)?;
            let m = f.metadata()?;
            if m.permissions().mode() & 0o077 != 0 || m.uid() != unsafe { libc::geteuid() } {
                return Err(invalid("private_directory_owner_or_mode"));
            }
            Ok(Self {
                path: path.to_owned(),
                _handle: f,
            })
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn child(&self, name: &str) -> io::Result<PathBuf> {
        if !file_name(name) {
            return Err(invalid("single_manifest_child_required"));
        }
        Ok(self.path.join(name))
    }
    pub fn create_new(&self, name: &str, bytes: &[u8], cap: usize) -> io::Result<()> {
        if bytes.len() > cap {
            return Err(invalid("private_write_cap"));
        }
        let path = self.child(name)?;
        #[cfg(windows)]
        {
            let _ = path;
            self.inner.create_new(name, bytes, cap).map_err(core_error)
        }
        #[cfg(unix)]
        {
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(path)?;
            f.write_all(bytes)?;
            f.sync_all()?;
            self._handle.sync_all()
        }
    }
    pub fn read(&self, name: &str, cap: usize) -> io::Result<Vec<u8>> {
        let path = self.child(name)?;
        #[cfg(windows)]
        {
            let _ = path;
            self.inner.read(name, cap).map_err(core_error)
        }
        #[cfg(unix)]
        {
            let f = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(path)?;
            let m = f.metadata()?;
            if !m.is_file()
                || m.nlink() != 1
                || m.uid() != unsafe { libc::geteuid() }
                || m.permissions().mode() & 0o077 != 0
                || m.len() > cap as u64
            {
                return Err(invalid("private_file_owner_link_mode_or_cap"));
            }
            let mut b = Vec::new();
            f.take(cap as u64 + 1).read_to_end(&mut b)?;
            if b.len() > cap {
                return Err(invalid("private_file_cap"));
            }
            Ok(b)
        }
    }
}
pub fn read_private(path: &Path, cap: usize) -> io::Result<Vec<u8>> {
    #[cfg(windows)]
    {
        rcu_windows::agent_binding::read_private_file_no_reparse(path, cap).map_err(core_error)
    }
    #[cfg(unix)]
    {
        let parent = path.parent().ok_or_else(|| invalid("private_parent"))?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| invalid("private_filename"))?;
        Directory::open(parent)?.read(name, cap)
    }
}
/// One private content-addressed evidence store, bounded across all retained
/// tasks. Exhaustion is an explicit failure/HOLD, never evidence deletion.
pub fn content(directory: &Directory, name: &str, bytes: &[u8], cap: usize) -> io::Result<()> {
    if directory.child(name)?.exists() {
        if directory.read(name, cap)? != bytes {
            return Err(invalid("content_address_collision"));
        }
        return Ok(());
    }
    let mut size = bytes.len() as u64;
    let mut count = 0;
    for entry in std::fs::read_dir(directory.path())? {
        let entry = entry?;
        let metadata = std::fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            return Err(invalid("evidence_store_symlink"));
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if metadata.file_attributes() & 0x400 != 0 {
                return Err(invalid("evidence_store_reparse_point"));
            }
        }
        if !metadata.is_file() {
            return Err(invalid("unexpected_evidence_store_entry"));
        }
        count += 1;
        size = size.saturating_add(metadata.len());
        if count >= 4096 || size > 512 * 1024 * 1024 {
            return Err(invalid("private_evidence_budget_exhausted_hold"));
        }
    }
    directory.create_new(name, bytes, cap)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_new_files_do_not_overwrite_or_traverse() {
        let t = tempfile::tempdir().unwrap();
        let d = Directory::create(&t.path().canonicalize().unwrap().join("private")).unwrap();
        d.create_new("only.txt", b"abc", 3).unwrap();
        assert_eq!(d.read("only.txt", 3).unwrap(), b"abc");
        assert!(d.read("only.txt", 2).is_err());
        assert!(d.create_new("only.txt", b"bad", 3).is_err());
        for n in ["../x", "/tmp/x", "a/b", "a:b", ".", ".."] {
            assert!(d.create_new(n, b"x", 1).is_err());
        }
    }
}
