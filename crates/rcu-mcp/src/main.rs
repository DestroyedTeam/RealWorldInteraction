//! Thin MCP executable; protocol adapter and process bootstrap are separate.
#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(feature = "native-agent-worker")]
mod agent_worker;
mod bootstrap;
mod eof_diagnostics;
#[cfg(feature = "link-fixture")]
mod link_fixture;
mod server;

fn main() {
    bootstrap::run();
}
