//! Model-facing projection only. Native authority and execution data stay unchanged.
use rwi_core::device::DeviceResult;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

pub(super) mod snapshot_summary;
mod targets;
mod targets_summary;

const COMPACT_BYTES: usize = 16 * 1024;
const PAGE_BYTES: usize = 8 * 1024;
const CACHE_BYTES: usize = 64 * 1024 * 1024;
const CACHE_ENTRIES: usize = 64;
const TTL: Duration = Duration::from_secs(300);

#[derive(Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ResultArgs {
    /// Opaque reference returned by this MCP adapter; never an execution identity.
    pub result_ref: String,
    /// JSON Pointer into retained metadata, e.g. /scene/elements. Empty selects root.
    #[serde(default)]
    pub pointer: String,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Optional byte offset for bounded UTF-8 JSON export of any selected value.
    #[serde(default)]
    pub byte_offset: Option<usize>,
}
fn default_limit() -> usize {
    16
}
struct Entry {
    id: String,
    at: Instant,
    tool: String,
    data: Value,
    bytes: usize,
    sha: String,
}
#[derive(Default)]
pub(super) struct Presentation {
    entries: VecDeque<Entry>,
}
fn bytes(v: &Value) -> usize {
    serde_json::to_vec(v).expect("JSON value").len()
}
// Serialize once for both the capacity check and the exact retained-data hash.
// The temporary buffer is dropped before retaining/cloning the JSON tree.
fn metadata_fingerprint(v: &Value) -> Option<(usize, String)> {
    let encoded = serde_json::to_vec(v).expect("JSON value");
    (encoded.len() <= CACHE_BYTES).then(|| (encoded.len(), rwi_core::digest(&encoded)))
}
fn error(code: &str, message: &str) -> DeviceResult {
    DeviceResult::error(code, message)
}

/// Optional text encoding only. StructuredSummaryV1 is for callers consuming
/// complete structuredContent; omit for unchanged legacy full JSON text.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum TargetsTextProjection {
    ExactDuplicatesV1,
    StructuredSummaryV1,
}

pub(super) fn targets_text_schema(schema: &mut Value) {
    schema["properties"]["text_projection"] =
        serde_json::to_value(schemars::schema_for!(TargetsTextProjection)).expect("JSON schema");
}
pub(super) fn take_targets_text_projection(
    arguments: &mut serde_json::Map<String, Value>,
) -> Result<Option<TargetsTextProjection>, DeviceResult> {
    match arguments.remove("text_projection") {
        None => Ok(None),
        Some(value) => serde_json::from_value::<TargetsTextProjection>(value)
            .map(Some)
            .map_err(|_| {
                DeviceResult::error(
                    "invalid_arguments",
                    "text_projection must be exact_duplicates_v1 or structured_summary_v1; omit it for unchanged full text.",
                )
            }),
    }
}

pub(super) fn targets_text_request(
    tool: &str,
    arguments: &mut Option<serde_json::Map<String, Value>>,
) -> Result<Option<TargetsTextProjection>, DeviceResult> {
    if tool != "computer.targets" {
        return Ok(None);
    }
    take_targets_text_projection(arguments.get_or_insert_with(Default::default))
}

pub(super) fn apply_text_projection(
    mut reply: rmcp::model::CallToolResult,
    text: Option<Value>,
) -> rmcp::model::CallToolResult {
    if reply.is_error == Some(false)
        && let Some(mut text) = text
    {
        super::portable_numbers(&mut text);
        let encoded = serde_json::to_string(&text).expect("JSON value");
        let full = reply
            .content
            .first()
            .and_then(|c| c.as_text())
            .map(|c| c.text.as_str());
        if targets_summary::fits(full, &text, &encoded) {
            reply.content[0] = rmcp::model::Content::text(encoded);
        }
    }
    reply
}

impl Presentation {
    pub(super) fn targets_text_for(
        &self,
        result: &DeviceResult,
        mode: TargetsTextProjection,
    ) -> Option<Value> {
        match mode {
            TargetsTextProjection::ExactDuplicatesV1 => self.targets_text_projection(result),
            TargetsTextProjection::StructuredSummaryV1 => targets_summary::project(result),
        }
    }

