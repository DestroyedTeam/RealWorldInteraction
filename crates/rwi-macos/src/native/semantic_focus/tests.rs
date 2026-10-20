// Pure tests of the actual Native route guards; not OS-native goal proof.
use super::*;
use std::cell::Cell;
#[test]
fn detailed_boolean_separates_copy_error_null_and_nonboolean_without_extra_reads() {
    let reads = Cell::new(0);
    for (status, value, cause, expected_reads) in [
        (
            -25204,
            Some(true),
            Some(FocusProbeUnknownCause::CopyAttributeFailed),
            0,
        ),
        (0, None, Some(FocusProbeUnknownCause::CopyReturnedNull), 0),
        (
            0,
            Some(false),
            Some(FocusProbeUnknownCause::NotCfBoolean),
            1,
        ),
    ] {
        reads.set(0);
        let p = classify_boolean_probe(status, value, |_| {
            reads.set(reads.get() + 1);
            None
        });
        assert_eq!(p.raw.native_status, status);
        assert_eq!(p.raw.value, None);
        assert_eq!(p.unknown_cause, cause);
        assert_eq!(reads.get(), expected_reads);
        assert!(p.validation_error_code.is_none());
    }
    for value in [false, true] {
        let p = classify_boolean_probe(0, Some(value), |v| Some(*v));
        assert_eq!(p.raw.value, Some(value));
        assert!(p.unknown_cause.is_none());
    }
}
#[test]
fn detailed_cf_match_separates_type_owner_provider_and_preserves_short_circuit() {
    for (stage, cause, expected_calls, code) in [
        (
            0,
            Some(FocusProbeUnknownCause::CopyAttributeFailed),
            [0, 0, 0, 0],
            None,
        ),
        (
            1,
            Some(FocusProbeUnknownCause::CopyReturnedNull),
            [0, 0, 0, 0],
            None,
        ),
        (
            2,
            Some(FocusProbeUnknownCause::NotAxElement),
            [1, 0, 0, 0],
            Some("focus_native_type_unknown"),
        ),
        (
            3,
            Some(FocusProbeUnknownCause::OwnerLookupFailed),
            [1, 1, 0, 0],
            Some("pid_unknown"),
        ),
        (
            4,
            Some(FocusProbeUnknownCause::ProviderValidationFailed),
            [1, 1, 1, 0],
            Some("delegate_unverified"),
        ),
        (5, None, [1, 1, 1, 1], None),
    ] {
        let calls = [Cell::new(0), Cell::new(0), Cell::new(0), Cell::new(0)];
        let p = classify_cf_focus_probe(
            if stage == 0 { -25204 } else { 0 },
            (stage != 1).then_some(()),
            |_| {
                calls[0].set(calls[0].get() + 1);
                if stage == 2 {
                    Err(fault("focus_native_type_unknown", "test"))
                } else {
                    Ok(())
                }
            },
            |_| {
                calls[1].set(calls[1].get() + 1);
                if stage == 3 {
                    Err(fault("pid_unknown", "test"))
                } else {
                    Ok(42)
                }
            },
            |_| {
                calls[2].set(calls[2].get() + 1);
                if stage == 4 {
                    Err(fault("delegate_unverified", "test"))
                } else {
                    Ok(())
                }
            },
            |_| {
                calls[3].set(calls[3].get() + 1);
                false
            },
        );
        assert_eq!(calls.map(|c| c.get()), expected_calls);
        assert_eq!(p.unknown_cause, cause);
        assert_eq!(p.validation_error_code.as_deref(), code);
        assert_eq!(p.raw.native_status, if stage == 0 { -25204 } else { 0 });
        assert_eq!(p.raw.value, if stage == 5 { Some(false) } else { None });
    }
}
#[test]
fn diagnostic_pair_preserves_all_status_value_pairs_without_reason_driven_disagreement() {
    for a_status in [0, -25204] {
        for a_value in [None, Some(false), Some(true)] {
            for b_status in [0, -25204] {
                for b_value in [None, Some(false), Some(true)] {
                    let pair = FocusReadbackAxisPair::new(
                        FocusReadbackProbe::known(FocusBooleanRead {
                            native_status: a_status,
                            value: a_value,
                        }),
                        FocusReadbackProbe::known(FocusBooleanRead {
                            native_status: b_status,
                            value: b_value,
                        }),
                    );
                    assert_eq!(pair.first.raw.native_status, a_status);
                    assert_eq!(pair.first.raw.value, a_value);
                    assert_eq!(pair.second.raw.native_status, b_status);
                    assert_eq!(pair.second.raw.value, b_value);
                    assert_eq!(pair.native_status_changed, a_status != b_status);
                    assert_eq!(pair.value_changed, a_value != b_value);
                }
            }
        }
    }
    let pair = FocusReadbackAxisPair::new(
        FocusReadbackProbe::unknown(0, FocusProbeUnknownCause::NotCfBoolean, None),
        FocusReadbackProbe::unknown(0, FocusProbeUnknownCause::CopyReturnedNull, None),
    );
    assert!(!pair.native_status_changed && !pair.value_changed);
}
fn facts(focused: bool) -> FocusNativeFacts {
    let yes = FocusBooleanRead {
        native_status: 0,
        value: Some(true),
    };
    FocusNativeFacts {
        platform: FocusPlatform::Macos,
        application_focus_custody: ApplicationFocusCustody::NoActiveForeignSibling,
        input_principal: ExactEvidence::Verified,
        accessibility_permission: ExactEvidence::Verified,
        retained_element: ExactEvidence::Verified,
        element_window_cf: ExactEvidence::Verified,
        process_lifetime: ExactEvidence::Verified,
        cg_window_binding: ExactEvidence::Verified,
        native_context: ExactEvidence::Verified,
        editable_class: EditableClass::PlainEditable,
        probe_origin: ProbeOrigin::FreshExactObject,
        value_writable: yes,
        focused_writable: yes,
        focused: FocusBooleanRead {
            native_status: 0,
            value: Some(focused),
        },
    }
}
#[test]
fn fresh_focused_refines_one_write_budget_to_zero_without_ack() {
    let n = Cell::new(0);
    let x = once(
        || {
            Ok((
                refine_plan(FocusWritePlan::SetAXFocusedTrueOnce, &facts(true))?,
                (),
            ))
        },
        |plan, sample| Ok((plan, sample)),
        |_| {
            n.set(n.get() + 1);
            Ok(0)
        },
    )
    .unwrap();
    assert_eq!(x.0, FocusWritePlan::NoWriteAlreadyFocused);
    assert_eq!(x.2, None);
    assert_eq!(n.get(), 0);
}
#[test]
fn zero_write_ceiling_never_upgrades_after_focus_changes() {
    assert_eq!(
        refine_plan(FocusWritePlan::NoWriteAlreadyFocused, &facts(false))
            .unwrap_err()
            .code,
        "focus_zero_write_budget_changed"
    );
}
#[test]
fn failed_native_status_is_returned_once_not_retried() {
    let n = Cell::new(0);
    let x = once(
        || Ok((FocusWritePlan::SetAXFocusedTrueOnce, ())),
        |plan, sample| Ok((plan, sample)),
        |_| {
            n.set(n.get() + 1);
            Ok(-25204)
        },
    )
    .unwrap();
    assert_eq!(x.2, Some(-25204));
    assert_eq!(n.get(), 1);
}
#[test]
fn before_boundary_error_never_enters_write() {
    let n = Cell::new(0);
    let x = once::<(), i32>(
        || Err(fault("changed", "no write")),
        |plan, sample| Ok((plan, sample)),
        |_| {
            n.set(n.get() + 1);
            Ok(0)
        },
    );
    assert!(x.is_err());
    assert_eq!(n.get(), 0);
}
#[test]
fn native_guard_rechecks_every_exact_evidence_axis() {
    for axis in 0..7 {
        let mut f = facts(false);
        let p = match axis {
            0 => &mut f.input_principal,
            1 => &mut f.accessibility_permission,
            2 => &mut f.retained_element,
            3 => &mut f.element_window_cf,
            4 => &mut f.process_lifetime,
            5 => &mut f.cg_window_binding,
            _ => &mut f.native_context,
        };
        *p = ExactEvidence::Unknown;
        assert!(refine_plan(FocusWritePlan::SetAXFocusedTrueOnce, &f).is_err());
    }
}
#[test]
fn sidecar_and_unclassified_class_are_not_native_authority() {
    for class in [
        EditableClass::PlainReadOnly,
        EditableClass::Secure,
        EditableClass::NotText,
        EditableClass::Unknown,
    ] {
        let mut f = facts(false);
        f.editable_class = class;
        assert!(refine_plan(FocusWritePlan::SetAXFocusedTrueOnce, &f).is_err());
    }
    let mut f = facts(false);
    f.probe_origin = ProbeOrigin::ObservationSidecar;
    assert!(refine_plan(FocusWritePlan::SetAXFocusedTrueOnce, &f).is_err());
}
#[test]
fn raw_probe_failure_cannot_be_overridden_by_true() {
    let mut f = facts(false);
    f.focused_writable = FocusBooleanRead {
        native_status: -25204,
        value: Some(true),
    };
    assert!(refine_plan(FocusWritePlan::SetAXFocusedTrueOnce, &f).is_err());
    f.focused_writable = FocusBooleanRead {
        native_status: 0,
        value: None,
    };
    assert!(refine_plan(FocusWritePlan::SetAXFocusedTrueOnce, &f).is_err());
}
#[test]
fn foreground_pid_alone_cannot_isolate_a_user_sibling() {
    let no = FocusBooleanRead {
        native_status: 0,
        value: Some(false),
    };
    assert_eq!(
        application_custody(7, 7, Some(false), no),
        ApplicationFocusCustody::ActiveForeignSibling
    );
    assert_eq!(
        application_custody(7, 8, None, no),
        ApplicationFocusCustody::NoActiveForeignSibling
    );
    assert_eq!(
        application_custody(
            7,
            8,
            None,
            FocusBooleanRead {
                native_status: 0,
                value: None
            }
        ),
        ApplicationFocusCustody::Unknown
    );
    assert_eq!(
        application_custody(
            7,
            8,
            None,
            FocusBooleanRead {
                native_status: -25204,
                value: Some(true)
            }
        ),
        ApplicationFocusCustody::Unknown
    );
}
#[test]
fn last_boundary_may_refine_to_zero_without_dispatch() {
    let n = Cell::new(0);
    let x = once(
        || Ok((FocusWritePlan::SetAXFocusedTrueOnce, ())),
        |_, sample| Ok((FocusWritePlan::NoWriteAlreadyFocused, sample)),
        |_| {
            n.set(n.get() + 1);
            Ok(0)
        },
    )
    .unwrap();
    assert_eq!(x.0, FocusWritePlan::NoWriteAlreadyFocused);
    assert_eq!(x.2, None);
    assert_eq!(n.get(), 0);
}
#[test]
fn last_boundary_cannot_upgrade_even_if_its_caller_is_wrong() {
    let n = Cell::new(0);
    let x = once(
        || Ok((FocusWritePlan::NoWriteAlreadyFocused, ())),
        |_, sample| Ok((FocusWritePlan::SetAXFocusedTrueOnce, sample)),
        |_| {
            n.set(n.get() + 1);
            Ok(0)
        },
    );
    assert_eq!(x.unwrap_err().code, "focus_zero_write_budget_changed");
    assert_eq!(n.get(), 0);
}

