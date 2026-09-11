//! AI-host boundary: native plugin packaging and model-facing output views.
//!
//! Does not own native dispatch, grants, task orchestration or authoritative
//! snapshots. Transport invokes presentation only after validating raw replies.
pub mod codex_plugin;
pub mod model_output;
