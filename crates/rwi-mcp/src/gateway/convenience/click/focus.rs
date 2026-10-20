//! Bounded observation for explicitly routed, same-process sibling-window clicks.
//! Two samples describe neither continuous preservation nor input causality.
use super::*;

const COVERAGE: &str = "two_samples_not_continuous_or_causal";

struct Sample {
    instance: String,
    context: Value,
    identity: Value,
    captured_ms: u64,
    foreground: Value,
}

pub(super) struct Probe {
    device_id: String,
    target: String,
    before: Sample,
}

fn text(value: &Value) -> Option<&str> {
    value.as_str().filter(|s| !s.is_empty() && s.len() <= 256)
}

fn stamp(value: &Value) -> Option<Value> {
    let pid = value["pid"]
        .as_u64()
        .filter(|p| *p > 0 && *p <= u32::MAX as u64)?;
    Some(json!({"pid":pid,"window":text(&value["window"])? ,
        "process_start":text(&value["process_start"])? ,
        "native_epoch":text(&value["native_epoch"])?}))
}

fn sample(result: &DeviceResult, device_id: &str, target: &str) -> Option<Sample> {
    if result.is_error {
        return None;
    }
    let devices: Vec<_> = result.data["devices"]
        .as_array()?
        .iter()
        .filter(|d| d["device_id"] == device_id)
        .collect();
    let targets: Vec<_> = result.data["targets"]
        .as_array()?
        .iter()
        .filter(|t| t["target"] == target)
        .collect();
    if devices.len() != 1 || targets.len() != 1 || devices[0]["is_error"] != false {
        return None;
    }
    let state = &devices[0]["state"];
    if state["context_valid"] != true {
        return None;
    }
    let context = state["native_context"]
        .as_object()
        .filter(|v| !v.is_empty())?;
    let identity = targets[0]["identity"]
        .as_object()
        .filter(|v| !v.is_empty())?;
    let live = &state["live_desktop_facts"];
    if live["identity"] != state["native_context"] {
        return None;
    }
    let foreground = stamp(&live["values"]["current_foreground"])?;
    // Do not compare a mixed live sample and target capability snapshot.
    if stamp(&targets[0]["capabilities"]["current_foreground"])? != foreground {
        return None;
    }
    Some(Sample {
        instance: text(&state["instance"])?.into(),
        context: json!(context),
        identity: json!(identity),
        captured_ms: live["captured_unix_ms"].as_u64().filter(|t| *t > 0)?,
        foreground,
    })
}

impl Probe {
    pub(super) fn from_preflight(
        result: &DeviceResult,
        device_id: &str,
        target: &str,
    ) -> Option<Self> {
        Some(Self {
            device_id: device_id.into(),
            target: target.into(),
            before: sample(result, device_id, target)?,
        })
    }

    pub(super) fn device_id(&self) -> &str {
        &self.device_id
    }

    pub(super) fn needs_post(&self, result: &DeviceResult) -> bool {
        let data = &result.data;
        !result.is_error
            && data["dispatch"] == "returned"
            && data["input_domain_fenced"] != true
            && data["resources_released"] != false
            && data["deduplicated"] != true
            && data.get("dispatch_error").is_none_or(Value::is_null)
            && data
                .get("operation")
                .is_none_or(|op| op["terminal"] == true && op["resources_released"] == true)
            && data["scope"]["pid"] == self.before.foreground["pid"]
            && text(&data["scope"]["window"]).is_some()
            && data["scope"]["window"] != self.before.foreground["window"]
    }

    pub(super) fn finish(self, result: &DeviceResult) -> Value {
        let mut observation = json!({"state":"unavailable","coverage":COVERAGE,
            "before":self.before.foreground,"before_captured_unix_ms":self.before.captured_ms,
            "after":null,"after_captured_unix_ms":null});
        if result.is_error {
            observation["reason"] = json!("focus_read_failed");
            observation["read_error"] = result.data.clone();
            return observation;
        }
        let Some(after) = sample(result, &self.device_id, &self.target) else {
            observation["reason"] = json!("focus_sample_unavailable");
            return observation;
        };
        if after.instance != self.before.instance
            || after.context != self.before.context
            || after.identity != self.before.identity
        {
            observation["reason"] = json!("focus_context_changed");
        } else if after.captured_ms <= self.before.captured_ms {
            observation["reason"] = json!("focus_sample_not_fresh");
        } else {
            observation["state"] = json!(if after.foreground == self.before.foreground {
                "same_at_samples"
            } else {
                "different_at_samples"
            });
            observation["after"] = after.foreground;
            observation["after_captured_unix_ms"] = json!(after.captured_ms);
        }
        observation
    }
}

pub(in crate::gateway::convenience) fn compact(value: &Value) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    let mut result = json!({"state":value["state"],"coverage":value["coverage"],
        "before_window":value["before"]["window"],"after_window":value["after"]["window"]});
    if let Some(reason) = value.get("reason") {
        result["reason"] = reason.clone();
    }
    if let Some(error) = value.get("read_error") {
        result["read_error"] = error["code"].clone();
    }
    result
}
