use super::*;

#[test]
fn r92_click_delivery_parser_feature_never_advertises_a_native_route() {
    let dir = tempfile::tempdir().unwrap();
    let policy = Principal {
        journal_identity: None,
        certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
        observe: true,
        input: false,
    };
    let runtime = Runtime::new(Config {
        version: 1,
        device_id: "click-feature-test".into(),
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
    assert_eq!(
        result.data["executor_features"]["click_delivery_options"],
        json!({
            "implemented":true,"requires_native_backend_support":true,"fallback":false,
            "scope":"intent_parser; selected native click capability remains required"
        })
    );
    assert!(result.data.get("pointer_click").is_none());
    assert_eq!(result.data["availability"], "unavailable");
    let act = result.data["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cap| cap["id"] == "computer.act")
        .unwrap();
    assert_eq!(act["authorized"], false);
    assert_eq!(act["available"], false);
}
