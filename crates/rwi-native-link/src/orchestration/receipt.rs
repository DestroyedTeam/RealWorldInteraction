//! Interpret optional evidence without hiding native failure codes behind serde errors.
use crate::{invalid, types::identifier};
use rwi_core::lifecycle::PreflightReceipt;
use serde_json::Value;
use std::io;

pub(super) fn preflight_inventory<'a>(
    receipt: &PreflightReceipt,
    value: &'a Value,
) -> io::Result<&'a Value> {
    if receipt.inventory.is_none() {
        if receipt.ready_for_launch {
            return Err(invalid("preflight_ready_without_inventory"));
        }
        if let Some(error) = &receipt.error {
            // The complete original fault/diagnostic is already persisted in the
            // wire + structured response blobs. Expose a bounded code, not paths
            // or arbitrary diagnostic text, in the top-level HOLD reason.
            let code = if identifier(&error.code, 128) {
                error.code.as_str()
            } else {
                "invalid_fault_code"
            };
            return Err(invalid(&format!("preflight_native_error:{code}")));
        }
        return Err(invalid("preflight_inventory_missing"));
    }
    value
        .get("inventory")
        .filter(|v| v.is_object())
        .ok_or_else(|| invalid("preflight_inventory_missing"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn failed() -> Value {
        json!({"authorization_id":"test","ready_for_launch":false,"inventory":null,
            "native_events":{"events":[],"complete":false,"hooks_released":true},
            "error":{"code":"window_gone","next_step":"Discover again."},
            "resources_released":true,"resource_release_uncertain":false,
            "reservation_created":false,"launch_dispatched":false,
            "activation_dispatched":false,"input_dispatched":false,"goal_verified":false})
    }

    #[test]
    fn missing_inventory_preserves_native_window_gone_not_serde_null_error() {
        let value = failed();
        let receipt = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            preflight_inventory(&receipt, &value)
                .unwrap_err()
                .to_string(),
            "preflight_native_error:window_gone"
        );
    }

    #[test]
    fn missing_inventory_never_claims_ready_or_invents_evidence() {
        let mut value = failed();
        value["error"] = Value::Null;
        let receipt = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            preflight_inventory(&receipt, &value)
                .unwrap_err()
                .to_string(),
            "preflight_inventory_missing"
        );
        value["ready_for_launch"] = json!(true);
        let receipt = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            preflight_inventory(&receipt, &value)
                .unwrap_err()
                .to_string(),
            "preflight_ready_without_inventory"
        );
    }
}
