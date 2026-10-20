//! Headless device-side composition, independent of MCP and business workflows.
mod browser_content;
pub mod config;
mod executor;
pub mod install;
mod ledger;
mod observed_backend;
mod platform;
pub use platform::{start_device_feedback, stop_device_feedback};
pub mod runtime;
