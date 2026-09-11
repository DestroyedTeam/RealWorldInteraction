//! Host/OS-neutral device RPC contracts. No workflow, window prerequisite or MCP SDK.
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub mod api;
pub mod catalog;

pub const PROTOCOL_VERSION: u16 = 1;
/// Mutation replay keys and monitoring IDs are separate namespaces.
/// Legacy unqualified queries are accepted only when their meaning is unique.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum OperationIdKind {
    #[default]
    Auto,
    Monitor,
    Request,
}
impl OperationIdKind {
    pub fn is_auto(&self) -> bool {
        *self == Self::Auto
    }
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
    RecoverInput {
        resource_domain: String,
    },
    Describe,
    OpenContext,
    CloseContext,
    Invoke {
        tool: String,
        arguments: Value,
    },
    Operation {
        operation_id: String,
        #[serde(default, skip_serializing_if = "OperationIdKind::is_auto")]
        id_kind: OperationIdKind,
    },
    Cancel {
        operation_id: String,
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
