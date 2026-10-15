use super::*;
fn observation() -> Value {
    let rows: Vec<_> = (0..128).map(|i| json!({"id":format!("element-{i}"),"name":format!("control-{i}"),"state_revision":"x".repeat(64),"visible":true,"occluded":false,"point":[i,12]})).collect();
    json!({"frame":"frame","artifact_id":"artifact","operation_ref":{"execution_nonce":"exact"},"input_domain_fenced":false,"operation":{"native_call_pending":false,"resources_released":true},"scene":{"elements":rows,"blocked":false,"mutation_ready":true},"adapter_status":{"input":true,"native_details":{"focused_element_ids":["element-1"],"uia":{"status":"available","walk":{"complete":false,"truncation":["budget"],"elements":rows,"details":[{"element_id":"element-1","keyboard_focus":true,"control_type":"Edit","debug":"x".repeat(60000)}]}}}}})
}
fn args(id: &str, pointer: &str, offset: usize) -> ResultArgs {
    ResultArgs {
        result_ref: id.into(),
        pointer: pointer.into(),
        offset,
        limit: 16,
        byte_offset: None,
    }
}
#[test]
fn retained_and_projected_metadata_keep_exact_serialized_fingerprint() {
    let raw = json!({"targets":[],"devices":[],"text":"界\\\"","unknown":null});
    let encoded = serde_json::to_vec(&raw).unwrap();
    let expected_sha = rwi_core::digest(&encoded);
    let mut cache = Presentation::default();
    let id = cache.retain("computer.targets", raw.clone()).unwrap();
    assert_eq!(cache.entries[0].bytes, encoded.len());
    assert_eq!(cache.entries[0].sha, expected_sha);
    assert_eq!(cache.read(args(&id, "", 0)).data["value"], raw);
    let projected = cache.project("computer.targets", DeviceResult::value(&raw));
    assert_eq!(
        projected.data["presentation"]["metadata_sha256"],
        expected_sha
    );
    assert_eq!(
        projected.data["presentation"]["original_metadata_bytes"],
        encoded.len()
    );
    assert_eq!(projected.data["unknown"], Value::Null);
    assert_eq!(cache.entries[1].data, raw);
}
#[test]
fn shared_admission_enforces_accounted_byte_budget_and_expiry() {
    let mut cache = Presentation::default();
    let first = cache.retain("nested", json!({"value":1})).unwrap();
    // Accounting-only fixture: no large allocation or native capture.
    cache.entries[0].bytes = CACHE_BYTES;
    cache.retain("nested", json!({"value":2})).unwrap();
    assert_eq!(cache.entries.len(), 1);
    assert_eq!(
        cache.read(args(&first, "", 0)).data["code"],
        "result_ref_expired"
    );
    cache.entries[0].at -= TTL;
    cache.project(
        "computer.targets",
        DeviceResult::value(json!({"targets":[]})),
    );
    assert_eq!(cache.entries.len(), 1);
    assert_eq!(cache.entries[0].tool, "computer.targets");
}
#[test]
fn compact_observation_retains_authority_focus_and_partial_coverage() {
    let raw = observation();
    let mut cache = Presentation::default();
    let p = cache
        .project("computer.observe", DeviceResult::value(&raw))
        .data;
    assert!(bytes(&p) <= COMPACT_BYTES);
    for key in [
        "frame",
        "artifact_id",
        "operation_ref",
        "input_domain_fenced",
        "operation",
    ] {
        assert_eq!(p[key], raw[key]);
    }
    assert_eq!(p["scene"]["elements"][1]["keyboard_focus"], true);
    assert_eq!(p["scene"]["elements"][1]["control_type"], "Edit");
    assert_eq!(
        p["adapter_status"]["native_details"]["uia"]["walk"]["complete"],
        false
    );
    assert_eq!(p["presentation"]["elements"]["native_complete"], false);
    let id = p["presentation"]["result_ref"].as_str().unwrap();
    assert_eq!(id.len(), 32);
    assert!(id.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let page = cache.read(args(id, "/scene/elements", 0));
    assert_eq!(page.data["value"][0], raw["scene"]["elements"][0]);
    assert_eq!(page.data["next_offset"], 16);
    assert_eq!(
        cache.read(args(id, "/operation_ref", 0)).data["value"],
        raw["operation_ref"]
    );
}
#[test]
fn failed_mutation_is_never_projected_or_loses_locator() {
    let mut cache = Presentation::default();
    let mut r = DeviceResult::error("effect_unknown", "Do not replay");
    r.data["operation_ref"] = json!({"execution_nonce":"exact"});
    let expected = r.data.clone();
    let r = cache.project("computer.act_checked", r);
    assert!(r.is_error);
    assert_eq!(r.data, expected);
    assert!(cache.entries.is_empty());
}
#[test]
fn expired_or_other_adapter_result_never_resolves_or_dispatches() {
    let mut cache = Presentation::default();
    let p = cache.project("computer.observe", DeviceResult::value(observation()));
    let id = p.data["presentation"]["result_ref"].as_str().unwrap();
    assert_eq!(
        Presentation::default().read(args(id, "", 0)).data["code"],
        "result_ref_expired"
    );
    cache.entries[0].at -= TTL;
    assert_eq!(
        cache.read(args(id, "", 0)).data["code"],
        "result_ref_expired"
    );
}
#[test]
fn metadata_pages_bounds_and_json_pointer_are_explicit() {
    let mut raw = observation();
    raw["a/b"] = json!({"~key":"kept"});
    let mut cache = Presentation::default();
    let p = cache.project("computer.observe", DeviceResult::value(raw));
    let id = p.data["presentation"]["result_ref"].as_str().unwrap();
    assert_eq!(cache.read(args(id, "/a~1b/~0key", 0)).data["value"], "kept");
    assert_eq!(
        cache.read(args(id, "/scene/elements", 129)).data["code"],
        "result_offset_invalid"
    );
    assert_eq!(cache.read(args(id, "", 0)).data["value_included"], false);
    assert_eq!(
        cache.read(args(id, "/missing", 0)).data["code"],
        "result_pointer_unknown"
    );
}
#[test]
fn utf8_export_reconstructs_large_original_metadata_without_native_calls() {
    let mut raw = observation();
    raw["long"] = json!("界\\\"".repeat(5000));
    let mut cache = Presentation::default();
    let p = cache.project("computer.observe", DeviceResult::value(&raw));
    let id = p.data["presentation"]["result_ref"].as_str().unwrap();
    let mut joined = String::new();
    let mut offset = 0;
    loop {
        let mut a = args(id, "", 0);
        a.byte_offset = Some(offset);
        let r = cache.read(a);
        assert!(!r.is_error);
        assert_eq!(r.data["native_dispatched"], false);
        joined.push_str(r.data["text"].as_str().unwrap());
        let Some(next) = r.data["next_byte_offset"].as_u64() else {
            break;
        };
        assert!(next as usize > offset);
        offset = next as usize;
    }
    assert_eq!(serde_json::from_str::<Value>(&joined).unwrap(), raw);
}
#[test]
fn count_eviction_and_unknown_future_readiness_are_not_hidden() {
    let mut cache = Presentation::default();
    let mut raw = observation();
    raw["adapter_status"]["future_block_reason"] = json!("unavailable");
    let p = cache.project("computer.observe", DeviceResult::value(&raw));
    assert_eq!(
        p.data["adapter_status"]["future_block_reason"],
        "unavailable"
    );
    let first = p.data["presentation"]["result_ref"]
        .as_str()
        .unwrap()
        .to_owned();
    for _ in 0..CACHE_ENTRIES {
        cache.project(
            "computer.targets",
            DeviceResult::value(json!({"targets":[],"devices":[]})),
        );
    }
    assert_eq!(cache.entries.len(), CACHE_ENTRIES);
    assert_eq!(
        cache.read(args(&first, "", 0)).data["code"],
        "result_ref_expired"
    );
}
#[test]
fn target_deduplication_preserves_devices_and_offline_errors() {
    let mut cache = Presentation::default();
    let target = json!({"target":"exact","capabilities":{"input":false}});
    let p=cache.project("computer.targets",DeviceResult::value(json!({"targets":[target],"devices":[{"device_id":"one","state":{"targets":[target],"availability":"available"}},{"device_id":"two","is_error":true,"state":{"code":"offline"}}]})));
    assert_eq!(p.data["targets"][0], target);
    assert_eq!(p.data["devices"][1]["state"]["code"], "offline");
    assert!(p.data["devices"][0]["state"].get("targets").is_none());
}
#[test]
fn unknown_observation_shape_is_not_rewritten_or_panicked() {
    let raw = json!({"scene":{"elements":[null]}});
    let result = Presentation::default().project("computer.observe", DeviceResult::value(&raw));
    assert_eq!(result.data, raw);
}
#[test]
#[ignore = "explicit saved metadata file; offline projection, not native GUI acceptance"]
fn saved_metadata_projection_is_losslessly_retrievable() {
    let path =
        std::env::var_os("RCU_OBSERVATION_REPLAY").expect("explicit saved JSON metadata path");
    let mut raw: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    super::super::portable_numbers(&mut raw);
    let mut cache = Presentation::default();
    let result = cache.project("computer.observe", DeviceResult::value(&raw));
    let id = result.data["presentation"]["result_ref"].as_str().unwrap();
    let mut output = String::new();
    let mut offset = 0;
    let mut pages = 0;
    loop {
        let mut a = args(id, "", 0);
        a.byte_offset = Some(offset);
        let page = cache.read(a);
        assert!(!page.is_error, "{}", page.data);
        output.push_str(page.data["text"].as_str().unwrap());
        pages += 1;
        let Some(next) = page.data["next_byte_offset"].as_u64() else {
            break;
        };
        offset = next as usize;
    }
    assert_eq!(serde_json::from_str::<Value>(&output).unwrap(), raw);
    for key in [
        "frame",
        "artifact_id",
        "operation_ref",
        "operation",
        "input_domain_fenced",
    ] {
        assert_eq!(result.data[key], raw[key]);
    }
    println!(
        "projection_audit={}",
        json!({"before_bytes":bytes(&raw),"after_bytes":bytes(&result.data),"raw_elements":raw["scene"]["elements"].as_array().unwrap().len(),"returned_elements":result.data["scene"]["elements"].as_array().unwrap().len(),"full_metadata_reconstructed":true,"export_pages":pages,"native_calls":0})
    );
}

#[cfg(test)]
#[test]
fn retained_control_reads_do_not_renew_cache_lifetime() {
    let mut cache = Presentation::default();
    let id = cache
        .retain(
            "computer.observe",
            json!({"frame":"f","scene":{"elements":[]}}),
        )
        .unwrap();
    assert_eq!(id.len(), 32);
    assert!(id.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let at = cache.entries[0].at;
    assert!(cache.observation_rows(&id, "f").is_ok());
    assert_eq!(cache.entries[0].at, at);
    cache.entries[0].at = Instant::now() - TTL - Duration::from_secs(1);
    assert_eq!(
        cache.observation_rows(&id, "f").unwrap_err().data["code"],
        "observation_expired"
    );
}

fn f16_targets() -> Value {
    let status = json!({"pid":71,"permissions":{"accessibility":true,"post_event":false,
        "screen_capture":null},"native_context":{"host":"exact-host","guest":null,
        "session":"exact-session","seat":"exact-seat","desktop":"exact-desktop"},
        "future_unknown":null,"text":"界"});
    let target = json!({"target":"full-exact-target","identity":status["native_context"],
        "capabilities":{"input":false,"reason":null},"point":[21,32]});
    json!({"devices":[{"device_id":"fixture","is_error":false,"state":{
        "availability":"available","runtime_status":status,"service_process_status":status,
        "future_unknown":null,"targets":[target]}}],"targets":[target],"service":"healthy"})
}
#[test]
fn f16_text_only_exact_duplicate_preserves_structured_cache_and_unknowns() {
    let raw = f16_targets();
    let mut cache = Presentation::default();
    let result = cache.project("computer.targets", DeviceResult::value(&raw));
    let projected = result.data.clone();
    let text = cache.targets_text_projection(&result).unwrap();
    let reply = super::super::mcp_result_with_text_projection(result, Some(text.clone()));
    assert_eq!(reply.structured_content.as_ref(), Some(&projected));
    assert!(
        text["devices"][0]["state"]
            .get("service_process_status")
            .is_none()
    );
    assert_eq!(
        text["devices"][0]["state"]["runtime_status"],
        raw["devices"][0]["state"]["runtime_status"]
    );
    assert_eq!(text["targets"], raw["targets"]);
    let reference = projected["presentation"]["result_ref"].as_str().unwrap();
    assert_eq!(
        cache
            .read(args(
                reference,
                "/devices/0/state/service_process_status",
                0
            ))
            .data["value"],
        raw["devices"][0]["state"]["service_process_status"]
    );
    assert_eq!(
        cache
            .read(args(reference, "/devices/0/state/runtime_status", 0))
            .data["value"],
        raw["devices"][0]["state"]["runtime_status"]
    );
    assert_eq!(
        cache.entries[0].sha,
        rwi_core::digest(serde_json::to_vec(&raw).unwrap().as_slice())
    );
    let mut restored = text;
    restored["devices"][0]["state"]["service_process_status"] =
        restored["devices"][0]["state"]["runtime_status"].clone();
    restored["presentation"]
        .as_object_mut()
        .unwrap()
        .remove("text_projection");
    assert_eq!(restored, projected);
}
#[test]
fn f16_uncached_expired_errors_and_images_are_not_compacted() {
    let raw = f16_targets();
    let mut cache = Presentation::default();
    let result = cache.project("computer.targets", DeviceResult::value(&raw));
    assert!(
        Presentation::default()
            .targets_text_projection(&result)
            .is_none()
    );
    cache.entries[0].at -= TTL;
    assert!(cache.targets_text_projection(&result).is_none());
    let mut error = result.clone();
    error.is_error = true;
    assert!(cache.targets_text_projection(&error).is_none());
    let reply = super::super::mcp_result_with_text_projection(error, Some(json!({"bad":"never"})));
    assert_eq!(reply.is_error, Some(true));
    assert!(reply.structured_content.unwrap().get("devices").is_some());
    let mut image = result;
    image.images.push(rwi_core::device::DeviceImage {
        mime: "image/png".into(),
        base64: "exact-image".into(),
    });
    assert!(cache.targets_text_projection(&image).is_none());
}
#[test]
fn f16_unequal_null_malformed_and_offline_targets_pass_through() {
    for replacement in [
        json!(null),
        json!(false),
        json!("unknown"),
        json!({}),
        json!({"pid":72}),
    ] {
        let mut raw = f16_targets();
        raw["devices"][0]["state"]["service_process_status"] = replacement;
        let mut cache = Presentation::default();
        let result = cache.project("computer.targets", DeviceResult::value(raw));
        assert!(cache.targets_text_projection(&result).is_none());
    }
    let mut raw = f16_targets();
    raw["devices"][0]["state"]["service_process_status"]["permissions"]["screen_capture"] =
        json!(false);
    let mut cache = Presentation::default();
    let result = cache.project("computer.targets", DeviceResult::value(raw));
    assert!(cache.targets_text_projection(&result).is_none());
    let mut raw = f16_targets();
    raw["devices"][0]["is_error"] = json!(true);
    let result = cache.project("computer.targets", DeviceResult::value(raw));
    assert!(cache.targets_text_projection(&result).is_none());
}
#[test]
fn f16_aliases_are_per_device_not_cross_peer_and_do_not_replace_existing_markers() {
    let mut raw = f16_targets();
    let mut other = raw["devices"][0].clone();
    other["device_id"] = json!("second");
    other["state"]["runtime_status"]["pid"] = json!(72);
    other["state"]["service_process_status"]["pid"] = json!(72);
    raw["devices"].as_array_mut().unwrap().push(other);
    let mut cache = Presentation::default();
    let mut result = cache.project("computer.targets", DeviceResult::value(raw));
    let text = cache.targets_text_projection(&result).unwrap();
    assert_eq!(
        text["presentation"]["text_projection"]["exact_duplicates"]["/devices/1/state/service_process_status"],
        "/devices/1/state/runtime_status"
    );
    assert_eq!(text["devices"][1]["state"]["runtime_status"]["pid"], 72);
    result.data["presentation"]["text_projection"] = json!({"future_unknown":null});
    assert!(cache.targets_text_projection(&result).is_none());
    result.data["presentation"]
        .as_object_mut()
        .unwrap()
        .remove("text_projection");
    result.data["targets"][0]["target"] = json!("not-the-retained-target");
    assert!(cache.targets_text_projection(&result).is_none());
    result.data["devices"][0]["state"]["runtime_status"]["pid"] = json!(999);
    result.data["devices"][0]["state"]["service_process_status"]["pid"] = json!(999);
    assert!(cache.targets_text_projection(&result).is_none());
}

#[test]
fn f16_typed_opt_in_is_adapter_only_and_default_absence_is_unchanged() {
    let mut arguments = json!({"device_id":"fixture"}).as_object().unwrap().clone();
    let original = arguments.clone();
    assert!(!take_targets_text_projection(&mut arguments).unwrap());
    assert_eq!(arguments, original);
    arguments.insert("text_projection".into(), json!("exact_duplicates_v1"));
    assert!(take_targets_text_projection(&mut arguments).unwrap());
    assert_eq!(arguments, original);
    assert!(
        rwi_core::device::catalog::validate("computer.targets", &Value::Object(arguments)).is_ok()
    );
    for invalid in [
        json!(null),
        json!(false),
        json!(true),
        json!("future_v2"),
        json!({"exact_duplicates_v1":true}),
    ] {
        let mut arguments = json!({"device_id":"fixture","text_projection":invalid})
            .as_object()
            .unwrap()
            .clone();
        assert!(take_targets_text_projection(&mut arguments).is_err());
    }
}
#[test]
fn f16_opt_in_schema_has_only_one_optional_typed_addition() {
    let definition = rwi_core::device::catalog::definitions()
        .into_iter()
        .find(|d| d.name == "computer.targets")
        .unwrap();
    let mut schema = definition.input_schema.clone();
    targets_text_schema(&mut schema);
    assert_eq!(schema["properties"]["text_projection"]["type"], "string");
    assert_eq!(
        schema["properties"]["text_projection"]["enum"],
        json!(["exact_duplicates_v1"])
    );
    schema["properties"]
        .as_object_mut()
        .unwrap()
        .remove("text_projection");
    assert_eq!(schema, definition.input_schema);
}

#[test]
fn f16_malformed_known_presentation_values_pass_through_without_alias() {
    let mut cache = Presentation::default();
    let result = cache.project("computer.targets", DeviceResult::value(f16_targets()));
    for (key, value) in [
        ("retention", json!(null)),
        ("retention", json!("future-retention-policy")),
        ("over_budget", json!(false)),
        ("over_budget", json!(null)),
        ("over_budget", json!("unknown")),
        ("original_metadata_bytes", json!(0)),
        ("metadata_sha256", json!("not-the-retained-hash")),
    ] {
        let mut malformed = result.clone();
        malformed.data["presentation"][key] = value;
        assert!(cache.targets_text_projection(&malformed).is_none(), "{key}");
    }
    let mut unknown = result;
    unknown.data["presentation"]["future_unknown"] = json!(null);
    assert!(cache.targets_text_projection(&unknown).is_none());
}

fn native_targets_with_known_detail() -> Value {
    let context =
        json!({"host":"HOST","guest":null,"session":"SERIAL","seat":"SEAT","desktop":"DESKTOP"});
    let fg = json!({"pid":42,"window":"cgwindow:77","process_start":"PROCESS_START","native_epoch":"EXACT_EPOCH"});
    let runtime = json!({"pid":10070,"uid":501,"euid":501,"status":"available","native_context":context,"lock_state":"unknown","permissions":{"accessibility":true,"listen_event":false,"post_event":null,"screen_capture":true,"requests_permission":false}});
    let capabilities = vec![
        json!({"id":"computer.focus_element","implemented":true,"authorized":true,"available":false,"reason":"selected_window_required"}),
        json!({"id":"computer.launch_application","implemented":false,"authorized":true,"available":false,"reason":"not_implemented_on_this_platform"}),
        json!({"id":"computer.custom","implemented":null,"available":null,"reason":"unknown"}),
    ];
    let caps = json!({"adapter":"rust-macos-native","evidence":"native_api_observed","window_selected":false,"current_foreground":fg,"permissions":runtime["permissions"],"atomicity":"Neither a returned post nor property equality proves the business goal.",
        "background_input":{"native_acknowledgement":false,"application_effect_attested":false,"receiver_preparation":{"keyboard":"legacy AppKit activation when inactive after payload/focus validation","pointer":"none"}},
        "background_operation":"DOC".repeat(16000),"binding_identity":"DOC".repeat(5000),
        "focus_element":{"implemented":true,"available":false,"attribute":"AXFocused","max_setter_attempts":1,"scope":"DOC".repeat(2000),"goal_attested_by_capability":false},
        "pointer_feedback":{"visible":false,"point":[123.5,456.75],"applied_point":null,"display_id":4},
        "custom_future_policy":{"scope":"DO NOT DROP FUTURE UNKNOWN","available":null},
        "mapping":{"point":[900.25,501.5],"native_epoch":"CONTROL_EPOCH"}});
    json!({"devices":[{"device_id":"mac-host","is_error":false,"adapter_transport":{"authenticated_connections":2,"request_writes_started":3,"custom_error":"unknown transport condition"},"state":{"device_id":"mac-host","platform":"macos","instance":"INSTANCE","resource_domain":"DOMAIN","context_valid":true,"native_context":context,"runtime_status":runtime,"service_process_status":runtime,"input_domain_fenced":false,"capabilities":capabilities,"live_desktop_facts":{"identity":context,"values":{"current_foreground":fg}},"recent_operations":[{"request_id":"old-once","dispatch":"uncertain","window":"cgwindow:77"}]}}],"targets":[{"target":"mac-host:INSTANCE:EXACT_TARGET","identity":context,"parent_target":null,"binding":{"window":"cgwindow:77","pid":42,"bounds":[1.5,2.5,800,600]},"capabilities":caps}],"service":"healthy","profile_error":null,"workflow_required":false})
}
#[test]
fn targets_compact_keeps_exact_routing_permission_fence_foreground_and_capability_facts() {
    let raw = native_targets_with_known_detail();
    let mut cache = Presentation::default();
    let out = cache
        .project("computer.targets", DeviceResult::value(&raw))
        .data;
    let old = &raw["devices"][0]["state"];
    let state = &out["devices"][0]["state"];
    for key in [
        "device_id",
        "platform",
        "instance",
        "resource_domain",
        "context_valid",
        "native_context",
        "input_domain_fenced",
        "capabilities",
        "live_desktop_facts",
    ] {
        assert_eq!(&state[key], &old[key], "{key}");
    }
    assert_eq!(state["runtime_status"]["pid"], 10070);
    assert_eq!(
        state["runtime_status"]["permissions"],
        old["runtime_status"]["permissions"]
    );
    assert_eq!(state["runtime_status"]["lock_state"], "unknown");
    assert!(state.get("service_process_status").is_none());
    assert!(state["runtime_status"].get("native_context").is_none());
    for key in ["target", "identity", "parent_target", "binding"] {
        assert_eq!(out["targets"][0][key], raw["targets"][0][key]);
    }
    let caps = &out["targets"][0]["capabilities"];
    for key in [
        "current_foreground",
        "permissions",
        "atomicity",
        "background_input",
        "pointer_feedback",
        "mapping",
        "custom_future_policy",
    ] {
        assert_eq!(caps[key], raw["targets"][0]["capabilities"][key], "{key}");
    }
    assert_eq!(caps["focus_element"]["implemented"], true);
    assert_eq!(caps["focus_element"]["available"], false);
    assert_eq!(caps["focus_element"]["attribute"], "AXFocused");
    assert_eq!(
        out["devices"][0]["adapter_transport"]["custom_error"],
        "unknown transport condition"
    );
    assert!(
        out["devices"][0]["adapter_transport"]
            .get("authenticated_connections")
            .is_none()
    );
    assert!(bytes(&out) <= COMPACT_BYTES);
    assert!(out["presentation"].get("over_budget").is_none());
}
#[test]
fn targets_compact_retains_original_detail_hash_and_byte_exact_cache_data() {
    let raw = native_targets_with_known_detail();
    let mut cache = Presentation::default();
    let out = cache
        .project("computer.targets", DeviceResult::value(&raw))
        .data;
    assert_eq!(cache.entries[0].data, raw);
    assert_eq!(
        out["presentation"]["metadata_sha256"],
        rwi_core::digest(&serde_json::to_vec(&raw).unwrap())
    );
    let id = out["presentation"]["result_ref"].as_str().unwrap();
    let full = cache.read(args(id, "/devices/0/state/recent_operations", 0));
    assert_eq!(
        full.data["value"],
        raw["devices"][0]["state"]["recent_operations"]
    );
    assert_eq!(
        cache
            .read(args(id, "/devices/0/state/service_process_status", 0))
            .data["value"],
        raw["devices"][0]["state"]["service_process_status"]
    );
    assert_eq!(
        cache
            .read(args(id, "/targets/0/capabilities/pointer_feedback", 0))
            .data["value"],
        raw["targets"][0]["capabilities"]["pointer_feedback"]
    );
    assert_eq!(out["presentation"]["source_tool"], "computer.targets");
    assert_eq!(out["presentation"]["details_tool"], "computer.result");
}
#[test]
fn targets_compact_never_hides_conflicting_or_unknown_native_status_context() {
    let mut raw = native_targets_with_known_detail();
    raw["devices"][0]["state"]["service_process_status"]["pid"] = json!(999);
    raw["devices"][0]["state"]["runtime_status"]["native_context"]["session"] = json!("DIFFERENT");
    raw["devices"][0]["state"]["runtime_status"]["permissions"]["accessibility"] = Value::Null;
    let mut cache = Presentation::default();
    let out = cache
        .project("computer.targets", DeviceResult::value(&raw))
        .data;
    assert_eq!(
        out["devices"][0]["state"]["service_process_status"],
        raw["devices"][0]["state"]["service_process_status"]
    );
    assert_eq!(
        out["devices"][0]["state"]["runtime_status"],
        raw["devices"][0]["state"]["runtime_status"]
    );
    assert_eq!(
        out["devices"][0]["state"]["capabilities"][2]["implemented"],
        Value::Null
    );
    assert!(
        out["devices"][0]["state"]["capabilities"][2]
            .get("authorized")
            .is_none()
    );
}
#[test]
fn targets_compact_unknown_adapter_and_oversized_future_data_are_not_silently_discarded() {
    let mut raw = native_targets_with_known_detail();
    raw["targets"][0]["capabilities"]["adapter"] = json!("future-platform");
    let mut cache = Presentation::default();
    let out = cache
        .project("computer.targets", DeviceResult::value(&raw))
        .data;
    assert_eq!(
        out["targets"][0]["capabilities"],
        raw["targets"][0]["capabilities"]
    );
    assert_eq!(out["presentation"]["over_budget"], true);
    assert_eq!(out["presentation"]["source_tool"], "computer.targets");
}
#[test]
fn targets_compact_preserves_null_and_wrong_type_prose_keys_and_does_not_touch_controls() {
    let mut raw = native_targets_with_known_detail();
    raw["targets"][0]["capabilities"]["focus_element"]["scope"] = Value::Null;
    let mut cache = Presentation::default();
    let out = cache
        .project("computer.targets", DeviceResult::value(&raw))
        .data;
    assert_eq!(
        out["targets"][0]["capabilities"]["focus_element"]["scope"],
        Value::Null
    );
    let observed = observation();
    let expected = Presentation::default()
        .project("computer.observe", DeviceResult::value(&observed))
        .data;
    let actual = cache
        .project("computer.observe", DeviceResult::value(observed))
        .data;
    assert_eq!(actual["scene"]["elements"], expected["scene"]["elements"]);
    assert_eq!(actual["operation_ref"], expected["operation_ref"]);
}

#[test]
fn targets_compact_legacy_text_projection_and_unknown_device_shapes_stay_compatible() {
    let raw = f16_targets();
    let mut cache = Presentation::default();
    let result = cache.project("computer.targets", DeviceResult::value(&raw));
    let text = cache
        .targets_text_projection(&result)
        .expect("legacy exact-duplicate text projection");
    assert_eq!(
        result.data["devices"][0]["state"]["runtime_status"],
        raw["devices"][0]["state"]["runtime_status"]
    );
    assert_eq!(
        result.data["devices"][0]["state"]["service_process_status"],
        raw["devices"][0]["state"]["service_process_status"]
    );
    assert_eq!(
        text["presentation"]["text_projection"]["exact_duplicates"]["/devices/0/state/service_process_status"],
        "/devices/0/state/runtime_status"
    );
    let mut unknown = native_targets_with_known_detail();
    unknown["targets"][0]["capabilities"]["adapter"] = json!("future-platform");
    let result = cache.project("computer.targets", DeviceResult::value(&unknown));
    assert_eq!(
        result.data["devices"][0]["state"]["runtime_status"],
        unknown["devices"][0]["state"]["runtime_status"]
    );
    assert_eq!(
        result.data["devices"][0]["state"]["service_process_status"],
        unknown["devices"][0]["state"]["service_process_status"]
    );
    assert_eq!(
        result.data["devices"][0]["adapter_transport"],
        unknown["devices"][0]["adapter_transport"]
    );
}

#[test]
fn native_plain_display_candidate_context_preserves_cache_and_exact_ref_frame_ttl() {
    let raw = json!({"frame":"plain-frame","scene":{"elements":[{"id":"plain","role":"AXTextField","actions":[],"unknown":null}]},
        "adapter_status":{"observation_consistency":{"ax_coverage_after":{"editable_focus_availability":{
            "scope":"returned_classified_plain_text_roles_only","input_authority":false,
            "elements":[{"element":"plain","native_status":-25204,"settable":null}]}}}}});
    let mut cache = Presentation::default();
    let id = cache.retain("computer.observe", raw.clone()).unwrap();
    let original_at = cache.entries[0].at;
    let original_sha = cache.entries[0].sha.clone();
    assert_eq!(cache.observation_data(&id, "plain-frame").unwrap(), &raw);
    assert_eq!(cache.entries[0].sha, original_sha);
    assert_eq!(cache.entries[0].at, original_at);
    assert_eq!(cache.entries[0].data, raw);
    assert_eq!(
        original_sha,
        rwi_core::digest(&serde_json::to_vec(&raw).unwrap())
    );
    assert_eq!(
        cache.observation_data(&id, "other").unwrap_err().data["code"],
        "observation_frame_mismatch"
    );
    let wrong_source = cache.retain("computer.targets", raw).unwrap();
    assert_eq!(
        cache
            .observation_data(&wrong_source, "plain-frame")
            .unwrap_err()
            .data["code"],
        "observation_frame_mismatch"
    );
    cache.entries[0].at -= TTL;
    assert_eq!(
        cache.observation_data(&id, "plain-frame").unwrap_err().data["code"],
        "observation_expired"
    );
}
