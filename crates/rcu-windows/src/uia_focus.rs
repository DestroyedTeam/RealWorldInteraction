//! Pure focus projection shared by native observation and offline regressions.
use std::collections::HashSet;

pub(crate) struct Projection {
    runtime: Option<Vec<i32>>,
    published: HashSet<String>,
}
impl Projection {
    pub(crate) fn new(runtime: Option<Vec<i32>>) -> Self {
        Self {
            runtime,
            published: HashSet::new(),
        }
    }
    pub(crate) fn focused(&self, runtime: Option<&Vec<i32>>) -> Option<bool> {
        self.runtime.as_ref().zip(runtime).map(|(a, b)| a == b)
    }
    pub(crate) fn publish(&mut self, id: Option<&str>) -> bool {
        id.is_none_or(|id| self.published.insert(id.into()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn container_focus_flag_cannot_compete_with_exact_focused_identity() {
        let p = Projection::new(Some(vec![42, 9]));
        // Both providers may claim HasKeyboardFocus; only GetFocusedElement wins.
        assert_eq!(p.focused(Some(&vec![42, 1])), Some(false));
        assert_eq!(p.focused(Some(&vec![42, 9])), Some(true));
        assert_eq!(p.focused(None), None);
        assert_eq!(Projection::new(None).focused(Some(&vec![42, 1])), None);
    }
    #[test]
    fn priority_focus_node_is_not_replaced_or_counted_twice_by_tree_walk() {
        let mut p = Projection::new(Some(vec![1]));
        assert!(p.publish(Some("focus-outside-bounded-tree")));
        assert!(p.publish(Some("root")));
        assert!(!p.publish(Some("focus-outside-bounded-tree")));
    }
}
