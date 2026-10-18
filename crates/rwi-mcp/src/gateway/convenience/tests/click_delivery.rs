use super::*;

#[test]
fn f235_zero_number_schema_and_strict_input_are_separate_from_plain_click() {
    let schema = input_schema::<ClickArgs>();
    assert!(
        schema
            .to_string()
            .contains("background_public_appkit_zero_number_experimental")
    );
    let value = json!({"target":"t","request_id":"zero-once","at":element(),"delivery":{"delivery":"background_public_appkit_zero_number_experimental"}});
    assert!(serde_json::from_value::<ClickArgs>(value).is_ok());
}

#[tokio::test]
async fn f235_zero_number_checks_specific_capability_before_one_pair_and_release() {
    for available in [false, true] {
        let args=serde_json::from_value(json!({"target":"t","request_id":"zero-once","at":element(),"delivery":{"delivery":"background_public_appkit_zero_number_experimental"}})).unwrap();
        let calls = RefCell::new(Vec::new());
        let result=click(args,Some("mac-host".into()),|tool,args,cleanup| {
            calls.borrow_mut().push((tool,args,cleanup));
            let mut result=reply(tool);
            if tool=="computer.targets" {
                result.data["targets"][0]["capabilities"]["pointer_click"]["background_public_appkit_zero_number_experimental"]=json!({"implemented":true,"available":available});
            }
            std::future::ready(result)
        }).await;
        let calls = calls.borrow();
        assert_eq!(result.is_error, !available);
        if available {
            let actions: Vec<_> = calls
                .iter()
                .filter(|x| x.0 == "computer.act_checked")
                .collect();
            assert_eq!(actions.len(), 1);
            assert_eq!(
                actions[0].1["value"],
                r#"{"delivery":"background_public_appkit_zero_number_experimental"}"#
            );
            assert_eq!(calls.last().unwrap().0, "computer.release");
            assert!(calls.last().unwrap().2);
            assert_eq!(
                result.data["requested_delivery"],
                "background_public_appkit_zero_number_experimental"
            );
            assert_eq!(result.data["goal_verified"], false);
        } else {
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, "computer.targets");
        }
    }
}

#[test]
fn r92_click_delivery_schema_is_explicit_and_null_is_not_omission() {
    let schema = input_schema::<ClickArgs>();
    assert!(schema["properties"].get("delivery").is_some());
    let raw = json!({"target":"t","request_id":"once","at":{"kind":"element","frame":"f","element":"e"},"delivery":{"delivery":"background_skylight_experimental"}});
    assert!(serde_json::from_value::<ClickArgs>(raw.clone()).is_ok());
    let mut null = raw;
    null["delivery"] = Value::Null;
    assert!(serde_json::from_value::<ClickArgs>(null).is_err());
}

fn arguments(at: Value, delivery: bool) -> ClickArgs {
    let mut args = json!({"target":"t","request_id":"once","at":at});
    if delivery {
        args["delivery"] = json!({"delivery":"background_skylight_experimental"});
    }
    serde_json::from_value(args).unwrap()
}
fn element() -> Value {
    json!({"kind":"element","frame":"f","element":"e"})
}
fn support() -> Value {
    json!({"devices":[{"device_id":"mac-host","is_error":false,"state":{
        "platform":"macos","executor_features":{"click_delivery_options":{"implemented":true}},
        "capabilities":[{"id":"computer.act","implemented":true,"authorized":true,"available":false},
                        {"id":"computer.act_checked","implemented":true,"authorized":true,"available":false}]
    }}],"targets":[{"target":"t","capabilities":{"window_selected":true,
        "pointer_click":{"background_skylight_experimental":{"implemented":true,"available":true}}
    }}]})
}
fn reply(tool: &str) -> DeviceResult {
    DeviceResult::value(match tool {
        "computer.targets" => support(),
        "computer.resolve_view" => json!({"reference":"exact-ref"}),
        "computer.claim" => json!({"lease":"lease"}),
        "computer.release" => json!({"released":true}),
        _ => json!({"dispatch":"returned","goal_verified":false}),
    })
}

