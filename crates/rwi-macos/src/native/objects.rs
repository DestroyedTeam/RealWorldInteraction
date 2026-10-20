//! Bounded, backend-owned native object identities. Public attribute hashes are
//! not object identity: two AX objects can share every visible attribute.
use rwi_core::{Result, digest, fault};
use std::collections::VecDeque;

const LIMIT: usize = super::AX_TREE_MAX_NODES * 2;
struct Entry<K> {
    object: K,
    serial: u64,
    id: String,
    attributes: String,
}
pub(super) struct Objects<K> {
    root: Option<K>,
    entries: VecDeque<Entry<K>>,
    serial: u64,
}
impl<K> Default for Objects<K> {
    fn default() -> Self {
        Self {
            root: None,
            entries: VecDeque::new(),
            serial: 0,
        }
    }
}
impl<K: PartialEq> Objects<K> {
    pub(super) fn identify(&mut self, root: K, object: K, attributes: &str) -> Result<String> {
        if self.root.as_ref() != Some(&root) {
            self.entries.clear();
            self.root = Some(root);
        }
        let index = self.entries.iter().position(|e| e.object == object);
        let new_serial = index.is_none_or(|i| self.entries[i].attributes != attributes);
        let serial = if new_serial {
            self.serial.checked_add(1).ok_or_else(|| {
                fault(
                    "semantic_object_identity_exhausted",
                    "Rebind explicitly; object serials are never reused.",
                )
            })?
        } else {
            self.entries[index.expect("existing entry")].serial
        };
        let mut entry = if let Some(index) = index {
            self.entries.remove(index).expect("known index")
        } else {
            Entry {
                object,
                serial,
                id: String::new(),
                attributes: String::new(),
            }
        };
        if new_serial {
            self.serial = serial;
        }
        entry.serial = serial;
        entry.attributes = attributes.into();
        entry.id = digest(format!("{attributes}:native-object:{}", entry.serial).as_bytes());
        let id = entry.id.clone();
        self.entries.push_back(entry);
        while self.entries.len() > LIMIT {
            self.entries.pop_front();
        }
        Ok(id)
    }
    pub(super) fn get(&self, root: &K, id: &str) -> Result<&K> {
        if self.root.as_ref() != Some(root) {
            return Err(fault(
                "semantic_window_object_changed",
                "The exact observed AX window object changed; no substitution.",
            ));
        }
        self.entries.iter().find(|e| e.id == id).map(|e| &e.object).ok_or_else(|| fault(
            "semantic_object_expired", "The exact observed native object is absent or evicted; obtain a fresh observation, never search by matching attributes."
        ))
    }
}

/// Target-only ancestry proof. The reader must return real native parents,
/// never a geometrically inferred ancestor. Unknown, cycles and limits refuse.
pub(super) fn require_ancestor<K: PartialEq>(
    mut object: K,
    root: &K,
    mut parent: impl FnMut(&K) -> Result<K>,
    mut checkpoint: impl FnMut() -> Result<()>,
) -> Result<()> {
    let mut seen = Vec::new();
    for _ in 0..32 {
        checkpoint()?;
        if &object == root {
            return Ok(());
        }
        if seen.contains(&object) {
            return Err(fault(
                "semantic_ancestry_cycle",
                "Native parent chain cycles; nothing dispatched.",
            ));
        }
        let next = parent(&object)?;
        seen.push(object);
        object = next;
    }
    Err(fault(
        "semantic_ancestry_incomplete",
        "Cannot prove target membership within the bounded native parent chain.",
    ))
}

mod native;
pub(super) use native::{Ax, describe, exact};

#[cfg(test)]
mod flow_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equal_attributes_do_not_alias_distinct_native_objects() {
        let mut o = Objects::default();
        let a = o.identify(1, 10, "same-attributes").unwrap();
        let b = o.identify(1, 11, "same-attributes").unwrap();
        assert_ne!(a, b);
        assert_eq!(*o.get(&1, &a).unwrap(), 10);
        assert_eq!(*o.get(&1, &b).unwrap(), 11);
        assert_eq!(o.identify(1, 10, "same-attributes").unwrap(), a);
    }
    #[test]
    fn replaced_object_or_window_cannot_restore_an_old_identity() {
        let mut o = Objects::default();
        let a = o.identify(1, 10, "attributes").unwrap();
        let b = o.identify(1, 20, "attributes").unwrap();
        assert_ne!(a, b);
        let c = o.identify(2, 10, "attributes").unwrap();
        assert_ne!(a, c);
        assert_eq!(
            o.get(&1, &a).unwrap_err().code,
            "semantic_window_object_changed"
        );
        assert_eq!(o.get(&2, &a).unwrap_err().code, "semantic_object_expired");
    }
    #[test]
    fn changed_attributes_invalidate_only_that_objects_old_id() {
        let mut o = Objects::default();
        let a = o.identify(1, 10, "old-geometry").unwrap();
        let b = o.identify(1, 11, "other").unwrap();
        let fresh = o.identify(1, 10, "new-geometry").unwrap();
        assert_ne!(a, fresh);
        assert!(o.get(&1, &a).is_err());
        assert_eq!(*o.get(&1, &fresh).unwrap(), 10);
        assert_eq!(*o.get(&1, &b).unwrap(), 11);
        assert_ne!(o.identify(1, 10, "old-geometry").unwrap(), a);
    }
    #[test]
    fn eviction_and_exhaustion_do_not_reuse_ids_or_grow_without_bound() {
        let mut o = Objects::default();
        let first = o.identify(1, 0, "same").unwrap();
        for object in 1..=LIMIT {
            o.identify(1, object, "same").unwrap();
        }
        assert_eq!(o.entries.len(), LIMIT);
        assert_eq!(
            o.get(&1, &first).unwrap_err().code,
            "semantic_object_expired"
        );
        assert_ne!(o.identify(1, 0, "same").unwrap(), first);
        o.serial = u64::MAX;
        assert_eq!(
            o.identify(1, LIMIT + 2, "same").unwrap_err().code,
            "semantic_object_identity_exhausted"
        );
        assert_eq!(o.entries.len(), LIMIT);
    }
    #[test]
    fn target_ancestry_ignores_unrelated_deep_subtrees() {
        let mut reads = Vec::new();
        require_ancestor(
            4,
            &1,
            |n| {
                reads.push(*n);
                Ok(n - 1)
            },
            || Ok(()),
        )
        .unwrap();
        assert_eq!(reads, [4, 3, 2]);
        // No children or sibling enumeration is provided by this contract.
    }
    #[test]
    fn missing_parent_cycle_depth_and_cancelled_proofs_fail_closed() {
        assert_eq!(
            require_ancestor(
                2,
                &1,
                |_| Err(fault("parent_unknown", "unknown")),
                || Ok(())
            )
            .unwrap_err()
            .code,
            "parent_unknown"
        );
        assert_eq!(
            require_ancestor(2, &1, |n| Ok(*n), || Ok(()))
                .unwrap_err()
                .code,
            "semantic_ancestry_cycle"
        );
        assert_eq!(
            require_ancestor(100, &1, |n| Ok(n - 1), || Ok(()))
                .unwrap_err()
                .code,
            "semantic_ancestry_incomplete"
        );
        let mut reads = 0;
        assert_eq!(
            require_ancestor(
                2,
                &1,
                |_| {
                    reads += 1;
                    Ok(1)
                },
                || Err(fault("cancelled", "stop"))
            )
            .unwrap_err()
            .code,
            "cancelled"
        );
        assert_eq!(reads, 0);
    }
}
