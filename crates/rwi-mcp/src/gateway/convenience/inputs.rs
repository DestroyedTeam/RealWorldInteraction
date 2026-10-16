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
    // Validate before view resolution or lease acquisition. Omission stays absent,
    // unlike explicit background_window; core owns all delivery/tuple/delta rules.
    let value = match args.options {
        None => None,
        Some(options) => {
            let encoded = match serde_json::to_string(&options) {
                Ok(value) => value,
                Err(_) => {
                    return DeviceResult::error("encoding_failed", "No input options encoded.");
                }
            };
            if let Err(error) = rwi_core::MovePointerOptions::parse(Some(&encoded)) {
                return DeviceResult::error(&error.code, &error.next_step);
            }
            Some(encoded)
        }
    };
    let (tool, mut action) = match args.at {
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
    if let Some(value) = value {
        action["value"] = json!(value);
    }
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

#[cfg(test)]
mod typed_hover_options_tests {
    use super::*;
    use std::cell::RefCell;
    async fn f05_typed_hover_common_case(
        input: Value,
        native: Value,
        native_error: bool,
        cleanup_error: bool,
    ) -> Value {
        let Ok(args) = serde_json::from_value::<HoverArgs>(input) else {
            return json!({"parsed":false,"calls":[]});
        };
        let calls = RefCell::new(vec![]);
        let result = hover(args, |tool, args, cleanup| {
            calls
                .borrow_mut()
                .push(json!({"tool":tool,"args":args,"cleanup":cleanup}));
            std::future::ready(match tool {
                "computer.resolve_view" => DeviceResult::value(json!({"reference":"exact-ref"})),
                "computer.claim" => DeviceResult::value(json!({"lease":"short-lease"})),
                "computer.release" if cleanup_error => {
                    DeviceResult::error("fixture_cleanup_error", "cleanup remains unconfirmed")
                }
                "computer.release" => DeviceResult::value(json!({"released":true})),
                _ => DeviceResult {
                    is_error: native_error,
                    data: native.clone(),
                    images: vec![],
                },
            })
        })
        .await;
        json!({"parsed":true,"is_error":result.is_error,"result":result.data,
        "compactable":compactable_receipt(&result),"calls":calls.into_inner()})
    }

    fn f05_typed_hover_one_action(case: &Value, at: &Value, value: Option<&Value>) -> bool {
        let Some(calls) = case["calls"].as_array() else {
            return false;
        };
        let view = at["kind"] == "view";
        let expected = if view {
            vec![
                "computer.resolve_view",
                "computer.claim",
                "computer.act",
                "computer.release",
            ]
        } else {
            vec!["computer.claim", "computer.act_checked", "computer.release"]
        };
        let names: Vec<_> = calls
            .iter()
            .filter_map(|call| call["tool"].as_str())
            .collect();
        let Some(action) = calls.iter().find(|call| {
            matches!(
                call["tool"].as_str(),
                Some("computer.act" | "computer.act_checked")
            )
        }) else {
            return false;
        };
        let supplied = action["args"].get("value");
        let value_equal = match value {
            None => supplied.is_none(),
            Some(value) => {
                supplied
                    .and_then(Value::as_str)
                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                    .as_ref()
                    == Some(value)
            }
        };
        case["parsed"] == true
            && names == expected
            && value_equal
            && action["args"]["intent"] == "move_pointer"
            && action["args"]["target"] == "t"
            && action["args"]["request_id"] == "hover-options-once"
            && action["args"]["lease"] == "short-lease"
            && action["args"].get("action").is_none()
            && action["args"].get("button").is_none()
            && calls.last().is_some_and(|c| c["cleanup"] == true)
            && calls
                .iter()
                .filter(|c| c["tool"] == "computer.claim")
                .all(|c| c["args"]["ttl_ms"] == 10000 && c["cleanup"] == false)
            && case["result"]["hover_effect"] == "unverified"
            && case["result"]["motion_history"] == "unknown"
    }

    fn f05_typed_hover_schema_has_no_null_option_defaults(value: &Value) -> bool {
        match value {
            Value::Object(object) => {
                let own = object
                    .get("properties")
                    .and_then(Value::as_object)
                    .is_none_or(|fields| {
                        ["options", "delta"].iter().all(|name| {
                            fields
                                .get(*name)
                                .is_none_or(|field| field.get("default") != Some(&Value::Null))
                        })
                    });
                own && object
                    .values()
                    .all(f05_typed_hover_schema_has_no_null_option_defaults)
            }
            Value::Array(items) => items
                .iter()
                .all(f05_typed_hover_schema_has_no_null_option_defaults),
            _ => true,
        }
    }

    async fn f05_typed_hover_common_receipt() -> Value {
        let endpoints = [
            json!({"kind":"element","frame":"f","element":"e"}),
            json!({"kind":"view","view_id":"v","x":4,"y":5}),
        ];
        let returned = json!({"dispatch":"returned","goal_verified":false});
        let mut default_ok = true;
        for at in &endpoints {
            let input = json!({"target":"t","request_id":"hover-options-once","at":at});
            let case = f05_typed_hover_common_case(input, returned.clone(), false, false).await;
            default_ok &= f05_typed_hover_one_action(&case, at, None) && case["is_error"] == false;
        }
        let options = [
            json!({"delivery":"background_window"}),
            json!({"delivery":"background_skylight_experimental"}),
            json!({"delivery":"background_appkit_experimental"}),
            json!({"delivery":"background_appkit_experimental","delta":[17,-9]}),
            json!({"delivery":"background_appkit_experimental","delta":[-128,128]}),
            json!({"delivery":"foreground_system","expected_foreground":{
            "pid":42,"window":"cgwindow:7","process_start":"42:1:2","native_epoch":"a".repeat(64)}}),
        ];
        let mut forwarded = true;
        for at in &endpoints {
            for option in &options {
                let input = json!({"target":"t","request_id":"hover-options-once","at":at,"options":option});
                let case = f05_typed_hover_common_case(input, returned.clone(), false, false).await;
                forwarded &= f05_typed_hover_one_action(&case, at, Some(option))
                    && case["is_error"] == false;
            }
        }
        let illegal = [
            Value::Null,
            json!({}),
            json!({"delivery":"unknown"}),
            json!({"delivery":"background_window","delta":[17,-9]}),
            json!({"delivery":"background_skylight_experimental","delta":[17,-9]}),
            json!({"delivery":"background_appkit_experimental","delta":null}),
            json!({"delivery":"background_appkit_experimental","delta":[0,0]}),
            json!({"delivery":"background_appkit_experimental","delta":[129,0]}),
            json!({"delivery":"background_appkit_experimental","delta":[0,-129]}),
            json!({"delivery":"background_appkit_experimental","delta":[17.5,-9]}),
            json!({"delivery":"background_appkit_experimental","delta":[17]}),
            json!({"delivery":"background_appkit_experimental","delta":[17,-9,1]}),
            json!({"delivery":"background_appkit_experimental","activate":true}),
            json!({"delivery":"foreground_system"}),
            json!({"delivery":"foreground_system","expected_foreground":{
            "pid":0,"window":"cgwindow:7","process_start":"42:1:2","native_epoch":"a".repeat(64)}}),
        ];
        let mut illegal_ok = true;
        for at in &endpoints {
            for option in &illegal {
                let input = json!({"target":"t","request_id":"hover-options-once","at":at,"options":option});
                let case = f05_typed_hover_common_case(input, returned.clone(), false, false).await;
                illegal_ok &= case["calls"].as_array().is_some_and(Vec::is_empty)
                    && (case["parsed"] == false || case["is_error"] == true);
            }
        }
        let explicit = json!({"delivery":"background_appkit_experimental","delta":[17,-9]});
        let refusal = json!({"code":"capability_unavailable","message":"Windows pointer move route unsupported",
        "diagnostic":{"platform":"windows","route":"background_appkit_experimental"},
        "system_accepted":null,"native_dispatch_entered":false,"goal_verified":false,
        "operation":{"terminal":true,"native_call_pending":false,"resources_released":true,"resource_release_uncertain":false}});
        let uncertain = json!({"dispatch":"returned","dispatch_error":{"code":"native_outcome_unknown"},
        "system_accepted":null,"input_domain_fenced":true,"goal_verified":false,
        "operation":{"terminal":false,"native_call_pending":true,"resources_released":false,"resource_release_uncertain":true}});
        let mut failures_ok = true;
        for at in &endpoints {
            for (native, error) in [(&refusal, true), (&uncertain, false)] {
                let input = json!({"target":"t","request_id":"hover-options-once","at":at,"options":explicit});
                let case = f05_typed_hover_common_case(input, native.clone(), error, false).await;
                let mut expected = native.clone();
                expected["hover_effect"] = json!("unverified");
                expected["motion_history"] = json!("unknown");
                failures_ok &= f05_typed_hover_one_action(&case, at, Some(&explicit))
                    && case["result"] == expected
                    && case["is_error"] == error
                    && case["compactable"] == false;
            }
            let input =
                json!({"target":"t","request_id":"hover-options-once","at":at,"options":explicit});
            let case = f05_typed_hover_common_case(input, uncertain.clone(), false, true).await;
            failures_ok &= f05_typed_hover_one_action(&case, at, Some(&explicit))
                && case["is_error"] == true
                && case["result"]["code"] == "input_cleanup_unconfirmed"
                && case["result"]["action"] == uncertain
                && case["result"]["cleanup"]["code"] == "fixture_cleanup_error"
                && case["result"]["replay_allowed"] == false
                && case["result"]["goal_verified"] == false;
        }
        let tool = tools()
            .into_iter()
            .find(|t| t.name == "computer.hover")
            .unwrap();
        let schema = serde_json::to_value(tool.input_schema).unwrap();
        let encoded = schema.to_string();
        let schema_ok = f05_typed_hover_schema_has_no_null_option_defaults(&schema)
            && schema["properties"].get("options").is_some()
            && schema["required"]
                .as_array()
                .is_some_and(|r| !r.contains(&json!("options")))
            && [
                "background_window",
                "background_skylight_experimental",
                "background_appkit_experimental",
                "foreground_system",
                "expected_foreground",
                "delta",
                "minimum",
                "maximum",
            ]
            .iter()
            .all(|name| encoded.contains(*name))
            && !schema["properties"]
                .as_object()
                .unwrap()
                .contains_key("value")
            && tool
                .description
                .as_ref()
                .unwrap()
                .contains("explicit foreground_system")
            && tool
                .description
                .as_ref()
                .unwrap()
                .contains("not tooltip success");
        json!({"missing_and_legacy_defaults_unchanged":default_ok,
        "explicit_options_accepted_and_forwarded":forwarded,
        "illegal_inputs_rejected":illegal_ok,"refusal_uncertainty_and_cleanup_preserved":failures_ok,
        "schema_explicit_optional_nonsemantic_options":schema_ok,
        "valid_options_cases":options.len()*endpoints.len(),"invalid_options_cases":illegal.len()*endpoints.len(),
        "native_getter_delivery_or_tracking_observed":false,"post_calls":0})
    }

    #[tokio::test]
    async fn typed_hover_options_preserve_defaults_forward_routes_and_refusals() {
        let receipt = f05_typed_hover_common_receipt().await;
        for key in [
            "explicit_options_accepted_and_forwarded",
            "refusal_uncertainty_and_cleanup_preserved",
            "schema_explicit_optional_nonsemantic_options",
        ] {
            assert_eq!(receipt[key], true, "{key}: {receipt}");
        }
        assert_eq!(receipt["missing_and_legacy_defaults_unchanged"], true);
        assert_eq!(receipt["illegal_inputs_rejected"], true);
        assert_eq!(
            receipt["native_getter_delivery_or_tracking_observed"],
            false
        );
    }
}
