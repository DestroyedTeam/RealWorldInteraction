//! Current hotkey/window-focus error presentation, never historical operation data.
use rwi_core::device::DeviceResult;

pub(super) fn promote_current_error(tool: &str, result: &mut DeviceResult) {
    if !matches!(tool, "computer.hotkey" | "computer.focus_window") {
        return;
    }
    let Some(failure) = result
        .data
        .get("receipt")
        .and_then(|receipt| receipt.get("dispatch_error"))
        .filter(|failure| failure.is_object())
        .cloned()
    else {
        return;
    };
    // Older Agents can wrap a current input/focus failure in a successful envelope.
    // Keep the complete original receipt, dispatch/uncertainty and operation ref.
    result.is_error = true;
    for (destination, source) in [("code", "code"), ("message", "next_step")] {
        if result.data.get(destination).is_none()
            && let Some(value) = failure.get(source)
        {
            result.data[destination] = value.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn failure() -> Value {
        json!({
            "receipt":{"dispatch":"rejected_before_dispatch","dispatch_error":{
                "code":"foreground_changed","next_step":"Global keyboard receiver changed; no further down event."
            },"request_id":"fixture-request","replay":"forbidden","goal_verified":false},
            "native_dispatch_entered":false,"operation_ref":{"fixture":"retained"},
            "unknown_future":{"button":"Close","point":[17,29]}
        })
    }

    #[test]
    fn f49_current_failure_promotes_flag_and_preserves_original_evidence() {
        let original = failure();
        let mut result = DeviceResult::value(original.clone());
        promote_current_error("computer.hotkey", &mut result);
        assert!(result.is_error);
        assert_eq!(result.data["code"], "foreground_changed");
        assert_eq!(
            result.data["message"],
            original["receipt"]["dispatch_error"]["next_step"]
        );
        let once = result.data.clone();
        promote_current_error("computer.hotkey", &mut result);
        assert_eq!(result.data, once);
        result.data.as_object_mut().unwrap().remove("code");
        result.data.as_object_mut().unwrap().remove("message");
        assert_eq!(result.data, original);
    }

    #[test]
    fn f49_success_absent_null_or_unknown_shapes_are_not_invented_errors() {
        for data in [
            Value::Null,
            json!([]),
            json!({}),
            json!({"receipt":null}),
            json!({"receipt":{"dispatch":"returned","dispatch_error":null,"goal_verified":false,"system_accepted":null}}),
            json!({"receipt":{"dispatch_error":false}}),
            json!({"receipt":{"dispatch_error":"unknown future representation"}}),
            json!({"receipt":{"dispatch_error":[]}}),
        ] {
            let mut result = DeviceResult::value(data.clone());
            promote_current_error("computer.hotkey", &mut result);
            assert!(!result.is_error);
            assert_eq!(result.data, data);
        }
    }

    #[test]
    fn f49_successful_history_queries_and_other_tools_keep_their_envelope() {
        for tool in [
            "computer.operation",
            "computer.cancel",
            "computer.targets",
            "computer.live",
            "computer.act",
            "browser.session",
        ] {
            let data = failure();
            let mut result = DeviceResult::value(data.clone());
            promote_current_error(tool, &mut result);
            assert!(!result.is_error, "{tool}");
            assert_eq!(result.data, data);
        }
    }

    #[test]
    fn f49_existing_diagnostics_and_uncertain_native_state_are_not_rewritten() {
        let mut data = failure();
        data["code"] = json!("existing_native_diagnostic");
        data["message"] = json!("Keep the original diagnostic.");
        data["receipt"]["dispatch"] = json!("uncertain");
        data["native_dispatch_entered"] = json!(true);
        data["operation"] =
            json!({"terminal":false,"native_call_pending":true,"resources_released":false});
        let mut result = DeviceResult::value(data.clone());
        result.is_error = true;
        promote_current_error("computer.hotkey", &mut result);
        assert!(result.is_error);
        assert_eq!(result.data, data);
    }

    #[test]
    fn f49_declared_empty_error_is_an_error_without_fabricated_diagnostics() {
        let data = json!({"receipt":{"dispatch_error":{}},"goal_verified":null});
        let mut result = DeviceResult::value(data.clone());
        promote_current_error("computer.hotkey", &mut result);
        assert!(result.is_error);
        assert_eq!(result.data, data);
        assert!(result.data.get("code").is_none());
        assert!(result.data.get("message").is_none());
    }

    fn focus_failure() -> Value {
        json!({
            "receipt":{"dispatch":"uncertain","dispatch_error":{
                "code":"foreground_identity_unknown",
                "next_step":"No exact foreground window number; do not guess."
            },"request_id":"fixture-focus","replay":"forbidden","goal_verified":false},
            "native_dispatch_entered":true,"operation_ref":{"fixture":"retained"},
            "diagnostic":{"native_request_counts":{"AXFrontmost":1,"AXRaise":1}},
            "operation":{"terminal":true,"native_call_pending":false,"resources_released":true}
        })
    }

    #[test]
    fn focus_receipt_error_preserves_uncertain_effect_and_original_evidence() {
        let original = focus_failure();
        let mut result = DeviceResult::value(original.clone());
        promote_current_error("computer.focus_window", &mut result);
        assert!(result.is_error);
        assert_eq!(result.data["code"], "foreground_identity_unknown");
        assert_eq!(
            result.data["message"],
            original["receipt"]["dispatch_error"]["next_step"]
        );
        let once = result.data.clone();
        promote_current_error("computer.focus_window", &mut result);
        assert_eq!(result.data, once);
        result.data.as_object_mut().unwrap().remove("code");
        result.data.as_object_mut().unwrap().remove("message");
        assert_eq!(result.data, original);
    }

    #[test]
    fn focus_receipt_error_does_not_invent_success_or_unknown_shape_failures() {
        for data in [
            Value::Null,
            json!([]),
            json!({}),
            json!({"receipt":null}),
            json!({"receipt":{"dispatch":"returned","dispatch_error":null,"goal_verified":false}}),
            json!({"receipt":{"dispatch":"uncertain"}}),
            json!({"receipt":{"dispatch_error":"future representation"}}),
            json!({"receipt":{"dispatch_error":false}}),
            json!({"receipt":{"dispatch_error":[]}}),
        ] {
            let mut result = DeviceResult::value(data.clone());
            promote_current_error("computer.focus_window", &mut result);
            assert!(!result.is_error);
            assert_eq!(result.data, data);
        }
        let data = json!({"receipt":{"dispatch_error":{}}});
        let mut result = DeviceResult::value(data.clone());
        promote_current_error("computer.focus_window", &mut result);
        assert!(result.is_error);
        assert_eq!(result.data, data, "do not fabricate diagnostics");
    }

    #[test]
    fn focus_receipt_error_keeps_existing_diagnostics_and_history_query_envelopes() {
        let mut data = focus_failure();
        data["code"] = json!("existing_native_code");
        data["message"] = json!("Keep the original diagnostic.");
        let mut current = DeviceResult::value(data.clone());
        promote_current_error("computer.focus_window", &mut current);
        assert!(current.is_error);
        assert_eq!(current.data, data);
        for tool in [
            "computer.operation",
            "computer.cancel",
            "computer.targets",
            "computer.act",
        ] {
            let mut query = DeviceResult::value(data.clone());
            promote_current_error(tool, &mut query);
            assert!(!query.is_error, "{tool}");
            assert_eq!(query.data, data);
        }
    }
}
