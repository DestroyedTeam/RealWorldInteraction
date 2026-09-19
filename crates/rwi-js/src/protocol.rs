//! Private parent/owned-worker framing, not a device transport or public MCP API.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CODE_CAP: usize = 64 * 1024;
pub const FRAME_CAP: usize = 32 * 1024 * 1024;
pub const CALL_CAP: usize = 64;
pub const OUTPUT_CAP: usize = 64 * 1024;
pub const MEMORY_CAP: usize = 64 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    pub code: String,
    pub timeout_ms: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Call { tool: String, arguments: Value },
    Emit { value: Value },
    Image { data: String, mime_type: String },
    Done { error: Option<String> },
}
