//! No windows, capture, event posting, permission prompts or foreground changes.
use super::*;
use core_foundation::{base::TCFType, string::CFString};

impl NativeMac {
    pub(crate) fn fixture_with_text_effect(effect: TextEffect) -> Self {
        let mut native = Self::fixture_for_live();
        native.text_effect = Some(effect);
        native
    }
}

#[test]
fn f21_text_effect_expired_budget_never_enters_another_read() {
    let budget = Budget(Instant::now() - Duration::from_secs(1));
    let mut calls = 0;
    let result = budget.query(|| {
        calls += 1;
        Ok(())
    });
    assert_eq!(result, Err(Reason::ReadbackBudgetExpired));
    assert_eq!(calls, 0);
    let result = plain_value_with(|_| Err(Reason::ReadbackBudgetExpired));
    assert_eq!(result, Err(Reason::ReadbackBudgetExpired));
}

#[test]
fn f21_text_effect_secure_unknown_and_nontext_never_read_value() {
    for (role, subrole) in [
        ("AXTextField", "AXSecureTextField"),
        ("AXTextField", "AXUnknown"),
        ("AXButton", ""),
        ("AXUnknown", ""),
    ] {
        let mut reads = Vec::new();
        let result = plain_value_with(|name| {
            reads.push(name.to_owned());
            match name {
                "AXRole" => Ok(role.into()),
                "AXSubrole" => Ok(subrole.into()),
                _ => panic!("unclassified value read"),
            }
        });
        assert_eq!(result, Err(Reason::NotPlainField));
        assert!(!reads.iter().any(|n| n == "AXValue"));
    }
}

#[test]
fn f21_text_effect_missing_subrole_never_reads_value() {
    let result = plain_value_with(|name| match name {
        "AXRole" => Ok("AXTextField".into()),
        "AXSubrole" => Err(Reason::ReadbackUnavailable),
        _ => panic!("unknown class value read"),
    });
    assert_eq!(result, Err(Reason::NotPlainField));
}

fn absent_attribute(name: &str, status: i32) -> rwi_core::Fault {
    let mut error = fault(
        "ax_attribute_unavailable",
        "literal native read error fixture",
    );
    error.diagnostic = Some(Box::new(serde_json::json!({
        "api": "AXUIElementCopyAttributeValue",
        "attribute": name,
        "ax_error": status,
        "operation_kind": "read",
        "replay_allowed": false,
    })));
    error
}

#[test]
fn f21_text_effect_optional_subrole_explicit_absence_matches_plain_snapshot() {
    let text = "RWI 中λ🙂𝄞 e\u{301}👩\u{200d}💻";
    for status in [-25205, -25212] {
        let mut value_reads = 0;
        let result = plain_value_with(|name| match name {
            "AXRole" => Ok("AXTextField".into()),
            "AXSubrole" => attribute_string(name, Err(absent_attribute(name, status)))
                .map_err(|_| Reason::ReadbackUnavailable),
            "AXValue" => {
                value_reads += 1;
                Ok(text.into())
            }
            _ => unreachable!(),
        });
        assert_eq!(result.as_deref(), Ok(text));
        assert_eq!(value_reads, 1);
    }
}

#[test]
fn f21_text_effect_optional_subrole_other_errors_remain_unknown_without_value() {
    for (attribute, status) in [
        ("AXSubrole", -25204),
        ("AXSubrole", -25202),
        ("AXSubrole", -25200),
        ("AXRole", -25205),
        ("AXValue", -25212),
    ] {
        let error =
            attribute_string("AXSubrole", Err(absent_attribute(attribute, status))).unwrap_err();
        assert_eq!(error.diagnostic.as_ref().unwrap()["ax_error"], status);
        assert_eq!(error.diagnostic.as_ref().unwrap()["attribute"], attribute);
        let result = plain_value_with(|name| match name {
            "AXRole" => Ok("AXTextField".into()),
            "AXSubrole" => Err(Reason::ReadbackUnavailable),
            _ => panic!("unknown specialization must not read AXValue"),
        });
        assert_eq!(result, Err(Reason::NotPlainField));
    }
}