    /// Retain nested-call metadata without projecting or treating it as native
    /// execution history. Same explicit adapter-local lifetime as observations.
    pub fn retain(&mut self, tool: &str, data: Value) -> Option<String> {
        let (size, sha) = metadata_fingerprint(&data)?;
        self.make_room(size);
        // This is an adapter-local lookup token, not a native execution ID.
        // Keep all UUID entropy while dropping separators from the MCP surface.
        let id = uuid::Uuid::new_v4().simple().to_string();
        self.entries.push_back(Entry {
            id: id.clone(),
            at: Instant::now(),
            tool: tool.into(),
            data,
            bytes: size,
            sha,
        });
        Some(id)
    }
    fn prune(&mut self) {
        self.entries.retain(|e| e.at.elapsed() < TTL);
    }
    fn make_room(&mut self, incoming_bytes: usize) {
        debug_assert!(incoming_bytes <= CACHE_BYTES);
        self.prune();
        while self.entries.len() >= CACHE_ENTRIES
            || self.entries.iter().map(|e| e.bytes).sum::<usize>() + incoming_bytes > CACHE_BYTES
        {
            self.entries.pop_front();
        }
    }

    /// Borrow the retained original rows without capture, RPC or lifetime renewal.
    #[cfg(test)]
    pub(super) fn observation_rows(
        &mut self,
        reference: &str,
        frame: &str,
    ) -> Result<&[Value], DeviceResult> {
        self.observation_data(reference, frame)?["scene"]["elements"]
            .as_array()
            .map(Vec::as_slice)
            .ok_or_else(|| {
                error(
                    "observation_shape_unknown",
                    "Retained data has no native element array.",
                )
            })
    }

    /// Same original observation/ref/frame/TTL, with no projection or renewal.
    pub(super) fn observation_data(
        &mut self,
        reference: &str,
        frame: &str,
    ) -> Result<&Value, DeviceResult> {
        self.prune();
        let entry = self.entries.iter().find(|e| e.id == reference)
            .ok_or_else(|| error("observation_expired", "Retained observation is unavailable; obtain a new snapshot, never replay input."))?;
        if entry.tool != "computer.observe" || entry.data["frame"] != frame {
            return Err(error(
                "observation_frame_mismatch",
                "Use observation_ref and frame from the same snapshot.",
            ));
        }
        if !entry.data["scene"]["elements"].is_array() {
            return Err(error(
                "observation_shape_unknown",
                "Retained data has no native element array.",
            ));
        }
        Ok(&entry.data)
    }

    pub fn project(&mut self, tool: &str, mut result: DeviceResult) -> DeviceResult {
        if result.is_error || !matches!(tool, "computer.observe" | "computer.targets") {
            return result;
        }
        // Unknown/future shapes pass through rather than being misrepresented.
        if !result.data.is_object()
            || (tool == "computer.observe" && !result.data["scene"]["elements"].is_array())
            || (tool == "computer.targets" && !result.data["targets"].is_array())
        {
            return result;
        }
        if tool == "computer.observe"
            && result.data["scene"]["elements"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| !e.is_object() || !e["id"].is_string())
        {
            return result;
        }
        let Some((original_bytes, sha)) = metadata_fingerprint(&result.data) else {
            return result;
        };
        self.make_room(original_bytes);
        let raw = std::mem::take(&mut result.data);
        // Keep exact native frame/element/target identifiers unchanged; only
        // this opaque adapter-local result handle uses the compact UUID form.
        let id = uuid::Uuid::new_v4().simple().to_string();
        let mut data = raw.clone();
        let mut omitted = Vec::new();
        if tool == "computer.targets" {
            targets::project_known_fields(&mut data, &mut omitted);
        } else {
            let details = &raw["adapter_status"]["native_details"]["uia"]["walk"]["details"];
            for element in data["scene"]["elements"].as_array_mut().unwrap() {
                if let Some(row) = details
                    .as_array()
                    .and_then(|rows| rows.iter().find(|r| r["element_id"] == element["id"]))
                {
                    for key in ["control_type", "keyboard_focus"] {
                        if let Some(value) = row.get(key) {
                            if key == "keyboard_focus" && value != &json!(true) {
                                continue;
                            }
                            element[key] = value.clone();
                        }
                    }
                }
                if let Some(object) = element.as_object_mut() {
                    object.remove("state_revision");
                }
            }
            if let Some(status) = data["adapter_status"].as_object_mut() {
                // Preserve unknown/future readiness and error fields. Remove only
                // known redundant diagnostic collections, never a broad allowlist.
                for key in [
                    "cancellation",
                    "input_contract",
                    "lifecycle_contract",
                    "lifecycle_preflight_contract",
                    "lifecycle_epoch_assurance",
                ] {
                    status.remove(key);
                }
                let grant = status.get("input_grant").cloned();
                if let Some(native) = status
                    .get_mut("native_details")
                    .and_then(Value::as_object_mut)
                {
                    native.remove("displays");
                    if grant
                        .as_ref()
                        .is_some_and(|g| native.get("input_grant") == Some(g))
                    {
                        native.remove("input_grant");
                    }
                    if let Some(walk) = native
                        .get_mut("uia")
                        .and_then(|v| v.get_mut("walk"))
                        .and_then(Value::as_object_mut)
                    {
                        walk.remove("elements");
                        walk.remove("details");
                    }
                }
            }
            omitted.extend([
                "/scene/elements/*/state_revision",
                "/adapter_status documented contract prose",
                "/adapter_status/native_details/input_grant when identical to /adapter_status/input_grant",
                "/adapter_status/native_details/displays",
                "/adapter_status/native_details/uia/walk/elements",
                "/adapter_status/native_details/uia/walk/details",
            ]);
        }
        data["presentation"] = json!({"version":"compact_v1","result_ref":id,"source_tool":tool,"metadata_sha256":sha,"original_metadata_bytes":original_bytes,"retention":"adapter-local; up to 300s, earlier eviction under pressure; lost on adapter exit","details_tool":"computer.result","omitted_sections":omitted});
        if tool == "computer.observe" {
            let total = data["scene"]["elements"].as_array().unwrap().len();
            let mut returned = total;
            loop {
                data["presentation"]["elements"] = json!({"returned":returned,"total":total,"next_offset":if returned < total {Some(returned)} else {None},"pointer":"/scene/elements","native_complete":raw["adapter_status"]["native_details"]["uia"]["walk"]["complete"]});
                if bytes(&data) <= COMPACT_BYTES || returned == 0 {
                    break;
                }
                data["scene"]["elements"].as_array_mut().unwrap().pop();
                returned -= 1;
            }
        }
        // Don't hide unexpected metadata just to satisfy a size target.
        // Detail remains retrievable even when this projection cannot meet the budget.
        if bytes(&data) > COMPACT_BYTES {
            data["presentation"]["over_budget"] = json!(true);
        }
        self.entries.push_back(Entry {
            id,
            at: Instant::now(),
            tool: tool.into(),
            data: raw,
            bytes: original_bytes,
            sha,
        });
        result.data = data;
        result
    }

