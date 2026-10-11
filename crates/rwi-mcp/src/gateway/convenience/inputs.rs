use super::*;

// Cleanup ignores Host cancellation, but never repeats an action. Native lease
// expiry remains authoritative if transport/shutdown prevents explicit release.
pub(super) async fn leased<F, Fut>(
    target: &str,
    request: &str,
    tool: &'static str,
    mut args: Value,
    mut call: F,
) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    if !device::valid_id(request) {
        return invalid("Supply a stable request_id (1..128 safe ASCII characters).");
    }
    let claim = call(
        "computer.claim",
        json!({"target":target,"ttl_ms":10000}),
        false,
    )
    .await;
    if claim.is_error {
        return claim;
    }
    let Some(lease) = claim.data["lease"].as_str() else {
        return DeviceResult::error(
            "invalid_lease_reply",
            "Claim returned no lease; no input was sent.",
        );
    };
    args["target"] = json!(target);
    args["request_id"] = json!(request);
    args["lease"] = json!(lease);
    let action = call(tool, args, false).await;
    let release = call("computer.release", json!({"lease":lease}), true).await;
    if release.is_error {
        return DeviceResult {
            is_error: true,
            data: json!({"code":"input_cleanup_unconfirmed","action":action.data,"cleanup":release.data,"goal_verified":false,"replay_allowed":false}),
            images: vec![],
        };
    }
    action
}
pub(super) async fn place_window<F, Fut>(args: PlaceWindowArgs, mut call: F) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    if !device::valid_id(&args.request_id) || args.display.validate().is_err() {
        return invalid("Use a stable request_id and an explicit valid display selector.");
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
    let Some(frame) = observed.data["frame"].as_str() else {
        return DeviceResult::error(
            "invalid_observation_reply",
            "No frame returned; nothing moved.",
        );
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
    let Some(reference) = reference.data["reference"].as_str() else {
        return DeviceResult::error(
            "invalid_reference_reply",
            "No window reference returned; nothing moved.",
        );
    };
    leased(
        &args.target,
        &args.request_id,
        "computer.move_window",
        json!({"reference":reference,"display":args.display}),
        call,
    )
    .await
}
pub(super) async fn hover<F, Fut>(args: HoverArgs, mut call: F) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    if args.target.is_empty() || !device::valid_id(&args.request_id) {
        return invalid("Supply an exact target and stable request_id.");
    }
    let (tool, action) = match args.at {
        HoverTarget::Element { frame, element } => {
            if frame.is_empty() || element.is_empty() {
                return invalid("Use an exact observed frame and element.");
            }
            (
                "computer.act_checked",
                json!({"frame":frame,"element":element,"intent":"move_pointer"}),
            )
        }
        HoverTarget::View {
            view_id,
            x,
            y,
            red_marker_cluster,
        } => {
            if view_id.is_empty() || !x.is_finite() || !y.is_finite() {
                return invalid("Use an exact delivered view and finite view coordinates.");
            }
            let reference = call(
                "computer.resolve_view",
                view_resolution_args(&args.target, &view_id, x, y, red_marker_cluster),
                false,
            )
            .await;
            if reference.is_error {
                return reference;
            }
            let Some(reference) = reference.data["reference"].as_str() else {
                return DeviceResult::error(
                    "invalid_reference_reply",
                    "No reference returned; no input issued.",
                );
            };
            (
                "computer.act",
                json!({"reference":reference,"intent":"move_pointer"}),
            )
        }
    };
    let mut result = leased(&args.target, &args.request_id, tool, action, call).await;
    // These describe the contract's limits, not native goal/effect evidence.
    result.data["hover_effect"] = json!("unverified");
    result.data["motion_history"] = json!("unknown");
    result
}
pub(super) async fn scroll<F, Fut>(args: ScrollArgs, mut call: F) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    if !device::valid_id(&args.request_id)
        || args.delta_y == 0
        || args.delta_y.unsigned_abs() > 2000
    {
        return invalid(
            "Use a stable request_id and nonzero bounded native delta_y; platform limits still apply.",
        );
    }
    let value = args.delta_y.to_string();
    let (tool, action) = match args.at {
        ScrollTarget::Element { frame, element } => {
            if frame.is_empty() || element.is_empty() {
                return invalid("Use an exact observed frame and element.");
            }
            (
                "computer.act_checked",
                json!({"frame":frame,"element":element,"intent":"scroll","value":value}),
            )
        }
        ScrollTarget::View { view_id, x, y } => {
            if view_id.is_empty() || !x.is_finite() || !y.is_finite() {
                return invalid("Use an exact delivered view and finite view coordinates.");
            }
            let result = call(
                "computer.resolve_view",
                json!({"target":args.target,"view_id":view_id,"x":x,"y":y}),
                false,
            )
            .await;
            if result.is_error {
                return result;
            }
            let Some(reference) = result.data["reference"].as_str().filter(|r| !r.is_empty())
            else {
                return DeviceResult::error(
                    "invalid_reference_reply",
                    "No reference returned; no scroll sent.",
                );
            };
            (
                "computer.act",
                json!({"reference":reference,"intent":"scroll","value":value}),
            )
        }
    };
    leased(&args.target, &args.request_id, tool, action, call).await
}
pub(super) async fn reveal<F, Fut>(args: RevealArgs, mut call: F) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    if !device::valid_id(&args.request_id) || args.frame.is_empty() || args.element.is_empty() {
        return invalid("Supply a stable request_id and exact observed frame/element.");
    }
    // Never let an unsupported backend reinterpret reveal as a normal click.
    // Capability discovery is read-only and precedes the lease.
    let catalog = call("computer.targets", json!({}), false).await;
    if catalog.is_error {
        return catalog;
    }
    let advertised = catalog.data["targets"].as_array().is_some_and(|targets| {
        targets.iter().any(|entry| {
            entry["target"] == args.target
                && entry["capabilities"]["native_semantic_actions"]
                    .as_array()
                    .is_some_and(|actions| {
                        actions.iter().any(|action| action == "AXScrollToVisible")
                    })
        })
    });
    if !advertised {
        return DeviceResult::error(
            "capability_unavailable",
            "The exact target does not advertise AXScrollToVisible; no lease or input was issued.",
        );
    }
    let mut result = leased(
        &args.target,
        &args.request_id,
        "computer.act_checked",
        json!({"frame":args.frame,"element":args.element,"intent":"click","value":"AXScrollToVisible"}),
        call,
    )
    .await;
    result.data["operation"] = json!("reveal_to_visible");
    result.data["native_action"] = json!("AXScrollToVisible");
    result
}
