//! Explicit structuredContent-aware text summary. Never changes the full data,
//! native handles, authority, result cache, images or execution path.
use rwi_core::device::DeviceResult;
use serde_json::{Map, Value, json};

const VERSION: &str = "targets_structured_summary_v1";
const MARKER: &str = "_text_projection";

fn state_summary(value: &Value, at: &str, omitted: &mut Vec<String>) -> Option<Value> {
    let state = value.as_object()?;
    if state.get("availability").and_then(Value::as_str) != Some("available") {
        return Some(value.clone());
    }
    let mut out = Map::new();
    for (key, value) in state {
        if (key == "capabilities" && value.is_array())
            || (key == "executor_features" && value.is_object())
        {
            omitted.push(format!("{at}/{key}"));
        } else {
            out.insert(key.clone(), value.clone());
        }
    }
    Some(Value::Object(out))
}

fn device_summary(value: &Value, index: usize, omitted: &mut Vec<String>) -> Option<Value> {
    let device = value.as_object()?;
    if device.get("is_error").and_then(Value::as_bool) != Some(false) {
        return Some(value.clone());
    }
    device
        .get("device_id")?
        .as_str()
        .filter(|id| !id.is_empty())?;
    device.get("state")?.as_object()?;
    let mut out = Map::new();
    for (key, value) in device {
        let projected = if key == "state" {
            state_summary(value, &format!("/devices/{index}/state"), omitted)?
        } else {
            value.clone()
        };
        out.insert(key.clone(), projected);
    }
    Some(Value::Object(out))
}

fn target_summary(value: &Value, index: usize, omitted: &mut Vec<String>) -> Option<Value> {
    let target = value.as_object()?;
    target.get("target")?.as_str().filter(|id| !id.is_empty())?;
    let mut out = Map::new();
    for (key, value) in target {
        if key == "capabilities" && value.is_object() {
            omitted.push(format!("/targets/{index}/capabilities"));
        } else {
            // Includes all identity, bindings, controls, labels, coordinates and
            // unknown/future fields. No handle shortening or schema guessing.
            out.insert(key.clone(), value.clone());
        }
    }
    Some(Value::Object(out))
}

pub(super) fn project(result: &DeviceResult) -> Option<Value> {
    if result.is_error || !result.images.is_empty() {
        return None;
    }
    let data = result.data.as_object()?;
    if data.contains_key(MARKER)
        || data.contains_key("dispatch_error")
        || data
            .get("profile_error")
            .is_some_and(|value| !value.is_null())
    {
        return None;
    }
    let devices = data.get("devices")?.as_array()?;
    let targets = data.get("targets")?.as_array()?;
    let mut omitted = Vec::new();
    let projected_devices = devices
        .iter()
        .enumerate()
        .map(|(i, value)| device_summary(value, i, &mut omitted))
        .collect::<Option<Vec<_>>>()?;
    let projected_targets = targets
        .iter()
        .enumerate()
        .map(|(i, value)| target_summary(value, i, &mut omitted))
        .collect::<Option<Vec<_>>>()?;
    if omitted.is_empty() {
        return None;
    }
    // Copy other root values unchanged, but do not first clone the full bulky
    // sections only to discard them. All originals remain in structuredContent.
    let mut out: Map<String, Value> = data
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "devices" | "targets"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    out.insert("devices".into(), Value::Array(projected_devices));
    out.insert("targets".into(), Value::Array(projected_targets));
    out.insert(
        MARKER.into(),
        json!({
            "version":VERSION,
            "scope":"content.text summary only; complete structuredContent unchanged",
            "full_details":"structuredContent",
            "omitted_from_text":omitted,
        }),
    );
    Some(Value::Object(out))
}

pub(super) fn fits(full: Option<&str>, text: &Value, encoded: &str) -> bool {
    // Preserve the legacy exact-duplicate alias contract. The new opt-in never
    // makes text larger; tiny or unsupported replies stay full and unmarked.
    text.pointer("/_text_projection/version")
        .and_then(Value::as_str)
        != Some(VERSION)
        || full.is_some_and(|full| encoded.len() < full.len())
}

#[cfg(test)]
mod tests {
    use super::super::{Presentation, TargetsTextProjection, take_targets_text_projection};
    use super::*;

