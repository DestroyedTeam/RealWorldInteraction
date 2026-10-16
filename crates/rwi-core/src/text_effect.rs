//! Bounded plain-field value evidence, never input authority or a goal oracle.
//!
//! The native caller must establish field classification and exact object custody.
//! A matching value is a sample, not proof that a particular event caused it.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_VALUE_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum State {
    ValueMatchesExpected,
    ValueDiffersObserved,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnknownReason {
    SelectionUnknown,
    InvalidUtf16Range,
    ValueTooLarge,
    NotPlainField,
    ReadbackUnavailable,
    ReadbackBudgetExpired,
    NativeIdentityChanged,
    DispatchIncomplete,
}

/// Only bounded states/counts cross the receipt boundary; no text is serialized.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TextEffect {
    pub state: State,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<UnknownReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_utf16_units: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_utf16_units: Option<u32>,
}

impl TextEffect {
    pub fn unknown(reason: UnknownReason) -> Self {
        Self {
            state: State::Unknown,
            reason: Some(reason),
            expected_utf16_units: None,
            observed_utf16_units: None,
        }
    }
}

/// Invocation-local private expected value. Deliberately not Serialize or Debug.
pub struct ExpectedEdit {
    value: String,
    utf16_units: u32,
}

fn byte_at_utf16(text: &str, offset: u64) -> Option<usize> {
    let mut units = 0_u64;
    for (at, ch) in text.char_indices() {
        if units == offset {
            return Some(at);
        }
        units += ch.len_utf16() as u64;
        if units > offset {
            return None;
        }
    }
    (units == offset).then_some(text.len())
}

impl ExpectedEdit {
    /// Selection offsets are UTF16, not bytes, scalars or grapheme positions.
    /// Call only with a full positively classified native plain-field value.
    pub fn new(
        before: &str,
        selection: Option<(u64, u64)>,
        inserted: &str,
    ) -> Result<Self, UnknownReason> {
        if before.len() > MAX_VALUE_BYTES || inserted.len() > MAX_VALUE_BYTES {
            return Err(UnknownReason::ValueTooLarge);
        }
        let (location, length) = selection.ok_or(UnknownReason::SelectionUnknown)?;
        let end = location
            .checked_add(length)
            .ok_or(UnknownReason::InvalidUtf16Range)?;
        let start = byte_at_utf16(before, location).ok_or(UnknownReason::InvalidUtf16Range)?;
        let end = byte_at_utf16(before, end).ok_or(UnknownReason::InvalidUtf16Range)?;
        let bytes = (before.len() - (end - start))
            .checked_add(inserted.len())
            .filter(|n| *n <= MAX_VALUE_BYTES)
            .ok_or(UnknownReason::ValueTooLarge)?;
        let mut value = String::with_capacity(bytes);
        value.push_str(&before[..start]);
        value.push_str(inserted);
        value.push_str(&before[end..]);
        let utf16_units = value.encode_utf16().count() as u32;
        Ok(Self { value, utf16_units })
    }

    /// A mismatching sample may be asynchronous or transformed by the app.
    /// Neither state changes the dispatch result, goal verification or no-replay.
    pub fn sample(&self, actual: Result<&str, UnknownReason>) -> TextEffect {
        let observed = match actual {
            Ok(v) if v.len() <= MAX_VALUE_BYTES => v,
            Ok(_) => return self.unknown(UnknownReason::ValueTooLarge),
            Err(reason) => return self.unknown(reason),
        };
        TextEffect {
            state: if observed == self.value {
                State::ValueMatchesExpected
            } else {
                State::ValueDiffersObserved
            },
            reason: None,
            expected_utf16_units: Some(self.utf16_units),
            observed_utf16_units: Some(observed.encode_utf16().count() as u32),
        }
    }

