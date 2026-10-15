use super::*;
use crate::{Binding, Element, Identity, MutationContext, Scope};
fn scenes() -> (Scene, Scene, Resolved) {
    let id = Identity {
        host: "mock-host".into(),
        guest: None,
        session: "mock-session".into(),
        seat: "mock-seat".into(),
        desktop: "mock-desktop".into(),
    };
    let element = Element {
        id: "mock-element".into(),
        name: "plain editable fixture".into(),
        role: Some("AXTextField".into()),
        actions: vec![],
        enabled: Some(true),
        selected: None,
        description: None,
        value: Some("fixture".into()),
        value_truncated: false,
        point: [10., 20.],
        visible: Some(true),
        occluded: Some(false),
        state_revision: Some("fixture-revision".into()),
    };
    let binding = Binding {
        scope: Scope {
            window: "cgwindow:7".into(),
            pid: 42,
            element: None,
        },
        focus_window: "cgwindow:7".into(),
        layout: "layout".into(),
        content: "pixels".into(),
        native_epoch: "a".repeat(64),
        mutation_context: Some(MutationContext {
            window_title: "mock-window".into(),
            document_state: Some("mock-document".into()),
        }),
    };
    let scene = Scene {
        identity: id,
        assurance: "synthetic-source4-only".into(),
        binding: Some(binding),
        mutation_ready: true,
        blocked: Some(false),
        image_size: [64, 64],
        pixel_to_desktop: None,
        elements: vec![element.clone()],
        captured_unix_ms: 1,
        atomicity: "mock-only".into(),
        image_normalization: "none".into(),
        png: vec![],
    };
    let reference = Resolved {
        native_action: None,
        reference: "mock-reference".into(),
        target: "mock-target".into(),
        frame: "mock-frame".into(),
        point: element.point,
        scope: Scope {
            window: "cgwindow:7".into(),
            pid: 42,
            element: Some(element.id.clone()),
        },
        element_revision: Some(element.revision()),
        placement_only: false,
        source_region: None,
        pixel_anchor: None,
        pixel_anchor_used: false,
        red_marker_anchor: None,
    };
    (scene.clone(), scene, reference)
}

