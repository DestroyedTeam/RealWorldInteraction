//! Explicit whole AXValue replacement, never keyboard insertion or a fallback.
use crate::semantic_focus::{
    FocusCustody, FocusElementRequest, FocusNativeFacts, FocusPolicyContext, assess_focus_element,
};
use crate::{Result, fault};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_SET_TEXT_BYTES: usize = 4096;
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SetTextRequest {
    pub target: String,
    pub frame: String,
    pub element: String,
    pub request_id: String,
    /// Entire replacement value; empty clears. No append/caret/selection semantics.
    pub text: String,
}
impl SetTextRequest {
    pub fn focus_identity(&self) -> FocusElementRequest {
        FocusElementRequest {
            target: self.target.clone(),
            frame: self.frame.clone(),
            element: self.element.clone(),
            request_id: self.request_id.clone(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        self.focus_identity().validate()?;
        validate_text(&self.text)
    }
}
pub fn validate_text(text: &str) -> Result<()> {
    if text.len() > MAX_SET_TEXT_BYTES || text.contains('\0') {
        return Err(fault(
            "invalid_set_text",
            "Supply the entire unencoded UTF-8 text, 0..4096 bytes, without NUL. No append, caret, clipboard or typing fallback.",
        ));
    }
    Ok(())
}
/// Issued by a native backend only. Not Deserialize, not a caller authority token.
pub struct SetTextNativeFacts {
    pub exact: FocusNativeFacts,
    pub ordinary_string_value: bool,
}
/// Reuse exact-object/session/lease/journal policy, but discard the focus write plan.
/// This operation never calls AXFocused or keyboard preparation.
pub fn assess_set_text(
    context: FocusPolicyContext<'_>,
    native: &SetTextNativeFacts,
    custody: FocusCustody,
) -> Result<()> {
    assess_focus_element(context, &native.exact, custody)?;
    if !native.ordinary_string_value {
        return Err(fault(
            "plain_string_value_required",
            "Only fresh ordinary AXTextField/AXTextArea with known nonspecialized subrole and a writable CFString AXValue. Secure, unknown, numeric/date/combobox roles are unsupported.",
        ));
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextValueRead {
    pub native_status: i32,
    pub cf_string: Option<bool>,
    pub matches_requested: Option<bool>,
    pub utf8_bytes: Option<usize>,
    pub sha256: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTextReadback {
    pub setter_attempts: u8,
    pub setter_status: i32,
    pub first: TextValueRead,
    pub second: TextValueRead,
    pub context_verified: bool,
    pub focus_readback: Option<serde_json::Value>,
    /// Optional post-pair observation. Never substitutes the literal initial reads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement: Option<SetTextSettlement>,
}
pub const SET_TEXT_SETTLEMENT_MAX_READS: usize = 2;
pub const SET_TEXT_SETTLEMENT_BUDGET_MS: u64 = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSettlementRead {
    pub value: TextValueRead,
    pub offset_since_setter_return_ms: f64,
    pub read_elapsed_ms: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetTextSettlement {
    pub reads: Vec<TextSettlementRead>,
    /// Starts only after the mandatory initial pair and context bracket.
    pub elapsed_ms: f64,
    pub budget_ms: u64,
    pub context_verified: bool,
    pub stop_reason: String,
}
pub fn set_text_value_read_known(r: &TextValueRead) -> bool {
    r.native_status == 0
        && r.cf_string == Some(true)
        && r.matches_requested.is_some()
        && r.utf8_bytes.is_some()
        && r.sha256.as_ref().is_some_and(|s| !s.is_empty())
}
pub fn set_text_pair_matches(first: &TextValueRead, second: &TextValueRead) -> bool {
    set_text_value_read_known(first)
        && set_text_value_read_known(second)
        && first.matches_requested == Some(true)
        && second.matches_requested == Some(true)
        && first.sha256 == second.sha256
        && first.utf8_bytes == second.utf8_bytes
}
/// A known mismatch can request observation, never another writer or authority.
pub fn set_text_settlement_needed(raw: &SetTextReadback) -> bool {
    raw.setter_attempts == 1
        && raw.setter_status == 0
        && raw.context_verified
        && set_text_value_read_known(&raw.first)
        && set_text_value_read_known(&raw.second)
        && (raw.first.matches_requested == Some(false)
            || raw.second.matches_requested == Some(false))
}
fn settlement_observed(raw: &SetTextReadback) -> bool {
    let Some(s) = &raw.settlement else {
        return false;
    };
    if !set_text_settlement_needed(raw)
        || !s.context_verified
        || s.stop_reason != "matched_pair"
        || s.reads.is_empty()
        || s.reads.len() > SET_TEXT_SETTLEMENT_MAX_READS
        || s.budget_ms != SET_TEXT_SETTLEMENT_BUDGET_MS
        || !s.elapsed_ms.is_finite()
        || !(0.0..=SET_TEXT_SETTLEMENT_BUDGET_MS as f64).contains(&s.elapsed_ms)
    {
        return false;
    }
    let mut previous = &raw.second;
    let mut previous_offset = 0.0;
    let mut matched = false;
    for read in &s.reads {
        if !set_text_value_read_known(&read.value)
            || !read.offset_since_setter_return_ms.is_finite()
            || read.offset_since_setter_return_ms < previous_offset
            || !read.read_elapsed_ms.is_finite()
            || read.read_elapsed_ms < 0.0
            || read.read_elapsed_ms > s.elapsed_ms
        {
            return false;
        }
        matched = set_text_pair_matches(previous, &read.value);
        previous = &read.value;
        previous_offset = read.offset_since_setter_return_ms;
    }
    matched
}
pub fn summarize_set_text_readback(raw: SetTextReadback) -> serde_json::Value {
    let known = |r: &TextValueRead| r.native_status == 0 && r.cf_string == Some(true);
    let matched = |r: &TextValueRead| known(r) && r.matches_requested == Some(true);
    let replaced = raw.setter_attempts == 1
        && raw.setter_status == 0
        && raw.context_verified
        && matched(&raw.first)
        && matched(&raw.second)
        && raw.first.sha256 == raw.second.sha256
        && raw.first.sha256.is_some()
        && raw.first.utf8_bytes == raw.second.utf8_bytes;
    let initial_state = if replaced {
        "observed_ax_replacement"
    } else if known(&raw.first) && known(&raw.second) {
        "partial"
    } else {
        "unknown"
    };
    let settled = settlement_observed(&raw);
    let state = if settled {
        "observed_after_readonly_settlement"
    } else {
        initial_state
    };
    let settlement = raw.settlement.as_ref().map(|s| {
        let mut value = serde_json::to_value(s).expect("serializable settlement readback");
        value["state"] = if settled {
            "observed_ax_replacement"
        } else if s.context_verified {
            "partial"
        } else {
            "unknown"
        }
        .into();
        value["verification_scope"] =
            "bounded retained exact-object AXValue observations; not DOM or keyboard".into();
        value["intentional_sleep_ms"] = 0.into();
        value["strong_native_preemption"] = false.into();
        value
    });
    let focus_readback = raw.focus_readback.map(|mut v| {
        if let Some(object) = v.as_object_mut() {
            // This is an observation bracket, not the shared focus route's write plan.
            object.insert("focus_operation_performed".into(), false.into());
            object.insert("focus_observation_only".into(), true.into());
            object.remove("write_plan");
        }
        v
    });
    let mut summary = serde_json::json!({"state":state,"observed_ax_replacement":replaced||settled,
        "verification_scope":if settled {"retained exact-object full CFString pair after bounded readonly settlement; not DOM/keyboard/composition"} else {"two full CFString AXValue reads on the retained exact object; not DOM/keyboard/composition"},
        "setter_attempts":raw.setter_attempts,"setter_status":raw.setter_status,
        "first":raw.first,"second":raw.second,"context_verified":raw.context_verified,
        "focus_readback":focus_readback,"goal_verified":false,"keyboard_authorized":false,
        "append_or_caret_semantics":false,"synthetic_keyboard_or_pointer_events_sent":false,"application_dom_events_verified":false});
    if let Some(settlement) = settlement {
        summary["initial_two_reads"] = serde_json::json!({
            "state":initial_state, "observed_ax_replacement":replaced,
            "first":raw.first, "second":raw.second
        });
        summary["settlement"] = settlement;
    }
    summary
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_set_text_strict_plaintext_empty_unicode_and_budget() {
        for text in ["", " 中文\t🙂\n𝄞 e\u{301} 👩\u{200d}💻 "] {
            let request = SetTextRequest {
                target: "t".into(),
                frame: "f".into(),
                element: "e".into(),
                request_id: "once".into(),
                text: text.into(),
            };
            request.validate().unwrap();
            let v = serde_json::to_value(&request).unwrap();
            assert_eq!(v["text"], text);
            for name in [
                "append",
                "caret",
                "value",
                "options",
                "fallback",
                "lease",
                "include_timings",
            ] {
                let mut invalid = v.clone();
                invalid[name] = true.into();
                assert!(serde_json::from_value::<SetTextRequest>(invalid).is_err());
            }
        }
        validate_text(&"🙂".repeat(1024)).unwrap();
        assert!(validate_text(&"🙂".repeat(1025)).is_err());
        assert!(validate_text("a\0b").is_err());
    }
    fn read(matches: Option<bool>) -> TextValueRead {
        TextValueRead {
            native_status: 0,
            cf_string: Some(true),
            matches_requested: matches,
            utf8_bytes: Some(4),
            sha256: Some("hash".into()),
        }
    }
    #[test]
    fn explicit_set_text_readback_never_promotes_ack_or_unknown_to_dom_goal() {
        let raw = SetTextReadback {
            setter_attempts: 1,
            setter_status: 0,
            first: read(Some(true)),
            second: read(Some(true)),
            context_verified: true,
            focus_readback: None,
            settlement: None,
        };
        let good = summarize_set_text_readback(raw.clone());
        assert_eq!(good["observed_ax_replacement"], true);
        assert_eq!(good["goal_verified"], false);
        assert_eq!(good["synthetic_keyboard_or_pointer_events_sent"], false);
        assert_eq!(good["application_dom_events_verified"], false);
        assert!(good.get("input_events_generated").is_none());
        for case in 0..5 {
            let mut bad = raw.clone();
            match case {
                0 => bad.second.matches_requested = Some(false),
                1 => bad.second.native_status = -25204,
                2 => bad.second.matches_requested = None,
                3 => bad.context_verified = false,
                _ => bad.setter_attempts = 2,
            }
            assert_eq!(
                summarize_set_text_readback(bad)["observed_ax_replacement"],
                false
            );
        }
    }
    fn settlement_case(second_matches: bool) -> SetTextReadback {
        SetTextReadback {
            setter_attempts: 1,
            setter_status: 0,
            first: read(Some(false)),
            second: read(Some(second_matches)),
            context_verified: true,
            focus_readback: Some(serde_json::json!({
                "write_plan":"no_write_already_focused", "element_focused":{"value":false},
                "keyboard_authorized":false
            })),
            settlement: Some(SetTextSettlement {
                reads: vec![TextSettlementRead {
                    value: read(Some(true)),
                    offset_since_setter_return_ms: 225.0,
                    read_elapsed_ms: 1.0,
                }],
                elapsed_ms: 5.0,
                budget_ms: 100,
                context_verified: true,
                stop_reason: "matched_pair".into(),
            }),
        }
    }
    #[test]
    fn bounded_set_text_settlement_preserves_initial_partial_and_no_authority() {
        let mut immediate = settlement_case(true);
        immediate.first = read(Some(true));
        immediate.settlement = None;
        let fast = summarize_set_text_readback(immediate);
        assert_eq!(fast["state"], "observed_ax_replacement");
        assert!(fast.get("initial_two_reads").is_none());
        assert!(fast.get("settlement").is_none());
        let raw = settlement_case(true);
        let summary = summarize_set_text_readback(raw);
        assert_eq!(summary["state"], "observed_after_readonly_settlement");
        assert_eq!(summary["initial_two_reads"]["state"], "partial");
        assert_eq!(summary["settlement"]["state"], "observed_ax_replacement");
        assert_eq!(summary["first"]["matches_requested"], false);
        assert_eq!(summary["second"]["matches_requested"], true);
        assert_eq!(summary["observed_ax_replacement"], true);
        assert_eq!(summary["goal_verified"], false);
        assert_eq!(summary["keyboard_authorized"], false);
        assert_eq!(summary["application_dom_events_verified"], false);
        assert_eq!(
            summary["focus_readback"]["focus_operation_performed"],
            false
        );
        assert_eq!(summary["focus_readback"]["focus_observation_only"], true);
        assert_eq!(summary["focus_readback"]["element_focused"]["value"], false);
        assert!(summary["focus_readback"].get("write_plan").is_none());
    }
    #[test]
    fn bounded_set_text_settlement_unknown_budget_and_unmatched_never_promote() {
        for case in 0..9 {
            let mut raw = settlement_case(true);
            match case {
                0 => raw.second.cf_string = None,
                1 => raw.context_verified = false,
                2 => raw.setter_status = -25204,
                3 => raw.setter_attempts = 2,
                4 => raw.settlement.as_mut().unwrap().context_verified = false,
                5 => raw.settlement.as_mut().unwrap().elapsed_ms = 100.1,
                6 => {
                    raw.settlement.as_mut().unwrap().reads[0]
                        .value
                        .matches_requested = None
                }
                7 => {
                    raw.settlement.as_mut().unwrap().reads[0].value.sha256 =
                        Some("different".into())
                }
                _ => {
                    raw.settlement.as_mut().unwrap().reads[0].offset_since_setter_return_ms =
                        f64::NAN
                }
            }
            assert_eq!(
                summarize_set_text_readback(raw)["observed_ax_replacement"],
                false
            );
        }
        let raw = settlement_case(false);
        assert_eq!(summarize_set_text_readback(raw)["state"], "partial");
    }
}
