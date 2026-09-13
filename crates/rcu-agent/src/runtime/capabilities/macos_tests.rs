//! Pure device-catalog contracts; never constructs a native backend or reads UI.
use super::*;
#[test]
fn macos_catalog_never_equates_one_driver_with_every_tool() {
    for tool in [
        "computer.discover",
        "computer.bind_window",
        "computer.observe",
        "computer.act_checked",
        "computer.recover_input",
    ] {
        assert!(macos_tool_implemented(tool));
    }
    for tool in [
        "computer.activate_window",
        "computer.launch_application",
        "computer.capture_visual",
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
    for tool in ["computer.discover", "computer.bind_window"] {
        assert_eq!(macos_prerequisite(tool, &caps), None);
    }
    assert_eq!(
        macos_prerequisite("computer.observe", &caps),
        Some("accessibility_permission_missing")
    );
}
#[test]
fn macos_permissions_are_independent_and_listening_is_only_recovery_proof() {
    let mut caps = json!({"permissions":{"accessibility":true,"post_event":true,"screen_capture":false,"screenshot_api":true,"listen_event":false}});
    assert_eq!(
        macos_prerequisite("computer.act", &caps),
        Some("selected_foreground_window_required")
    );
    assert_eq!(
        macos_prerequisite("computer.observe", &caps),
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
        Some("selected_foreground_window_required")
    );
    caps["permissions"]["post_event"] = json!(false);
    assert_eq!(
        macos_prerequisite("computer.act_checked", &caps),
        Some("input_permission_missing")
    );
    assert_eq!(
        macos_prerequisite("computer.verify", &caps),
        Some("selected_foreground_window_required")
    );
}