#[test]
fn f21_text_effect_optional_subrole_secure_unknown_and_wrong_type_stay_closed() {
    for subrole in ["AXSecureTextField", "AXUnknown"] {
        let string = CFString::new(subrole);
        let cf = unsafe { Cf::owned(ffi::CFRetain(string.as_concrete_TypeRef().cast())).unwrap() };
        let specialization = attribute_string("AXSubrole", Ok(cf)).unwrap();
        assert_ne!(
            TextClass::classify("AXTextField", Some(&specialization)),
            TextClass::Plain
        );
    }
    let boolean = core_foundation::boolean::CFBoolean::true_value();
    let wrong = unsafe { Cf::owned(ffi::CFRetain(boolean.as_concrete_TypeRef().cast())).unwrap() };
    assert!(attribute_string("AXSubrole", Ok(wrong)).is_err());
    assert!(attribute_string("AXValue", Err(absent_attribute("AXValue", -25212))).is_err());
}

#[test]
fn f21_text_effect_plain_reader_is_exact_and_non_lossy() {
    let text = "🙂𝄞 e\u{301}👩\u{200d}💻";
    assert_eq!(
        plain_value_with(|name| Ok(match name {
            "AXRole" => "AXTextField",
            "AXSubrole" => "",
            _ => text,
        }
        .into()))
        .unwrap(),
        text
    );
}

#[test]
fn f21_text_effect_changed_class_does_not_publish_value_evidence() {
    let mut value_reads = 0;
    let result = plain_value_with(|name| match name {
        "AXRole" => Ok("AXTextField".into()),
        "AXSubrole" => Ok(if value_reads == 0 {
            ""
        } else {
            "AXSecureTextField"
        }
        .into()),
        "AXValue" => {
            value_reads += 1;
            Ok("not published".into())
        }
        _ => unreachable!(),
    });
    assert_eq!(result, Err(Reason::NotPlainField));
    assert_eq!(value_reads, 1);
}

fn make_range(location: isize, length: isize) -> Cf {
    let r = NativeRange { location, length };
    unsafe { Cf::owned(ffi::AXValueCreate(4, (&r as *const NativeRange).cast())).unwrap() }
}
#[test]
fn f21_text_effect_native_range_type_and_negative_checks() {
    assert_eq!(range(&make_range(0, 0)), Ok((0, 0)));
    assert_eq!(range(&make_range(1, 2)), Ok((1, 2)));
    assert_eq!(range(&make_range(-1, 0)), Err(Reason::InvalidUtf16Range));
    assert_eq!(range(&make_range(0, -1)), Err(Reason::InvalidUtf16Range));
    let string = CFString::new("not an AXValue");
    let wrong = unsafe { Cf::owned(ffi::CFRetain(string.as_concrete_TypeRef().cast())).unwrap() };
    assert_eq!(range(&wrong), Err(Reason::SelectionUnknown));
}

#[test]
fn f21_text_effect_native_drain_is_once_and_nontext_clears() {
    let mut native = NativeMac::fixture_for_live();
    native.record_text_effect(Some(Attempt(Err(Reason::SelectionUnknown))), true);
    assert_eq!(
        native.take_text_effect(),
        Some(TextEffect::unknown(Reason::SelectionUnknown))
    );
    assert_eq!(native.take_text_effect(), None);
    native.record_text_effect(Some(Attempt(Err(Reason::SelectionUnknown))), true);
    native.record_text_effect(None, true);
    assert_eq!(native.take_text_effect(), None);
    assert_eq!(native.input_dispatch_count(), Some(0));
}

#[test]
fn f21_text_effect_incomplete_dispatch_is_not_a_value_sample() {
    let mut native = NativeMac::fixture_for_live();
    native.record_text_effect(Some(Attempt(Err(Reason::SelectionUnknown))), false);
    assert_eq!(
        native.take_text_effect(),
        Some(TextEffect::unknown(Reason::DispatchIncomplete))
    );
    assert_eq!(native.input_dispatch_count(), Some(0));
}
