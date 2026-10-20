//! Explicit structured-aware text encoding only; no Native calls or reference aliases.
use rwi_core::device::DeviceResult;
use serde_json::{Value, json};

const MODE: &str = "structured_summary_v1";
const MARKER: &str = "_text_projection";
const SECTIONS: [&str; 4] = ["elements", "mapping", "coverage", "consistency"];

pub(in crate::gateway) fn advertise(tools: &mut [rmcp::model::Tool]) {
    let Some(tool) = tools.iter_mut().find(|t| t.name == "computer.snapshot") else {
        return;
    };
    if let Some(properties) = std::sync::Arc::make_mut(&mut tool.input_schema)
        .get_mut("properties")
        .and_then(Value::as_object_mut)
    {
        properties.insert("text_projection".into(), json!({
            "type":"string","enum":[MODE],
            "description":"Opt-in for clients reading structuredContent: concise text references its complete controls/mapping/coverage/consistency. Images and structuredContent are unchanged. Omit for full legacy text."
        }));
    }
}

/// Adapter-only field is consumed before convenience decoding or device routing.
pub(in crate::gateway) fn request(
    tool: &str,
    arguments: &mut Option<serde_json::Map<String, Value>>,
) -> Result<bool, DeviceResult> {
    if tool != "computer.snapshot" {
        return Ok(false);
    }
    match arguments.as_mut().and_then(|a| a.remove("text_projection")) {
        None => Ok(false),
        Some(Value::String(mode)) if mode == MODE => Ok(true),
        Some(_) => Err(DeviceResult::error(
            "invalid_arguments",
            "Snapshot text_projection must be structured_summary_v1; omit for full legacy text.",
        )),
    }
}

/// Only a normal, positively grounded image snapshot can use this projection.
/// Errors, uncertainty, fences and degraded/unknown shapes retain full text.
pub(in crate::gateway) fn project(requested: bool, result: &DeviceResult) -> Option<Value> {
    if !requested
        || result.is_error
        || result.images.len() != 1
        || result.images[0].mime != "image/png"
        || result.images[0].base64.is_empty()
    {
        return None;
    }
    let data = result.data.as_object()?;
    if data.contains_key(MARKER)
        || [
            "code",
            "message",
            "diagnostic",
            "dispatch",
            "dispatch_error",
            "receipt",
            "operation",
        ]
        .iter()
        .any(|k| data.contains_key(*k))
        || data.get("input_domain_fenced")? != false
        || data.get("blocked")? != false
        || data.get("visual_only")? != false
        || data.get("visual_available")? != true
        || data.get("mutation_ready")? != true
        || data.get("input_reference_available")? != true
    {
        return None;
    }
    for key in [
        "target",
        "frame",
        "view_id",
        "observation_ref",
        "details_ref",
    ] {
        if data.get(key)?.as_str()?.is_empty() {
            return None;
        }
    }
    let window = data.get("window")?.as_object()?;
    if window.get("pid")?.as_u64()? == 0 || window.get("window")?.as_str()?.is_empty() {
        return None;
    }
    let controls = data.get("elements")?.as_array()?;
    if data.get("available_controls")?.as_u64()? < controls.len() as u64
        || controls.iter().any(|e| {
            e.get("id")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
        })
    {
        return None;
    }
    for key in ["mapping", "coverage", "consistency"] {
        data.get(key)?.as_object()?;
    }
    let mut text = result.data.clone();
    let object = text.as_object_mut()?;
    for key in SECTIONS {
        object.remove(key);
    }
    object.insert(MARKER.into(), json!({
        "version":"snapshot_summary_v1",
        "scope":"content.text only; structuredContent and images unchanged",
        "structured_paths":{"elements":"/elements","mapping":"/mapping","coverage":"/coverage","consistency":"/consistency"},
        "returned_controls":controls.len()
    }));
    // Never expand a tiny reply. Unknown root fields survive; nested fields in
    // the four referenced sections remain byte-equivalent in structuredContent.
    (serde_json::to_vec(&text).ok()?.len() < serde_json::to_vec(&result.data).ok()?.len())
        .then_some(text)
}

#[cfg(test)]
mod tests;
