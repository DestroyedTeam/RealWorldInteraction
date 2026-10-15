//! Host/OS-neutral device RPC contracts. No workflow, window prerequisite or MCP SDK.
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub mod api;
pub mod browser;
pub mod catalog;

pub const PROTOCOL_VERSION: u16 = 2;
pub const EXECUTION_REF_VERSION: u16 = 1;
/// Service-issued routing envelope; authority always remains in agent records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperationRef {
    pub ref_version: u16,
    pub device_id: String,
    pub agent_instance: String,
    pub context_generation: String,
    pub execution_nonce: String,
}
impl OperationRef {
    pub fn valid_shape(&self) -> bool {
        self.ref_version == EXECUTION_REF_VERSION
            && valid_id(&self.device_id)
            && valid_id(&self.agent_instance)
            && valid_id(&self.context_generation)
            && valid_id(&self.execution_nonce)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OperationSelector {
    Execution {
        operation_ref: OperationRef,
    },
    Request {
        device_id: String,
        request_id: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceInfo {
    pub device_id: String,
    pub instance: String,
    pub platform: String,
    pub availability: String,
    pub capabilities: Vec<Capability>,
    pub native_context: Option<crate::Identity>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    pub id: String,
    pub implemented: bool,
    pub available: bool,
    pub authorized: bool,
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u16,
    pub id: String,
    pub device: String,
    #[serde(default)]
    pub context: Option<String>,
    pub call: Call,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Call {
    Describe,
    /// Explicit browser-content family, independent of native contexts/references.
    BrowserSession {
        arguments: browser::SessionArgs,
    },
    BrowserSnapshot {
        arguments: browser::SnapshotArgs,
    },
    OpenContext,
    CloseContext,
    /// Metadata-only acceptance. No native calls or input-resource acquisition.
    Prepare {
        tool: String,
        arguments: Value,
    },
    Execute {
        operation_ref: OperationRef,
    },
    /// Strictly bounded cached queries, never native execution or input leases.
    Cached {
        tool: String,
        arguments: Value,
    },
    Operation {
        selector: OperationSelector,
    },
    Cancel {
        operation_ref: OperationRef,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub version: u16,
    pub id: String,
    pub device: String,
    pub instance: String,
    pub result: DeviceResult,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceResult {
    pub is_error: bool,
    pub data: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<DeviceImage>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceImage {
    pub mime: String,
    pub base64: String,
}
impl DeviceResult {
    pub fn value(data: impl Serialize) -> Self {
        match serde_json::to_value(data) {
            Ok(data) => Self {
                is_error: false,
                data,
                images: vec![],
            },
            Err(_) => Self::error("encoding_failed", "No success result could be encoded."),
        }
    }
    pub fn error(code: &str, message: &str) -> Self {
        Self {
            is_error: true,
            data: serde_json::json!({"code":code,"message":message,
            "goal_verified":false}),
            images: vec![],
        }
    }
}
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_:.".contains(&b))
}
