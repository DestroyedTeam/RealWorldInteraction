use super::*;
fn exact(kind: &str) -> DeviceResult {
    DeviceResult::value(
        json!({"receipt":{"dispatch":"uncertain","replay":"forbidden","dispatch_error":{"code":"window_local_selection_unconfirmed","diagnostic":{"version":1,"operation_kind":"window_local_selection","kind":kind,"native_window_role":"AXWindow","api":"AXUIElementSetAttributeValue","attribute":if kind=="main"{"AXMain"}else{"AXFocused"},"native_setter_attempted":true,"native_setter_attempts":1,"native_status":-25202,"native_dispatch_counter_known":true,"native_dispatch_count_before":4,"native_dispatch_count_after":5,"native_call_pending":false,"native_resources_released":true,"held_input_resources":false,"replay_allowed":false}}}}),
    )
}
fn fence(r: &DeviceResult, release: bool, pending: bool) -> bool {
    needs_execution_input_fence(
        "computer.select_window_local",
        true,
        true,
        release,
        pending,
        r,
    )
}
#[test]
fn window_local_no_hid_exception_requires_exact_kind_window_and_one_setter_counter() {
    for kind in ["main", "key"] {
        let r = exact(kind);
        assert!(!fence(&r, true, false));
        assert!(fence(&r, false, false));
        assert!(fence(&r, true, true));
        for key in [
            "version",
            "operation_kind",
            "kind",
            "native_window_role",
            "api",
            "attribute",
            "native_setter_attempted",
            "native_setter_attempts",
            "native_status",
            "native_dispatch_counter_known",
            "native_dispatch_count_before",
            "native_dispatch_count_after",
            "native_call_pending",
            "native_resources_released",
            "held_input_resources",
            "replay_allowed",
        ] {
            let mut bad = r.clone();
            bad.data["receipt"]["dispatch_error"]["diagnostic"]
                .as_object_mut()
                .unwrap()
                .remove(key);
            assert!(fence(&bad, true, false), "{kind} missing {key}");
        }
        for (key, value) in [
            ("native_setter_attempts", json!(2)),
            ("native_window_role", json!("AXTextField")),
            ("kind", json!("focused")),
            ("native_dispatch_count_before", json!(u64::MAX)),
            ("native_dispatch_count_after", json!(6)),
            ("native_call_pending", json!(true)),
            ("native_resources_released", json!(false)),
            ("held_input_resources", json!(true)),
            ("replay_allowed", json!(true)),
        ] {
            let mut bad = r.clone();
            bad.data["receipt"]["dispatch_error"]["diagnostic"][key] = value;
            assert!(fence(&bad, true, false), "{kind} {key}");
        }
        let mut wrong = r.clone();
        wrong.data["receipt"]["dispatch_error"]["diagnostic"]["attribute"] =
            json!(if kind == "main" {
                "AXFocused"
            } else {
                "AXMain"
            });
        assert!(fence(&wrong, true, false));
        assert!(
            needs_execution_input_fence("computer.focus_element", true, true, true, false, &r),
            "window proof must not exempt original element-focus route"
        );
    }
}
#[test]
fn window_local_nested_unknown_or_cleanup_fences_while_returned_partial_has_no_held_input() {
    let unknown =
        DeviceResult::value(json!({"receipt":{"dispatch":"uncertain","replay":"forbidden"}}));
    assert!(fence(&unknown, true, false));
    let returned = DeviceResult::value(
        json!({"receipt":{"dispatch":"returned","replay":"forbidden","goal_verified":false},"window_readback":{"state":"partial","typing_authorized":false}}),
    );
    assert!(!fence(&returned, true, false));
    assert!(fence(&returned, false, false));
    assert!(fence(&returned, true, true));
    let mut cleanup = exact("main");
    cleanup.data["cleanup"] = json!({"resources_released":null});
    assert!(fence(&cleanup, true, false));
    let mut release = exact("key");
    release.data["resource_release_uncertain"] = json!(true);
    assert!(fence(&release, true, false));
}
