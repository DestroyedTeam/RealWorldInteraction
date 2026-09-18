//! Purpose-specific native revalidation. Never changes observation authority.
use std::time::Duration;

/// A cropped frame still binds the entire window geometry, not just its ROI.
pub(crate) fn layout_fingerprint<D: serde::Serialize>(
    displays: &D,
    capture: rcu_imaging::Rect,
    selected: &crate::WindowRecord,
) -> rcu_core::Result<String> {
    serde_json::to_vec(&(
        displays,
        capture,
        selected.rectangle,
        selected.dwm_visible_frame,
        selected.window_dpi,
    ))
    .map(|bytes| rcu_core::digest(&bytes))
    .map_err(|_| {
        rcu_core::fault(
            "native_layout_unavailable",
            "Native capture/window geometry could not be serialized.",
        )
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Scope {
    FullTree,
    RootIdentity,
    TargetElement,
}
impl Scope {
    pub fn for_reference(native_element_policy: bool, has_element: bool) -> Self {
        if !native_element_policy {
            Self::FullTree
        } else if has_element {
            Self::TargetElement
        } else {
            Self::RootIdentity
        }
    }
}

#[derive(Default, serde::Serialize)]
pub(crate) struct Timings {
    pub full_tree: Measurement,
    pub root_identity: Measurement,
    pub target_element: Measurement,
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
            Scope::TargetElement => &mut self.target_element,
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
    fn same_roi_pixels_do_not_hide_outer_window_resize_dpi_or_display_change() {
        use rcu_imaging::Rect;
        let bounds = Rect {
            x: 100,
            y: 100,
            width: 1000,
            height: 700,
        };
        let roi = Rect {
            x: 150,
            y: 150,
            width: 100,
            height: 100,
        };
        let selected = crate::WindowRecord {
            window: "window".into(),
            pid: 42,
            process_created: Some(1),
            native_epoch: Some("epoch".into()),
            title: "fixture".into(),
            title_truncated: false,
            class: "fixture".into(),
            rectangle: Some(bounds),
            dwm_visible_frame: Some(bounds),
            visible: true,
            enabled: true,
            minimized: false,
            cloaked: Some(false),
            foreground: true,
            window_dpi: Some(96),
            window_dpi_awareness: 2,
            owner_chain: vec![],
            owner_chain_complete: true,
            parent: None,
            z_index: 0,
        };
        let displays = serde_json::json!([{"device":"synthetic","width":1920}]);
        let old = layout_fingerprint(&displays, roi, &selected).unwrap();
        for change in 0..5 {
            let mut window = selected.clone();
            let mut display = displays.clone();
            match change {
                0 => window.rectangle.as_mut().unwrap().width += 1,
                1 => window.dwm_visible_frame.as_mut().unwrap().height += 1,
                2 => window.window_dpi = Some(192),
                3 => window.dwm_visible_frame = None,
                _ => display[0]["width"] = 2560.into(),
            }
            assert_ne!(
                old,
                layout_fingerprint(&display, roi, &window).unwrap(),
                "{change}"
            );
        }
        assert_eq!(old, layout_fingerprint(&displays, roi, &selected).unwrap());
    }
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
        assert_eq!(Scope::for_reference(true, true), Scope::TargetElement);
        assert_eq!(Scope::for_reference(false, true), Scope::FullTree);
        assert_eq!(Scope::for_reference(false, false), Scope::FullTree);
    }
    #[test]
    fn timing_preserves_failures_and_separates_scopes() {
        let mut t = Timings::default();
        t.record(Scope::RootIdentity, Duration::from_millis(2), true);
        t.record(Scope::RootIdentity, Duration::from_millis(5), false);
        t.record(Scope::FullTree, Duration::from_millis(1000), true);
        t.record(Scope::TargetElement, Duration::from_millis(7), true);
        assert_eq!(t.root_identity.calls, 2);
        assert_eq!(t.root_identity.failures, 1);
        assert_eq!(t.root_identity.total_ms, 7.);
        assert_eq!(t.root_identity.max_ms, 5.);
        assert_eq!(t.full_tree.calls, 1);
        assert_eq!(t.full_tree.total_ms, 1000.);
        assert_eq!(t.target_element.calls, 1);
        assert_eq!(t.target_element.total_ms, 7.);
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
            placement_only: false,
            scope,
            element_revision: None,
            source_region: None,
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

    #[test]
    fn exact_target_still_binds_revision_identity_point_visibility_and_native_context() {
        use rcu_core::{Binding, Element, Identity, Resolved, Scene, Scope, grounding_matches};
        let element = Element {
            enabled: None,
            actions: vec![],
            role: None,
            id: "original".into(),
            name: "same label".into(),
            point: [10., 20.],
            visible: Some(true),
            occluded: Some(false),
            state_revision: Some("known-state".into()),
            selected: None,
            value: None,
            value_truncated: false,
            description: None,
        };
        let scope = Scope {
            window: "window".into(),
            pid: 9,
            element: None,
        };
        let binding = Binding {
            scope: scope.clone(),
            focus_window: "owner".into(),
            layout: "layout".into(),
            content: "not-compared-for-semantic".into(),
            native_epoch: "window-root-owner-epochs".into(),
            mutation_context: None,
        };
        let target = Resolved {
            reference: "r".into(),
            target: "t".into(),
            frame: "f".into(),
            point: element.point,
            placement_only: false,
            scope: Scope {
                element: Some(element.id.clone()),
                ..scope
            },
            element_revision: Some(element.revision()),
            source_region: None,
        };
        let scene = Scene {
            identity: Identity {
                host: "host".into(),
                guest: None,
                session: "session".into(),
                seat: "seat".into(),
                desktop: "desktop".into(),
            },
            assurance: "fixture".into(),
            binding: Some(binding.clone()),
            mutation_ready: true,
            blocked: Some(false),
            image_size: [100, 100],
            pixel_to_desktop: None,
            elements: vec![element],
            captured_unix_ms: 1,
            atomicity: "fixture".into(),
            image_normalization: "fixture".into(),
            png: vec![],
        };
        assert!(grounding_matches(&binding, &scene, &target));
        for field in [
            "id",
            "name",
            "point",
            "visible",
            "occluded",
            "state_revision",
            "layout",
            "native_epoch",
            "focus_window",
        ] {
            let mut changed = serde_json::to_value(&scene).unwrap();
            match field {
                "visible" | "occluded" => changed["elements"][0][field] = serde_json::Value::Null,
                "point" => changed["elements"][0][field] = serde_json::json!([11., 20.]),
                "layout" | "native_epoch" | "focus_window" => {
                    changed["binding"][field] = "changed".into()
                }
                _ => changed["elements"][0][field] = "changed".into(),
            }
            assert!(
                !grounding_matches(&binding, &serde_json::from_value(changed).unwrap(), &target),
                "{field}"
            );
        }
    }
}