#[tokio::test]
async fn r92_click_delivery_is_negotiated_before_one_checked_action_and_release() {
    for at in [element(), json!({"kind":"view","view_id":"v","x":4,"y":5})] {
        let calls = RefCell::new(Vec::new());
        let result = click(
            arguments(at, true),
            Some("mac-host".into()),
            |tool, args, cleanup| {
                calls.borrow_mut().push((tool, args, cleanup));
                std::future::ready(reply(tool))
            },
        )
        .await;
        assert!(!result.is_error);
        assert_eq!(
            result.data["requested_delivery"],
            "background_skylight_experimental"
        );
        assert_eq!(result.data["goal_verified"], false);
        let calls = calls.borrow();
        assert_eq!(calls[0].0, "computer.targets");
        assert_eq!(calls[0].1, json!({"device_id":"mac-host"}));
        let actions: Vec<_> = calls
            .iter()
            .filter(|row| matches!(row.0, "computer.act" | "computer.act_checked"))
            .collect();
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0].1["value"],
            r#"{"delivery":"background_skylight_experimental"}"#
        );
        assert_eq!(actions[0].1["intent"], "click");
        assert_eq!(actions[0].1["request_id"], "once");
        assert_eq!(calls.last().unwrap().0, "computer.release");
        assert!(calls.last().unwrap().2);
    }
}

#[tokio::test]
async fn r92_click_delivery_rejects_native_action_and_double_before_any_call() {
    for raw in [
        json!({"kind":"element","frame":"f","element":"e","action":"AXPress"}),
        element(),
    ] {
        let mut args = arguments(raw, true);
        if matches!(&args.at, ClickTarget::Element { action: None, .. }) {
            args.click_count = 2;
        }
        let calls = RefCell::new(Vec::new());
        let result = click(args, Some("mac-host".into()), |tool, _, _| {
            calls.borrow_mut().push(tool);
            std::future::ready(DeviceResult::error("unexpected_call", tool))
        })
        .await;
        assert_eq!(result.data["code"], "invalid_arguments");
        assert!(calls.borrow().is_empty());
    }
}

#[tokio::test]
async fn r92_click_delivery_cannot_inherit_unsupported_peer_or_another_target() {
    for (path, value) in [
        ("/devices/0/state/platform", json!("windows")),
        ("/devices/0/state/executor_features", json!({})),
        ("/devices/0/state/capabilities/1/authorized", json!(false)),
        ("/targets/0/target", json!("other")),
        ("/targets/0/capabilities/window_selected", json!(false)),
        ("/targets/0/capabilities/pointer_click", json!({})),
        (
            "/targets/0/capabilities/pointer_click/background_skylight_experimental/available",
            json!(false),
        ),
        (
            "/targets/0/capabilities/pointer_click/background_skylight_experimental/implemented",
            json!("true"),
        ),
    ] {
        let mut caps = support();
        *caps.pointer_mut(path).unwrap() = value;
        let calls = RefCell::new(0);
        let result = click(
            arguments(element(), true),
            Some("mac-host".into()),
            |tool, _, _| {
                assert_eq!(tool, "computer.targets");
                *calls.borrow_mut() += 1;
                std::future::ready(DeviceResult::value(caps.clone()))
            },
        )
        .await;
        assert_eq!(result.data["code"], "capability_unavailable", "{path}");
        assert_eq!(*calls.borrow(), 1);
    }
}

#[tokio::test]
async fn r92_click_delivery_preserves_peer_error_and_uncertain_action_without_retry() {
    for failed in ["computer.targets", "computer.act_checked"] {
        let calls = RefCell::new(Vec::new());
        let result = click(
            arguments(element(), true),
            Some("mac-host".into()),
            |tool, _, cleanup| {
                calls.borrow_mut().push((tool, cleanup));
                std::future::ready(if tool == failed {
                    DeviceResult::error("peer_schema_unsupported", "original failure")
                } else {
                    reply(tool)
                })
            },
        )
        .await;
        assert_eq!(result.data["code"], "peer_schema_unsupported");
        assert_eq!(result.data["message"], "original failure");
        assert_eq!(
            calls.borrow().iter().filter(|row| row.0 == failed).count(),
            1
        );
        assert_eq!(
            calls.borrow().last().unwrap().0,
            if failed == "computer.targets" {
                "computer.targets"
            } else {
                "computer.release"
            }
        );
    }
}

#[tokio::test]
async fn r92_click_delivery_omission_preserves_native_default_and_action_value() {
    for at in [
        element(),
        json!({"kind":"element","frame":"f","element":"e","action":"AXPress"}),
        json!({"kind":"view","view_id":"v","x":4,"y":5}),
    ] {
        let expected = at.get("action").cloned().unwrap_or(Value::Null);
        let view = at["kind"] == "view";
        let calls = RefCell::new(Vec::new());
        let result = click(arguments(at, false), None, |tool, args, cleanup| {
            assert_ne!(tool, "computer.targets");
            calls.borrow_mut().push((tool, args, cleanup));
            std::future::ready(reply(tool))
        })
        .await;
        assert!(result.data.get("requested_delivery").is_none());
        let calls = calls.borrow();
        let action = calls
            .iter()
            .find(|row| matches!(row.0, "computer.act" | "computer.act_checked"))
            .unwrap();
        if view {
            assert!(action.1.get("value").is_none());
        } else {
            assert_eq!(action.1["value"], expected);
        }
    }
}

