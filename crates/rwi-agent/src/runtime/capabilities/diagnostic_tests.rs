#[test]
fn volatile_desktop_facts_replace_only_same_identity_and_keep_owner_binding() {
    let identity = serde_json::json!({"host":"own","session":"same"});
    let foreground = serde_json::json!({"pid":9,"window":"cgwindow:99",
            "process_start":"new-process","native_epoch":"new-window"});
    let mut targets = serde_json::json!([
        {"target":"owned","identity":identity,"capabilities":{
            "current_foreground":{"pid":7},"user_input_recent_seconds":99,
            "window_selected":true,"input_opt_in":false,"owner_marker":"unchanged"}},
        {"target":"foreign-session","identity":{"host":"own","session":"other"},
            "capabilities":{"current_foreground":{"pid":7},"owner_marker":"other"}}
    ]);
    super::refresh_volatile_target_facts(
        &mut targets,
        &serde_json::json!({
            "identity":identity,"values":{"current_foreground":foreground,"user_input_recent_seconds":0.5}}),
    );
    assert_eq!(targets[0]["capabilities"]["current_foreground"], foreground);
    assert_eq!(targets[0]["capabilities"]["user_input_recent_seconds"], 0.5);
    assert_eq!(targets[0]["capabilities"]["window_selected"], true);
    assert_eq!(targets[0]["capabilities"]["input_opt_in"], false);
    assert_eq!(targets[0]["capabilities"]["owner_marker"], "unchanged");
    assert!(targets[1]["capabilities"]["current_foreground"].is_null());
    assert_eq!(targets[1]["capabilities"]["owner_marker"], "other");
}

#[test]
fn volatile_desktop_facts_missing_identity_never_preserves_stale_foreground() {
    let mut targets = serde_json::json!([{"identity":{"host":"own"},
            "capabilities":{"current_foreground":{"pid":7},"user_input_recent_seconds":0.0}}]);
    super::refresh_volatile_target_facts(
        &mut targets,
        &serde_json::json!({
            "identity":null,"values":{"current_foreground":{"pid":9},"user_input_recent_seconds":1.0}}),
    );
    assert!(targets[0]["capabilities"]["current_foreground"].is_null());
    assert!(targets[0]["capabilities"]["user_input_recent_seconds"].is_null());
}
use super::*;
#[test]
fn foreground_permission_scope_is_ax_and_listen_not_cgevent_post() {
    assert!(macos_tool_implemented("computer.focus_window"));
    assert!(!macos_tool_implemented("computer.activate_window"));
    let mut caps =
        json!({"permissions":{"accessibility":true,"post_event":false,"listen_event":false}});
    assert_eq!(
        macos_prerequisite("computer.focus_window", &caps),
        Some("listen_event_permission_missing")
    );
    caps["permissions"]["listen_event"] = json!(true);
    assert_eq!(
        macos_prerequisite("computer.focus_window", &caps),
        Some("selected_window_required")
    );
    caps["permissions"]["accessibility"] = json!(false);
    assert_eq!(
        macos_prerequisite("computer.focus_window", &caps),
        Some("accessibility_permission_missing")
    );
}
#[test]
fn service_diagnostics_neither_require_a_window_nor_bypass_observation_policy() {
    let dir = tempfile::tempdir().unwrap();
    let mut policy = Principal {
        journal_identity: None,
        certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
        observe: true,
        input: false,
    };
    let runtime = Runtime::new(Config {
        version: 1,
        device_id: "diagnostic-test".into(),
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
    assert_eq!(result.data["service"], "healthy");
    assert_eq!(
        result.data["executor_features"]["resolve_view_red_marker_cluster"],
        json!({
            "implemented":true,"opt_in_required":true,"fallback":false,
            "unavailable_error":"red_marker_anchor_unavailable",
            "scope":"executor_parameter; native_anchor_is_checked_per_view"
        })
    );

    assert_eq!(result.data["availability"], "unavailable");
    assert_eq!(
        result.data["service_process_status"]["native_token_measured"],
        false
    );
    assert!(result.data["service_process_status"].get("pid").is_none());
    assert_eq!(
        result.data["executor_features"]["move_pointer_intent"]["implemented"],
        true
    );
    assert_eq!(
        result.data["executor_features"]["move_pointer_intent"]["requires_native_backend_support"],
        true
    );
    assert_eq!(
        result.data["executor_features"]["move_pointer_intent"]["fallback"],
        false
    );
    policy.observe = false;
    let denied = runtime.describe(&policy);
    assert_eq!(
        denied.data["service_process_status"],
        json!({"status":"not_authorized"})
    );
    assert_eq!(denied.data["service"], "healthy");
}
