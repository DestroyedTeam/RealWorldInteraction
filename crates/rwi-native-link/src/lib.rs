//! Bounded real transport and Windows-user worker supervision. No public fixture trust.
use sha2::{Digest, Sha256};
pub const TRANSPORT_VERSION: u8 = 2;
#[cfg(feature = "legacy-workflow")]
pub mod deployment;
#[cfg(feature = "legacy-workflow")]
pub mod integration;
#[cfg(feature = "legacy-workflow")]
pub mod orchestration;
#[cfg(feature = "legacy-workflow")]
pub mod protocol;
#[cfg(feature = "legacy-workflow")]
pub mod transport;
#[cfg(feature = "legacy-workflow")]
pub use integration::{codex_plugin as plugin, model_output as presentation};
// Stable public facade; modules now reflect ownership without changing wire APIs.
#[cfg(feature = "legacy-workflow")]
pub use deployment::{entry, install, installation, plan, private};
#[cfg(feature = "legacy-workflow")]
pub use orchestration::{budget, control, mcp, model, runtime, store, worker};
#[cfg(feature = "legacy-workflow")]
pub use protocol::{frame, types};
#[cfg(feature = "legacy-workflow")]
pub use transport::{client, net, server};
pub fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn invalid(reason: &str) -> std::io::Error {
    std::io::Error::other(reason.to_owned())
}
pub mod device;
#[path = "protocol/frame.rs"]
pub mod wire;
