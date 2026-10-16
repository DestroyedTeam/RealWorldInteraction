//! Actual compiled Native control flow/guard tests, not operating-system effects.
use super::*;
use std::cell::Cell;
fn b(v: bool) -> FocusBooleanRead {
    FocusBooleanRead {
        native_status: 0,
        value: Some(v),
    }
}
fn facts(kind: WindowLocalKind) -> WindowLocalNativeFacts {
    WindowLocalNativeFacts {
        kind,
        platform: FocusPlatform::Macos,
        application_focus_custody: ApplicationFocusCustody::NoActiveForeignSibling,
        input_principal: ExactEvidence::Verified,
        accessibility_permission: ExactEvidence::Verified,
        retained_window: ExactEvidence::Verified,
        provider_lifetime: ExactEvidence::Verified,
        process_lifetime: ExactEvidence::Verified,
        cg_window_binding: ExactEvidence::Verified,
        native_context: ExactEvidence::Verified,
        probe_origin: ProbeOrigin::FreshExactObject,
        main_writable: b(true),
        focused_writable: b(false),
        window_main: b(false),
        application_main_window_cf_match: b(false),
        window_focused: b(false),
        application_focused_window_cf_match: b(false),
    }
}
#[test]
fn main_and_key_are_explicit_not_substituted() {
    assert_eq!(WindowLocalKind::Main.attribute(), "AXMain");
    assert_eq!(WindowLocalKind::Key.attribute(), "AXFocused");
    assert_eq!(
        refine(
            WindowLocalWritePlan::SetAttributeTrueOnce,
            &facts(WindowLocalKind::Main)
        )
        .unwrap(),
        WindowLocalWritePlan::SetAttributeTrueOnce
    );
    assert_eq!(
        refine(
            WindowLocalWritePlan::SetAttributeTrueOnce,
            &facts(WindowLocalKind::Key)
        )
        .unwrap_err()
        .code,
        "window_local_kind_unsupported"
    );
}
#[test]
fn zero_budget_never_upgrades() {
    assert_eq!(
        refine(
            WindowLocalWritePlan::NoWriteAlreadySelected,
            &facts(WindowLocalKind::Main)
        )
        .unwrap_err()
        .code,
        "window_local_zero_write_budget_changed"
    );
}
#[test]
fn one_budget_can_refine_to_zero_only_both_axes() {
    let mut f = facts(WindowLocalKind::Main);
    f.window_main = b(true);
    assert_eq!(
        refine(WindowLocalWritePlan::SetAttributeTrueOnce, &f).unwrap(),
        WindowLocalWritePlan::SetAttributeTrueOnce
    );
    f.application_main_window_cf_match = b(true);
    f.main_writable = FocusBooleanRead {
        native_status: -25204,
        value: None,
    };
    assert_eq!(
        refine(WindowLocalWritePlan::SetAttributeTrueOnce, &f).unwrap(),
        WindowLocalWritePlan::NoWriteAlreadySelected
    );
}
#[test]
fn raw_error_or_missing_chosen_axis_never_writes() {
    for raw in [
        FocusBooleanRead {
            native_status: 0,
            value: None,
        },
        FocusBooleanRead {
            native_status: -25204,
            value: Some(true),
        },
    ] {
        let mut f = facts(WindowLocalKind::Main);
        f.application_main_window_cf_match = raw;
        assert!(refine(WindowLocalWritePlan::SetAttributeTrueOnce, &f).is_err());
        let mut f = facts(WindowLocalKind::Main);
        f.main_writable = raw;
        assert!(refine(WindowLocalWritePlan::SetAttributeTrueOnce, &f).is_err());
    }
}
#[test]
fn main_selection_does_not_make_key_selected() {
    let mut f = facts(WindowLocalKind::Key);
    f.window_main = b(true);
    f.application_main_window_cf_match = b(true);
    assert_eq!(
        refine(WindowLocalWritePlan::SetAttributeTrueOnce, &f)
            .unwrap_err()
            .code,
        "window_local_kind_unsupported"
    );
}
#[test]
fn active_sibling_and_unknown_custody_refuse() {
    for state in [
        ApplicationFocusCustody::ActiveForeignSibling,
        ApplicationFocusCustody::Unknown,
    ] {
        let mut f = facts(WindowLocalKind::Main);
        f.application_focus_custody = state;
        assert!(refine(WindowLocalWritePlan::SetAttributeTrueOnce, &f).is_err());
    }
    let fg = rwi_core::ForegroundState {
        pid: 7,
        window: "cgwindow:8".into(),
        process_start: "7:birth".into(),
        native_epoch: "a".repeat(64),
    };
    assert_eq!(
        custody(7, fg.pid, Some(fg.window == "cgwindow:9"), b(false)),
        ApplicationFocusCustody::ActiveForeignSibling
    );
    assert_eq!(
        custody(7, fg.pid, Some(fg.window == "cgwindow:9"), b(true)),
        ApplicationFocusCustody::Unknown
    );
    assert_eq!(
        custody(6, fg.pid, Some(fg.window == "cgwindow:9"), b(false)),
        ApplicationFocusCustody::NoActiveForeignSibling
    );
}
#[test]
fn every_identity_proof_is_required() {
    for index in 0..7 {
        let mut f = facts(WindowLocalKind::Main);
        let proof = match index {
            0 => &mut f.input_principal,
            1 => &mut f.accessibility_permission,
            2 => &mut f.retained_window,
            3 => &mut f.provider_lifetime,
            4 => &mut f.process_lifetime,
            5 => &mut f.cg_window_binding,
            _ => &mut f.native_context,
        };
        *proof = ExactEvidence::Unknown;
        assert!(refine(WindowLocalWritePlan::SetAttributeTrueOnce, &f).is_err());
    }
    let mut f = facts(WindowLocalKind::Main);
    f.probe_origin = ProbeOrigin::ObservationSidecar;
    assert!(refine(WindowLocalWritePlan::SetAttributeTrueOnce, &f).is_err());
}
#[test]
fn independent_nonatomic_reads_cannot_false_match() {
    assert_eq!(consistent(b(true), b(false)).value, None);
    assert_eq!(
        consistent(
            b(true),
            FocusBooleanRead {
                native_status: -25204,
                value: None
            }
        )
        .native_status,
        -25204
    );
    assert_eq!(consistent(b(false), b(false)).value, Some(false));
}
#[test]
fn native_once_has_one_call_and_no_retry_on_raw_error() {
    for status in [0, -25204, -25205] {
        let n = Cell::new(0);
        let (p, (), raw) = once(
            || Ok((WindowLocalWritePlan::SetAttributeTrueOnce, ())),
            |p, s| Ok((p, s)),
            |_| {
                n.set(n.get() + 1);
                Ok(status)
            },
        )
        .unwrap();
        assert_eq!(p, WindowLocalWritePlan::SetAttributeTrueOnce);
        assert_eq!(raw, Some(status));
        assert_eq!(n.get(), 1);
    }
}
#[test]
fn final_gate_failures_or_zero_never_enter_setter() {
    let n = Cell::new(0);
    let e = once(
        || Ok((WindowLocalWritePlan::NoWriteAlreadySelected, ())),
        |_, s| Ok((WindowLocalWritePlan::SetAttributeTrueOnce, s)),
        |_| {
            n.set(n.get() + 1);
            Ok(0)
        },
    )
    .unwrap_err();
    assert_eq!(e.code, "window_local_zero_write_budget_changed");
    assert_eq!(n.get(), 0);
    let (_, (), status) = once(
        || Ok((WindowLocalWritePlan::SetAttributeTrueOnce, ())),
        |_, s| Ok((WindowLocalWritePlan::NoWriteAlreadySelected, s)),
        |_| {
            n.set(n.get() + 1);
            Ok(0)
        },
    )
    .unwrap();
    assert_eq!(status, None);
    assert_eq!(n.get(), 0);
    let e = once::<(), i32>(
        || Ok((WindowLocalWritePlan::SetAttributeTrueOnce, ())),
        |_, _| Err(fault("exact_changed", "no write")),
        |_| {
            n.set(n.get() + 1);
            Ok(0)
        },
    )
    .unwrap_err();
    assert_eq!(e.code, "exact_changed");
    assert_eq!(n.get(), 0);
}

