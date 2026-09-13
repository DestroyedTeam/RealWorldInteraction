//! Public fixture path through real acceptance/worker dispatch, without native UI.
use super::*;
use crate::config::PlatformPolicy;
use base64::{Engine as _, engine::general_purpose::STANDARD};
fn runtime(dir: &std::path::Path) -> (Arc<Runtime>, Principal) {
    let principal = Principal {
        certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
        observe: true,
        input: false,
    };
    let runtime = Runtime::new(Config {
        version: 1,
        device_id: "device".into(),
        bind: "127.0.0.1:0".parse().unwrap(),
        identity: rcu_link_trust::IdentityPaths::in_profile(dir),
        principals: vec![principal.clone()],
        state_root: dir.join("state"),
        platform: PlatformPolicy::Fixture,
    })
    .unwrap();
    (runtime, principal)
}
#[tokio::test]
async fn visual_uses_native_operation_envelope_without_input_permission_or_grounding_cache() {
    let dir = tempfile::tempdir().unwrap();
    let (r, p) = runtime(dir.path());
    let info = r.describe(&p);
    let capability = info.data["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "computer.capture_visual")
        .unwrap();
    assert_eq!(capability["implemented"], true);
    assert_eq!(capability["authorized"], true);
    let opened = r.call(&p.certificate, &p, None, Call::OpenContext).await;
    let token = opened.data["context"].as_str().unwrap();
    let context = r.context(&p.certificate, Some(token)).unwrap();
    for feedback in ["exclude", "include"] {
        let args = json!({"target":context.target,"feedback":feedback});
        let cached = r
            .call(
                &p.certificate,
                &p,
                Some(token),
                Call::Cached {
                    tool: "computer.capture_visual".into(),
                    arguments: args.clone(),
                },
            )
            .await;
        assert!(cached.is_error && cached.images.is_empty());
        let prepared = r.prepare(
            &p.certificate,
            &p,
            Some(token),
            "computer.capture_visual".into(),
            args,
        );
        assert!(!prepared.is_error, "{:?}", prepared.data);
        let reference: OperationRef =
            serde_json::from_value(prepared.data["operation_ref"].clone()).unwrap();
        let result = r.execute(&p.certificate, &p, &reference).await;
        assert!(!result.is_error, "{:?}", result.data);
        assert_eq!(result.data["operation_ref"], json!(reference));
        assert_eq!(result.data["native_dispatch_entered"], false);
        assert_eq!(result.data["input_domain_fenced"], false);
        assert_eq!(result.data["frame_created"], false);
        assert_eq!(result.data["input_authority_granted"], false);
        assert_eq!(result.data["capture"]["feedback"]["requested"], feedback);
        assert!(result.data.get("frame").is_none() && result.data.get("artifact_id").is_none());
        assert_eq!(result.images.len(), 1);
        let bytes = STANDARD.decode(&result.images[0].base64).unwrap();
        assert_eq!(rcu_imaging::decode_original(&bytes).unwrap().size, [1, 1]);
        assert_eq!(
            bytes.len(),
            result.data["png_bytes"].as_u64().unwrap() as usize
        );
        let visual = result.data["visual_id"].as_str().unwrap();
        let mut executor = context.executor.lock().unwrap();
        assert_eq!(executor.engine.snapshot_count(), 0);
        assert_eq!(executor.engine.artifact_bytes(), 0);
        assert!(
            executor
                .invoke(
                    "computer.resolve_view",
                    json!({"target":context.target,"view_id":visual,"x":0,"y":0})
                )
                .is_err()
        );
    }
}
#[tokio::test]
async fn cancellation_before_visual_execute_produces_no_image_and_no_dispatch() {
    let dir = tempfile::tempdir().unwrap();
    let (r, p) = runtime(dir.path());
    let opened = r.call(&p.certificate, &p, None, Call::OpenContext).await;
    let token = opened.data["context"].as_str().unwrap();
    let context = r.context(&p.certificate, Some(token)).unwrap();
    let prepared = r.prepare(
        &p.certificate,
        &p,
        Some(token),
        "computer.capture_visual".into(),
        json!({"target":context.target}),
    );
    let reference = serde_json::from_value(prepared.data["operation_ref"].clone()).unwrap();
    let cancelled = r.cancel(&p.certificate, &reference);
    assert_eq!(cancelled.data["terminal"], true);
    assert_eq!(cancelled.data["dispatch_started"], false);
    let result = r.execute(&p.certificate, &p, &reference).await;
    assert!(result.is_error && result.images.is_empty());
    assert!(!context.entered.load(Ordering::Acquire));
    assert_eq!(context.executor.lock().unwrap().engine.snapshot_count(), 0);
}
