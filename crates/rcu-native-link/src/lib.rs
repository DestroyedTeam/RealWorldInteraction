//! Bounded real transport and Windows-user worker supervision. No public fixture trust.
use sha2::{Digest, Sha256};
pub const TRANSPORT_VERSION: u8 = 2;
pub mod budget;
pub mod client;
pub mod control;
pub mod entry;
pub mod frame;
pub mod install;
pub mod installation;
pub mod integration;
pub mod mcp;
pub mod model;
pub mod net;
pub mod plan;
pub use integration::{codex_plugin as plugin, model_output as presentation};
pub mod private;
pub mod runtime;
pub mod server;
pub mod store;
pub mod types;
pub mod worker;
pub fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn invalid(reason: &str) -> std::io::Error {
    std::io::Error::other(reason.to_owned())
}
