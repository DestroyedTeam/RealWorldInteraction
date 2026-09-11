//! Public offline regressions from immutable r5 evidence, not a live OS replay.
use super::*;
use serde_json::Value;

const PREFLIGHT: &str = include_str!("../tests/fixtures/r5-preflight.json");
const GRANT: &str = include_str!("../tests/fixtures/r5-launch-grant.json");

fn bounded(value: &Value) -> String {
    assert_eq!(value["truncated"], false);
    let text = value["value_prefix"].as_str().unwrap();
    assert_eq!(value["sha256_utf8"], digest(text.as_bytes()));
    assert_eq!(value["utf8_bytes"], text.len());
    assert_eq!(value["utf16_units"], text.encode_utf16().count());
    text.into()
}

fn token(value: &Value) -> TokenIdentity {
    TokenIdentity {
        sid: bounded(&value["sid"]),
        source: bounded(&value["source"]),
        session: value["session"].as_u64().unwrap().try_into().unwrap(),
        authentication_id: value["authentication_id"].as_u64().unwrap(),
    }
}

fn actual_r5() -> (ObservedPackage, WindowIdentity, PackageSnapshot) {
    assert_eq!(
        digest(PREFLIGHT.as_bytes()),
        "76c36b81973e9e3143ee5c4d8b63b8458523b9efbb4b4bd7c10471dc15558539"
    );
    assert_eq!(
        digest(GRANT.as_bytes()),
        "465ab927a32930ef81420fac29cf648edee114d9076c773665ae26792fedf75c"
    );
    let pre: Value = serde_json::from_str(PREFLIGHT).unwrap();
    let grant: Value = serde_json::from_str(GRANT).unwrap();
    assert_eq!(pre["error"]["code"], "observed_package_unknown_properties");
    for field in [
        "reservation_created",
        "launch_dispatched",
        "activation_dispatched",
        "input_dispatched",
        "goal_verified",
        "ready_for_launch",
    ] {
        assert_eq!(pre[field], false);
    }
    let d = &pre["error"]["diagnostic"];
    assert_eq!(d["stage"], "before_package_image_hash");
    assert_eq!(d["query_provenance"]["complete"], true);
    assert_eq!(
        d["query_provenance"]["entries"].as_array().unwrap().len(),
        55
    );
    let cause = &d["cause"];
    assert_eq!(
        cause["failed_predicates"],
        serde_json::json!(["unknown_properties"])
    );
    assert_eq!(cause["head_flags"]["raw"], 16);
    assert_eq!(cause["origin"]["raw"], 3);
    let spec =
        serde_json::from_value(grant["launches"][0]["observed_packages"][0].clone()).unwrap();
    let key: WindowIdentity = serde_json::from_value(d["allocated_window"].clone()).unwrap();
    assert_eq!(
        grant["launches"][0]["allowed_existing_windows"][0],
        serde_json::to_value(&key).unwrap()
    );
    let s = &cause["snapshot"];
    let p = &s["process"];
    let current = PackageSnapshot {
        process: ProcessIdentity {
            pid: p["pid"].as_u64().unwrap().try_into().unwrap(),
            birth: p["birth"].as_u64().unwrap(),
            live: p["live"].as_bool().unwrap(),
            image: bounded(&p["image"]),
            full_name: bounded(&p["full_name"]),
            family: bounded(&p["family"]),
            token: token(&p["token"]),
        },
        worker: token(&s["worker"]),
        head_count: s["head_count"].as_u64().unwrap().try_into().unwrap(),
        full_name: bounded(&s["full_name"]),
        family: bounded(&s["family"]),
        family_from_full_name: bounded(&s["family_from_full_name"]),
        graph_path: bounded(&s["graph_path"]),
        install_path: bounded(&s["install_path"]),
        effective_path: bounded(&s["effective_path"]),
        flags: cause["head_flags"]["raw"]
            .as_u64()
            .unwrap()
            .try_into()
            .unwrap(),
        origin: cause["origin"]["raw"].as_i64().unwrap().try_into().unwrap(),
    };
    (spec, key, current)
}

#[test]
fn actual_r5_snapshot_gets_narrow_compatibility_not_a_live_pin_or_goal_verdict() {
    let (spec, key, current) = actual_r5();
    // The old mask demonstrably rejects this exact record, not an invented flag.
    assert_eq!(current.flags & !KNOWN_PROPERTIES, 0x10);
    validate(&spec, &key, &current).unwrap();
    assert_eq!(current.flags, 16); // No normalization of raw snapshot evidence.
    assert_eq!(current.process.birth, 134_335_198_145_995_687);
}

#[test]
fn observed_annotation_preserves_all_single_bit_rejections_and_no_generic_filters() {
    let (spec, key, current) = actual_r5();
    for shift in 0..32 {
        let bit = 1u32 << shift;
        let mut altered = current.clone();
        altered.flags = OBSERVED_SINGLE_HEAD_ANNOTATION | bit;
        let allowed = bit & KNOWN_SINGLE_HEAD_FLAGS != 0 && bit != DEVELOPMENT_MODE;
        assert_eq!(
            validate(&spec, &key, &altered).is_ok(),
            allowed,
            "bit {bit:#010x}"
        );
    }
    // Input filter/information constants are NOT a returned-flags allowlist.
    for filter in [0x20, 0x40, 0x80, 0x100, 0x20000, 0x30, 0xf0] {
        let mut altered = current.clone();
        altered.flags = filter | OBSERVED_SINGLE_HEAD_ANNOTATION;
        let e = validate(&spec, &key, &altered).unwrap_err();
        assert_eq!(e.code, "observed_package_unknown_properties");
        assert_eq!(
            e.diagnostic.unwrap()["head_flags"]["unknown_bits"],
            filter & !0x10
        );
    }
}

