use super::*;

fn args() -> DragBetweenArgs {
    serde_json::from_value(json!({
        "source":{"target":"source","view_id":"sv","point":[10.5,20]},
        "destination":{"target":"dest","view_id":"dv","point":[80,50]},
        "request_id":"paired-once","duration_ms":1200
    }))
    .unwrap()
}

#[test]
fn paired_device_preflight_refuses_cross_device_or_foreign_view_before_rpc() {
    let g = Gateway::new(Ok(AdapterProfile::default()));
    g.remember("mac", &json!({"target":"source","view_id":"sv"}));
    g.remember("windows", &json!({"target":"dest","view_id":"dv"}));
    assert_eq!(
        g.paired_device(&args()).unwrap_err().data["code"],
        "drag_domain_mismatch"
    );
    let mut a = args();
    a.source.view_id = "dv".into();
    assert_eq!(
        g.paired_device(&a).unwrap_err().data["code"],
        "foreign_resource"
    );
    let g = Gateway::new(Ok(AdapterProfile::default()));
    g.remember("mac", &json!({"target":"source","view_id":"sv"}));
    g.remember("mac", &json!({"target":"dest","view_id":"dv"}));
    assert_eq!(g.paired_device(&args()).unwrap(), "mac");
}

#[test]
fn paired_schema_is_typed_strict_and_does_not_widen_ordinary_drag() {
    let catalog = tools();
    let tool = catalog
        .iter()
        .find(|t| t.name == "computer.drag_between")
        .unwrap();
    assert!(tool.title.is_some() && handles("computer.drag_between"));
    assert!(
        tool.description
            .as_ref()
            .unwrap()
            .contains("Receipt is not a drop goal")
    );
    assert!(
        input_schema::<DragBetweenArgs>()["properties"]
            .get("source")
            .is_some()
    );
    assert!(
        input_schema::<DragArgs>()["properties"]
            .get("destination")
            .is_none()
    );
    for extra in ["activate", "device_id", "clipboard", "button", "fallback"] {
        let mut v = json!({"source":{"target":"s","view_id":"v","point":[1,2]},"destination":{"target":"d","view_id":"dv","point":[3,4]},"request_id":"r"});
        v[extra] = json!(true);
        assert!(serde_json::from_value::<DragBetweenArgs>(v).is_err());
    }
}

#[tokio::test]
async fn paired_drag_resolves_two_native_views_then_uses_one_shared_lease_and_one_action() {
    let calls = RefCell::new(vec![]);
    let r = paired_drag(args(), |tool, a, cleanup| {
        let result = match tool {
            "computer.resolve_view" if a["target"] == "source" => {
                DeviceResult::value(json!({"reference":"sr","point":[-100.25,150.5]}))
            }
            "computer.resolve_view" => {
                DeviceResult::value(json!({"reference":"dr","point":[2100.5,705.25]}))
            }
            "computer.claim" => DeviceResult::value(json!({"lease":"shared"})),
            "computer.release" => DeviceResult::value(json!({"released":true})),
            _ => DeviceResult::value(json!({"dispatch":"returned","goal_verified":false})),
        };
        calls.borrow_mut().push((tool, a, cleanup));
        std::future::ready(result)
    })
    .await;
    assert!(!r.is_error && r.data["goal_verified"] == false);
    let c = calls.borrow();
    assert_eq!(
        c.iter().map(|c| c.0).collect::<Vec<_>>(),
        [
            "computer.resolve_view",
            "computer.resolve_view",
            "computer.claim",
            "computer.act",
            "computer.release"
        ]
    );
    assert_eq!(c[0].1["view_id"], "sv");
    assert_eq!(c[1].1["view_id"], "dv");
    assert_eq!(c[2].1["target"], "source");
    assert_eq!(c[3].1["reference"], "sr");
    assert_eq!(c[3].1["intent"]["drag_between"]["reference"], "dr");
    assert_eq!(c[3].1["intent"]["drag_between"]["target"], "dest");
    assert_eq!(
        c[3].1["intent"]["drag_between"]["path"]["waypoints"][1]["offset"],
        json!([2200.75, 554.75])
    );
    assert_eq!(c[3].1["request_id"], "paired-once");
    assert!(c[4].2);
}

#[tokio::test]
async fn either_endpoint_refusal_or_malformed_mapping_never_claims_or_inputs() {
    for which in 0..4 {
        let calls = RefCell::new(vec![]);
        let r = paired_drag(args(), |tool, a, _| {
            assert_eq!(tool, "computer.resolve_view");
            let i = calls.borrow().len();
            calls.borrow_mut().push(a);
            std::future::ready(if (which == 0 && i == 0) || (which == 1 && i == 1) {
                DeviceResult::error("stale_reference", "Endpoint refused")
            } else if i == 1 && which == 2 {
                DeviceResult::value(json!({"reference":"dr","point":[1]}))
            } else if i == 1 && which == 3 {
                DeviceResult::value(json!({"point":[1,2]}))
            } else {
                DeviceResult::value(json!({"reference":"ref","point":[1,2]}))
            })
        })
        .await;
        assert!(r.is_error);
        assert_eq!(calls.borrow().len(), if which == 0 { 1 } else { 2 });
    }
}

#[tokio::test]
async fn paired_native_uncertainty_and_release_failure_preserve_no_replay() {
    for fail_release in [false, true] {
        let calls = RefCell::new(vec![]);
        let r = paired_drag(args(), |tool, a, cleanup| {
            calls.borrow_mut().push((tool, a, cleanup));
            std::future::ready(match tool {
                "computer.resolve_view" => {
                    DeviceResult::value(json!({"reference":"ref","point":[1,2]}))
                }
                "computer.claim" => DeviceResult::value(json!({"lease":"shared"})),
                "computer.release" if fail_release => {
                    DeviceResult::error("transport_unavailable", "No release confirmation")
                }
                "computer.release" => DeviceResult::value(json!({"released":true})),
                _ => DeviceResult::error("native_uncertain", "Never replay"),
            })
        })
        .await;
        assert!(r.is_error);
        assert_eq!(
            calls
                .borrow()
                .iter()
                .filter(|c| c.0 == "computer.act")
                .count(),
            1
        );
        assert!(calls.borrow().last().unwrap().2);
        if fail_release {
            assert_eq!(r.data["code"], "input_cleanup_unconfirmed");
            assert_eq!(r.data["action"]["code"], "native_uncertain");
            assert_eq!(r.data["replay_allowed"], false);
        } else {
            assert_eq!(r.data["code"], "native_uncertain");
        }
    }
}

#[tokio::test]
async fn invalid_endpoint_schema_values_never_call_device() {
    for which in 0..7 {
        let mut a = args();
        match which {
            0 => a.destination.target = a.source.target.clone(),
            1 => a.destination.view_id.clear(),
            2 => a.source.target.clear(),
            3 => a.duration_ms = 5001,
            4 => a.request_id.clear(),
            5 => a.destination.point = [f64::NAN, 0.],
            _ => a.duration_ms = 15,
        }
        let r = paired_drag(a, |_, _, _| {
            std::future::ready({
                panic!("Invalid endpoints cannot call a device");
                #[allow(unreachable_code)]
                DeviceResult::value(json!({}))
            })
        })
        .await;
        assert_eq!(r.data["code"], "invalid_arguments");
    }
}
