//! Bounded native text/value reads shared by observation and preconditions.
use super::*;
pub(super) fn bounded_document_value(e: &IUIAutomationElement) -> Result<String> {
    let _call = native_call("uia_document_precondition")?;
    // TextPattern has a request-time bound; ValuePattern has only a post-read
    // bound. Never claim a hard allocator/native-provider preemption guarantee.
    let actual = if let Some(text) = pattern::<IUIAutomationTextPattern>(e, UIA_TextPatternId)? {
        let range = api("uia_document_range_unavailable", unsafe {
            text.DocumentRange()
        })?;
        api("uia_document_read_failed", unsafe { range.GetText(16_385) })?
    } else if let Some(value) = pattern::<IUIAutomationValuePattern>(e, UIA_ValuePatternId)? {
        api("uia_document_read_failed", unsafe { value.CurrentValue() })?
    } else {
        return Err(fault(
            "uia_document_pattern_unavailable",
            "Document preconditions require a bounded native text/value read.",
        ));
    };
    if actual.len() > 16_384 {
        return Err(fault(
            "uia_document_value_limit",
            "Document value exceeds the checked-action budget.",
        ));
    }
    String::from_utf16(&actual).map_err(|_| {
        fault(
            "uia_document_non_unicode",
            "Document text is not valid UTF-16.",
        )
    })
}
