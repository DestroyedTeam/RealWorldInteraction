//! Public configuration/auth contracts; no native factory or OS UI calls.
use rcu_agent::{
    config::{Config, PlatformPolicy, Principal},
    runtime::Runtime,
};
use rcu_core::device::{Call, PROTOCOL_VERSION, Request};
use rcu_link_trust::{IdentityPaths, PeerPin};

fn config(root: &std::path::Path) -> Config {
    Config {
        version: 1,
        device_id: "mac-test-device".into(),
        bind: "127.0.0.1:0".parse().unwrap(),
        identity: IdentityPaths::in_profile(root),
        principals: vec![Principal {
            certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
            observe: false,
            input: false,
        }],
        state_root: root.join("state"),
        platform: PlatformPolicy::MacOs {
            user_uid: 501,
            authorized_display: None,
        },
    }
}
#[test]
fn macos_policy_requires_only_a_native_uid_not_window_or_windows_identity() {
    let dir = tempfile::tempdir().unwrap();
    let c = config(dir.path());
    let data = serde_json::to_value(&c).unwrap();
    assert_eq!(
        data["platform"],
        serde_json::json!({"kind":"macos","user_uid":501})
    );
    assert!(
        serde_json::from_value::<Config>(data.clone())
            .unwrap()
            .validate()
            .is_ok()
    );
    for field in ["window", "pid", "user_sid", "privilege"] {
        let mut malformed = data.clone();
        malformed["platform"][field] = serde_json::json!("invented");
        assert!(serde_json::from_value::<Config>(malformed).is_err());
    }
    let mut invalid = c;
    invalid.platform = PlatformPolicy::MacOs {
        user_uid: u32::MAX,
        authorized_display: None,
    };
    assert!(invalid.validate().is_err());
}
#[tokio::test]
async fn service_and_authentication_have_no_mac_window_or_permission_dependency() {
    let dir = tempfile::tempdir().unwrap();
    let c = config(dir.path());
    let peer = c.principals[0].certificate.clone();
    let r = Runtime::new(c).unwrap();
    let invoke = |peer, call| {
        r.clone().handle(
            peer,
            Request {
                version: PROTOCOL_VERSION,
                id: uuid::Uuid::new_v4().to_string(),
                device: "mac-test-device".into(),
                context: None,
                call,
            },
        )
    };
    let result = invoke(peer.clone(), Call::Describe).await;
    assert_eq!(result.result.data["service"], "healthy");
    assert_eq!(result.result.data["platform"], "macos");
    assert!(result.result.data["native_context"].is_null());
    assert_eq!(
        result.result.data["service_process_status"]["status"],
        "not_authorized"
    );
    assert_eq!(
        invoke(peer, Call::OpenContext).await.result.data["code"],
        "permission_denied"
    );
    assert_eq!(
        invoke(PeerPin::parse(&"b".repeat(64)).unwrap(), Call::Describe)
            .await
            .result
            .data["code"],
        "principal_denied"
    );
}
