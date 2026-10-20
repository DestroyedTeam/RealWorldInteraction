use super::*;

fn observation() -> DomObservation {
    serde_json::from_value(json!({"observed_unix_ms":1234,"document_visible":false,
        "intersecting":true,"image_hidden":true,"image_src_present":false,
        "natural_size":[0,0],"panel_size":[320,310],"cursor":null}))
    .unwrap()
}
fn setup() -> (Views, String, String, String) {
    let views = Views::default();
    let (key, page, update) = views.test_app_page("owned");
    views.app_event(&key, page.clone(), 1, false).unwrap();
    (views, key, page, update.view_id)
}

#[test]
fn f376_dom_observation_is_bounded_plain_metadata_not_an_authority_payload() {
    let value = serde_json::to_value(observation()).unwrap();
    assert!(observation().valid());
    for (field, invalid) in [
        ("observed_unix_ms", json!(0)),
        ("observed_unix_ms", json!(SAFE_INTEGER + 1)),
        ("natural_size", json!([2049, 1])),
        ("panel_size", json!([320, 4097])),
        ("cursor", json!({"generation":0,"sequence":1})),
        (
            "cursor",
            json!({"generation":1,"sequence":SAFE_INTEGER + 1}),
        ),
    ] {
        let mut v = value.clone();
        v[field] = invalid;
        assert!(!serde_json::from_value::<DomObservation>(v).unwrap().valid());
    }
    for (field, invalid) in [
        ("target", json!("foreign")),
        ("src", json!("data:image/png;base64,secret")),
        ("natural_size", json!([1.5, 1])),
        ("panel_size", json!([1, 2, 3])),
        ("image_hidden", json!("true")),
    ] {
        let mut v = value.clone();
        v[field] = invalid;
        assert!(serde_json::from_value::<DomObservation>(v).is_err());
    }
}

#[test]
fn f376_dom_record_and_status_do_not_renew_presence_lifetime_or_device_sequence() {
    let (v, key, page, binding) = setup();
    let v = Arc::new(v);
    assert_eq!(
        v.image_read(&key, &page, 1).err(),
        Some("live_image_hidden")
    );
    let facts = {
        let e = v.entries.lock().unwrap();
        let x = &e[&key];
        (x.expires, x.device_sequence, x.ui_sequence)
    };
    assert!(v.observe_dom(&key, &page, 1, observation()));
    let mut data = json!({"host_view_binding":binding,"status":"running"});
    v.decorate_dom_observation("owned", &mut data);
    assert_eq!(data["component_observation"]["read_only"], true);
    assert_eq!(
        data["component_observation"]["capture_or_presence_renewed"],
        false
    );
    assert_eq!(
        data["component_observation"]["events"][0]["dom"]["image_src_present"],
        false
    );
    let e = v.entries.lock().unwrap();
    let x = &e[&key];
    assert_eq!(facts, (x.expires, x.device_sequence, x.ui_sequence));
    drop(e);
    assert_eq!(
        v.image_read(&key, &page, 1).err(),
        Some("live_image_hidden")
    );
}

#[test]
fn f376_dom_wrong_page_key_or_sequence_cannot_borrow_an_accepted_heartbeat() {
    let (v, k, p, _) = setup();
    assert!(!v.observe_dom("foreign", &p, 1, observation()));
    assert!(!v.observe_dom(&k, "foreign", 1, observation()));
    for sequence in [0, 2, SAFE_INTEGER] {
        assert!(!v.observe_dom(&k, &p, sequence, observation()));
    }
    assert!(v.observe_dom(&k, &p, 1, observation()));
    assert!(!v.observe_dom(&k, &p, 1, observation()));
    v.app_event(&k, p.clone(), 2, false).unwrap();
    assert!(!v.observe_dom(&k, &p, 1, observation()));
    assert_eq!(v.entries.lock().unwrap()[&k].dom.0.len(), 1);
}

#[test]
fn f376_dom_read_requires_exact_target_and_native_view_binding() {
    let (v, k, p, b) = setup();
    v.observe_dom(&k, &p, 1, observation());
    for (target, binding) in [("foreign", b.as_str()), ("owned", "foreign")] {
        let mut d = json!({"host_view_binding":binding});
        let before = d.clone();
        v.decorate_dom_observation(target, &mut d);
        assert_eq!(d, before);
    }
    let mut d = json!({"status":"running"});
    let before = d.clone();
    v.decorate_dom_observation("owned", &mut d);
    assert_eq!(d, before);
}

#[test]
fn f376_dom_replacement_expiry_and_remove_do_not_resurrect_old_evidence() {
    let (v, k, p, b) = setup();
    v.observe_dom(&k, &p, 1, observation());
    v.app_page("owned").unwrap();
    assert!(!v.observe_dom(&k, &p, 1, observation()));
    assert!(v.entries.lock().unwrap()[&k].dom.0.is_empty());
    let mut d = json!({"host_view_binding":b});
    let original = d.clone();
    v.decorate_dom_observation("owned", &mut d);
    assert_eq!(d, original);
    let (_, new, _) = v.app_page("owned").unwrap();
    v.app_event(&k, new.clone(), 1, false).unwrap();
    v.observe_dom(&k, &new, 1, observation());
    v.entries.lock().unwrap().get_mut(&k).unwrap().expires = Instant::now();
    assert!(!v.observe_dom(&k, &new, 1, observation()));
    v.decorate_dom_observation("owned", &mut d);
    assert_eq!(d, original);
    v.remove_target("owned");
    v.decorate_dom_observation("owned", &mut d);
    assert_eq!(d, original);
}

#[test]
fn f376_dom_log_keeps_only_eight_latest_events() {
    let (v, k, p, b) = setup();
    for sequence in 1..=20 {
        if sequence > 1 {
            v.app_event(&k, p.clone(), sequence, false).unwrap();
        }
        assert!(v.observe_dom(&k, &p, sequence, observation()));
    }
    let mut d = json!({"host_view_binding":b});
    v.decorate_dom_observation("owned", &mut d);
    let events = d["component_observation"]["events"].as_array().unwrap();
    assert_eq!(events.len(), 8);
    assert_eq!(events[0]["presence_sequence"], 13);
    assert_eq!(events[7]["presence_sequence"], 20);
}

#[test]
fn f376_dom_missing_old_client_diagnostic_leaves_status_byte_identical() {
    let (v, _, _, b) = setup();
    let mut d = json!({"host_view_binding":b,"status":"running","frames_captured":3});
    let original = serde_json::to_vec(&d).unwrap();
    v.decorate_dom_observation("owned", &mut d);
    assert_eq!(serde_json::to_vec(&d).unwrap(), original);
}
