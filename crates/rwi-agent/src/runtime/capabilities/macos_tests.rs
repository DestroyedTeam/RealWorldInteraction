//! Pure device-catalog contracts; never constructs a native backend or reads UI.
use super::*;
#[test]
fn macos_catalog_never_equates_one_driver_with_every_tool() {
    for tool in [
        "computer.discover",
        "computer.bind_window",
        "computer.attach_window",
        "computer.detach_window",
        "computer.observe",
        "computer.capture_visual",
        "computer.live",
        "computer.act_checked",
        "computer.recover_input",
        "computer.move_window",
    ] {
        assert!(macos_tool_implemented(tool));
    }
    for tool in [
        "computer.activate_window",
        "computer.launch_application",
        "computer.lifecycle_preflight",
        "computer.lifecycle_receipt",
        "computer.future_unknown",
    ] {
        assert!(!macos_tool_implemented(tool));
    }
}
#[test]
fn macos_discovery_and_binding_do_not_require_tcc_or_selected_window() {
    let caps = json!({"permissions":{}});
    for tool in [
        "computer.discover",
        "computer.bind_window",
        "computer.attach_window",
        "computer.detach_window",
    ] {
        assert_eq!(macos_prerequisite(tool, &caps), None);
    }
    assert_eq!(
        macos_prerequisite("computer.observe", &caps),
        Some("accessibility_permission_missing")
    );
    assert_eq!(
        macos_prerequisite("computer.capture_visual", &caps),
        Some("screen_capture_permission_missing")
    );
}
#[test]
fn macos_permissions_are_independent_and_binding_replaces_the_foreground_gate() {
    let mut caps = json!({"permissions":{"accessibility":true,"post_event":true,"screen_capture":false,"screenshot_api":true,"listen_event":false}});
    assert_eq!(
        macos_prerequisite("computer.act", &caps),
        Some("selected_window_required")
    );
    assert_eq!(
        macos_prerequisite("computer.observe", &caps),
        Some("screen_capture_permission_missing")
    );
    assert_eq!(
        macos_prerequisite("computer.capture_visual", &caps),
        Some("screen_capture_permission_missing")
    );
    assert_eq!(
        macos_prerequisite("computer.recover_input", &caps),
        Some("reconciliation_unavailable")
    );
    caps["permissions"]["screen_capture"] = json!(true);
    caps["permissions"]["listen_event"] = json!(true);
    assert_eq!(macos_prerequisite("computer.recover_input", &caps), None);
    assert_eq!(
        macos_prerequisite("computer.observe", &caps),
        Some("selected_window_required")
    );
    assert_eq!(
        macos_prerequisite("computer.capture_visual", &caps),
        Some("selected_window_required")
    );
    caps["permissions"]["post_event"] = json!(false);
    assert_eq!(
        macos_prerequisite("computer.act_checked", &caps),
        Some("input_permission_missing")
    );
    assert_eq!(
        macos_prerequisite("computer.verify", &caps),
        Some("selected_window_required")
    );
}

#[test]
fn pointer_feedback_refresh_uses_same_identity_sample_without_replacing_binding() {
    let mut targets = json!([{"identity":{"session":"own"},"capabilities":{
        "pointer_feedback":{"visible":true},"window_selected":false,"capture":"exact"}}]);
    refresh_volatile_target_facts(
        &mut targets,
        &json!({"identity":{"session":"own"},
        "values":{"pointer_feedback":{"visible":false},"current_foreground":null,
                  "user_input_recent_seconds":0.0}}),
    );
    assert_eq!(
        targets[0]["capabilities"]["pointer_feedback"]["visible"],
        false
    );
    assert_eq!(targets[0]["capabilities"]["window_selected"], false);
    assert_eq!(targets[0]["capabilities"]["capture"], "exact");
}

#[test]
fn pointer_feedback_refresh_clears_stale_value_when_sample_is_unavailable() {
    let mut targets = json!([{"identity":{"session":"own"},
        "capabilities":{"pointer_feedback":{"visible":true}}}]);
    refresh_volatile_target_facts(
        &mut targets,
        &json!({"identity":{"session":"own"},"values":{}}),
    );
    assert!(targets[0]["capabilities"]["pointer_feedback"].is_null());
}

#[test]
fn pointer_feedback_refresh_never_copies_another_desktop_sample() {
    for identity in [json!({"session":"other"}), serde_json::Value::Null] {
        let mut targets = json!([{"identity":{"session":"own"},
            "capabilities":{"pointer_feedback":{"visible":true}}}]);
        refresh_volatile_target_facts(
            &mut targets,
            &json!({"identity":identity,"values":{"pointer_feedback":{"visible":false}}}),
        );
        assert!(targets[0]["capabilities"]["pointer_feedback"].is_null());
    }
}
