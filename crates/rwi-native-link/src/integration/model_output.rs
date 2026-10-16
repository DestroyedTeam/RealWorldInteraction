//! Model-facing projection only. The agent validates/audits the original RPC;
//! this relay persists exact received bytes before emitting a compact view.
use crate::{frame, private, sha};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, io, path::Path};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    #[default]
    Full,
    CompactV1,
}

pub struct Presenter {
    evidence: private::Directory,
}
impl Presenter {
    pub fn open(root: &Path) -> io::Result<Self> {
        let path = root.join("model-output-evidence");
        let evidence = match private::Directory::create(&path) {
            Ok(directory) => directory,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => private::Directory::open(&path)?,
            Err(e) => return Err(e),
        };
        Ok(Self { evidence })
    }

    pub fn render(&self, raw: Vec<u8>) -> io::Result<Vec<u8>> {
        let mut wire: Value = serde_json::from_slice(&raw)?;
        let Some(result) = wire.get_mut("result") else {
            return Ok(raw);
        };
        if !result
            .get("structuredContent")
            .is_some_and(Value::is_object)
            || !result.get("content").is_some_and(Value::is_array)
        {
            return Ok(raw);
        }
        let hash = sha(&raw);
        let name = format!("{hash}.json");
        private::content(&self.evidence, &name, &raw, frame::OUTBOUND)?;
        let evidence =
            json!({"sha256":hash,"bytes":raw.len(),"local_path":self.evidence.child(&name)?});
        project_result(result, evidence);
        let mut output = serde_json::to_vec(&wire)?;
        output.push(b'\n');
        if output.len() > frame::OUTBOUND {
            return Err(crate::invalid("projected_response_cap"));
        }
        Ok(output)
    }
}

