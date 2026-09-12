//! Model-facing projection only. Native authority and execution data stay unchanged.
use rcu_core::device::DeviceResult;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

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
fn error(code: &str, message: &str) -> DeviceResult {
    DeviceResult::error(code, message)
}

impl Presentation {
    fn prune(&mut self) {
        self.entries.retain(|e| e.at.elapsed() < TTL);
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
        let raw = result.data.clone();
        let original_bytes = bytes(&raw);
        if original_bytes > CACHE_BYTES {
            return result;
        }
        self.prune();
        while self.entries.len() >= CACHE_ENTRIES
            || self.entries.iter().map(|e| e.bytes).sum::<usize>() + original_bytes > CACHE_BYTES
        {
            self.entries.pop_front();
        }
        let id = uuid::Uuid::new_v4().to_string();
        let sha = rcu_core::digest(&serde_json::to_vec(&raw).expect("JSON value"));
        let mut data = raw.clone();
        let mut omitted = Vec::new();
        if tool == "computer.targets" {
            if let Some(devices) = data["devices"].as_array_mut() {
                for device in devices {
                    if let Some(state) = device["state"].as_object_mut() {
                        // Top-level targets remain authoritative routing handles.
                        state.remove("targets");
                    }
                }
                omitted.push("/devices/*/state/targets (see /targets)");
            }
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
            data["selected_sha256"] = json!(rcu_core::digest(text.as_bytes()));
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
mod tests {
    use super::*;
    fn observation() -> Value {
        let rows: Vec<_> = (0..128).map(|i| json!({"id":format!("element-{i}"),"name":format!("control-{i}"),"state_revision":"x".repeat(64),"visible":true,"occluded":false,"point":[i,12]})).collect();
        json!({"frame":"frame","artifact_id":"artifact","operation_ref":{"execution_nonce":"exact"},"input_domain_fenced":false,"operation":{"native_call_pending":false,"resources_released":true},"scene":{"elements":rows,"blocked":false,"mutation_ready":true},"adapter_status":{"input":true,"native_details":{"focused_element_ids":["element-1"],"uia":{"status":"available","walk":{"complete":false,"truncation":["budget"],"elements":rows,"details":[{"element_id":"element-1","keyboard_focus":true,"control_type":"Edit","debug":"x".repeat(60000)}]}}}}})
    }
    fn args(id: &str, pointer: &str, offset: usize) -> ResultArgs {
        ResultArgs {
            result_ref: id.into(),
            pointer: pointer.into(),
            offset,
            limit: 16,
            byte_offset: None,
        }
    }
    #[test]
    fn compact_observation_retains_authority_focus_and_partial_coverage() {
        let raw = observation();
        let mut cache = Presentation::default();
        let p = cache
            .project("computer.observe", DeviceResult::value(&raw))
            .data;
        assert!(bytes(&p) <= COMPACT_BYTES);
        for key in [
            "frame",
            "artifact_id",
            "operation_ref",
            "input_domain_fenced",
            "operation",
        ] {
            assert_eq!(p[key], raw[key]);
        }
        assert_eq!(p["scene"]["elements"][1]["keyboard_focus"], true);
        assert_eq!(p["scene"]["elements"][1]["control_type"], "Edit");
        assert_eq!(
            p["adapter_status"]["native_details"]["uia"]["walk"]["complete"],
            false
        );
        assert_eq!(p["presentation"]["elements"]["native_complete"], false);
        let id = p["presentation"]["result_ref"].as_str().unwrap();
        let page = cache.read(args(id, "/scene/elements", 0));
        assert_eq!(page.data["value"][0], raw["scene"]["elements"][0]);
        assert_eq!(page.data["next_offset"], 16);
        assert_eq!(
            cache.read(args(id, "/operation_ref", 0)).data["value"],
            raw["operation_ref"]
        );
    }
    #[test]
    fn failed_mutation_is_never_projected_or_loses_locator() {
        let mut cache = Presentation::default();
        let mut r = DeviceResult::error("effect_unknown", "Do not replay");
        r.data["operation_ref"] = json!({"execution_nonce":"exact"});
        let expected = r.data.clone();
        let r = cache.project("computer.act_checked", r);
        assert!(r.is_error);
        assert_eq!(r.data, expected);
        assert!(cache.entries.is_empty());
    }
    #[test]
    fn expired_or_other_adapter_result_never_resolves_or_dispatches() {
        let mut cache = Presentation::default();
        let p = cache.project("computer.observe", DeviceResult::value(observation()));
        let id = p.data["presentation"]["result_ref"].as_str().unwrap();
        assert_eq!(
            Presentation::default().read(args(id, "", 0)).data["code"],
            "result_ref_expired"
        );
        cache.entries[0].at -= TTL;
        assert_eq!(
            cache.read(args(id, "", 0)).data["code"],
            "result_ref_expired"
        );
    }
    #[test]
    fn metadata_pages_bounds_and_json_pointer_are_explicit() {
        let mut raw = observation();
        raw["a/b"] = json!({"~key":"kept"});
        let mut cache = Presentation::default();
        let p = cache.project("computer.observe", DeviceResult::value(raw));
        let id = p.data["presentation"]["result_ref"].as_str().unwrap();
        assert_eq!(cache.read(args(id, "/a~1b/~0key", 0)).data["value"], "kept");
        assert_eq!(
            cache.read(args(id, "/scene/elements", 129)).data["code"],
            "result_offset_invalid"
        );
        assert_eq!(cache.read(args(id, "", 0)).data["value_included"], false);
        assert_eq!(
            cache.read(args(id, "/missing", 0)).data["code"],
            "result_pointer_unknown"
        );
    }
    #[test]
    fn utf8_export_reconstructs_large_original_metadata_without_native_calls() {
        let mut raw = observation();
        raw["long"] = json!("界\\\"".repeat(5000));
        let mut cache = Presentation::default();
        let p = cache.project("computer.observe", DeviceResult::value(&raw));
        let id = p.data["presentation"]["result_ref"].as_str().unwrap();
        let mut joined = String::new();
        let mut offset = 0;
        loop {
            let mut a = args(id, "", 0);
            a.byte_offset = Some(offset);
            let r = cache.read(a);
            assert!(!r.is_error);
            assert_eq!(r.data["native_dispatched"], false);
            joined.push_str(r.data["text"].as_str().unwrap());
            let Some(next) = r.data["next_byte_offset"].as_u64() else {
                break;
            };
            assert!(next as usize > offset);
            offset = next as usize;
        }
        assert_eq!(serde_json::from_str::<Value>(&joined).unwrap(), raw);
    }
    #[test]
    fn count_eviction_and_unknown_future_readiness_are_not_hidden() {
        let mut cache = Presentation::default();
        let mut raw = observation();
        raw["adapter_status"]["future_block_reason"] = json!("unavailable");
        let p = cache.project("computer.observe", DeviceResult::value(&raw));
        assert_eq!(
            p.data["adapter_status"]["future_block_reason"],
            "unavailable"
        );
        let first = p.data["presentation"]["result_ref"]
            .as_str()
            .unwrap()
            .to_owned();
        for _ in 0..CACHE_ENTRIES {
            cache.project(
                "computer.targets",
                DeviceResult::value(json!({"targets":[],"devices":[]})),
            );
        }
        assert_eq!(cache.entries.len(), CACHE_ENTRIES);
        assert_eq!(
            cache.read(args(&first, "", 0)).data["code"],
            "result_ref_expired"
        );
    }
    #[test]
    fn target_deduplication_preserves_devices_and_offline_errors() {
        let mut cache = Presentation::default();
        let target = json!({"target":"exact","capabilities":{"input":false}});
        let p=cache.project("computer.targets",DeviceResult::value(json!({"targets":[target],"devices":[{"device_id":"one","state":{"targets":[target],"availability":"available"}},{"device_id":"two","is_error":true,"state":{"code":"offline"}}]})));
        assert_eq!(p.data["targets"][0], target);
        assert_eq!(p.data["devices"][1]["state"]["code"], "offline");
        assert!(p.data["devices"][0]["state"].get("targets").is_none());
    }
    #[test]
    fn unknown_observation_shape_is_not_rewritten_or_panicked() {
        let raw = json!({"scene":{"elements":[null]}});
        let result = Presentation::default().project("computer.observe", DeviceResult::value(&raw));
        assert_eq!(result.data, raw);
    }
    #[test]
    #[ignore = "explicit saved metadata file; offline projection, not native GUI acceptance"]
    fn saved_metadata_projection_is_losslessly_retrievable() {
        let path =
            std::env::var_os("RCU_OBSERVATION_REPLAY").expect("explicit saved JSON metadata path");
        let mut raw: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        super::super::portable_numbers(&mut raw);
        let mut cache = Presentation::default();
        let result = cache.project("computer.observe", DeviceResult::value(&raw));
        let id = result.data["presentation"]["result_ref"].as_str().unwrap();
        let mut output = String::new();
        let mut offset = 0;
        let mut pages = 0;
        loop {
            let mut a = args(id, "", 0);
            a.byte_offset = Some(offset);
            let page = cache.read(a);
            assert!(!page.is_error, "{}", page.data);
            output.push_str(page.data["text"].as_str().unwrap());
            pages += 1;
            let Some(next) = page.data["next_byte_offset"].as_u64() else {
                break;
            };
            offset = next as usize;
        }
        assert_eq!(serde_json::from_str::<Value>(&output).unwrap(), raw);
        for key in [
            "frame",
            "artifact_id",
            "operation_ref",
            "operation",
            "input_domain_fenced",
        ] {
            assert_eq!(result.data[key], raw[key]);
        }
        println!(
            "projection_audit={}",
            json!({"before_bytes":bytes(&raw),"after_bytes":bytes(&result.data),"raw_elements":raw["scene"]["elements"].as_array().unwrap().len(),"returned_elements":result.data["scene"]["elements"].as_array().unwrap().len(),"full_metadata_reconstructed":true,"export_pages":pages,"native_calls":0})
        );
    }
}