#[test]
fn real_native_binding_guard_accepts_only_pixel_hash_difference() {
    let a: Binding = serde_json::from_value(serde_json::json!({
        "scope":{"window":"cgwindow:7","pid":7,"element":null},
        "focus_window":"cgwindow:7","layout":"layout","content":"pixel_hash",
        "native_epoch":"native_epoch","mutation_context":{"window_title":"owned","document_state":null}
    })).unwrap();
    let mut b = a.clone();
    b.content = "native_epoch".into();
    assert!(same_native_binding(&a, &b));
    for axis in 0..5 {
        let mut changed = b.clone();
        match axis {
            0 => changed.scope.pid = 8,
            1 => changed.focus_window = "cgwindow:8".into(),
            2 => changed.layout = "new_layout".into(),
            3 => changed.native_epoch = "new_epoch".into(),
            _ => changed.mutation_context = None,
        }
        assert!(!same_native_binding(&a, &changed));
    }
}

#[test]
fn conflicting_same_pid_foreground_window_cannot_grant_focus_custody() {
    let yes = FocusBooleanRead {
        native_status: 0,
        value: Some(true),
    };
    assert_eq!(
        application_custody(7, 7, Some(false), yes),
        ApplicationFocusCustody::Unknown
    );
    assert_eq!(
        application_custody(7, 7, None, yes),
        ApplicationFocusCustody::Unknown
    );
    assert_eq!(
        application_custody(7, 7, Some(true), yes),
        ApplicationFocusCustody::NoActiveForeignSibling
    );
    assert_eq!(
        application_custody(7, 8, None, yes),
        ApplicationFocusCustody::NoActiveForeignSibling
    );
}

