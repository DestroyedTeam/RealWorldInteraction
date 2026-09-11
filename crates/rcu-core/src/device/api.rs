//! Shared typed capability arguments, used by adapter schema and device execution.
use crate::{Intent, Locator, Predicate};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecoverInputArgs {
    pub device_id: String,
    pub resource_domain: String,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObserveArgs {
    pub target: String,
    #[serde(default = "yes")]
    pub inline_image: bool,
    pub operation_id: Option<String>,
    pub region: Option<[u32; 4]>,
}
fn yes() -> bool {
    true
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolveArgs {
    pub target: String,
    pub frame: String,
    pub locator: Locator,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClaimArgs {
    pub target: String,
    #[serde(default = "ttl")]
    pub ttl_ms: u64,
}
fn ttl() -> u64 {
    30000
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseArgs {
    #[serde(default)]
    pub device_id: Option<String>,
    pub lease: String,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActArgs {
    pub target: String,
    pub reference: String,
    pub lease: String,
    pub request_id: String,
    pub intent: Intent,
    pub value: Option<String>,
    pub operation_id: Option<String>,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckedActArgs {
    pub target: String,
    pub frame: String,
    pub element: String,
    pub lease: String,
    pub request_id: String,
    pub intent: Intent,
    pub value: Option<String>,
    pub operation_id: Option<String>,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LaunchArgs {
    pub target: String,
    pub lease: String,
    pub authorization_id: String,
    pub request_id: String,
    #[serde(default = "timeout")]
    pub timeout_ms: u64,
    pub operation_id: Option<String>,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LifecyclePreflightArgs {
    pub target: String,
    pub authorization_id: String,
    pub operation_id: Option<String>,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActivateArgs {
    pub target: String,
    pub lease: String,
    pub authorization_id: String,
    pub request_id: String,
    pub expected_foreground: Option<crate::lifecycle::WindowIdentity>,
    pub operation_id: Option<String>,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LifecycleReceiptArgs {
    pub target: String,
    pub kind: String,
    pub authorization_id: String,
    pub request_id: String,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VerifyArgs {
    pub target: String,
    pub frame: String,
    pub element: Option<String>,
    pub request_id: Option<String>,
    pub predicate: Predicate,
    pub expected: String,
    #[serde(default = "timeout")]
    pub timeout_ms: u64,
    #[serde(default = "samples")]
    pub stable_samples: u32,
    pub operation_id: Option<String>,
}
fn timeout() -> u64 {
    1000
}
fn samples() -> u32 {
    2
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtifactArgs {
    #[serde(default)]
    pub device_id: Option<String>,
    pub artifact_id: String,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "chunk")]
    pub length: usize,
    #[serde(default)]
    pub as_image: bool,
}
fn chunk() -> usize {
    1024 * 1024
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TargetArgs {
    pub target: String,
    pub operation_id: Option<String>,
    pub cursor: Option<String>,
    pub pid: Option<u32>,
    pub title_contains: Option<String>,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BindArgs {
    pub target: String,
    pub selection: crate::WindowSelection,
    pub operation_id: Option<String>,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperationArgs {
    #[serde(default)]
    pub device_id: Option<String>,
    pub operation_id: String,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperationQueryArgs {
    #[serde(default)]
    pub device_id: Option<String>,
    pub operation_id: String,
    /// auto refuses cross-namespace ambiguity; request looks up a mutation request_id.
    #[serde(default)]
    pub id_kind: super::OperationIdKind,
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImageArgs {
    pub target: String,
    pub frame: String,
    pub artifact_id: String,
    pub crop: Option<[u32; 4]>,
    #[serde(default = "edge")]
    pub max_edge: u32,
    pub operation_id: Option<String>,
}
fn edge() -> u32 {
    1200
}
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolveViewArgs {
    pub target: String,
    pub view_id: String,
    pub x: f64,
    pub y: f64,
}
