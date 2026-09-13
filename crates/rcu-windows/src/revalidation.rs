//! Purpose-specific native revalidation. Never changes observation authority.
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Scope {
    FullTree,
    RootIdentity,
}
impl Scope {
    pub fn for_reference(native_element_policy: bool, has_element: bool) -> Self {
        if native_element_policy && !has_element {
            Self::RootIdentity
        } else {
            Self::FullTree
        }
    }
}

#[derive(Default, serde::Serialize)]
pub(crate) struct Timings {
    pub full_tree: Measurement,
    pub root_identity: Measurement,
}
#[derive(Default, serde::Serialize)]
pub(crate) struct Measurement {
    calls: u64,
    failures: u64,
    total_ms: f64,
    last_ms: f64,
    max_ms: f64,
}
impl Timings {
    pub fn record(&mut self, scope: Scope, elapsed: Duration, success: bool) {
        let m = match scope {
            Scope::FullTree => &mut self.full_tree,
            Scope::RootIdentity => &mut self.root_identity,
        };
        m.calls += 1;
        m.failures += u64::from(!success);
        m.last_ms = elapsed.as_secs_f64() * 1000.;
        m.total_ms += m.last_ms;
        m.max_ms = m.max_ms.max(m.last_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_native_element_policy_pixel_revalidation_can_omit_tree() {
        for native in [false, true] {
            for semantic in [false, true] {
                assert_eq!(
                    Scope::for_reference(native, semantic) == Scope::RootIdentity,
                    native && !semantic
                );
            }
        }
    }
    #[test]
    fn timing_preserves_failures_and_separates_scopes() {
        let mut t = Timings::default();
        t.record(Scope::RootIdentity, Duration::from_millis(2), true);
        t.record(Scope::RootIdentity, Duration::from_millis(5), false);
        t.record(Scope::FullTree, Duration::from_millis(1000), true);
        assert_eq!(t.root_identity.calls, 2);
        assert_eq!(t.root_identity.failures, 1);
        assert_eq!(t.root_identity.total_ms, 7.);
        assert_eq!(t.root_identity.max_ms, 5.);
        assert_eq!(t.full_tree.calls, 1);
        assert_eq!(t.full_tree.total_ms, 1000.);
    }

    #[test]
    fn empty_tree_pixel_scene_still_requires_exact_image_and_native_binding() {
        use rcu_core::{Binding, Identity, MutationContext, Resolved, Scene, grounding_matches};
        let scope = rcu_core::Scope {
            window: "native-window".into(),
            pid: 7,
            element: None,
        };
        let binding = Binding {
            scope: scope.clone(),
            focus_window: "native-window".into(),
            layout: "physical-layout".into(),
            content: "full-png-hash".into(),
            native_epoch: "window-and-uia-root-epoch".into(),
            mutation_context: Some(MutationContext {
                window_title: "title".into(),
                document_state: None,
            }),
        };
        let scene = Scene {
            identity: Identity {
                host: "host".into(),
                guest: None,
                session: "logon".into(),
                seat: "seat".into(),
                desktop: "desktop".into(),
            },
            assurance: "native-attested".into(),
            binding: Some(binding.clone()),
            mutation_ready: true,
            blocked: Some(false),
            image_size: [100, 100],
            pixel_to_desktop: Some([1., 0., 0., 1., 0., 0.]),
            elements: vec![],
            captured_unix_ms: 1,
            atomicity: "test".into(),
            image_normalization: "test".into(),
            png: vec![],
        };
        let target = Resolved {
            reference: "reference".into(),
            target: "target".into(),
            frame: "frame".into(),
            point: [10., 10.],
            scope,
            element_revision: None,
        };
        assert!(grounding_matches(&binding, &scene, &target));
        for field in ["content", "layout", "native_epoch", "focus_window"] {
            let mut changed = serde_json::to_value(&scene).unwrap();
            changed["binding"][field] = "changed".into();
            let changed = serde_json::from_value::<Scene>(changed).unwrap();
            assert!(!grounding_matches(&binding, &changed, &target), "{field}");
        }
        let mut semantic = target.clone();
        semantic.scope.element = Some("missing-exact-element".into());
        assert!(!grounding_matches(&binding, &scene, &semantic));
        let mut changed = scene.clone();
        changed
            .binding
            .as_mut()
            .unwrap()
            .mutation_context
            .as_mut()
            .unwrap()
            .window_title = "other document".into();
        assert!(!grounding_matches(&binding, &changed, &target));
        let mut changed = scene;
        changed.binding.as_mut().unwrap().scope.pid = 8;
        assert!(!grounding_matches(&binding, &changed, &target));
    }
}