#[test]
fn f14_local_public_application_custody_other_app_does_not_need_a_full_window() {
    for kind in [WindowLocalKind::Main, WindowLocalKind::Key] {
        let mut f = facts(kind);
        f.application_focus_custody = custody(7, 8, None, b(false));
        assert_eq!(
            f.application_focus_custody,
            ApplicationFocusCustody::NoActiveForeignSibling
        );
        if kind == WindowLocalKind::Key {
            f.focused_writable = b(true);
        }
        assert_eq!(
            refine(WindowLocalWritePlan::SetAttributeTrueOnce, &f).unwrap(),
            WindowLocalWritePlan::SetAttributeTrueOnce
        );
    }
}
#[test]
fn f14_local_public_application_custody_same_app_keeps_full_window_and_sibling_guard() {
    assert_eq!(
        custody(7, 7, Some(true), b(true)),
        ApplicationFocusCustody::NoActiveForeignSibling
    );
    for match_window in [None, Some(false)] {
        assert_eq!(
            custody(7, 7, match_window, b(true)),
            ApplicationFocusCustody::Unknown
        );
    }
    assert_eq!(
        custody(7, 7, Some(true), b(false)),
        ApplicationFocusCustody::ActiveForeignSibling
    );
    assert_eq!(
        custody(7, 7, Some(false), b(false)),
        ApplicationFocusCustody::ActiveForeignSibling
    );
}
#[test]
fn f14_local_public_application_custody_unknown_pid_or_raw_axis_refuses() {
    for (bound, public) in [(0, 7), (7, 0), (0, 0)] {
        assert_eq!(
            custody(bound, public, None, b(true)),
            ApplicationFocusCustody::Unknown
        );
    }
    for raw in [
        FocusBooleanRead {
            native_status: -25204,
            value: Some(true),
        },
        FocusBooleanRead {
            native_status: 0,
            value: None,
        },
    ] {
        let mut f = facts(WindowLocalKind::Main);
        f.application_focus_custody = custody(7, 8, None, raw);
        assert!(refine(WindowLocalWritePlan::SetAttributeTrueOnce, &f).is_err());
    }
}
#[test]
fn f14_local_public_application_custody_full_tuple_remains_exact() {
    let before = rwi_core::ForegroundState {
        pid: 7,
        window: "cgwindow:8".into(),
        process_start: "7:birth".into(),
        native_epoch: "a".repeat(64),
    };
    assert!(same_full_foreground(Some(&before), Some(&before)));
    assert!(same_full_foreground(None, None)); // Only paired public Native witnesses may use this branch.
    assert!(!same_full_foreground(Some(&before), None));
    assert!(!same_full_foreground(None, Some(&before)));
    for index in 0..4 {
        let mut after = before.clone();
        match index {
            0 => after.pid = 8,
            1 => after.window = "cgwindow:9".into(),
            2 => after.process_start = "7:other-birth".into(),
            _ => after.native_epoch = "b".repeat(64),
        }
        assert!(!same_full_foreground(Some(&before), Some(&after)));
    }
}
#[test]
fn f14_local_public_application_custody_boundary_or_raw_error_never_retries() {
    let n = Cell::new(0);
    let error = once::<(), i32>(
        || Ok((WindowLocalWritePlan::SetAttributeTrueOnce, ())),
        |_, _| {
            Err(fault(
                "focus_public_application_changed",
                "exact Native witness changed",
            ))
        },
        |_| {
            n.set(n.get() + 1);
            Ok(0)
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "focus_public_application_changed");
    assert_eq!(n.get(), 0);
    let (_, (), result) = once(
        || Ok((WindowLocalWritePlan::SetAttributeTrueOnce, ())),
        |p, s| Ok((p, s)),
        |_| {
            n.set(n.get() + 1);
            Ok(-25204)
        },
    )
    .unwrap();
    assert_eq!(result, Some(-25204));
    assert_eq!(n.get(), 1);
}
#[test]
fn f14_local_public_application_custody_response_is_not_a_global_or_typing_ticket() {
    let raw = WindowLocalReadback {
        kind: WindowLocalKind::Main,
        write_plan: WindowLocalWritePlan::NoWriteAlreadySelected,
        setter_attempt: FocusSetterAttempt::NotAttempted,
        setter_status: None,
        window_main: b(true),
        application_main_window_cf_match: b(true),
        window_focused: b(false),
        application_focused_window_cf_match: b(false),
        os_foreground_before: None,
        os_foreground_after: None,
        public_application_custody: Some(
            rwi_core::semantic_focus::FocusPublicApplicationReadback {
                scope: "public_frontmost_application_identity_only",
                before_pid: 8,
                before_process_start: "8:birth".into(),
                after_pid: 8,
                after_process_start: "8:birth".into(),
                retained_application_equal: Some(true),
                process_lifetime_equal: Some(true),
                full_foreground_window_known: false,
            },
        ),
    };
    let summary = rwi_core::window_local::summarize_window_local(raw);
    assert!(summary.selected_goal_verified);
    assert!(!summary.typing_authorized);
    assert_eq!(summary.semantic_element_ready, None);
    assert_eq!(summary.os_foreground_unchanged, None);
    let json = serde_json::to_value(summary).unwrap();
    assert!(json["os_foreground_before"].is_null());
    assert!(json["os_foreground_after"].is_null());
    assert!(json["os_foreground_unchanged"].is_null());
    assert_eq!(
        json["public_application_custody"]["full_foreground_window_known"],
        false
    );
    assert_eq!(
        json["public_application_custody"]["retained_application_equal"],
        true
    );
}
