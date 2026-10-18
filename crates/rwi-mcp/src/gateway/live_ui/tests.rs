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
        json!(["inline", "fullscreen", "pip"])
    );
    assert!(
        resource["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("availableDisplayModes: ['inline', 'fullscreen', 'pip']")
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
        json!(["inline", "fullscreen", "pip"])
    );
    assert_eq!(
        resource["contents"][0]["_meta"]["openai/ui"]["preferredDisplayMode"],
        "inline"
    );
    assert_eq!(URI, "ui://real-world-interaction/live/v6.html");
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
