//! One fresh pointer ancestry proof, with the existing root/target depth bounds.
use rcu_core::{Result, fault};

pub(crate) struct PointerAncestry {
    visited: usize,
    target_seen: bool,
    wanted: bool,
}
impl PointerAncestry {
    pub fn new(wanted: bool) -> Self {
        Self {
            visited: 0,
            target_seen: false,
            wanted,
        }
    }
    pub fn needs_target(&self) -> bool {
        self.wanted && !self.target_seen && self.visited < 8
    }
    pub fn visit(&mut self, root: bool, target: bool) -> Result<bool> {
        if self.visited >= 32 {
            return Err(fault(
                "uia_ancestry_incomplete",
                "Pointer root ancestry exceeded 32 levels; no input.",
            ));
        }
        self.target_seen |= self.needs_target() && target;
        self.visited += 1;
        if root {
            if self.wanted && !self.target_seen {
                return Err(fault(
                    "pointer_target_changed",
                    "Fresh hit ancestry does not identify the grounded element within eight levels; no input.",
                ));
            }
            return Ok(true);
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_alone_never_proves_root_and_a_new_sample_has_no_cached_authority() {
        let mut p = PointerAncestry::new(true);
        assert!(!p.visit(false, true).unwrap());
        assert!(p.visit(true, false).unwrap());
        assert_eq!(
            PointerAncestry::new(true)
                .visit(true, false)
                .unwrap_err()
                .code,
            "pointer_target_changed"
        );
        assert!(PointerAncestry::new(false).visit(true, false).unwrap());
    }
    #[test]
    fn target_depth_eight_is_allowed_but_nine_is_not() {
        for depth in [7, 8] {
            let mut p = PointerAncestry::new(true);
            for _ in 0..depth {
                assert!(!p.visit(false, false).unwrap());
            }
            let result = p.visit(true, true);
            assert_eq!(result.is_ok(), depth == 7);
        }
    }
    #[test]
    fn root_budget_is_thirty_two_and_foreign_trees_fail_closed() {
        let mut p = PointerAncestry::new(false);
        for _ in 0..31 {
            assert!(!p.visit(false, false).unwrap());
        }
        assert!(p.visit(true, false).unwrap());
        let mut p = PointerAncestry::new(true);
        for _ in 0..32 {
            assert!(!p.visit(false, true).unwrap());
        }
        assert_eq!(
            p.visit(true, true).unwrap_err().code,
            "uia_ancestry_incomplete"
        );
    }
}
