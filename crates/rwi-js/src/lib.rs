//! Isolated JS orchestration worker. Device access exists only through the
//! supervisor's authenticated MCP admission path, never through OS bindings.
pub mod protocol;
mod worker;

pub use worker::run;
