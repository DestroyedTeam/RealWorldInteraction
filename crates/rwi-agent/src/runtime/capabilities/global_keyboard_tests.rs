use super::*;
#[test]
fn global_hotkey_capability_is_desktop_scoped_and_keeps_native_permission_prerequisites() {
    assert!(macos_tool_implemented("computer.hotkey"));
    let caps = serde_json::json!({"permissions":{"accessibility":true,"post_event":true,"listen_event":true},
            "foreground_keyboard":{"implemented":true},"window_selected":false});
    assert_eq!(macos_prerequisite("computer.hotkey", &caps), None);
    for (permission, reason) in [
        ("accessibility", "accessibility_permission_missing"),
        ("post_event", "input_permission_missing"),
        ("listen_event", "listen_event_permission_missing"),
    ] {
        let mut missing = caps.clone();
        missing["permissions"][permission] = serde_json::json!(false);
        assert_eq!(
            macos_prerequisite("computer.hotkey", &missing),
            Some(reason)
        );
    }
    let mut attachment = caps;
    attachment["foreground_keyboard"]["implemented"] = serde_json::json!(false);
    assert_eq!(
        macos_prerequisite("computer.hotkey", &attachment),
        Some("desktop_global_keyboard_grant_required")
    );
}
