//! Allocate the existing eight mismatch rows fairly, without extra native reads.
use super::TextClass;
use serde_json::Value;

pub(super) fn class(text: TextClass) -> &'static str {
    if text == TextClass::Plain {
        "plain_text"
    } else {
        "other"
    }
}
pub(super) fn can_record(rows: &[Value], text: TextClass) -> bool {
    rows.len() < 8
        && rows
            .iter()
            .filter(|row| row["diagnostic_class"] == class(text))
            .count()
            < 4
}

#[cfg(test)]
mod tests {
    use super::*;
    fn collect(classes: &[TextClass]) -> Vec<Value> {
        let mut rows = Vec::new();
        for &text in classes {
            if can_record(&rows, text) {
                rows.push(serde_json::json!({"diagnostic_class":class(text)}));
            }
        }
        rows
    }
    #[test]
    fn f406_plain_text_diagnostics_are_not_starved_by_early_generic_controls() {
        for (first, second) in [
            (TextClass::NotText, TextClass::Plain),
            (TextClass::Plain, TextClass::NotText),
        ] {
            let rows = collect(&[vec![first; 100], vec![second; 100]].concat());
            assert_eq!(rows.len(), 8);
            assert_eq!(
                rows.iter()
                    .filter(|r| r["diagnostic_class"] == "plain_text")
                    .count(),
                4
            );
            assert_eq!(
                rows.iter()
                    .filter(|r| r["diagnostic_class"] == "other")
                    .count(),
                4
            );
        }
    }
    #[test]
    fn f406_unknown_and_secure_do_not_acquire_plain_text_status_or_more_budget() {
        assert_eq!(class(TextClass::Unknown), "other");
        assert_eq!(class(TextClass::Secure), "other");
        assert_eq!(collect(&[TextClass::NotText; 100]).len(), 4);
        assert_eq!(collect(&[TextClass::Plain; 100]).len(), 4);
        assert!(!can_record(&vec![Value::Null; 8], TextClass::Plain));
    }
}
