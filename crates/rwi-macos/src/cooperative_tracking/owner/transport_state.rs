use super::*;

pub(super) struct Task {
    pub(super) request: Request,
    pub(super) identity: Peer,
    pub(super) stream: UnixStream,
    pub(super) reply: mpsc::SyncSender<Reply>,
}
pub(super) struct History(pub(super) BTreeMap<String, (String, Reply)>);
impl History {
    pub(super) fn reserve(
        &mut self,
        nonce: &str,
        body: &Request,
    ) -> std::result::Result<(), Reply> {
        if !rwi_core::device::valid_id(nonce) {
            return Err(reject("invalid_nonce"));
        }
        let hash = rwi_core::digest(&serde_json::to_vec(body).map_err(|_| reject("encoding"))?);
        if let Some((old, reply)) = self.0.get(nonce) {
            return Err(if *old == hash {
                reply.clone()
            } else {
                reject("nonce_payload_conflict")
            });
        }
        if self.0.len() >= HISTORY_LIMIT {
            return Err(reject("history_full_no_eviction"));
        }
        self.0
            .insert(nonce.into(), (hash, reject("execution_reserved_uncertain")));
        Ok(())
    }
}
pub(super) fn cleanup_owned(
    directory: &Path,
    directory_inode: u64,
    endpoint: &Path,
    socket_inode: Option<u64>,
) {
    if let (Some(expected), Ok(meta)) = (socket_inode, std::fs::symlink_metadata(endpoint))
        && meta.file_type().is_socket()
        && meta.ino() == expected
        && meta.uid() == unsafe { libc::geteuid() }
    {
        let _ = std::fs::remove_file(endpoint);
    }
    if let Ok(meta) = std::fs::symlink_metadata(directory)
        && meta.is_dir()
        && meta.ino() == directory_inode
        && meta.uid() == unsafe { libc::geteuid() }
    {
        let _ = std::fs::remove_dir(directory); // only this original empty directory inode.
    }
}
pub(super) struct CreatedPaths {
    pub(super) directory: PathBuf,
    pub(super) directory_inode: u64,
    pub(super) endpoint: PathBuf,
    pub(super) socket_inode: Option<u64>,
    pub(super) armed: bool,
}
impl Drop for CreatedPaths {
    fn drop(&mut self) {
        if self.armed {
            cleanup_owned(
                &self.directory,
                self.directory_inode,
                &self.endpoint,
                self.socket_inode,
            );
        }
    }
}