#[tokio::test]
async fn r92_click_delivery_ambiguous_metadata_or_view_failure_never_claims() {
    for duplicate in ["devices", "targets", "tools"] {
        let mut caps = support();
        let rows = if duplicate == "tools" {
            caps.pointer_mut("/devices/0/state/capabilities").unwrap()
        } else {
            &mut caps[duplicate]
        };
        let extra = if duplicate == "tools" {
            rows[1].clone()
        } else {
            rows[0].clone()
        };
        rows.as_array_mut().unwrap().push(extra);
        let result = click(
            arguments(element(), true),
            Some("mac-host".into()),
            |tool, _, _| {
                assert_eq!(tool, "computer.targets");
                std::future::ready(DeviceResult::value(caps.clone()))
            },
        )
        .await;
        assert_eq!(result.data["code"], "capability_unavailable", "{duplicate}");
    }
    let calls = RefCell::new(Vec::new());
    let result = click(
        arguments(json!({"kind":"view","view_id":"v","x":4,"y":5}), true),
        Some("mac-host".into()),
        |tool, _, _| {
            calls.borrow_mut().push(tool);
            std::future::ready(if tool == "computer.targets" {
                reply(tool)
            } else {
                assert_eq!(tool, "computer.resolve_view");
                DeviceResult::error("stale_reference", "No new view")
            })
        },
    )
    .await;
    assert_eq!(result.data["code"], "stale_reference");
    assert_eq!(
        *calls.borrow(),
        ["computer.targets", "computer.resolve_view"]
    );
}

#[tokio::test]
async fn r92_click_delivery_nested_device_error_remains_the_original_failure() {
    let caps = json!({"devices":[{"device_id":"mac-host","is_error":true,"state":{
        "code":"paired_device_offline","message":"original native failure","goal_verified":false
    }}],"targets":[]});
    let result = click(
        arguments(element(), true),
        Some("mac-host".into()),
        |tool, _, _| {
            assert_eq!(tool, "computer.targets");
            std::future::ready(DeviceResult::value(caps.clone()))
        },
    )
    .await;
    assert!(result.is_error);
    assert_eq!(result.data, caps["devices"][0]["state"]);
}

#[test]
fn r92_click_delivery_generic_value_and_compact_request_identity_survive() {
    let value = r#"{"delivery":"background_skylight_experimental"}"#;
    let raw = json!({"target":"t","reference":"r","lease":"l","request_id":"once","intent":"click","value":value});
    device::catalog::validate("computer.act", &raw).unwrap();
    let args: device::api::ActArgs = serde_json::from_value(raw).unwrap();
    assert_eq!(args.value.as_deref(), Some(value));
    assert!(matches!(args.intent, rwi_core::Intent::Click));
    let projected = compact_action_reply(
        &json!({"dispatch":"returned","requested_delivery":"background_skylight_experimental","goal_verified":true,"system_accepted":null}),
        None,
    );
    assert_eq!(
        projected["requested_delivery"],
        "background_skylight_experimental"
    );
    assert_eq!(projected["goal_verified"], false);
    assert!(projected.get("system_accepted").unwrap().is_null());
}

#[tokio::test]
async fn r98_appkit_click_only_uses_its_own_capability_and_exact_json_value() {
    let encoded = r#"{"delivery":"background_skylight_appkit_experimental"}"#;
    let mut arguments = arguments(element(), false);
    arguments.delivery = rwi_core::ClickDeliveryOptions::parse(Some(encoded)).unwrap();
    for supported in [false, true] {
        let calls = RefCell::new(Vec::new());
        let mut caps = support();
        if supported {
            caps["targets"][0]["capabilities"]["pointer_click"]["background_skylight_appkit_experimental"] =
                json!({"implemented":true,"available":true});
        }
        let result = click(
            arguments_for_appkit(&arguments),
            Some("mac-host".into()),
            |tool, args, cleanup| {
                calls.borrow_mut().push((tool, args, cleanup));
                std::future::ready(if tool == "computer.targets" {
                    DeviceResult::value(caps.clone())
                } else {
                    reply(tool)
                })
            },
        )
        .await;
        let calls = calls.borrow();
        if !supported {
            assert!(result.is_error);
            assert_eq!(result.data["code"], "capability_unavailable");
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, "computer.targets");
        } else {
            assert!(!result.is_error);
            assert_eq!(
                result.data["requested_delivery"],
                "background_skylight_appkit_experimental"
            );
            assert_eq!(result.data["goal_verified"], false);
            let actions: Vec<_> = calls
                .iter()
                .filter(|c| c.0 == "computer.act_checked")
                .collect();
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].1["value"], encoded);
            assert_eq!(actions[0].1["intent"], "click");
        }
    }
    let raw = json!({"target":"t","reference":"r","lease":"l","request_id":"once","intent":"click","value":encoded});
    device::catalog::validate("computer.act", &raw).unwrap();
    let parsed: device::api::ActArgs = serde_json::from_value(raw).unwrap();
    assert_eq!(parsed.value.as_deref(), Some(encoded));
}
fn arguments_for_appkit(a: &ClickArgs) -> ClickArgs {
    let mut clone = arguments(element(), false);
    clone.delivery = a.delivery.clone();
    clone
}

