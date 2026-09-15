//! The existing nineteen-tool wire schemas, unchanged by module extraction.
use rcu_core::{Intent, Locator, Predicate};
// New device-gateway intents must not alter this legacy fixture handshake.
#[derive(serde::Deserialize, schemars::JsonSchema)]
#[schemars(rename = "Intent")]
#[serde(rename_all = "snake_case")]
enum LegacyIntent {
    Click,
    DoubleClick,
    ContextMenu,
    Text,
    KeyChord,
    Scroll,
}
fn legacy_intent<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Intent, D::Error> {
    Ok(
        match <LegacyIntent as serde::Deserialize>::deserialize(d)? {
            LegacyIntent::Click => Intent::Click,
            LegacyIntent::DoubleClick => Intent::DoubleClick,
            LegacyIntent::ContextMenu => Intent::ContextMenu,
            LegacyIntent::Text => Intent::Text,
            LegacyIntent::KeyChord => Intent::KeyChord,
            LegacyIntent::Scroll => Intent::Scroll,
        },
    )
}
use schemars::JsonSchema;
use serde::Deserialize;
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ObserveArgs {
    pub(super) target: String,
    #[serde(default = "yes")]
    pub(super) inline_image: bool,
    pub(super) operation_id: Option<String>,
    pub(super) region: Option<[u32; 4]>,
}
fn yes() -> bool {
    true
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ResolveArgs {
    pub(super) target: String,
    pub(super) frame: String,
    pub(super) locator: Locator,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ClaimArgs {
    pub(super) target: String,
    #[serde(default = "ttl")]
    pub(super) ttl_ms: u64,
}
fn ttl() -> u64 {
    30000
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ReleaseArgs {
    pub(super) lease: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ActArgs {
    pub(super) target: String,
    pub(super) reference: String,
    pub(super) lease: String,
    pub(super) request_id: String,
    #[schemars(with = "LegacyIntent")]
    #[serde(deserialize_with = "legacy_intent")]
    pub(super) intent: Intent,
    pub(super) value: Option<String>,
    pub(super) operation_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct MoveWindowArgs {
    pub(super) target: String,
    pub(super) lease: String,
    pub(super) reference: String,
    pub(super) display: rcu_core::DisplayTarget,
    pub(super) operation_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct CheckedActArgs {
    pub(super) target: String,
    pub(super) frame: String,
    pub(super) element: String,
    pub(super) lease: String,
    pub(super) request_id: String,
    #[schemars(with = "LegacyIntent")]
    #[serde(deserialize_with = "legacy_intent")]
    pub(super) intent: Intent,
    pub(super) value: Option<String>,
    pub(super) operation_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct LaunchArgs {
    pub(super) target: String,
    pub(super) lease: String,
    pub(super) authorization_id: String,
    pub(super) request_id: String,
    #[serde(default = "timeout")]
    pub(super) timeout_ms: u64,
    pub(super) operation_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct LifecyclePreflightArgs {
    pub(super) target: String,
    pub(super) authorization_id: String,
    pub(super) operation_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ActivateArgs {
    pub(super) target: String,
    pub(super) lease: String,
    pub(super) authorization_id: String,
    pub(super) request_id: String,
    pub(super) expected_foreground: Option<rcu_core::lifecycle::WindowIdentity>,
    pub(super) operation_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct LifecycleReceiptArgs {
    pub(super) target: String,
    pub(super) kind: String,
    pub(super) authorization_id: String,
    pub(super) request_id: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct VerifyArgs {
    pub(super) target: String,
    pub(super) frame: String,
    pub(super) element: Option<String>,
    pub(super) request_id: Option<String>,
    pub(super) predicate: Predicate,
    pub(super) expected: String,
    #[serde(default = "timeout")]
    pub(super) timeout_ms: u64,
    #[serde(default = "samples")]
    pub(super) stable_samples: u32,
    pub(super) operation_id: Option<String>,
}
fn timeout() -> u64 {
    1000
}
fn samples() -> u32 {
    2
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ArtifactArgs {
    pub(super) artifact_id: String,
    #[serde(default)]
    pub(super) offset: usize,
    #[serde(default = "chunk")]
    pub(super) length: usize,
    #[serde(default)]
    pub(super) as_image: bool,
}
fn chunk() -> usize {
    1024 * 1024
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct TargetArgs {
    pub(super) target: String,
    pub(super) operation_id: Option<String>,
    pub(super) cursor: Option<String>,
    pub(super) pid: Option<u32>,
    pub(super) title_contains: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct BindArgs {
    pub(super) target: String,
    pub(super) selection: rcu_core::WindowSelection,
    pub(super) operation_id: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct OperationArgs {
    pub(super) operation_id: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ImageArgs {
    pub(super) target: String,
    pub(super) frame: String,
    pub(super) artifact_id: String,
    pub(super) crop: Option<[u32; 4]>,
    #[serde(default = "edge")]
    pub(super) max_edge: u32,
    pub(super) operation_id: Option<String>,
}
fn edge() -> u32 {
    1200
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ResolveViewArgs {
    pub(super) target: String,
    pub(super) view_id: String,
    pub(super) x: f64,
    pub(super) y: f64,
}
