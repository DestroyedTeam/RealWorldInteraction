use super::*;
fn config(principals: Vec<Principal>) -> Config {
    Config {
        version: 1,
        device_id: "test".into(),
        bind: "127.0.0.1:0".parse().unwrap(),
        identity: IdentityPaths::in_profile(&std::env::temp_dir().join("unused-profile")),
        principals,
        state_root: std::env::temp_dir().join("unused-state"),
        platform: PlatformPolicy::Unsupported {
            platform: "fixture".into(),
        },
    }
}
#[test]
fn journal_continuity_is_explicit_unambiguous_and_not_an_authentication_pin() {
    let old = PeerPin::parse(&"a".repeat(64)).unwrap();
    let new = PeerPin::parse(&format!("spki:{}", "b".repeat(64))).unwrap();
    let legacy: Principal =
        serde_json::from_value(serde_json::json!({"certificate":old,"observe":true,"input":true}))
            .unwrap();
    assert_eq!(legacy.identity(), &old);
    let migrated = Principal {
        certificate: new.clone(),
        journal_identity: Some(old.clone()),
        observe: true,
        input: true,
    };
    assert!(config(vec![migrated.clone()]).validate().is_ok());
    assert!(config(vec![migrated.clone(), legacy]).validate().is_err());
    let mut other = migrated.clone();
    other.certificate = PeerPin::parse(&format!("spki:{}", "c".repeat(64))).unwrap();
    assert!(config(vec![migrated, other.clone()]).validate().is_err());
    other.journal_identity = Some(new);
    assert!(config(vec![other.clone()]).validate().is_err());
    other.certificate = old.clone();
    other.journal_identity = Some(old);
    assert!(config(vec![other]).validate().is_err());
}
