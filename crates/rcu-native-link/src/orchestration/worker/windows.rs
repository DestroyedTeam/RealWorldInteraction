//! Retain the exact child process and prove its image/token before pipe binding.
use crate::{invalid, private::core_error, types::AgentConfig};
use rcu_windows::agent_binding::OwnedProcess;
use std::{io, sync::Arc};

pub(super) fn retain_worker(
    child: &tokio::process::Child,
    config: &AgentConfig,
) -> io::Result<Arc<OwnedProcess>> {
    let raw = child
        .raw_handle()
        .ok_or_else(|| invalid("owned_worker_handle_missing"))?;
    // The caller retains the Child lock during handle duplication and proof.
    let retained =
        unsafe { rcu_windows::agent_binding::retain_owned_process(raw, &config.worker_image.path) }
            .map_err(core_error)?;
    if retained.record().image_sha256 != config.worker_image.sha256 {
        return Err(invalid("spawned_worker_image_digest_mismatch"));
    }
    retained
        .verify_privilege(config.native_privilege_mode()?)
        .map_err(core_error)?;
    Ok(Arc::new(retained))
}