#[test]
fn f13_public_application_custody_different_app_never_probes_foreign_window() {
    let reads = Cell::new(0);
    let full = read_full_foreground_if_same_application(7, 8, || {
        reads.set(reads.get() + 1);
        Err(fault(
            "ax_attribute_unavailable",
            "Unrelated app AX window unavailable.",
        ))
    })
    .unwrap();
    assert!(full.is_none());
    assert_eq!(reads.get(), 0);
    for ids in [(0, 8), (7, 0)] {
        assert!(
            read_full_foreground_if_same_application(ids.0, ids.1, || {
                reads.set(reads.get() + 1);
                Err(fault("test", "Must not run."))
            })
            .is_err()
        );
    }
    assert_eq!(reads.get(), 0);
}
#[test]
fn f13_public_application_custody_same_app_keeps_full_window_errors_and_identity() {
    let reads = Cell::new(0);
    let error = read_full_foreground_if_same_application(7, 7, || {
        reads.set(reads.get() + 1);
        Err(fault(
            "ax_attribute_unavailable",
            "Actual same-app window unavailable.",
        ))
    })
    .unwrap_err();
    assert_eq!(error.code, "ax_attribute_unavailable");
    assert_eq!(reads.get(), 1);
    let full = rwi_core::ForegroundState {
        pid: 7,
        window: "cgwindow:9".into(),
        process_start: "7:1:2".into(),
        native_epoch: "a".repeat(64),
    };
    assert_eq!(
        read_full_foreground_if_same_application(7, 7, || Ok(full.clone())).unwrap(),
        Some(full.clone())
    );
    let mut wrong = full;
    wrong.pid = 8;
    wrong.process_start = "8:1:2".into();
    assert_eq!(
        read_full_foreground_if_same_application(7, 7, || Ok(wrong))
            .unwrap_err()
            .code,
        "focus_public_application_changed"
    );
}
#[test]
fn f13_public_application_custody_object_pid_birth_and_unknown_each_refuse() {
    let before = PublicApplicationIdentity {
        pid: 8,
        process_start: "8:1:2".into(),
    };
    let same = PublicApplicationIdentity {
        pid: 8,
        process_start: "8:1:2".into(),
    };
    assert!(require_public_application_pair(&before, &same, Some(true)).is_ok());
    for (pid, birth, equal) in [
        (9, "9:1:2", Some(true)),
        (8, "8:1:3", Some(true)),
        (8, "8:1:2", Some(false)),
        (8, "8:1:2", None),
    ] {
        let after = PublicApplicationIdentity {
            pid,
            process_start: birth.into(),
        };
        assert_eq!(
            require_public_application_pair(&before, &after, equal)
                .unwrap_err()
                .code,
            "focus_public_application_changed"
        );
    }
}
#[test]
fn f13_public_application_custody_selected_unknown_and_same_app_sibling_stay_refused() {
    let yes = FocusBooleanRead {
        native_status: 0,
        value: Some(true),
    };
    let no = FocusBooleanRead {
        native_status: 0,
        value: Some(false),
    };
    let unknown = FocusBooleanRead {
        native_status: -25204,
        value: None,
    };
    assert_eq!(
        application_custody(7, 8, None, yes),
        ApplicationFocusCustody::NoActiveForeignSibling
    );
    assert_eq!(
        application_custody(7, 8, None, unknown),
        ApplicationFocusCustody::Unknown
    );
    assert_eq!(
        application_custody(7, 7, Some(false), no),
        ApplicationFocusCustody::ActiveForeignSibling
    );
    assert_eq!(
        application_custody(7, 7, None, yes),
        ApplicationFocusCustody::Unknown
    );
    assert_eq!(
        application_custody(7, 7, Some(true), yes),
        ApplicationFocusCustody::NoActiveForeignSibling
    );
}
#[test]
fn f13_public_application_custody_boundary_change_stops_before_only_setter() {
    let writes = Cell::new(0);
    let before = PublicApplicationIdentity {
        pid: 8,
        process_start: "8:1:2".into(),
    };
    let changed = PublicApplicationIdentity {
        pid: 8,
        process_start: "8:1:3".into(),
    };
    let result = once(
        || Ok((FocusWritePlan::SetAXFocusedTrueOnce, ())),
        |plan, token| {
            require_public_application_pair(&before, &changed, Some(true))?;
            Ok((plan, token))
        },
        |_| {
            writes.set(writes.get() + 1);
            Ok(0)
        },
    );
    assert!(result.is_err());
    assert_eq!(writes.get(), 0);
}
#[test]
fn f13_public_application_custody_response_keeps_global_window_unknown() {
    let yes = FocusBooleanRead {
        native_status: 0,
        value: Some(true),
    };
    let pair = || {
        FocusReadbackAxisPair::new(
            FocusReadbackProbe::known(yes),
            FocusReadbackProbe::known(yes),
        )
    };
    let mut diagnostic = FocusReadbackDiagnostics::new(pair(), pair(), pair());
    diagnostic.public_application_custody = Some(FocusPublicApplicationReadback {
        scope: "public_frontmost_application_identity_only",
        before_pid: 8,
        before_process_start: "8:1:2".into(),
        after_pid: 8,
        after_process_start: "8:1:2".into(),
        retained_application_equal: Some(true),
        process_lifetime_equal: Some(true),
        full_foreground_window_known: false,
    });
    let summary = rwi_core::semantic_focus::summarize_focus_readback(FocusReadback {
        write_plan: FocusWritePlan::SetAXFocusedTrueOnce,
        setter_attempt: FocusSetterAttempt::Attempted,
        setter_status: Some(0),
        element_focused: yes,
        focused_ui_element_cf_match: yes,
        application_focused_window_cf_match: yes,
        os_foreground_before: None,
        os_foreground_after: None,
        readback_diagnostics: Some(diagnostic),
    });
    assert_eq!(
        summary.state,
        rwi_core::semantic_focus::FocusReadbackState::Ready
    );
    assert!(summary.goal_verified); // Exact local3axes, not a global foreground goal.
    assert!(summary.os_foreground_before.is_none() && summary.os_foreground_after.is_none());
    assert_eq!(summary.os_foreground_unchanged, None);
    let json = serde_json::to_value(summary).unwrap();
    assert_eq!(json["readback_diagnostics"]["input_authority"], false);
    assert_eq!(
        json["readback_diagnostics"]["public_application_custody"]["full_foreground_window_known"],
        false
    );
    assert!(json["os_foreground_before"].is_null());
    assert!(json["os_foreground_after"].is_null());
    assert!(json["os_foreground_unchanged"].is_null());
}