#[test]
fn observed_annotation_keeps_development_and_each_unknown_bit_in_combined_diagnostics() {
    let (spec, key, current) = actual_r5();
    for shift in 0..32 {
        let bit = 1u32 << shift;
        if bit & KNOWN_SINGLE_HEAD_FLAGS != 0 {
            continue;
        }
        let mut altered = current.clone();
        altered.flags |= DEVELOPMENT_MODE | bit;
        altered.origin = 5;
        let e = validate(&spec, &key, &altered).unwrap_err();
        assert_eq!(e.code, "observed_package_development");
        let d = e.diagnostic.unwrap();
        assert_eq!(
            d["failed_predicates"],
            serde_json::json!(["development", "unknown_properties", "disallowed_origin"])
        );
        assert_eq!(d["schema"], "rcu.windows.package_policy_rejection.v2");
        assert_eq!(d["head_flags"]["raw"], altered.flags);
        assert_eq!(d["head_flags"]["unknown_bits"], bit);
        assert_eq!(
            d["head_flags"]["documented_property_bits"],
            DEVELOPMENT_MODE
        );
        assert_eq!(d["head_flags"]["observed_single_head_annotation_bits"], 16);
        assert_eq!(d["head_flags"]["known_properties_mask"], KNOWN_PROPERTIES);
    }
}

#[test]
fn observed_annotation_keeps_store_inbox_and_denies_all_other_origins() {
    let (spec, key, current) = actual_r5();
    for origin in [i32::MIN, -1, 0, 1, 2, 3, 4, 5, 6, 7, i32::MAX] {
        let mut altered = current.clone();
        altered.origin = origin;
        assert_eq!(
            validate(&spec, &key, &altered).is_ok(),
            matches!(origin, 2 | 3)
        );
    }
}

#[test]
fn annotation_never_shortcuts_single_head_process_package_or_same_user_checks() {
    let (spec, key, current) = actual_r5();
    for field in 0..20 {
        let mut altered = current.clone();
        match field {
            0 => altered.head_count = 0,
            1 => altered.head_count = 2,
            2 => altered.full_name.push('X'),
            3 => altered.family.push('X'),
            4 => altered.family_from_full_name.push('X'),
            5 => altered.process.full_name.push('X'),
            6 => altered.process.family.push('X'),
            7 => altered.process.pid += 1,
            8 => altered.process.birth += 1,
            9 => altered.process.live = false,
            10 => altered.process.image.push('X'),
            11 => altered.worker.sid.push('X'),
            12 => altered.worker.session += 1,
            13 => altered.worker.authentication_id += 1,
            14 => altered.worker.source = "worker_thread".into(),
            15 => altered.process.token.source.clear(),
            16 => altered.process.token.sid.clear(),
            17 => altered.full_name.clear(),
            18 => altered.family.clear(),
            _ => altered.family_from_full_name.clear(),
        }
        assert!(validate(&spec, &key, &altered).is_err(), "field {field}");
    }
}

#[test]
fn annotation_requires_existing_selected_root_and_image_hash_format_checks_first() {
    let (spec, key, current) = actual_r5();
    for path in [
        r"C:\Users\user\Mutable",
        r"D:\External",
        r"C:\OtherPackage",
        "",
    ] {
        for field in 0..3 {
            let mut altered = current.clone();
            match field {
                0 => altered.graph_path = path.into(),
                1 => altered.install_path = path.into(),
                _ => altered.effective_path = path.into(),
            }
            assert!(validate(&spec, &key, &altered).is_err());
        }
    }
    let mut altered = spec.clone();
    altered.install_path.push_str("-lookalike");
    assert!(validate(&altered, &key, &current).is_err());
    for hash in ["", "abcd", &"x".repeat(64)] {
        let mut altered = spec.clone();
        altered.application.sha256 = hash.into();
        assert_eq!(
            validate(&altered, &key, &current).unwrap_err().code,
            "invalid_application_sha256"
        );
    }
}

#[test]
fn annotation_does_not_normalize_before_after_evidence_or_cached_pin_keys() {
    let (spec, key, current) = actual_r5();
    let mut no_annotation = current.clone();
    no_annotation.flags = 0;
    // No official returned-bit guarantee: do not invent a mandatory HEAD bit.
    validate(&spec, &key, &no_annotation).unwrap();
    assert!(unchanged(&current, &no_annotation).is_err());
    for bit in [1, 0x40000, 0x80000, 0x100000, 0x200000] {
        let mut altered = current.clone();
        altered.flags |= bit;
        validate(&spec, &key, &altered).unwrap();
        assert!(unchanged(&current, &altered).is_err());
    }
    let mut changed_spec = spec.clone();
    changed_spec.application.sha256 = "b".repeat(64);
    assert!(!matches(&spec, &key, &changed_spec, &key));
}
