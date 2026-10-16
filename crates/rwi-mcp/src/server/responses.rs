//! MCP SDK response envelopes; no dispatch, input or native execution.
use rmcp::model::{CallToolResult, Content};
use rwi_core::{Result as CoreResult, fault};
use serde_json::{Value, json};

pub(super) fn with_operation(
    mut response: CallToolResult,
    status: rwi_core::operation::Status,
) -> CallToolResult {
    let metadata = json!({
        "operation_id": status.operation_id,
        "operation": status,
        "cancellation": "cooperative; native preemption unavailable"
    });
    let mut structured = match response.structured_content.take() {
        Some(Value::Object(fields))
            if !metadata
                .as_object()
                .expect("operation metadata object")
                .keys()
                .any(|key| fields.contains_key(key)) =>
        {
            fields
        }
        // Existing object-shaped SDK fields stay at their original locations.
        // Preserve non-object payloads too, rather than discarding their value.
        Some(payload) => serde_json::Map::from_iter([("result".into(), payload)]),
        None => serde_json::Map::new(),
    };
    for (key, value) in metadata.as_object().expect("operation metadata object") {
        structured.insert(key.clone(), value.clone());
    }
    response.structured_content = Some(Value::Object(structured));
    // Keep all original text/image blocks intact and support clients that only
    // expose content rather than structuredContent.
    response.content.push(Content::text(metadata.to_string()));
    response
}
pub(super) fn error(e: rwi_core::Fault) -> CallToolResult {
    let mut payload = json!({"error":e.code,"next_step":e.next_step});
    if let Some(diagnostic) = e.diagnostic {
        payload["diagnostic"] = *diagnostic;
    }
    CallToolResult::structured_error(payload)
}
pub(super) fn value<T: serde::Serialize>(v: T) -> CoreResult<CallToolResult> {
    Ok(CallToolResult::structured(
        serde_json::to_value(v)
            .map_err(|_| fault("serialization_error", "Inspect server diagnostics."))?,
    ))
}
