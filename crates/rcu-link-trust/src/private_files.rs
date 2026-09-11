//! Unix descriptor-relative private files. Windows uses the agent ACL/HANDLE
//! importer callback; unsupported platforms fail closed rather than read keys.
use crate::invalid;
#[cfg(unix)]
use std::path::PathBuf;
use std::{io, path::Path};

#[cfg(unix)]
mod unix {
    use super::*;
    use std::{
        ffi::CString,
        fs::{File, OpenOptions},
        io::{Read, Write},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{
                ffi::OsStrExt,
                fs::{MetadataExt, OpenOptionsExt},
            },
        },
    };

    pub struct Reader {
        dir: File,
        path: PathBuf,
    }

    fn check_path(path: &Path) -> io::Result<()> {
        if !path.is_absolute() || path.canonicalize()? != path {
            return Err(invalid("trust_profile_path_noncanonical"));
        }
        let mut prefix = PathBuf::new();
        for component in path.components() {
            if matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            ) {
                return Err(invalid("trust_profile_path_traversal"));
            }
            prefix.push(component);
            if std::fs::symlink_metadata(&prefix)?.file_type().is_symlink() {
                return Err(invalid("trust_profile_symlink_component"));
            }
        }
        Ok(())
    }

    fn current_uid() -> u32 {
        // SAFETY: geteuid has no pointer arguments or preconditions.
        unsafe { libc::geteuid() }
    }

    fn private_metadata(metadata: &std::fs::Metadata, directory: bool) -> io::Result<()> {
        if metadata.uid() != current_uid()
            || metadata.mode() & 0o077 != 0
            || (directory && !metadata.is_dir())
            || (!directory && (!metadata.is_file() || metadata.nlink() != 1))
        {
            return Err(invalid("trust_file_not_private_owned_regular_single_link"));
        }
        Ok(())
    }

    fn basename(path: &Path) -> io::Result<CString> {
        let name = path
            .file_name()
            .ok_or_else(|| invalid("trust_file_basename_missing"))?;
        CString::new(name.as_bytes()).map_err(|_| invalid("trust_file_basename_nul"))
    }

    impl Reader {
        pub fn open(path: &Path) -> io::Result<Self> {
            check_path(path)?;
            let dir = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(path)?;
            private_metadata(&dir.metadata()?, true)?;
            let before = std::fs::symlink_metadata(path)?;
            let opened = dir.metadata()?;
            if before.dev() != opened.dev() || before.ino() != opened.ino() {
                return Err(invalid("trust_profile_directory_changed"));
            }
            Ok(Self {
                dir,
                path: path.to_owned(),
            })
        }

        pub fn read(&self, path: &Path, cap: usize) -> io::Result<Vec<u8>> {
            if path.parent() != Some(self.path.as_path()) || cap == 0 || cap > 1024 * 1024 {
                return Err(invalid("trust_reader_path_or_cap_invalid"));
            }
            let name = basename(path)?;
            // SAFETY: retained directory fd is valid, name is a NUL-terminated
            // basename; O_NOFOLLOW disallows leaf links, O_NONBLOCK avoids FIFO
            // stalls before fstat rejects non-regular files.
            let fd = unsafe {
                libc::openat(
                    self.dir.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: openat returned a new owned fd, transferred exactly once.
            let file = unsafe { File::from_raw_fd(fd) };
            let before = file.metadata()?;
            private_metadata(&before, false)?;
            if before.len() == 0 || before.len() > cap as u64 {
                return Err(invalid("trust_file_empty_or_oversize"));
            }
            let mut bytes = zeroize::Zeroizing::new(Vec::with_capacity(before.len() as usize));
            (&file).take(cap as u64 + 1).read_to_end(&mut bytes)?;
            let after = file.metadata()?;
            private_metadata(&after, false)?;
            if bytes.len() > cap
                || bytes.len() as u64 != before.len()
                || before.len() != after.len()
                || before.mtime() != after.mtime()
                || before.mtime_nsec() != after.mtime_nsec()
                || before.ctime() != after.ctime()
                || before.ctime_nsec() != after.ctime_nsec()
            {
                return Err(invalid("trust_file_changed_during_bounded_read"));
            }
            Ok(std::mem::take(&mut *bytes))
        }

        pub fn create(path: &Path) -> io::Result<Self> {
            let parent = path
                .parent()
                .ok_or_else(|| invalid("trust_profile_parent_missing"))?;
            check_path(parent)?;
            let parent_fd = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(parent)?;
            if parent_fd.metadata()?.uid() != current_uid() {
                return Err(invalid("trust_profile_parent_not_owned"));
            }
            let name = basename(path)?;
            // SAFETY: valid parent fd and checked basename; mkdirat fails if any
            // existing entry (including a symlink) occupies the destination.
            if unsafe { libc::mkdirat(parent_fd.as_raw_fd(), name.as_ptr(), 0o700) } != 0 {
                return Err(io::Error::last_os_error());
            }
            parent_fd.sync_all()?;
            Self::open(path)
        }

        pub fn write_new(&self, name: &str, bytes: &[u8]) -> io::Result<()> {
            if name.is_empty() || name.contains(['/', '\\']) || name == "." || name == ".." {
                return Err(invalid("trust_output_basename_invalid"));
            }
            let name = CString::new(name).map_err(|_| invalid("trust_output_basename_nul"))?;
            // SAFETY: retained dir fd/name are valid. O_EXCL makes writes
            // no-overwrite; all files start private before any bytes are written.
            let fd = unsafe {
                libc::openat(
                    self.dir.as_raw_fd(),
                    name.as_ptr(),
                    libc::O_WRONLY
                        | libc::O_CREAT
                        | libc::O_EXCL
                        | libc::O_NOFOLLOW
                        | libc::O_CLOEXEC,
                    0o600,
                )
            };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: openat returned a new owned fd, transferred exactly once.
            let mut file = unsafe { File::from_raw_fd(fd) };
            private_metadata(&file.metadata()?, false)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            self.dir.sync_all()
        }
    }
}

#[cfg(unix)]
pub use unix::Reader;

#[cfg(not(unix))]
pub struct Reader;

#[cfg(not(unix))]
impl Reader {
    pub fn open(_path: &Path) -> io::Result<Self> {
        Err(invalid("trust_native_private_acl_reader_required"))
    }
    pub fn create(_path: &Path) -> io::Result<Self> {
        Err(invalid(
            "trust_private_provisioning_requires_unix_preparation_host",
        ))
    }
    pub fn read(&self, _path: &Path, _cap: usize) -> io::Result<Vec<u8>> {
        Err(invalid("trust_native_private_acl_reader_required"))
    }
    pub fn write_new(&self, _name: &str, _bytes: &[u8]) -> io::Result<()> {
        Err(invalid(
            "trust_private_provisioning_requires_unix_preparation_host",
        ))
    }
}