    /// Explicit opt-in text-only encoding. The legacy structured result and immutable raw
    /// cache remain unchanged. Each alias resolves within this exact text response.
    pub(super) fn targets_text_projection(&self, result: &DeviceResult) -> Option<Value> {
        if result.is_error || !result.images.is_empty() {
            return None;
        }
        let data = &result.data;
        let presentation = data.get("presentation")?.as_object()?;
        if presentation.get("version")? != "compact_v1"
            || presentation.get("source_tool")? != "computer.targets"
            || presentation.get("details_tool")? != "computer.result"
            || presentation.contains_key("text_projection")
            || !data.get("targets")?.is_array()
        {
            return None;
        }
        let id = presentation.get("result_ref")?.as_str()?;
        let fingerprint = presentation.get("metadata_sha256")?.as_str()?;
        // A native/future presentation-shaped value cannot forge cache admission.
        let entry = self.entries.iter().find(|e| {
            e.id == id
                && e.tool == "computer.targets"
                && e.sha == fingerprint
                && e.at.elapsed() < TTL
        })?;
        let known = [
            "version",
            "result_ref",
            "source_tool",
            "metadata_sha256",
            "original_metadata_bytes",
            "retention",
            "details_tool",
            "omitted_sections",
            "over_budget",
        ];
        if presentation
            .keys()
            .any(|key| !known.contains(&key.as_str()))
            || presentation.get("retention")?
                != "adapter-local; up to 300s, earlier eviction under pressure; lost on adapter exit"
            || presentation
                .get("over_budget")
                .is_some_and(|value| value != true)
            || presentation.get("original_metadata_bytes")? != &json!(entry.bytes)
            || presentation.get("omitted_sections")?
                != &json!(["/devices/*/state/targets (see /targets)"])
        {
            return None;
        }
        let mut expected = entry.data.clone();
        for device in expected.get_mut("devices")?.as_array_mut()? {
            device.get_mut("state")?.as_object_mut()?.remove("targets");
        }
        expected["presentation"] = data["presentation"].clone();
        if expected != *data {
            return None;
        }
        let devices = data.get("devices")?.as_array()?;
        let raw_devices = entry.data.get("devices")?.as_array()?;
        if devices.is_empty() || devices.len() != raw_devices.len() {
            return None;
        }
        let mut duplicates = serde_json::Map::new();
        for (i, device) in devices.iter().enumerate() {
            if device.get("is_error")? != false || device.get("device_id")?.as_str()?.is_empty() {
                return None;
            }
            let state = device.get("state")?.as_object()?;
            if state.get("availability")? != "available" || state.contains_key("dispatch_error") {
                return None;
            }
            let runtime = state.get("runtime_status")?.as_object()?;
            let service = state.get("service_process_status")?.as_object()?;
            if runtime.is_empty() || runtime != service {
                return None;
            }
            let canonical = format!("/devices/{i}/state/runtime_status");
            let duplicate = format!("/devices/{i}/state/service_process_status");
            if entry.data.pointer(&canonical) != data.pointer(&canonical)
                || entry.data.pointer(&duplicate) != data.pointer(&duplicate)
            {
                return None;
            }
            duplicates.insert(duplicate, json!(canonical));
        }
        let mut text = data.clone();
        for device in text["devices"].as_array_mut()? {
            device["state"]
                .as_object_mut()?
                .remove("service_process_status");
        }
        text["presentation"]["text_projection"] = json!({
            "version":"targets_text_v1", "scope":"content.text only; structuredContent unchanged",
            "exact_duplicates":duplicates
        });
        Some(text)
    }