#[test]
fn public_appkit_click_schema_is_explicit_and_uses_the_existing_prelease_policy() {
    let a: super::super::ClickArgs = serde_json::from_value(serde_json::json!({"target":"mac:exact","request_id":"new-public-pair","at":{"kind":"view","view_id":"fresh-view","x":1.0,"y":2.0},"delivery":{"delivery":"background_public_appkit_experimental"}})).unwrap();
    assert_eq!(
        super::super::click::validate_click_delivery(&a).unwrap(),
        "click"
    );
    assert_eq!(
        a.delivery.unwrap().delivery_name(),
        "background_public_appkit_experimental"
    );
    let schema = serde_json::to_string(&schemars::schema_for!(super::super::ClickArgs)).unwrap();
    assert!(schema.contains("background_public_appkit_experimental"));
}

#[tokio::test]
async fn f188_touch_click_old_peer_refuses_before_claim_and_new_peer_receives_exact_option() {
    for available in [false, true] {
        let mut cap = support();
        cap["targets"][0]["capabilities"]["pointer_click"] =
            json!({"background_public_appkit_experimental":{"implemented":true,"available":true}});
        if available {
            cap["targets"][0]["capabilities"]["pointer_click"]["background_public_appkit_experimental"]
                ["mouse_subtypes"] = json!({"touch":{"implemented":true,"available":true}});
        }
        let mut a = arguments(element(), false);
        a.delivery = serde_json::from_value(
            json!({"delivery":"background_public_appkit_experimental","mouse_subtype":"touch"}),
        )
        .unwrap();
        let calls = RefCell::new(Vec::new());
        let result = click(a, Some("mac-host".into()), |tool, args, cleanup| {
            calls.borrow_mut().push((tool, args, cleanup));
            std::future::ready(if tool == "computer.targets" {
                DeviceResult::value(cap.clone())
            } else {
                reply(tool)
            })
        })
        .await;
        let calls = calls.borrow();
        if !available {
            assert!(result.is_error);
            assert_eq!(result.data["code"], "capability_unavailable");
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].0, "computer.targets");
        } else {
            assert!(!result.is_error);
            assert_eq!(result.data["requested_mouse_subtype"], "touch");
            assert_eq!(result.data["goal_verified"], false);
            assert_eq!(
                calls.iter().map(|r| r.0).collect::<Vec<_>>(),
                [
                    "computer.targets",
                    "computer.claim",
                    "computer.act_checked",
                    "computer.release"
                ]
            );
            let value: Value = serde_json::from_str(calls[2].1["value"].as_str().unwrap()).unwrap();
            assert_eq!(
                value,
                json!({"delivery":"background_public_appkit_experimental","mouse_subtype":"touch"})
            );
            assert!(calls[3].2);
        }
    }
}
#[tokio::test]
async fn f188_touch_click_schema_and_action_exclusion_remain_strict() {
    let schema = serde_json::to_string(&input_schema::<ClickArgs>()).unwrap();
    assert!(schema.contains("mouse_subtype") && schema.contains("touch"));
    for at in [
        element(),
        json!({"kind":"element","frame":"f","element":"e","action":"AXPress"}),
    ] {
        let mut a = arguments(at, false);
        a.delivery = serde_json::from_value(
            json!({"delivery":"background_public_appkit_experimental","mouse_subtype":"touch"}),
        )
        .unwrap();
        if matches!(&a.at, ClickTarget::Element { action: None, .. }) {
            a.click_count = 2;
        }
        let calls = RefCell::new(0);
        let result = click(a, Some("mac-host".into()), |_, _, _| {
            *calls.borrow_mut() += 1;
            std::future::ready(DeviceResult::value(json!({})))
        })
        .await;
        assert!(result.is_error);
        assert_eq!(*calls.borrow(), 0);
    }
}
