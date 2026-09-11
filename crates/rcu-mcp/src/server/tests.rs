use super::*;
struct RasterBackend(Vec<u8>);
impl rcu_core::Backend for RasterBackend {
    fn identity(&self) -> rcu_core::Identity {
        rcu_core::Identity {
            host: "offline-image-test".into(),
            guest: None,
            session: "fixture".into(),
            seat: "fixture".into(),
            desktop: "fixture".into(),
        }
    }
    fn capabilities(&self) -> Value {
        json!({"backend":"synthetic-image-fixture","input":false})
    }
    fn observe(&mut self) -> CoreResult<rcu_core::Scene> {
        Ok(rcu_core::Scene {
            identity: self.identity(),
            assurance: "synthetic-fixture".into(),
            binding: Some(rcu_core::Binding {
                scope: rcu_core::Scope {
                    window: "fixture".into(),
                    pid: 42,
                    element: None,
                },
                focus_window: "fixture".into(),
                layout: "physical-fixture".into(),
                content: rcu_core::digest(&self.0),
                native_epoch: "fixture".into(),
                mutation_context: None,
            }),
            mutation_ready: false,
            blocked: Some(false),
            image_size: [200, 100],
            pixel_to_desktop: Some([1., 0., 0., 1., -300., -100.]),
            elements: vec![],
            captured_unix_ms: 0,
            atomicity: "fixture".into(),
            image_normalization: "fixture RGB".into(),
            png: self.0.clone(),
        })
    }
    fn act(
        &mut self,
        _: &rcu_core::Binding,
        _: &rcu_core::Resolved,
        _: &Intent,
        _: Option<&str>,
    ) -> CoreResult<Option<bool>> {
        Err(fault("fixture_read_only", "No input exists."))
    }
    fn inspect(
        &mut self,
        _: &rcu_core::Scope,
        _: &Predicate,
        _: &str,
    ) -> CoreResult<rcu_core::Oracle> {
        Err(fault("fixture_read_only", "Not an oracle fixture."))
    }
}
#[tokio::test]
async fn image_tool_emits_viewable_png_and_pixel_mapping_is_exact_frame_scoped() {
    let state = tempfile::tempdir().unwrap();
    let mut e = Engine::new(state.path().into()).unwrap();
    let original = rcu_imaging::encode_rgb([200, 100], &vec![77; 200 * 100 * 3]).unwrap();
    e.register("raster".into(), Box::new(RasterBackend(original.clone())))
        .unwrap();
    let one = e.observe("raster").unwrap();
    let two = e.observe("raster").unwrap();
    let aid = one.artifact_id.unwrap();
    assert!(e.original("raster", &two.frame, &aid).is_err());
    let server = Server::new(e);
    let image = server
        .image(Parameters(ImageArgs {
            target: "raster".into(),
            frame: one.frame.clone(),
            artifact_id: aid.clone(),
            crop: Some([20, 10, 100, 80]),
            max_edge: 1200,
            operation_id: None,
        }))
        .await;
    let wire = serde_json::to_value(&image).unwrap();
    let content = wire["content"].as_array().unwrap();
    let png = content.iter().find(|c| c["type"] == "image").unwrap();
    assert_eq!(png["mimeType"], "image/png");
    let bytes = STANDARD.decode(png["data"].as_str().unwrap()).unwrap();
    assert_eq!(
        rcu_imaging::decode_original(&bytes).unwrap().size,
        [100, 80]
    );
    let view_id = wire["structuredContent"]["view_id"]
        .as_str()
        .unwrap()
        .to_string();
    let resolved = server
        .resolve_view(Parameters(ResolveViewArgs {
            target: "raster".into(),
            view_id: view_id.clone(),
            x: 4.,
            y: 5.,
        }))
        .await;
    assert_eq!(
        resolved.structured_content.unwrap()["point"],
        json!([-276., -85.])
    );
    let wrong = server
        .resolve_view(Parameters(ResolveViewArgs {
            target: "other".into(),
            view_id,
            x: 4.,
            y: 5.,
        }))
        .await;
    assert_eq!(wrong.structured_content.unwrap()["error"], "unknown_view");
    let engine = server.engine.lock().unwrap();
    assert_eq!(
        engine.original("raster", &one.frame, &aid).unwrap().0,
        original
    );
}
#[test]
fn instructions_fit_first_512_characters() {
    assert!(INSTRUCTIONS.len() <= 512);
}
#[test]
fn schema_rejects_unknown_action_arguments() {
    assert!(serde_json::from_value::<ActArgs>(json!({"target":"a","reference":"b","lease":"c","request_id":"d","intent":"click","args":[]})).is_err());
}
#[test]
fn checked_schema_accepts_only_exact_element_and_no_locator_overrides() {
    let valid = json!({"target":"a","frame":"b","element":"exact-id","lease":"c","request_id":"d","intent":"click"});
    assert!(serde_json::from_value::<CheckedActArgs>(valid.clone()).is_ok());
    for field in ["locator", "reference", "name", "x", "y", "args"] {
        let mut args = valid.clone();
        args[field] = json!({"kind":"pixel","x":1,"y":2});
        assert!(serde_json::from_value::<CheckedActArgs>(args).is_err());
    }
    for element in [
        json!({"kind":"semantic","name":"Input"}),
        json!([1, 2]),
        json!(true),
        Value::Null,
    ] {
        let mut args = valid.clone();
        args["element"] = element;
        assert!(serde_json::from_value::<CheckedActArgs>(args).is_err());
    }
}
#[tokio::test]
async fn checked_endpoint_dispatches_fixture_once_and_preserves_operation_receipt() {
    let state = tempfile::tempdir().unwrap();
    let fixture = rcu_core::fixture::Fixture::default();
    let mut engine = Engine::new(state.path().into()).unwrap();
    engine
        .register("fixture".into(), Box::new(fixture.clone()))
        .unwrap();
    let frame = engine.observe("fixture").unwrap().frame;
    let lease = engine.claim("fixture", 60_000).unwrap();
    let server = Server::new(engine);
    let args = || CheckedActArgs {
        target: "fixture".into(),
        frame: frame.clone(),
        element: "Input".into(),
        lease: lease.clone(),
        request_id: "checked-once".into(),
        intent: Intent::Text,
        value: Some("checked fixture value".into()),
        operation_id: None,
    };
    let result = server.act_checked(Parameters(args())).await;
    assert_ne!(result.is_error, Some(true), "{result:?}");
    let receipt = result.structured_content.as_ref().unwrap();
    assert_eq!(receipt["request_id"], "checked-once");
    assert_eq!(receipt["scope"]["element"], "Input");
    assert_eq!(receipt["dispatch"], "returned");
    assert_eq!(receipt["goal_verified"], false);
    assert_eq!(receipt["replay"], "forbidden");
    query_response_operation(&server, &result).await;
    let replay = server.act_checked(Parameters(args())).await;
    assert_ne!(replay.is_error, Some(true), "{replay:?}");
    assert_eq!(
        replay.structured_content.as_ref().unwrap()["deduplicated"],
        true
    );
    query_response_operation(&server, &replay).await;
    let app = fixture.0.lock().unwrap();
    assert_eq!(app.calls, 1);
    assert_eq!(app.value, "checked fixture value");
}
#[tokio::test]
async fn checked_endpoint_rejects_changed_document_without_dispatch() {
    let state = tempfile::tempdir().unwrap();
    let fixture = rcu_core::fixture::Fixture::default();
    let mut engine = Engine::new(state.path().into()).unwrap();
    engine
        .register("fixture".into(), Box::new(fixture.clone()))
        .unwrap();
    let frame = engine.observe("fixture").unwrap().frame;
    let lease = engine.claim("fixture", 60_000).unwrap();
    fixture.0.lock().unwrap().value = "document changed after observation".into();
    let server = Server::new(engine);
    let response = server
        .act_checked(Parameters(CheckedActArgs {
            target: "fixture".into(),
            frame,
            element: "Toggle test option".into(),
            lease,
            request_id: "changed-document".into(),
            intent: Intent::Click,
            value: None,
            operation_id: Some("checked-context-error".into()),
        }))
        .await;
    assert_eq!(response.is_error, Some(true));
    assert_eq!(
        response.structured_content.as_ref().unwrap()["error"],
        "checked_precondition_changed"
    );
    let op = query_response_operation(&server, &response).await;
    assert_eq!(op["operation_id"], "checked-context-error");
    assert_eq!(op["terminal"], true);
    assert_eq!(op["resources_released"], true);
    assert_eq!(fixture.0.lock().unwrap().calls, 0);
    assert!(
        state
            .path()
            .join("receipts")
            .read_dir()
            .unwrap()
            .next()
            .is_none()
    );
}
#[test]
fn boolean_is_not_a_coordinate() {
    assert!(
        serde_json::from_value::<ResolveArgs>(
            json!({"target":"a","frame":"b","locator":{"kind":"pixel","x":true,"y":0}})
        )
        .is_err()
    );
}
async fn query_response_operation(server: &Server, response: &CallToolResult) -> Value {
    let data = response.structured_content.as_ref().unwrap();
    let operation_id = data["operation_id"].as_str().unwrap();
    assert!(!operation_id.is_empty());
    assert_eq!(data["operation"]["operation_id"], operation_id);
    let query = server
        .operation(Parameters(OperationArgs {
            operation_id: operation_id.into(),
        }))
        .await;
    assert_ne!(query.is_error, Some(true));
    let snapshot = query.structured_content.unwrap();
    assert_eq!(data["operation"], snapshot);
    let wire = serde_json::to_value(response).unwrap();
    let last = wire["content"].as_array().unwrap().last().unwrap();
    let metadata: Value = serde_json::from_str(last["text"].as_str().unwrap()).unwrap();
    assert_eq!(metadata["operation_id"], operation_id);
    assert_eq!(metadata["operation"], snapshot);
    snapshot
}
#[tokio::test]
async fn early_native_pin_error_keeps_diagnostic_through_mcp_worker_and_text_blocks() {
    let state = tempfile::tempdir().unwrap();
    let server = Server::new(Engine::new(state.path().into()).unwrap());
    let diagnostic = json!({"schema":"rcu.windows.pin.v1","api":"CreateFileW","stage":"open",
        "request":{"authorized_file":"C:\\Protected\\app.exe","path":"C:\\Protected","desired_access":32,"share_mode":1,"flags":0x0220_0000},
        "native_error":{"hresult":"0x80070005","win32_code":5},
        "verification_context":{"phase":"launch_preflight","role":"selected_launcher"},
        "effective_token":{"status":"unknown","reason":"offline fixture"}});
    let mut failure = fault(
        "application_directory_pin_failed",
        "Original native failure; no dispatch.",
    );
    failure.diagnostic = Some(Box::new(diagnostic.clone()));
    let response = server.execute(move |_| Err(failure)).await;
    let payload = response.structured_content.as_ref().unwrap();
    assert_eq!(response.is_error, Some(true));
    assert_eq!(payload["error"], "application_directory_pin_failed");
    assert_eq!(payload["diagnostic"], diagnostic);
    let wire = serde_json::to_value(&response).unwrap();
    let first_text: Value =
        serde_json::from_str(wire["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(first_text["diagnostic"], diagnostic);
    let status = query_response_operation(&server, &response).await;
    assert_eq!(status["terminal"], true);
    assert_eq!(status["resources_released"], true);
    assert_eq!(status["native_call_depth"], 0);
    let old = error(fault("legacy", "unchanged"));
    assert_eq!(
        old.structured_content.unwrap(),
        json!({"error":"legacy","next_step":"unchanged"})
    );
}
#[tokio::test]
async fn auto_operation_id_and_actual_snapshot_survive_success_and_core_error() {
    let state = tempfile::tempdir().unwrap();
    let server = Server::new(Engine::new(state.path().into()).unwrap());
    let successful = server
        .execute(|_| value(json!({"existing_field": 7})))
        .await;
    assert_eq!(
        successful.structured_content.as_ref().unwrap()["existing_field"],
        7
    );
    let snapshot = query_response_operation(&server, &successful).await;
    assert_eq!(snapshot["terminal"], true);
    assert_eq!(snapshot["resources_released"], true);
    let failed = server
        .execute(|_| {
            Err(fault(
                "offline_failure",
                "The original next step is preserved.",
            ))
        })
        .await;
    assert_eq!(failed.is_error, Some(true));
    assert_eq!(
        failed.structured_content.as_ref().unwrap()["error"],
        "offline_failure"
    );
    assert_eq!(
        failed.structured_content.as_ref().unwrap()["next_step"],
        "The original next step is preserved."
    );
    let snapshot = query_response_operation(&server, &failed).await;
    assert_eq!(snapshot["terminal"], true);
    assert_eq!(snapshot["resources_released"], true);
    assert_ne!(
        successful.structured_content.unwrap()["operation_id"],
        failed.structured_content.unwrap()["operation_id"]
    );
}
#[tokio::test]
async fn worker_panic_returns_queryable_auto_id_and_cleanup_uncertainty() {
    let state = tempfile::tempdir().unwrap();
    let server = Server::new(Engine::new(state.path().into()).unwrap());
    let failed = server
        .execute(|_| {
            struct Cleanup;
            impl Drop for Cleanup {
                fn drop(&mut self) {
                    rcu_core::operation::resource_release_uncertain();
                }
            }
            let _cleanup = Cleanup;
            panic!("offline worker-panic fixture");
        })
        .await;
    assert_eq!(failed.is_error, Some(true));
    assert_eq!(
        failed.structured_content.as_ref().unwrap()["error"],
        "worker_failed"
    );
    let snapshot = query_response_operation(&server, &failed).await;
    assert_eq!(snapshot["terminal"], true);
    assert_eq!(snapshot["resource_release_uncertain"], true);
    assert_eq!(snapshot["resources_released"], false);
    assert_eq!(snapshot["native_call_pending"], false);
}
#[tokio::test]
async fn core_error_does_not_hide_failed_resource_cleanup() {
    let state = tempfile::tempdir().unwrap();
    let server = Server::new(Engine::new(state.path().into()).unwrap());
    let failed = server
        .execute_operation(Some("cleanup-uncertain".into()), |_| {
            rcu_core::operation::resource_release_uncertain();
            Err(fault(
                "cleanup_fixture_failed",
                "Do not claim resources were released.",
            ))
        })
        .await;
    let snapshot = query_response_operation(&server, &failed).await;
    assert_eq!(snapshot["operation_id"], "cleanup-uncertain");
    assert_eq!(snapshot["terminal"], true);
    assert_eq!(snapshot["resource_release_uncertain"], true);
    assert_eq!(snapshot["resources_released"], false);
}
#[tokio::test]
async fn completed_join_cannot_fabricate_terminal_with_a_live_native_guard() {
    let state = tempfile::tempdir().unwrap();
    let server = Server::new(Engine::new(state.path().into()).unwrap());
    let (tx, rx) = std::sync::mpsc::channel();
    let response = server
        .execute(move |_| {
            // This only exercises Rust lifecycle counters; it invokes no OS API.
            tx.send(rcu_core::operation::native_call("retained_offline_guard")?)
                .unwrap();
            value(json!({"worker_returned": true}))
        })
        .await;
    let snapshot = query_response_operation(&server, &response).await;
    assert_eq!(snapshot["finish_requested"], true);
    assert_eq!(snapshot["native_call_depth"], 1);
    assert_eq!(snapshot["native_call_pending"], true);
    assert_eq!(snapshot["terminal"], false);
    assert_eq!(snapshot["resources_released"], false);
    drop(rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap());
    let query = server
        .operation(Parameters(OperationArgs {
            operation_id: snapshot["operation_id"].as_str().unwrap().into(),
        }))
        .await;
    let completed = query.structured_content.unwrap();
    assert_eq!(completed["terminal"], true);
    assert_eq!(completed["resources_released"], true);
    assert_eq!(completed["native_call_depth"], 0);
    // The earlier response remains a truthful earlier snapshot.
    assert_eq!(
        response.structured_content.unwrap()["operation"]["terminal"],
        false
    );
}
#[tokio::test]
async fn non_object_or_reserved_payload_is_preserved_without_spoofing_operation() {
    let state = tempfile::tempdir().unwrap();
    let server = Server::new(Engine::new(state.path().into()).unwrap());
    for original in [
        json!([1, 2]),
        json!({"operation_id":"not-the-registered-id","payload":9}),
    ] {
        let payload = original.clone();
        let response = server.execute(move |_| value(payload)).await;
        assert_eq!(
            response.structured_content.as_ref().unwrap()["result"],
            original
        );
        let snapshot = query_response_operation(&server, &response).await;
        assert_ne!(snapshot["operation_id"], "not-the-registered-id");
    }
}
#[test]
fn lifecycle_preflight_schema_does_not_accept_dispatch_fields() {
    let good = json!({"target":"explicit","authorization_id":"launch","operation_id":null});
    assert!(serde_json::from_value::<LifecyclePreflightArgs>(good.clone()).is_ok());
    for key in ["lease", "arguments", "activate", "allow_input"] {
        let mut bad = good.clone();
        bad[key] = true.into();
        assert!(serde_json::from_value::<LifecyclePreflightArgs>(bad).is_err());
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_endpoint_remains_available_but_native_slot_is_held_until_cleanup() {
    let state = tempfile::tempdir().unwrap();
    let server = Server::new(Engine::new(state.path().into()).unwrap());
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (finish_tx, finish_rx) = std::sync::mpsc::channel();
    let worker = server.clone();
    let request = tokio::spawn(async move {
        worker
            .execute_operation(Some("native-pending".into()), move |_| {
                let native = rcu_core::operation::native_call("synthetic_blocking_native_call")?;
                started_tx.send(()).unwrap();
                finish_rx
                    .recv_timeout(std::time::Duration::from_secs(3))
                    .unwrap();
                drop(native);
                rcu_core::operation::checkpoint("after_native")?;
                value(json!({"done":true}))
            })
            .await
    });
    started_rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap();
    let cancelled = server
        .cancel(Parameters(OperationArgs {
            operation_id: "native-pending".into(),
        }))
        .await;
    let s = cancelled.structured_content.unwrap();
    assert_eq!(s["native_call_pending"], true);
    assert_eq!(s["resources_released"], false);
    assert!(server.gate.clone().try_acquire_owned().is_err());
    finish_tx.send(()).unwrap();
    let result = request.await.unwrap();
    assert_eq!(
        result.structured_content.unwrap()["error"],
        "operation_cancelled"
    );
    let s = server.operations.get("native-pending", false).unwrap();
    assert!(s.terminal && s.resources_released && !s.native_call_pending);
    assert!(server.gate.clone().try_acquire_owned().is_ok());
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropped_async_request_cancels_without_detaching_a_second_native_worker() {
    let state = tempfile::tempdir().unwrap();
    let server = Server::new(Engine::new(state.path().into()).unwrap());
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (finish_tx, finish_rx) = std::sync::mpsc::channel();
    let worker = server.clone();
    let request = tokio::spawn(async move {
        worker
            .execute_operation(Some("dropped".into()), move |_| {
                let _native = rcu_core::operation::native_call("synthetic_pending")?;
                started_tx.send(()).unwrap();
                finish_rx
                    .recv_timeout(std::time::Duration::from_secs(3))
                    .unwrap();
                value(json!({}))
            })
            .await
    });
    started_rx
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap();
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    let s = server.operations.get("dropped", false).unwrap();
    assert!(s.cancel_requested && !s.terminal && s.native_call_pending);
    assert!(server.gate.clone().try_acquire_owned().is_err());
    finish_tx.send(()).unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !server.operations.get("dropped", false).unwrap().terminal {
        assert!(std::time::Instant::now() < until);
        tokio::task::yield_now().await;
    }
}