    fn unknown(&self, reason: UnknownReason) -> TextEffect {
        TextEffect {
            expected_utf16_units: Some(self.utf16_units),
            ..TextEffect::unknown(reason)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f21_text_effect_replaces_selection_not_assumed_append() {
        let edit = ExpectedEdit::new("prefix OLD suffix", Some((7, 3)), "new").unwrap();
        assert_eq!(
            edit.sample(Ok("prefix new suffix")).state,
            State::ValueMatchesExpected
        );
        assert_eq!(
            edit.sample(Ok("prefix OLD suffixnew")).state,
            State::ValueDiffersObserved
        );
    }

    #[test]
    fn f21_text_effect_empty_field_and_end_insertion() {
        assert_eq!(
            ExpectedEdit::new("", Some((0, 0)), "🙂")
                .unwrap()
                .sample(Ok("🙂"))
                .expected_utf16_units,
            Some(2)
        );
        assert_eq!(
            ExpectedEdit::new("a🙂", Some((3, 0)), "z")
                .unwrap()
                .sample(Ok("a🙂z"))
                .state,
            State::ValueMatchesExpected
        );
    }

    #[test]
    fn f21_text_effect_non_bmp_selection_counts_utf16_not_bytes() {
        let edit = ExpectedEdit::new("a🙂z", Some((1, 2)), "𝄞").unwrap();
        let sample = edit.sample(Ok("a𝄞z"));
        assert_eq!(sample.state, State::ValueMatchesExpected);
        assert_eq!(sample.expected_utf16_units, Some(4));
    }

    #[test]
    fn f21_text_effect_surrogate_midpoints_never_slice_or_round() {
        for range in [(1, 0), (0, 1), (1, 1)] {
            assert!(matches!(
                ExpectedEdit::new("🙂", Some(range), "x"),
                Err(UnknownReason::InvalidUtf16Range)
            ));
        }
    }

    #[test]
    fn f21_text_effect_missing_selection_is_unknown_not_append() {
        assert!(matches!(
            ExpectedEdit::new("text", None, "x"),
            Err(UnknownReason::SelectionUnknown)
        ));
    }

    #[test]
    fn f21_text_effect_overflow_and_outside_selection_refuse() {
        for range in [(u64::MAX, 1), (1, u64::MAX), (99, 0), (0, 99)] {
            assert!(matches!(
                ExpectedEdit::new("a", Some(range), "x"),
                Err(UnknownReason::InvalidUtf16Range)
            ));
        }
    }

    #[test]
    fn f21_text_effect_does_not_normalize_combining_or_joiner() {
        let edit = ExpectedEdit::new("", Some((0, 0)), "e\u{301}👩\u{200d}💻").unwrap();
        assert_eq!(
            edit.sample(Ok("e\u{301}👩\u{200d}💻")).state,
            State::ValueMatchesExpected
        );
        assert_eq!(
            edit.sample(Ok("é👩\u{200d}💻")).state,
            State::ValueDiffersObserved
        );
    }

    #[test]
    fn f21_text_effect_known_f20_loss_is_mismatch_not_success() {
        let text = "RWI 中λ🙂𝄞 e\u{301}👩\u{200d}💻";
        let dropped: String = text.chars().filter(|c| u32::from(*c) <= 65535).collect();
        let sample = ExpectedEdit::new("", Some((0, 0)), text)
            .unwrap()
            .sample(Ok(&dropped));
        assert_eq!(sample.state, State::ValueDiffersObserved);
        assert_eq!(sample.expected_utf16_units, Some(18));
        assert_eq!(sample.observed_utf16_units, Some(10));
    }

    #[test]
    fn f21_text_effect_input_and_combined_size_are_bounded() {
        let full = "a".repeat(MAX_VALUE_BYTES);
        assert!(matches!(
            ExpectedEdit::new(&full, Some((0, 0)), "x"),
            Err(UnknownReason::ValueTooLarge)
        ));
        assert!(ExpectedEdit::new(&full, Some((0, MAX_VALUE_BYTES as u64)), "x").is_ok());
        assert!(matches!(
            ExpectedEdit::new(&(full + "x"), Some((0, 0)), ""),
            Err(UnknownReason::ValueTooLarge)
        ));
    }

    #[test]
    fn f21_text_effect_unknown_readback_keeps_expected_only() {
        let edit = ExpectedEdit::new("", Some((0, 0)), "🙂").unwrap();
        let sample = edit.sample(Err(UnknownReason::NativeIdentityChanged));
        assert_eq!(sample.state, State::Unknown);
        assert_eq!(sample.reason, Some(UnknownReason::NativeIdentityChanged));
        assert_eq!(sample.expected_utf16_units, Some(2));
        assert_eq!(sample.observed_utf16_units, None);
        assert_eq!(
            edit.sample(Ok(&"a".repeat(MAX_VALUE_BYTES + 1))).reason,
            Some(UnknownReason::ValueTooLarge)
        );
    }

    #[test]
    fn f21_text_effect_serialization_has_no_text_or_goal_claim() {
        let edit = ExpectedEdit::new("private-prefix", Some((14, 0)), "private-input").unwrap();
        let effect = edit.sample(Ok("private-prefixprivate-input"));
        let json = serde_json::to_string(&effect).unwrap();
        assert!(!json.contains("private"));
        assert!(!json.contains("goal_verified"));
        assert!(!json.contains("system_accepted"));
        assert!(json.len() < 200);
        assert_eq!(serde_json::from_str::<TextEffect>(&json).unwrap(), effect);
    }
}
