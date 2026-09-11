//! Pure bounded optimistic UI document-state digest, not a tab/file identity.
use rcu_core::digest;

#[derive(Default)]
pub(crate) struct Documents {
    rows: Vec<(String, String)>,
    units: usize,
    pub(crate) unavailable: Option<String>,
}
impl Documents {
    pub(crate) fn reject(&mut self, why: &str) {
        if self.unavailable.is_none() {
            self.unavailable = Some(why.chars().take(256).collect());
        }
    }
    pub(crate) fn push(&mut self, id: &str, value: &str) {
        self.units = self.units.saturating_add(value.encode_utf16().count());
        if self.rows.len() >= 8 || self.units > 16_384 || self.rows.iter().any(|r| r.0 == id) {
            self.reject("document count/value budget or duplicate identity");
            return;
        }
        self.rows.push((id.into(), digest(value.as_bytes())));
    }
    pub(crate) fn finish(&mut self, complete: bool) -> Option<String> {
        if !complete {
            self.reject("UIA walk incomplete; document state is unknown");
        }
        if self.unavailable.is_some() {
            return None;
        }
        self.rows.sort();
        Some(digest(
            &serde_json::to_vec(&self.rows).expect("bounded document hashes"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn value_change_with_same_editor_identity_changes_document_precondition() {
        let mut a = Documents::default();
        a.push("same-runtime-id", "run-owned document");
        let mut b = Documents::default();
        b.push("same-runtime-id", "other document");
        assert_ne!(a.finish(true), b.finish(true));
    }
    #[test]
    fn unknown_coverage_provider_or_oversize_cannot_become_empty_proof() {
        let mut a = Documents::default();
        assert!(a.finish(false).is_none());
        let mut b = Documents::default();
        b.reject("provider failed");
        assert!(b.finish(true).is_none());
        let mut c = Documents::default();
        c.push("e", &"x".repeat(16_385));
        assert!(c.finish(true).is_none());
        let mut d = Documents::default();
        for i in 0..9 {
            d.push(&i.to_string(), "x");
        }
        assert!(d.finish(true).is_none());
    }
    #[test]
    fn native_value_digest_ignores_walk_order_but_not_whitespace() {
        let mut a = Documents::default();
        a.push("a", "x ");
        a.push("b", "y");
        let mut b = Documents::default();
        b.push("b", "y");
        b.push("a", "x ");
        assert_eq!(a.finish(true), b.finish(true));
        let mut c = Documents::default();
        c.push("a", "x");
        c.push("b", "y");
        assert_ne!(a.finish(true), c.finish(true));
    }
}