    pub fn read(&mut self, args: ResultArgs) -> DeviceResult {
        self.prune();
        if args.result_ref.len() > 128
            || args.pointer.len() > 1024
            || !(1..=64).contains(&args.limit)
        {
            return error(
                "invalid_arguments",
                "Use an exact result_ref, bounded JSON pointer and limit 1..64.",
            );
        }
        let Some(entry) = self.entries.iter().find(|e| e.id == args.result_ref) else {
            return error(
                "result_ref_expired",
                "Metadata is absent/evicted in this adapter. This says nothing about native execution or input effects; never replay an action to reconstruct it.",
            );
        };
        let Some(value) = entry.data.pointer(&args.pointer) else {
            return error(
                "result_pointer_unknown",
                "Select a JSON Pointer from this retained metadata result.",
            );
        };
        let mut data = json!({"result_ref":entry.id,"source_tool":entry.tool,"metadata_sha256":entry.sha,"pointer":args.pointer,"native_dispatched":false});
        if let Some(offset) = args.byte_offset {
            if args.offset != 0 {
                return error(
                    "invalid_arguments",
                    "Do not combine array offset with byte_offset.",
                );
            }
            let text = serde_json::to_string(value).expect("JSON value");
            if offset > text.len() || !text.is_char_boundary(offset) {
                return error(
                    "result_offset_invalid",
                    "byte_offset must be a UTF-8 boundary in the selected JSON encoding.",
                );
            }
            let mut end = (offset + PAGE_BYTES / 2).min(text.len());
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            data["encoding"] = json!("utf8-json");
            data["text"] = json!(&text[offset..end]);
            data["byte_offset"] = json!(offset);
            data["next_byte_offset"] = json!(if end < text.len() { Some(end) } else { None });
            data["total_bytes"] = json!(text.len());
            data["selected_sha256"] = json!(rwi_core::digest(text.as_bytes()));
            return DeviceResult::value(data);
        }
        match value {
            Value::Array(rows) => {
                if args.offset > rows.len() {
                    return error(
                        "result_offset_invalid",
                        "Offset exceeds this retained array.",
                    );
                }
                let mut page = Vec::new();
                for row in rows.iter().skip(args.offset).take(args.limit) {
                    page.push(row.clone());
                    if bytes(&json!(page)) > PAGE_BYTES {
                        page.pop();
                        break;
                    }
                }
                if page.is_empty() && args.offset < rows.len() {
                    return error(
                        "result_item_too_large",
                        "Select an item/child JSON Pointer, or use byte_offset=0 for bounded JSON export.",
                    );
                }
                let next = args.offset + page.len();
                data["offset"] = json!(args.offset);
                data["total"] = json!(rows.len());
                data["next_offset"] = json!(if next < rows.len() { Some(next) } else { None });
                data["value"] = json!(page);
            }
            Value::Object(object) if bytes(value) > PAGE_BYTES => {
                if args.offset != 0 {
                    return error("result_offset_invalid", "Offset is only valid for arrays.");
                }
                data["value_included"] = json!(false);
                let mut keys = object.keys().take(64).collect::<Vec<_>>();
                while bytes(&json!(keys)) > PAGE_BYTES {
                    keys.pop();
                }
                data["children_complete"] = json!(object.len() == keys.len());
                data["children"] = json!(keys);
                data["next"] =
                    json!("Select a child with a JSON Pointer; '~' and '/' in keys use ~0 and ~1.");
            }
            _ => {
                if args.offset != 0 {
                    return error("result_offset_invalid", "Offset is only valid for arrays.");
                }
                if bytes(value) > PAGE_BYTES {
                    return error(
                        "result_value_too_large",
                        "Use byte_offset=0 for bounded UTF-8 JSON export of this scalar.",
                    );
                }
                data["value"] = value.clone();
            }
        }
        DeviceResult::value(data)
    }
}

#[cfg(test)]
mod tests;
