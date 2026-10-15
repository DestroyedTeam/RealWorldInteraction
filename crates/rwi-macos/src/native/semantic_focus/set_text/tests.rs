use super::*;
#[test]
fn explicit_set_text_native_plain_roles_reject_secure_unknown_specialized_before_value() {
    for role in ["AXTextField", "AXTextArea"] {
        assert!(ordinary_role(role, Some("")));
    }
    for role in [
        "AXComboBox",
        "AXDateField",
        "AXTimeField",
        "AXSlider",
        "AXStaticText",
        "AXCheckBox",
    ] {
        assert!(!ordinary_role(role, Some("")));
    }
    for subrole in [
        None,
        Some("AXUnknown"),
        Some("AXSecureTextField"),
        Some("AXSearchField"),
        Some("AXNumericTextField"),
    ] {
        assert!(!ordinary_role("AXTextField", subrole));
    }
}
#[test]
fn explicit_set_text_native_bad_class_never_reads_or_writes_value() {
    for (role, subrole) in [
        ("AXTextField", Some("AXSecureTextField")),
        ("AXTextField", None),
        ("AXComboBox", Some("")),
        ("AXDateField", Some("")),
    ] {
        assert!(
            checked_plain_value::<String>(role, subrole, || panic!(
                "no secure/unknown AXValue access"
            ))
            .is_err()
        );
    }
}
#[test]
fn explicit_set_text_native_cfstring_roundtrip_is_exact_not_scalar_keyboard() {
    for text in ["", " 中文\t🙂\n𝄞 e\u{301} 👩\u{200d}💻 "] {
        let value = CFString::new(text);
        assert_eq!(
            unsafe { ffi::string(value.as_concrete_TypeRef().cast()) }.unwrap(),
            text
        );
    }
}
fn settlement_initial(first: bool, second: bool) -> SetTextReadback {
    let read = |matches| TextValueRead {
        native_status: 0,
        cf_string: Some(true),
        matches_requested: Some(matches),
        utf8_bytes: Some(if matches { 33 } else { 0 }),
        sha256: Some(if matches { "requested" } else { "empty" }.into()),
    };
    SetTextReadback {
        setter_attempts: 1,
        setter_status: 0,
        first: read(first),
        second: read(second),
        context_verified: true,
        focus_readback: None,
        settlement: None,
    }
}
#[test]
fn bounded_set_text_settlement_fast_path_and_two_read_progression_no_sleep() {
    let good = settlement_initial(true, true);
    assert!(
        collect_settlement(
            &good,
            || panic!("no extra timer"),
            |_| panic!("no extra read")
        )
        .unwrap()
        .is_none()
    );
    for second in [false, true] {
        let initial = settlement_initial(false, second);
        let clock = std::cell::Cell::new(Duration::ZERO);
        let count = std::cell::Cell::new(0);
        let result = collect_settlement(
            &initial,
            || clock.get(),
            |remaining| {
                assert!(remaining <= Duration::from_millis(100));
                clock.set(clock.get() + Duration::from_millis(1));
                count.set(count.get() + 1);
                Ok(TextSettlementRead {
                    value: settlement_initial(true, true).second,
                    offset_since_setter_return_ms: 220.0 + count.get() as f64,
                    read_elapsed_ms: 1.0,
                })
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(count.get(), if second { 1 } else { 2 });
        assert_eq!(result.stop_reason, "matched_pair");
        assert_eq!(initial.first.matches_requested, Some(false));
        assert_eq!(initial.setter_attempts, 1);
    }
}
#[test]
fn bounded_set_text_settlement_errors_and_fake_deadline_stop_without_replay() {
    let initial = settlement_initial(false, false);
    for code in [
        "cancelled",
        "lease_expired_during_dispatch",
        "native_context_changed",
        "plain_string_value_required",
    ] {
        let count = std::cell::Cell::new(0);
        let error = collect_settlement(
            &initial,
            || Duration::ZERO,
            |_| {
                count.set(count.get() + 1);
                Err(fault(code, "read refused"))
            },
        )
        .unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(count.get(), 1);
        let detail = error.diagnostic.unwrap();
        assert_eq!(detail["setter_attempts"], 1);
        assert_eq!(
            detail["initial_readback"]["first"]["matches_requested"],
            false
        );
        assert_eq!(detail["replay_allowed"], false);
    }
    let count = std::cell::Cell::new(0);
    let error = collect_settlement(
        &initial,
        || Duration::from_millis(100),
        |_| {
            count.set(count.get() + 1);
            panic!("expired budget must not read")
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "set_text_settlement_budget_expired");
    assert_eq!(count.get(), 0);
    let count = std::cell::Cell::new(0);
    let error = collect_settlement(
        &initial,
        || Duration::ZERO,
        |_| {
            count.set(count.get() + 1);
            let mut value = settlement_initial(true, true).second;
            value.cf_string = None;
            Ok(TextSettlementRead {
                value,
                offset_since_setter_return_ms: 221.0,
                read_elapsed_ms: 1.0,
            })
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "set_text_settlement_read_unknown");
    assert_eq!(count.get(), 1);
    let clock = std::cell::Cell::new(Duration::ZERO);
    let error = collect_settlement(
        &initial,
        || clock.get(),
        |_| {
            clock.set(Duration::from_millis(101));
            Ok(TextSettlementRead {
                value: settlement_initial(true, true).second,
                offset_since_setter_return_ms: 321.0,
                read_elapsed_ms: 101.0,
            })
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "set_text_settlement_budget_expired");
    assert_eq!(
        error.diagnostic.unwrap()["settlement"]["reads"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