fn project_result(result: &mut Value, evidence: Value) {
    let is_error = result.get("isError") == Some(&Value::Bool(true));
    let original = result["structuredContent"].clone();
    let data = result["structuredContent"]
        .as_object_mut()
        .expect("checked object");
    // Only project successful observations. Fault payloads, action receipts and
    // independent verification facts remain exact, including uncertainty.
    if !is_error && data.get("scene").is_some_and(Value::is_object) && result_is_success(data) {
        let details: BTreeMap<String, Value> = data
            .get("adapter_status")
            .and_then(|a| a.pointer("/native_details/uia/walk/details"))
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|d| Some((d.get("element_id")?.as_str()?.to_owned(), d.clone())))
            .collect();
        let coverage = data
            .get("adapter_status")
            .and_then(|a| a.pointer("/native_details/uia/walk"))
            .map(|w| {
                let mut w = w.clone();
                if let Some(o) = w.as_object_mut() {
                    o.remove("elements");
                    o.remove("details");
                }
                w
            })
            .unwrap_or(Value::Null);
        if let Some(adapter) = data
            .get_mut("adapter_status")
            .and_then(Value::as_object_mut)
        {
            adapter.retain(|k, _| {
                matches!(
                    k.as_str(),
                    "backend"
                        | "execution_arch"
                        | "input"
                        | "input_dispatch"
                        | "input_grant"
                        | "lifecycle_grant"
                )
            });
        }
        let scene = data.get_mut("scene").expect("scene checked");
        let context = scene
            .pointer("/binding/mutation_context")
            .is_some_and(|v| !v.is_null())
            && scene["mutation_ready"] == true
            && scene["blocked"] == false;
        let mut omitted = 0;
        if let Some(elements) = scene.get_mut("elements").and_then(Value::as_array_mut) {
            elements.retain_mut(|element| {
                let detail = element["id"].as_str().and_then(|id| details.get(id));
                let focused = detail.is_some_and(|d| d["keyboard_focus"] == true);
                if !focused && (element["visible"] == false || element["occluded"] == true) {
                    omitted += 1;
                    return false;
                }
                if let (Some(object), Some(detail)) = (element.as_object_mut(), detail) {
                    for key in ["control_type", "enabled", "keyboard_focus", "password"] {
                        if let Some(value) = detail.get(key) {
                            object.insert(key.into(), value.clone());
                        }
                    }
                }
                true
            });
        }
        data.insert("observation_view".into(), json!({
            "checked_context_available":context,
            "action_authorized":false,
            "next_step":if context { "Choose an exact visible/unoccluded element; claim just before act_checked. Server revalidation, grants and lease are still required." } else { "Checked context unavailable. Inspect coverage/binding; do not refresh-loop or fall back to legacy/pixel input." },
            "uia_coverage":coverage,
            "omitted_nonvisible_or_occluded_elements":omitted,
            "focus_note":"keyboard_focus is a provider claim; multiple candidates are not unique focus; occluded focused nodes are not actionable"
        }));
    }
    let pending = data.get("operation").is_some_and(|o| {
        o["cancel_requested"] == true
            || o["native_call_pending"] == true
            || o["terminal"] == false
            || o["resources_released"] == false
    });
    if !pending {
        data.remove("cancellation");
    }
    data.insert(
        "presentation".into(),
        json!({"schema":"rcu.model-output.compact.v1","full_response":evidence}),
    );
    let text = Value::Object(data.clone()).to_string();
    // Remove only exact structured-content mirrors. Extra text can carry the
    // mapping/view_id for an inline downscaled image and MUST remain available.
    // Unknown textual content and every image/resource block are retained.
    let content = result["content"].as_array_mut().expect("checked content");
    content.retain(|c| {
        if c["type"] != "text" {
            return true;
        }
        let mirror = c["text"]
            .as_str()
            .and_then(|text| serde_json::from_str::<Value>(text).ok());
        !mirror
            .as_ref()
            .and_then(Value::as_object)
            .is_some_and(|fields| {
                !fields.is_empty()
                    && fields
                        .iter()
                        .all(|(key, value)| original.get(key) == Some(value))
            })
    });
    content.insert(0, json!({"type":"text","text":text}));
}
fn result_is_success(data: &serde_json::Map<String, Value>) -> bool {
    !data.contains_key("error")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn presenter() -> (tempfile::TempDir, Presenter) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let presenter = Presenter::open(&root).unwrap();
        (temp, presenter)
    }
    #[test]
    fn partial_context_is_not_ready_and_evidence_is_exact() {
        let (_temp, p) = presenter();
        let original = json!({"jsonrpc":"2.0","id":1,"result":{"content":[{"type":"text","text":"duplicated"},{"type":"image","data":"abc","mimeType":"image/png"}],"structuredContent":{"scene":{"mutation_ready":true,"blocked":false,"binding":{"mutation_context":null},"elements":[{"id":"visible","visible":true,"occluded":false},{"id":"hidden","visible":false}]},"operation":{"terminal":true,"resources_released":true},"cancellation":"cooperative; native preemption unavailable"},"isError":false}});
        let raw = serde_json::to_vec(&original).unwrap();
        let output: Value = serde_json::from_slice(&p.render(raw.clone()).unwrap()).unwrap();
        let s = &output["result"]["structuredContent"];
        assert_eq!(s["observation_view"]["checked_context_available"], false);
        assert_eq!(s["scene"]["elements"].as_array().unwrap().len(), 1);
        assert!(s.get("cancellation").is_none());
        assert_eq!(
            output["result"]["content"][2],
            original["result"]["content"][1]
        );
        assert_eq!(
            std::fs::read(
                s["presentation"]["full_response"]["local_path"]
                    .as_str()
                    .unwrap()
            )
            .unwrap(),
            raw
        );
        let fallback: Value =
            serde_json::from_str(output["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(fallback, *s);
    }
    #[test]
    fn error_and_pending_cancellation_stay_exact_and_no_goal_is_synthesized() {
        let (_temp, p) = presenter();
        let facts = json!({"error":"checked_precondition_changed","next_step":"do not replay","diagnostic":{"native":123},"operation":{"terminal":false,"resources_released":false},"cancellation":"pending"});
        let raw = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":2,"result":{"isError":true,"content":[],"structuredContent":facts}})).unwrap();
        let out: Value = serde_json::from_slice(&p.render(raw).unwrap()).unwrap();
        for (key, value) in facts.as_object().unwrap() {
            assert_eq!(out["result"]["structuredContent"][key], *value);
        }
        assert_eq!(out["result"]["isError"], true);
        assert!(
            out["result"]["structuredContent"]
                .get("goal_verified")
                .is_none()
        );
    }
    #[test]
    fn handshake_and_tools_list_are_byte_exact() {
        let (_temp, p) = presenter();
        for value in [
            json!({"jsonrpc":"2.0","id":1,"result":{"tools":[]}}),
            json!({"jsonrpc":"2.0","id":2,"error":{"code":-1,"message":"error"}}),
        ] {
            let raw = serde_json::to_vec(&value).unwrap();
            assert_eq!(p.render(raw.clone()).unwrap(), raw);
        }
    }

    #[test]
    fn action_ack_is_not_promoted_to_verified_goal() {
        let (_temp, p) = presenter();
        let receipt = json!({"request_id":"actual-request","goal_verified":false,"system_accepted":true,"status":"returned"});
        let raw = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":3,"result":{"content":[],"structuredContent":receipt,"isError":false}})).unwrap();
        let out: Value = serde_json::from_slice(&p.render(raw).unwrap()).unwrap();
        for (key, value) in receipt.as_object().unwrap() {
            assert_eq!(out["result"]["structuredContent"][key], *value);
        }
    }

    #[test]
    fn corrupted_archive_refuses_to_emit_compact_success() {
        let (_temp, p) = presenter();
        let raw = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":4,"result":{"content":[],"structuredContent":{"goal_verified":false},"isError":false}})).unwrap();
        p.evidence
            .create_new(
                &format!("{}.json", sha(&raw)),
                b"not original bytes",
                frame::OUTBOUND,
            )
            .unwrap();
        assert!(p.render(raw).is_err());
    }
    #[test]
    fn auxiliary_image_mapping_survives_while_exact_mirrors_are_removed() {
        let (_temp, p) = presenter();
        let facts = json!({"frame":"f1","scene":{"elements":[],"mutation_ready":false,"blocked":true},"operation_id":"op1"});
        let mapping = json!({"type":"text","text":json!({"view_id":"v1","mapping":{"scale":0.25},"resolve_tool":"computer.resolve_view"}).to_string()});
        let image = json!({"type":"image","data":"unchanged","mimeType":"image/png"});
        let metadata = json!({"type":"text","text":json!({"operation_id":"op1"}).to_string()});
        let raw = serde_json::to_vec(&json!({"jsonrpc":"2.0","id":5,"result":{"structuredContent":facts,"content":[{"type":"text","text":facts.to_string()},mapping,image,metadata]}})).unwrap();
        let out: Value = serde_json::from_slice(&p.render(raw).unwrap()).unwrap();
        assert_eq!(out["result"]["content"].as_array().unwrap().len(), 3);
        assert_eq!(out["result"]["content"][1], mapping);
        assert_eq!(out["result"]["content"][2], image);
    }
}
