//! Explicit route negotiation is read-only and never retries or claims input.
use super::*;

pub(super) fn validate_click_delivery(a: &ClickArgs) -> Result<&'static str, DeviceResult> {
    let action = matches!(
        &a.at,
        ClickTarget::Element {
            action: Some(_),
            ..
        }
    );
    if a.delivery.is_some() && (action || a.click_count != 1) {
        return Err(invalid(
            "Explicit delivery requires click_count=1 and no native action.",
        ));
    }
    click_intent(a.click_count, action)
}

fn click_capability(
    a: &ClickArgs,
    device_id: &str,
    tool: &str,
    result: &DeviceResult,
) -> Result<(), DeviceResult> {
    let unavailable = || {
        DeviceResult::error(
            "capability_unavailable",
            "The exact macOS target/peer does not advertise explicit click delivery; no lease, input or fallback.",
        )
    };
    let devices: Vec<_> = result.data["devices"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["device_id"] == device_id)
        .collect();
    if devices.len() != 1 {
        return Err(unavailable());
    }
    let device = devices[0];
    if device["is_error"] == true {
        return Err(DeviceResult {
            is_error: true,
            data: device["state"].clone(),
            images: vec![],
        });
    }
    let state = &device["state"];
    let tools: Vec<_> = state["capabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| row["id"] == tool)
        .collect();
    let authorized =
        tools.len() == 1 && tools[0]["implemented"] == true && tools[0]["authorized"] == true;
    if device["is_error"] != false
        || state["platform"] != "macos"
        || !authorized
        || state["executor_features"]["click_delivery_options"]["implemented"] != true
    {
        return Err(unavailable());
    }
    let targets: Vec<_> = result.data["targets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| row["target"] == a.target)
        .collect();
    if targets.len() != 1 {
        return Err(unavailable());
    }
    let capabilities = &targets[0]["capabilities"];
    if capabilities["window_selected"] != true {
        return Err(unavailable());
    }
    a.delivery
        .as_ref()
        .ok_or_else(unavailable)?
        .require_capability(capabilities)
        .map_err(|error| DeviceResult::error(&error.code, &error.next_step))
}

pub(super) async fn click<F, Fut>(
    a: ClickArgs,
    device_id: Option<String>,
    mut call: F,
) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    let intent = match validate_click_delivery(&a) {
        Ok(intent) => intent,
        Err(error) => return error,
    };
    let requested_delivery = a.delivery.as_ref().map(|delivery| delivery.delivery_name());
    let tool = if matches!(&a.at, ClickTarget::Element { .. }) {
        "computer.act_checked"
    } else {
        "computer.act"
    };
    let value = if let Some(delivery) = &a.delivery {
        if a.target.is_empty() || !device::valid_id(&a.request_id) {
            return invalid("Use an exact target and stable request_id.");
        }
        let Some(device_id) = device_id else {
            return DeviceResult::error(
                "device_unknown",
                "No selected peer for explicit click delivery.",
            );
        };
        let inspected = call("computer.targets", json!({"device_id":device_id}), false).await;
        if inspected.is_error {
            return inspected;
        }
        if let Err(error) = click_capability(&a, &device_id, tool, &inspected) {
            return error;
        }
        match serde_json::to_string(delivery) {
            Ok(value) => Some(value),
            Err(_) => return invalid("Cannot encode explicit click delivery."),
        }
    } else {
        None
    };
    let action = match a.at {
        ClickTarget::Element {
            frame,
            element,
            action,
        } => json!({"frame":frame,"element":element,"intent":intent,"value":value.or(action)}),
        ClickTarget::View {
            view_id,
            x,
            y,
            red_marker_cluster,
        } => {
            if !x.is_finite() || !y.is_finite() {
                return invalid("Coordinates must be finite.");
            }
            let resolved = call(
                "computer.resolve_view",
                view_resolution_args(&a.target, &view_id, x, y, red_marker_cluster),
                false,
            )
            .await;
            if resolved.is_error {
                return resolved;
            }
            let mut action = json!({"reference":resolved.data["reference"],"intent":intent});
            if let Some(value) = value {
                action["value"] = json!(value);
            }
            action
        }
    };
    let mut result = leased(&a.target, &a.request_id, tool, action, call).await;
    if let Some(requested_delivery) = requested_delivery {
        result.data["requested_delivery"] = json!(requested_delivery);
        result.data["goal_verified"] = json!(false);
    }
    result
}
