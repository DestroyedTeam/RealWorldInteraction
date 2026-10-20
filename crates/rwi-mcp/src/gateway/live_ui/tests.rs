use super::*;
#[test]
fn explicit_host_presentation_fullscreen_resource_is_predeclared_not_presence() {
    let value = serde_json::to_value(read(FULLSCREEN_URI).unwrap()).unwrap();
    assert_eq!(value["contents"][0]["uri"], FULLSCREEN_URI);
    assert_eq!(
        value["contents"][0]["_meta"]["openai/ui"]["availableDisplayModes"],
        json!(["fullscreen"])
    );
    assert_eq!(
        value["contents"][0]["_meta"]["openai/ui"]["preferredDisplayMode"],
        "fullscreen"
    );
    let html = value["contents"][0]["text"].as_str().unwrap();
    assert!(html.contains("availableDisplayModes: ['fullscreen']"));
    assert!(html.contains("const defaultPresentation = 'fullscreen';"));
    assert!(html.contains("if (defaultPresentation === 'fullscreen' && !capability) return;"));
    assert!(value["contents"][0]["_meta"].get("visible").is_none());
}
#[test]
fn explicit_host_presentation_expanded_entry_has_static_resource_and_no_mode_ambiguity() {
    let tool = serde_json::to_value(expanded_start_tool()).unwrap();
    assert_eq!(tool["name"], EXPANDED_START_TOOL);
    assert_eq!(tool["_meta"]["ui"]["resourceUri"], FULLSCREEN_URI);
    assert!(
        tool["inputSchema"]["properties"]
            .get("presentation")
            .is_none()
    );
    assert!(is_start(START_TOOL) && is_start(EXPANDED_START_TOOL));
    assert!(!is_start(PRESENCE_TOOL) && !is_start("computer.click"));
}
#[test]
fn explicit_host_presentation_defaults_and_strict_values() {
    let input = json!({"target":"fixture","options":{"display":null,"label":"fixture","max_edge":64,"interval_ms":500,"duration_ms":1000}});
    assert!(matches!(
        serde_json::from_value::<StartArgs>(input.clone())
            .unwrap()
            .presentation,
        HostPresentation::Automatic
    ));
    let mut explicit = input.clone();
    explicit["presentation"] = json!("fullscreen");
    assert!(matches!(
        serde_json::from_value::<StartArgs>(explicit)
            .unwrap()
            .presentation,
        HostPresentation::Fullscreen
    ));
    for invalid in [Value::Null, json!(true), json!("pip"), json!("visible")] {
        let mut value = input.clone();
        value["presentation"] = invalid;
        assert!(serde_json::from_value::<StartArgs>(value).is_err());
    }
}
#[test]
fn explicit_host_presentation_schema_is_optional_and_host_only() {
    let schema = serde_json::to_value(start_tool()).unwrap();
    assert!(
        !schema["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("presentation"))
    );
    assert_eq!(
        schema["inputSchema"]["$defs"]["HostPresentation"]["enum"],
        json!(["automatic", "fullscreen"])
    );
    let native = serde_json::to_value(schemars::schema_for!(rwi_core::LiveOptions)).unwrap();
    assert!(native["properties"].get("presentation").is_none());
}
#[test]
fn explicit_host_presentation_does_not_create_or_replace_owned_capabilities() {
    let mut missing = None;
    HostPresentation::Fullscreen.decorate(&mut missing);
    assert!(missing.is_none());
    let original = meta(
        json!({"rwi/live_panel":{"view_key":"owned","page":"page","adapter_instance":"instance"}}),
    );
    let mut automatic = Some(original.clone());
    HostPresentation::Automatic.decorate(&mut automatic);
    assert_eq!(automatic, Some(original.clone()));
    let mut explicit = Some(original);
    HostPresentation::Fullscreen.decorate(&mut explicit);
    let m = explicit.unwrap();
    assert_eq!(m.0["rwi/live_panel"]["view_key"], "owned");
    assert_eq!(m.0["rwi/live_panel"]["page"], "page");
    assert_eq!(m.0["rwi/live_panel"]["presentation"], "fullscreen");
    assert!(m.0["rwi/live_panel"].get("visible").is_none());
}
#[test]
fn apps_support_is_explicit_and_html_specific() {
    for (value, expected) in [
        (json!({}), false),
        (
            json!({"experimental":{EXTENSION:{"mimeTypes":[MIME]}}}),
            false,
        ),
        (
            json!({"extensions":{EXTENSION:{"mimeTypes":["image/png"]}}}),
            false,
        ),
        (json!({"extensions":{EXTENSION:{"mimeTypes":[MIME]}}}), true),
    ] {
        assert_eq!(supports(&serde_json::from_value(value).unwrap()), expected);
    }
}
#[test]
fn only_show_live_schema_allows_display_omission_and_keeps_bindings_private() {
    let tool = serde_json::to_value(start_tool()).unwrap();
    let options = &tool["inputSchema"]["$defs"]["LiveOptions"];
    assert!(
        !options["required"]
            .as_array()
            .unwrap()
            .contains(&json!("display"))
    );
    for name in ["host_view_id", "host_image_only"] {
        assert!(options["properties"].get(name).is_none());
    }
    let native = serde_json::to_value(schemars::schema_for!(rwi_core::LiveOptions)).unwrap();
    assert!(
        native["required"]
            .as_array()
            .unwrap()
            .contains(&json!("display"))
    );
    assert!(
        tool["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("options"))
    );
}
#[test]
fn component_events_cannot_select_targets_or_replay_old_pages() {
    let views = presence::Views::default();
    // Allocate only registry state; no Agent, native window or input.
    let (key, page, _) = views.test_app_page("fixture:target");
    let valid = PresenceArgs {
        view_key: key.clone(),
        page: page.clone(),
        sequence: 1,
        visible: true,
        close: false,
        dom_observation: None,
    };
    assert!(valid.valid());
    assert!(views.app_event(&key, page.clone(), 1, true).is_ok());
    assert!(views.app_event(&key, page.clone(), 1, false).is_err());
    let (_, newer, hidden) = views.app_page("fixture:target").unwrap();
    assert!(!hidden.visible);
    assert!(views.app_event(&key, page, 2, true).is_err());
    assert!(views.app_event(&key, newer, 1, false).is_ok());
    assert!(serde_json::from_value::<PresenceArgs>(json!({"view_key":key,"page":"a".repeat(32),"sequence":2,"visible":true,"target":"other"})).is_err());
    assert!(
        !PresenceArgs {
            close: true,
            ..valid
        }
        .valid()
    );
}

#[test]
fn f376_presence_parser_keeps_old_clients_and_rejects_invalid_dom_before_dispatch() {
    let old = json!({"view_key":"a".repeat(32),"page":"b".repeat(32),
        "sequence":1,"visible":false});
    let parsed: PresenceArgs = serde_json::from_value(old.clone()).unwrap();
    assert!(parsed.valid());
    assert!(parsed.dom_observation.is_none());
    let mut with_dom = old;
    with_dom["dom_observation"] = json!({"observed_unix_ms":1234,
        "document_visible":false,"intersecting":true,"image_hidden":true,
        "image_src_present":false,"natural_size":[0,0],"panel_size":[320,310],"cursor":null});
    assert!(
        serde_json::from_value::<PresenceArgs>(with_dom.clone())
            .unwrap()
            .valid()
    );
    with_dom["dom_observation"]["natural_size"] = json!([2049, 1]);
    assert!(
        !serde_json::from_value::<PresenceArgs>(with_dom.clone())
            .unwrap()
            .valid()
    );
    with_dom["dom_observation"]["target"] = json!("foreign");
    assert!(serde_json::from_value::<PresenceArgs>(with_dom).is_err());
}
#[test]
fn component_owner_diagnostics_do_not_delegate_foreign_view_authority() {
    let owner = Gateway::new(Ok(AdapterProfile::default()));
    let other = Gateway::new(Ok(AdapterProfile::default()));
    assert_ne!(owner.instance, other.instance);
    assert_eq!(owner.instance, owner.clone().instance);
    let (key, page, _) = owner.presence.test_app_page("fixture:owned-target");
    assert!(
        owner
            .presence
            .app_event(&key, page.clone(), 1, false)
            .is_ok()
    );
    let reason = other.presence.app_event(&key, page, 1, true).unwrap_err();
    let rejected = other.rejected_app_event(reason);
    assert!(rejected.is_error);
    assert_eq!(rejected.data["code"], "live_ui_event_rejected");
    assert_eq!(rejected.data["message"], "view_ended");
    assert_eq!(rejected.data["adapter_instance"], other.instance);
    assert!(rejected.data.get("view_key").is_none());
    assert!(rejected.data.get("target").is_none());
}
#[test]
fn independent_native_live_does_not_diverge_host_display_mode_declarations() {
    let resource = serde_json::to_value(read(URI).unwrap()).unwrap();
    assert_eq!(
        resource["contents"][0]["_meta"]["openai/ui"]["availableDisplayModes"],
        json!(["inline", "fullscreen"])
    );
    assert!(
        resource["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("availableDisplayModes: ['inline', 'fullscreen']")
    );
    // Offering a mode does not prove this Host supports or renders it.
    assert_eq!(
        resource["contents"][0]["_meta"]["openai/ui"]["preferredDisplayMode"],
        "inline"
    );
}
#[test]
fn ui_resource_is_static_closed_network_and_exact_uri() {
    let resource = serde_json::to_value(read(URI).unwrap()).unwrap();
    assert_eq!(resource["contents"][0]["mimeType"], MIME);
    assert_eq!(
        resource["contents"][0]["_meta"]["ui"]["csp"]["connectDomains"],
        json!([])
    );
    assert!(read("file:///etc/passwd").is_err());
    assert_eq!(
        resource["contents"][0]["_meta"]["openai/ui"]["availableDisplayModes"],
        json!(["inline", "fullscreen"])
    );
    assert_eq!(
        resource["contents"][0]["_meta"]["openai/ui"]["preferredDisplayMode"],
        "inline"
    );
    assert_eq!(URI, "ui://real-world-interaction/live/v9.html");
    for uri in LEGACY_URIS {
        let legacy = serde_json::to_value(read(uri).unwrap()).unwrap();
        assert_eq!(legacy["contents"][0]["uri"], uri);
        assert_eq!(
            legacy["contents"][0]["_meta"],
            resource["contents"][0]["_meta"]
        );
        assert_eq!(
            legacy["contents"][0]["text"],
            resource["contents"][0]["text"]
        );
    }
    let tool = serde_json::to_value(presence_tool()).unwrap();
    assert_eq!(tool["_meta"]["ui"]["visibility"], json!(["app"]));
    assert!(tool["inputSchema"]["properties"].get("target").is_none());
    let render = serde_json::to_value(start_tool()).unwrap();
    assert_eq!(render["_meta"]["ui"]["resourceUri"], URI);
    assert!(render["inputSchema"]["properties"].get("action").is_none());
}

#[test]
fn conversation_live_no_pip_canonical_and_legacy_resources_are_task_contained() {
    for uri in std::iter::once(URI).chain(LEGACY_URIS) {
        let value = serde_json::to_value(read(uri).unwrap()).unwrap();
        assert_eq!(
            value["contents"][0]["_meta"]["openai/ui"]["availableDisplayModes"],
            json!(["inline", "fullscreen"])
        );
        let html = value["contents"][0]["text"].as_str().unwrap();
        assert!(html.contains("availableDisplayModes: ['inline', 'fullscreen']"));
        assert!(html.contains("!detachedHostMode && enabled"));
    }
}
#[test]
fn conversation_live_no_pip_fullscreen_aliases_keep_exact_requested_uri() {
    for uri in [
        FULLSCREEN_URI,
        "ui://real-world-interaction/live/v6-fullscreen.html",
    ] {
        let value = serde_json::to_value(read(uri).unwrap()).unwrap();
        assert_eq!(value["contents"][0]["uri"], uri);
        assert_eq!(
            value["contents"][0]["_meta"]["openai/ui"]["availableDisplayModes"],
            json!(["fullscreen"])
        );
    }
}
#[test]
fn conversation_live_no_pip_does_not_add_native_options_or_visibility_authority() {
    let native = serde_json::to_value(schemars::schema_for!(rwi_core::LiveOptions)).unwrap();
    assert!(native["properties"].get("presentation").is_none());
    let tool = serde_json::to_value(start_tool()).unwrap();
    assert_eq!(tool["_meta"]["ui"]["resourceUri"], URI);
    assert!(
        tool["description"]
            .as_str()
            .unwrap()
            .contains("never requests PiP")
    );
    assert!(tool["inputSchema"]["properties"].get("visible").is_none());
}

fn f340_state(pending: bool) -> Value {
    json!({"host_view_binding":"owned", "session_id":"live:42", "status":"running", "presentation_suspended":true, "capture_pending":pending, "native_call_pending":false,"worker_done":false,"frames_captured":12,"source":{"private":"not projected"}})
}
#[test]
fn f340_native_feedback_compact_state_is_exact_and_not_a_visibility_grant() {
    let v = hidden_capture_state(&f340_state(false), "owned");
    assert_eq!(v["state"], "suspended");
    assert_eq!(v["frames_captured"], 12);
    assert!(v.get("source").is_none() && v.get("host_view_binding").is_none());
    assert!(v.get("accepted").is_none() && v.get("visible").is_none());
}
#[test]
fn f340_native_feedback_missing_or_changed_identity_is_not_false_completion() {
    let mut v = f340_state(false);
    assert_eq!(hidden_capture_state(&v, "other")["state"], "unknown");
    v.as_object_mut().unwrap().remove("capture_pending");
    assert_eq!(hidden_capture_state(&v, "owned")["state"], "unknown");
    v["capture_pending"] = json!("false");
    assert_eq!(hidden_capture_state(&v, "owned")["state"], "unknown");
}
#[tokio::test]
async fn f340_native_feedback_pending_settles_by_read_only_status_not_republish() {
    let mut reads = 0;
    let v = observe_hidden_capture("owned", || {
        reads += 1;
        std::future::ready(DeviceResult::value(f340_state(reads == 1)))
    })
    .await;
    assert_eq!(reads, 2);
    assert_eq!(v["state"], "suspended");
    assert_eq!(v["status_reads"], 2);
    assert_eq!(v["read_only"], true);
}
#[tokio::test]
async fn f340_native_feedback_pending_has_finite_read_count() {
    let mut reads = 0;
    let v = observe_hidden_capture("owned", || {
        reads += 1;
        std::future::ready(DeviceResult::value(f340_state(true)))
    })
    .await;
    assert_eq!(reads, 8);
    assert_eq!(v["state"], "pending");
    assert_eq!(v["capture_pending"], true);
}
#[tokio::test]
async fn f340_native_feedback_error_or_replaced_view_stops_without_retry() {
    for error in [false, true] {
        let mut reads = 0;
        let v = observe_hidden_capture("owned", || {
            reads += 1;
            let mut v = f340_state(false);
            v["host_view_binding"] = json!("replacement");
            std::future::ready(if error {
                DeviceResult::error("offline", "no status")
            } else {
                DeviceResult::value(v)
            })
        })
        .await;
        assert_eq!(reads, 1);
        assert_eq!(v["state"], "unknown");
        assert!(v.get("frames_captured").is_none());
    }
}
#[tokio::test]
async fn f340_native_feedback_slow_status_is_bounded_and_unknown() {
    let v = observe_hidden_capture("owned", std::future::pending::<DeviceResult>).await;
    assert_eq!(v["state"], "unknown");
    assert_eq!(v["reason"], "observation_timeout");
    assert_eq!(v["status_reads"], 1);
}