    fn fixture() -> Value {
        json!({
            "service":"healthy", "profile_error":null, "workflow_required":false,
            "devices":[{"device_id":"mac-fixture","is_error":false,"state":{
                "availability":"available", "service":"healthy", "context_valid":true,
                "platform":"macos", "instance":"instance-exact", "input_domain_fenced":false,
                "resource_domain":"domain-exact", "native_context":{"host":"host-exact","session":"session-exact"},
                "runtime_status":{"pid":42,"permissions":{"screen_capture":false,"post_event":true,"listen_event":null},"hardware_attestation":false},
                "live_desktop_facts":{"scope":"no input","future":null},
                "capabilities":[{"id":"computer.click","implemented":true,"authorized":true,"available":false,"reason":"selected_window_required"}],
                "executor_features":{"future_detail":"x".repeat(2048)},"unknown_state":{"keep":[false,null,12]}
            }}],
            "targets":[{"target":"mac-fixture:instance-exact:context-exact","identity":{"session":"session-exact"},"parent_target":null,
                "capabilities":{"policy":"y".repeat(4096)},
                "controls":[{"name":"X close","id":"exact-control","bounds":[10,20,44,44]}],
                "view_id":"exact-view","future_target":{"x":3.5}}],
            "future_root":{"keep":true},"goal_verified":false
        })
    }
    fn reply(raw: Value) -> Value {
        let result = DeviceResult::value(raw);
        let text = Presentation::default()
            .targets_text_for(&result, TargetsTextProjection::StructuredSummaryV1);
        serde_json::to_value(super::super::super::mcp_result_with_text_projection(
            result, text,
        ))
        .unwrap()
    }
    fn text(reply: &Value) -> Value {
        serde_json::from_str(reply["content"][0]["text"].as_str().unwrap()).unwrap()
    }
    #[test]
    fn f45_summary_schema_and_typed_adapter_only_argument() {
        let mut args = json!({"device_id":"fixture","text_projection":"structured_summary_v1"})
            .as_object()
            .unwrap()
            .clone();
        assert_eq!(
            take_targets_text_projection(&mut args).unwrap(),
            Some(TargetsTextProjection::StructuredSummaryV1)
        );
        assert_eq!(Value::Object(args.clone()), json!({"device_id":"fixture"}));
        assert!(
            rwi_core::device::catalog::validate("computer.targets", &Value::Object(args)).is_ok()
        );
        for value in [
            json!(null),
            json!(true),
            json!(1),
            json!("structured_summary_v2"),
            json!({}),
        ] {
            let mut args = json!({"text_projection":value})
                .as_object()
                .unwrap()
                .clone();
            assert!(take_targets_text_projection(&mut args).is_err());
        }
        let mut other = Some(
            json!({"text_projection":"structured_summary_v1"})
                .as_object()
                .unwrap()
                .clone(),
        );
        let before = other.clone();
        assert_eq!(
            super::super::targets_text_request("computer.observe", &mut other).unwrap(),
            None
        );
        assert_eq!(other, before);
    }
    #[test]
    fn f45_summary_full_structured_data_and_coordinates_unchanged() {
        let raw = fixture();
        let reply = reply(raw.clone());
        let text = text(&reply);
        assert_eq!(reply["structuredContent"], raw);
        assert_eq!(
            text["targets"][0]["controls"],
            raw["targets"][0]["controls"]
        );
        for key in [
            "target",
            "identity",
            "parent_target",
            "view_id",
            "future_target",
        ] {
            assert_eq!(text["targets"][0][key], raw["targets"][0][key]);
        }
        assert_eq!(
            text["devices"][0]["state"]["runtime_status"],
            raw["devices"][0]["state"]["runtime_status"]
        );
        assert_eq!(
            text["devices"][0]["state"]["live_desktop_facts"],
            raw["devices"][0]["state"]["live_desktop_facts"]
        );
        assert_eq!(text["future_root"], raw["future_root"]);
        assert_eq!(text["goal_verified"], false);
        assert_eq!(
            text[MARKER]["omitted_from_text"],
            json!([
                "/devices/0/state/capabilities",
                "/devices/0/state/executor_features",
                "/targets/0/capabilities"
            ])
        );
        assert!(
            reply["content"][0]["text"].as_str().unwrap().len()
                < serde_json::to_vec(&raw).unwrap().len() / 2
        );
    }
    #[test]
    fn f45_summary_default_and_exact_duplicate_modes_remain_legacy() {
        let raw = fixture();
        let result = DeviceResult::value(raw.clone());
        let cache = Presentation::default();
        assert!(
            cache
                .targets_text_for(&result, TargetsTextProjection::ExactDuplicatesV1)
                .is_none()
        );
        let full = serde_json::to_value(super::super::super::mcp_result_with_text_projection(
            result, None,
        ))
        .unwrap();
        assert_eq!(text(&full), raw);
        assert_eq!(full["structuredContent"], raw);
    }
    #[test]
    fn f45_summary_mixed_offline_devices_keep_errors_and_identities() {
        let mut raw = fixture();
        let error = json!({"device_id":"win-fixture","is_error":true,"state":{"availability":"unavailable","code":"offline","message":"not woken","future":null}});
        raw["devices"].as_array_mut().unwrap().push(error.clone());
        raw["targets"].as_array_mut().unwrap().push(json!({"target":"win-fixture:exact","identity":{"session":5},"capabilities":{"unknown":true},"buttons":[{"x":12,"y":20}]}));
        let reply = reply(raw.clone());
        let text = text(&reply);
        assert_eq!(reply["structuredContent"], raw);
        assert_eq!(text["devices"][1], error);
        assert_eq!(text["targets"][1]["target"], "win-fixture:exact");
        assert_eq!(text["targets"][1]["buttons"], raw["targets"][1]["buttons"]);
    }
    #[test]
    fn f45_summary_malformed_and_reserved_values_fall_back_without_mutation() {
        for (path, value) in [
            ("/devices", json!(null)),
            ("/targets", json!(false)),
            ("/devices/0/state", json!(null)),
            ("/targets/0/target", json!(null)),
            ("/devices/0/device_id", json!("")),
        ] {
            let mut raw = fixture();
            *raw.pointer_mut(path).unwrap() = value;
            let before = raw.clone();
            assert!(
                project(&DeviceResult::value(raw.clone())).is_none(),
                "{path}"
            );
            assert_eq!(raw, before);
        }
        for (key, value) in [
            (MARKER, json!(null)),
            ("profile_error", json!("private-profile-error")),
            ("dispatch_error", json!({"code":"error"})),
        ] {
            let mut raw = fixture();
            raw[key] = value;
            assert!(project(&DeviceResult::value(raw)).is_none(), "{key}");
        }
    }
    #[test]
    fn f45_summary_unknown_known_section_types_stay_inline() {
        let mut raw = fixture();
        raw["devices"][0]["state"]["capabilities"] = json!("future-enum");
        raw["devices"][0]["state"]["executor_features"] = json!(null);
        raw["targets"][0]["capabilities"] = json!(["future"]);
        assert!(project(&DeviceResult::value(raw.clone())).is_none());
        assert_eq!(text(&reply(raw.clone())), raw);
    }
    #[test]
    fn f45_summary_tiny_payload_does_not_expand_output() {
        let raw = json!({"devices":[{"device_id":"a","is_error":false,"state":{"availability":"available","capabilities":[]}}],"targets":[{"target":"x"}]});
        let reply = reply(raw.clone());
        assert_eq!(text(&reply), raw);
        assert_eq!(reply["structuredContent"], raw);
        assert!(!fits(
            None,
            &json!({"_text_projection":{"version":VERSION}}),
            "x"
        ));
    }
    #[test]
    fn f45_summary_portable_numbers_match_full_structured_unknowns() {
        let mut raw = fixture();
        raw["future_root"]["large"] = json!(9_007_199_254_740_993_u64);
        let mut expected = raw.clone();
        super::super::super::portable_numbers(&mut expected);
        let reply = reply(raw);
        assert_eq!(reply["structuredContent"], expected);
        assert_eq!(text(&reply)["future_root"], expected["future_root"]);
    }
    #[test]
    fn f45_summary_errors_and_images_never_use_summary() {
        let mut result = DeviceResult::value(fixture());
        result.is_error = true;
        assert!(project(&result).is_none());
        let mut result = DeviceResult::value(fixture());
        result.images.push(rwi_core::device::DeviceImage {
            mime: "image/png".into(),
            base64: "exact-image".into(),
        });
        assert!(project(&result).is_none());
        let mut raw = fixture();
        raw["dispatch_error"] = json!({"code":"denied","next_step":"inspect original"});
        let result = reply(raw);
        assert_eq!(result["isError"], true);
        assert!(text(&result).get(MARKER).is_none());
    }
    #[test]
    fn f45_summary_legacy_alias_encoding_is_not_size_gated() {
        let legacy = json!({"presentation":{"text_projection":{"version":"targets_text_v1"}}});
        assert!(fits(Some("x"), &legacy, &"x".repeat(100)));
        assert!(fits(None, &legacy, "x"));
    }
}