fn request() -> FocusElementRequest {
    FocusElementRequest {
        target: "mock-target".into(),
        frame: "mock-frame".into(),
        element: "mock-element".into(),
        request_id: "mock-request".into(),
    }
}
fn native() -> FocusNativeFacts {
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
            value: Some(false),
        },
    }
}
fn custody() -> FocusCustody {
    FocusCustody {
        lease: LeaseCustody::HeldExactDomainTarget,
        journal: JournalCustody::NewExactReservation,
    }
}
fn assess(native: &FocusNativeFacts, custody: FocusCustody) -> Result<FocusWritePlan> {
    let (old, fresh, reference) = scenes();
    let request = request();
    assess_focus_element(
        FocusPolicyContext {
            request: &request,
            reference: &reference,
            original: &old,
            fresh: &fresh,
            proposal_age_ms: 1,
        },
        native,
        custody,
    )
}
fn readback() -> FocusReadback {
    let yes = FocusBooleanRead {
        native_status: 0,
        value: Some(true),
    };
    FocusReadback {
        write_plan: FocusWritePlan::SetAXFocusedTrueOnce,
        setter_attempt: FocusSetterAttempt::Attempted,
        setter_status: Some(0),
        element_focused: yes,
        focused_ui_element_cf_match: yes,
        application_focused_window_cf_match: yes,
        os_foreground_before: None,
        os_foreground_after: None,
        readback_diagnostics: None,
    }
}
#[test]
fn strict_request_has_no_action_value_or_fallback() {
    for extra in ["value", "options", "action", "fallback"] {
        let mut value = serde_json::to_value(request()).unwrap();
        value[extra] = serde_json::json!(true);
        assert!(serde_json::from_value::<FocusElementRequest>(value).is_err());
    }
    let mut empty = request();
    empty.frame.clear();
    assert_eq!(
        empty.validate().unwrap_err().code,
        "invalid_focus_element_request"
    );
}
#[test]
fn raw_probe_error_cannot_be_overridden_by_true() {
    let mut n = native();
    n.focused_writable = FocusBooleanRead {
        native_status: -25204,
        value: Some(true),
    };
    let e = assess(&n, custody()).unwrap_err();
    assert_eq!(e.code, "focus_native_probe_unknown");
    assert_eq!(e.diagnostic.unwrap()["native_status"], -25204);
    n.focused_writable = FocusBooleanRead {
        native_status: 0,
        value: None,
    };
    assert_eq!(
        assess(&n, custody()).unwrap_err().code,
        "focus_native_probe_unknown"
    );
}
#[test]
fn observation_sidecar_or_budget_unknown_is_not_a_ticket() {
    for origin in [ProbeOrigin::ObservationSidecar, ProbeOrigin::Unknown] {
        let mut n = native();
        n.probe_origin = origin;
        assert_eq!(
            assess(&n, custody()).unwrap_err().code,
            "fresh_focus_probe_required"
        );
    }
}
#[test]
fn lease_and_durable_request_require_exact_new_custody() {
    let mut c = custody();
    c.lease = LeaseCustody::Expired;
    assert_eq!(
        assess(&native(), c).unwrap_err().code,
        "focus_lease_unavailable"
    );
    for journal in [
        JournalCustody::PendingOrUncertain,
        JournalCustody::Conflict,
        JournalCustody::AlreadyAttempted,
    ] {
        let mut c = custody();
        c.journal = journal;
        assert_eq!(
            assess(&native(), c).unwrap_err().code,
            "focus_request_not_new"
        );
    }
    assert_eq!(
        assess(&native(), custody()).unwrap(),
        FocusWritePlan::SetAXFocusedTrueOnce
    );
}
#[test]
fn user_sibling_and_unknown_app_custody_cannot_produce_a_plan() {
    for app in [
        ApplicationFocusCustody::ActiveForeignSibling,
        ApplicationFocusCustody::Unknown,
    ] {
        let mut n = native();
        n.application_focus_custody = app;
        assert_eq!(
            assess(&n, custody()).unwrap_err().code,
            "focus_application_custody_unknown"
        );
    }
}
#[test]
fn plain_readonly_secure_nottext_and_other_platforms_refuse() {
    for class in [
        EditableClass::PlainReadOnly,
        EditableClass::Secure,
        EditableClass::NotText,
        EditableClass::Unknown,
    ] {
        let mut n = native();
        n.editable_class = class;
        assert_eq!(
            assess(&n, custody()).unwrap_err().code,
            "plain_editable_element_required"
        );
    }
    for platform in [
        FocusPlatform::Windows,
        FocusPlatform::Linux,
        FocusPlatform::Other,
    ] {
        let mut n = native();
        n.platform = platform;
        assert_eq!(
            assess(&n, custody()).unwrap_err().code,
            "capability_unavailable"
        );
    }
}
#[test]
fn visibility_and_escaped_pixel_fallback_remain_rejected() {
    let (old, mut fresh, mut reference) = scenes();
    let req = request();
    let n = native();
    fresh.elements[0].visible = None;
    assert_eq!(
        assess_focus_element(
            FocusPolicyContext {
                request: &req,
                reference: &reference,
                original: &old,
                fresh: &fresh,
                proposal_age_ms: 1
            },
            &n,
            custody()
        )
        .unwrap_err()
        .code,
        "element_changed"
    );
    fresh = old.clone();
    reference.pixel_anchor_used = true;
    assert_eq!(
        assess_focus_element(
            FocusPolicyContext {
                request: &req,
                reference: &reference,
                original: &old,
                fresh: &fresh,
                proposal_age_ms: 1
            },
            &n,
            custody()
        )
        .unwrap_err()
        .code,
        "exact_semantic_element_required"
    );
}
#[test]
fn three_local_axes_keep_partial_and_unknown_separate() {
    let mut r = readback();
    r.application_focused_window_cf_match.value = Some(false);
    let s = summarize_focus_readback(r);
    assert_eq!(s.state, FocusReadbackState::Partial);
    assert!(!s.goal_verified);
    let mut r = readback();
    r.focused_ui_element_cf_match.native_status = -25204;
    let s = summarize_focus_readback(r);
    assert_eq!(s.state, FocusReadbackState::Unknown);
    assert!(!s.goal_verified);
}
#[test]
fn foreground_changes_are_reported_not_masked_or_used_as_local_focus() {
    let fg = |pid| ForegroundState {
        pid,
        window: "cgwindow:9".into(),
        process_start: "mock-start".into(),
        native_epoch: "b".repeat(64),
    };
    let mut r = readback();
    r.os_foreground_before = Some(fg(42));
    r.os_foreground_after = Some(fg(43));
    let s = summarize_focus_readback(r);
    assert_eq!(s.os_foreground_unchanged, Some(false));
    assert!(s.goal_verified);
}
#[test]
fn no_write_and_attempted_unknown_ack_are_not_conflated() {
    let mut r = readback();
    r.write_plan = FocusWritePlan::NoWriteAlreadyFocused;
    r.setter_attempt = FocusSetterAttempt::NotAttempted;
    r.setter_status = None;
    let s = summarize_focus_readback(r);
    assert_eq!(s.state, FocusReadbackState::Ready);
    assert_eq!(s.setter_acknowledged, None);
    let mut r = readback();
    r.setter_status = None;
    let s = summarize_focus_readback(r);
    assert_eq!(s.state, FocusReadbackState::Unknown);
    assert_eq!(s.setter_acknowledged, None);
}
fn diagnostic_pair(
    first: FocusReadbackProbe,
    second: FocusReadbackProbe,
) -> FocusReadbackDiagnostics {
    let yes = FocusReadbackProbe::known(FocusBooleanRead {
        native_status: 0,
        value: Some(true),
    });
    FocusReadbackDiagnostics::new(
        FocusReadbackAxisPair::new(first, second),
        FocusReadbackAxisPair::new(yes.clone(), yes.clone()),
        FocusReadbackAxisPair::new(yes.clone(), yes),
    )
}
#[test]
fn focus_unknown_diagnostic_preserves_both_samples_without_promoting_ready() {
    let mut r = readback();
    r.element_focused.value = None;
    r.readback_diagnostics = Some(diagnostic_pair(
        FocusReadbackProbe::known(FocusBooleanRead {
            native_status: 0,
            value: Some(false),
        }),
        FocusReadbackProbe::known(FocusBooleanRead {
            native_status: 0,
            value: Some(true),
        }),
    ));
    let s = summarize_focus_readback(r);
    assert_eq!(s.state, FocusReadbackState::Unknown);
    assert!(!s.goal_verified);
    let v = serde_json::to_value(s).unwrap();
    let d = &v["readback_diagnostics"];
    assert_eq!(d["element_focused"]["first"]["value"], false);
    assert_eq!(d["element_focused"]["second"]["value"], true);
    assert_eq!(d["element_focused"]["native_status_changed"], false);
    assert_eq!(d["element_focused"]["value_changed"], true);
    assert_eq!(d["input_authority"], false);
    assert_eq!(d["non_atomic_samples"], true);
}
#[test]
fn focus_status_zero_null_and_nonboolean_causes_are_not_sample_disagreement() {
    for cause in [
        FocusProbeUnknownCause::CopyReturnedNull,
        FocusProbeUnknownCause::NotCfBoolean,
    ] {
        let mut r = readback();
        r.element_focused.value = None;
        let p = FocusReadbackProbe::unknown(0, cause, None);
        r.readback_diagnostics = Some(diagnostic_pair(p.clone(), p));
        let s = summarize_focus_readback(r);
        assert_eq!(s.state, FocusReadbackState::Unknown);
        assert!(!s.goal_verified);
        let pair = s.readback_diagnostics.unwrap().element_focused;
        assert!(!pair.value_changed && !pair.native_status_changed);
        assert_eq!(pair.first.unknown_cause, Some(cause));
        assert_eq!(pair.first.raw.native_status, 0);
    }
}
#[test]
fn focus_status_error_to_true_preserves_original_error_without_false_ready() {
    let mut r = readback();
    r.element_focused.value = None;
    r.readback_diagnostics = Some(diagnostic_pair(
        FocusReadbackProbe::unknown(-25204, FocusProbeUnknownCause::CopyAttributeFailed, None),
        FocusReadbackProbe::known(FocusBooleanRead {
            native_status: 0,
            value: Some(true),
        }),
    ));
    let s = summarize_focus_readback(r);
    assert_eq!(s.state, FocusReadbackState::Unknown);
    assert!(!s.goal_verified);
    let d = s.readback_diagnostics.unwrap();
    assert_eq!(d.element_focused.first.raw.native_status, -25204);
    assert!(d.element_focused.native_status_changed && d.element_focused.value_changed);
}
#[test]
fn focus_known_ready_partial_outputs_are_byte_equivalent_even_with_diagnostics() {
    for partial in [false, true] {
        let mut r = readback();
        r.element_focused.value = Some(!partial);
        let expected = serde_json::to_vec(&summarize_focus_readback(r.clone())).unwrap();
        r.readback_diagnostics = Some(diagnostic_pair(
            FocusReadbackProbe::unknown(0, FocusProbeUnknownCause::NotCfBoolean, None),
            FocusReadbackProbe::unknown(0, FocusProbeUnknownCause::CopyReturnedNull, None),
        ));
        let actual = serde_json::to_vec(&summarize_focus_readback(r)).unwrap();
        assert_eq!(actual, expected);
        assert!(
            !String::from_utf8(actual)
                .unwrap()
                .contains("readback_diagnostics")
        );
    }
}
#[test]
fn focus_validation_fault_code_is_actual_bounded_utf8_evidence_not_native_status() {
    let code = format!("pid_unknown{}", "界".repeat(30));
    let p = FocusReadbackProbe::unknown(0, FocusProbeUnknownCause::OwnerLookupFailed, Some(&code));
    let saved = p.validation_error_code.unwrap();
    assert!(saved.len() <= 64 && code.starts_with(&saved));
    assert_eq!(p.raw.native_status, 0);
    assert_eq!(p.raw.value, None);
}
