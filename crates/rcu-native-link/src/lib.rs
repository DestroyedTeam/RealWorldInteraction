//! Bounded real transport and Windows-user worker supervision. No public fixture trust.
use sha2::{Digest, Sha256};
pub const TRANSPORT_VERSION: u8 = 2;
pub mod deployment;
pub mod integration;
pub mod orchestration;
pub mod protocol;
pub mod transport;
pub use integration::{codex_plugin as plugin, model_output as presentation};
// Stable public facade; modules now reflect ownership without changing wire APIs.
pub use deployment::{entry, install, installation, plan, private};
pub use orchestration::{budget, control, mcp, model, runtime, store, worker};
pub use protocol::{frame, types};
pub use transport::{client, net, server};
pub fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn invalid(reason: &str) -> std::io::Error {
    std::io::Error::other(reason.to_owned())
}
