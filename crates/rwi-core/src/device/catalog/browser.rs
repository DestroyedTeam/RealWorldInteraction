//! Browser-content tool descriptions remain separate from native tool contracts.
use super::ToolDefinition;

pub(super) fn definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "browser.snapshot",
            description: "Read bounded main-document content and original viewport PNG from an exact owned browser.session. Requires separate explicit snapshot grant, not Native TCC or lifecycle-only permission. Returns AX nodes, CSS-document rectangles, document identity and honest partial/nonatomic coverage. No activation, user browser attach, JS, native frame or input authority; not browser chrome/OS dialogs or hover/click. A screenshot is not proof an action succeeded.",
            input_schema: serde_json::to_value(schemars::schema_for!(
                crate::device::browser::SnapshotArgs
            ))
            .unwrap(),
            writes: false,
        },
        ToolDefinition {
            name: "browser.session",
            description: "Explicit owned browser-content lifecycle on an authorized macOS Agent: create a NEW isolated Chrome/profile, close one exact session, or query a durable request. Separate browser policy; no existing profile/port attach, native window/TCC binding, automatic activation, fallback or replay. request_id is stable; query uses the ORIGINAL request_id. Content reads use browser.snapshot with its own grant; lifecycle does not authorize hover/click, browser toolbar or OS dialogs. Background creation is requested, not OS focus proof. Sessions expire after 600s; close invalidates browser references even when cleanup is uncertain.",
            input_schema: serde_json::to_value(schemars::schema_for!(
                crate::device::browser::SessionArgs
            ))
            .unwrap(),
            writes: true,
        },
    ]
}
