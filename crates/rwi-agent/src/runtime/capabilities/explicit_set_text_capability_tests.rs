#[test]
fn explicit_set_text_capability_requires_native_implementation_and_existing_ax() {
    assert!(super::macos_tool_implemented("computer.set_text"));
    let no = serde_json::json!({"permissions":{"accessibility":false}});
    assert_eq!(
        super::macos_prerequisite("computer.set_text", &no),
        Some("accessibility_permission_missing")
    );
    let yes = serde_json::json!({"permissions":{"accessibility":true}});
    assert_eq!(
        super::macos_prerequisite("computer.set_text", &yes),
        Some("selected_window_required")
    );
}
