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
