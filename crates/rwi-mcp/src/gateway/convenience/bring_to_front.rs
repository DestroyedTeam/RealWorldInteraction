//! One explicit foreground request, using the existing device-owned guards.
use super::*;

pub(super) fn bring_to_front_returned(result: &DeviceResult) -> bool {
    !result.is_error
        && matches!(
            result.data["activation"]["state"].as_str(),
            Some("foreground_observed" | "already_foreground")
        )
        && serde_json::from_value::<rwi_core::ForegroundState>(
            result.data["activation"]["foreground"].clone(),
        )
        .is_ok_and(|value| value.validate().is_ok())
        && result.data["receipt"]["dispatch"] == "returned"
        && result.data["receipt"]["dispatch_error"].is_null()
        && result.data["input_domain_fenced"] == false
        && result.data["operation"]["terminal"] == true
        && result.data["operation"]["native_call_pending"] == false
        && result.data["operation"]["resources_released"] == true
        && result.data["operation"]["resource_release_uncertain"] == false
}

pub(super) fn compact_bring_to_front_reply(
    result: &DeviceResult,
    details_ref: Option<String>,
) -> Value {
    if !bring_to_front_returned(result) {
        let mut data = result.data.clone();
        data["details_ref"] = json!(details_ref);
        return data;
    }
    let mut reply = compact_action_reply(&result.data["receipt"], details_ref);
    for key in [
        "activation",
        "operation_ref",
        "input_domain_fenced",
        "stage_timings",
        "elapsed_ms",
    ] {
        if let Some(value) = result.data.get(key) {
            reply[key] = value.clone();
        }
    }
    reply
}

pub(super) async fn bring_to_front<F, Fut>(args: BringToFrontArgs, mut call: F) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    if args.target.is_empty() || !device::valid_id(&args.request_id) {
        return invalid("Supply an exact selected target and stable request_id.");
    }
    let observed = call(
        "computer.observe",
        json!({"target":args.target,"inline_image":false}),
        false,
    )
    .await;
    if observed.is_error {
        return observed;
    }
    let Some(frame) = observed.data["frame"].as_str().filter(|s| !s.is_empty()) else {
        return DeviceResult::error(
            "invalid_observation_reply",
            "No fresh frame returned; no foreground request sent.",
        );
    };
    // Do not infer an unknown foreground from a PID, the selected window, a
    // prior targets response or a cached frame. The device rechecks this exact
    // tuple immediately before its sole public native foreground request.
    let foreground = match serde_json::from_value::<rwi_core::ForegroundState>(
        observed.data["adapter_status"]["current_foreground"].clone(),
    ) {
        Ok(value) if value.validate().is_ok() => value,
        _ => {
            return DeviceResult::error(
                "foreground_identity_unavailable",
                "Fresh complete native foreground identity is required; no foreground request sent.",
            );
        }
    };
    let reference = call(
        "computer.resolve",
        json!({"target":args.target,"frame":frame,"locator":{"kind":"window"}}),
        false,
    )
    .await;
    if reference.is_error {
        return reference;
    }
    let Some(reference) = reference.data["reference"]
        .as_str()
        .filter(|s| !s.is_empty())
    else {
        return DeviceResult::error(
            "invalid_reference_reply",
            "No exact window reference returned; no foreground request sent.",
        );
    };
    // Same cancellation-resistant cleanup and durable request identity as the
    // other typed inputs. Never retry activation or reinterpret a refusal.
    leased(
        &args.target,
        &args.request_id,
        "computer.focus_window",
        json!({"reference":reference,"expected_foreground":foreground}),
        call,
    )
    .await
}

#[cfg(test)]
mod tests;
