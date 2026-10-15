use super::*;
#[test]
fn window_local_requires_actual_native_support_and_accessibility() {
    for caps in [
        json!({}),
        json!({"select_window_local":{"implemented":null}}),
        json!({"select_window_local":{"implemented":false,"available":true}}),
    ] {
        assert!(!window_local_native_implemented(&caps));
    }
    let caps =
        json!({"select_window_local":{"implemented":true},"permissions":{"accessibility":true}});
    assert!(window_local_native_implemented(&caps));
    assert!(macos_tool_implemented("computer.select_window_local"));
    assert_eq!(
        macos_prerequisite("computer.select_window_local", &caps),
        Some("selected_window_required")
    );
    assert_eq!(
        macos_prerequisite(
            "computer.select_window_local",
            &json!({"select_window_local":{"implemented":true}})
        ),
        Some("accessibility_permission_missing")
    );
}
#[test]
fn window_local_unsupported_platform_is_not_advertised_as_available() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Principal {
        journal_identity: None,
        certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
        observe: true,
        input: true,
    };
    let runtime = Runtime::new(Config {
        version: 1,
        device_id: "window-local-test".into(),
        bind: "127.0.0.1:0".parse().unwrap(),
        identity: rwi_link_trust::IdentityPaths::in_profile(dir.path()),
        principals: vec![policy.clone()],
        state_root: dir.path().join("state"),
        platform: PlatformPolicy::Unsupported {
            platform: "offline-test".into(),
        },
    })
    .unwrap();
    let d = runtime.describe(&policy);
    let cap = d.data["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["id"] == "computer.select_window_local")
        .unwrap();
    assert_eq!(cap["implemented"], false);
    assert_eq!(cap["available"], false);
    assert_eq!(cap["authorized"], true);
}
