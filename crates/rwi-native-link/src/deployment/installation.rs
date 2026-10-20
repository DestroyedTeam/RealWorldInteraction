//! OS-owned cooperative installation lease, not a PID or marker-presence lock.
#[cfg(unix)]
use crate::invalid;
use crate::private::Directory;
use std::io;
pub struct Lease {
    #[cfg(unix)]
    _file: std::fs::File,
    #[cfg(windows)]
    _native: rwi_windows::agent_binding::InstallationLock,
}
impl Lease {
    pub fn acquire(directory: &Directory) -> io::Result<Self> {
        #[cfg(windows)]
        {
            let dir = rwi_windows::agent_binding::PrivateDirectory::open(directory.path())
                .map_err(crate::private::core_error)?;
            let native = rwi_windows::agent_binding::acquire_installation_lock(&dir)
                .map_err(crate::private::core_error)?;
            Ok(Self { _native: native })
        }
        #[cfg(unix)]
        {
            use std::os::{fd::AsRawFd, unix::fs::OpenOptionsExt};
            let name = "agent.lock";
            if !directory.path().join(name).exists() {
                match directory.create_new(name, b"os-owned-lock-not-a-state-reset\n", 128) {
                    Ok(()) => {}
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(e),
                }
            }
            directory.read(name, 128)?;
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
                .open(directory.child(name)?)?;
            // SAFETY: file retains this descriptor; LOCK_NB forbids blocking.
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                return Err(invalid("installation_already_owned_or_lock_failed"));
            }
            Ok(Self { _file: file })
        }
    }
}
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn same_installation_second_owner_refused_marker_survives_normal_release() {
        let temp = tempfile::tempdir().unwrap();
        let dir =
            Directory::create(&temp.path().canonicalize().unwrap().join("installation")).unwrap();
        let a = Lease::acquire(&dir).unwrap();
        assert!(Lease::acquire(&dir).is_err());
        drop(a);
        assert!(dir.path().join("agent.lock").exists());
        let _b = Lease::acquire(&dir).unwrap();
    }
}
