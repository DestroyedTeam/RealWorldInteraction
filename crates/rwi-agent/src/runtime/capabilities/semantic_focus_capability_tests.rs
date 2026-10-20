use super::*;
#[test]
fn semantic_focus_support_requires_actual_native_implementation() {
    for caps in [
        json!({}),
        json!({"focus_element":{"implemented":null}}),
        json!({"focus_element":{"implemented":false,"available":true}}),
    ] {
        assert!(!semantic_focus_native_implemented(&caps));
    }
    let actual = json!({"focus_element":{"implemented":true},"permissions":{"accessibility":true}});
    assert!(semantic_focus_native_implemented(&actual));
    assert!(macos_tool_implemented("computer.focus_element"));
    assert_eq!(
        macos_prerequisite("computer.focus_element", &actual),
        Some("selected_window_required")
    );
    assert_eq!(
        macos_prerequisite(
            "computer.focus_element",
            &json!({"focus_element":{"implemented":true}})
        ),
        Some("accessibility_permission_missing")
    );
}
#[test]
fn semantic_focus_unsupported_platform_is_not_advertised() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Principal {
        journal_identity: None,
        certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
        observe: true,
        input: true,
    };
    let runtime = Runtime::new(Config {
        version: 1,
        device_id: "semantic-focus-test".into(),
        bind: "127.0.0.1:0".parse().unwrap(),
        identity: rwi_link_trust::IdentityPaths::in_profile(dir.path()),
        principals: vec![policy.clone()],
        state_root: dir.path().join("state"),
        platform: PlatformPolicy::Unsupported {
            platform: "offline-test".into(),
        },
    })
    .unwrap();
    let result = runtime.describe(&policy);
    let cap = result.data["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == "computer.focus_element")
        .unwrap();
    assert_eq!(cap["implemented"], false);
    assert_eq!(cap["available"], false);
    assert_eq!(cap["authorized"], true);
}
