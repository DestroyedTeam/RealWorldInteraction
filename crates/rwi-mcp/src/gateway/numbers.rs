//! Exact integer conversion at the JSON/MCP boundary.
use serde_json::{Value, json};

pub(super) fn numeric_inputs(v: &mut Value) {
    match v {
        Value::Object(o) => {
            for (k, v) in o {
                if matches!(
                    k.as_str(),
                    "process_created" | "creation_filetime_100ns" | "logon_time_100ns"
                ) && let Some(n) = v.as_str().and_then(|s| s.parse::<u64>().ok())
                {
                    *v = json!(n);
                }
                numeric_inputs(v)
            }
        }
        Value::Array(a) => {
            for v in a {
                numeric_inputs(v)
            }
        }
        _ => {}
    }
}
pub(super) fn portable_numbers(v: &mut Value) {
    match v {
        Value::Number(n) => {
            if let Some(n) = n.as_u64()
                && n > 9_007_199_254_740_991
            {
                *v = json!(n.to_string());
            }
        }
        Value::Object(o) => {
            for v in o.values_mut() {
                portable_numbers(v)
            }
        }
        Value::Array(a) => {
            for v in a {
                portable_numbers(v)
            }
        }
        _ => {}
    }
}
pub(super) fn schema_numbers(v: &mut Value) {
    match v {
        Value::Object(o) => {
            if let Some(props) = o.get_mut("properties").and_then(Value::as_object_mut) {
                for (k, v) in props {
                    if matches!(
                        k.as_str(),
                        "process_created" | "creation_filetime_100ns" | "logon_time_100ns"
                    ) {
                        *v = json!({"type":"string","pattern":"^[0-9]{1,20}$"});
                    }
                }
            }
            for v in o.values_mut() {
                schema_numbers(v)
            }
        }
        Value::Array(a) => {
            for v in a {
                schema_numbers(v)
            }
        }
        _ => {}
    }
}
