//! Independent positive exact-window hit for sparse AX canvases. Read-only;
//! no global timeout mutation, cursor, activation, event, or permission prompt.
use super::{Cf, application_hit_geometry, ax_rect, displays, ffi, hit_test};

fn matches(owner: bool, object: bool, number: bool, geometry: bool, display: bool) -> bool {
    owner && object && number && geometry && display
}
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub(super) struct Evidence {
    pub matched: Option<bool>,
    stage: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    ax_error: Option<i64>,
    predicates: Option<Predicates>,
}
#[derive(Debug, Clone, Copy, serde::Serialize)]
struct Predicates {
    owner: bool,
    exact_object: bool,
    exact_number: bool,
    point_geometry: bool,
    display_mapping: bool,
}
impl Evidence {
    fn unavailable(stage: &'static str) -> Self {
        Self {
            matched: None,
            stage,
            ax_error: None,
            predicates: None,
        }
    }
}
pub(super) fn read(window: &Cf, pid: u32, number: u32, point: [f64; 2]) -> Evidence {
    let Ok(screens) = displays() else {
        return Evidence::unavailable("display_inventory_unavailable");
    };
    let containing = screens
        .iter()
        .filter(|d| d.rect.contains(point))
        .collect::<Vec<_>>();
    if containing.len() != 1 || containing[0].rotation != 0. {
        return Evidence {
            matched: Some(false),
            ..Evidence::unavailable("display_mapping_unsupported")
        };
    }
    let Ok(system) = (unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide()) }) else {
        return Evidence::unavailable("system_object_unavailable");
    };
    // NEVER AXUIElementSetMessagingTimeout on SystemWide: it changes all objects.
    let hit = match hit_test(&system, point) {
        Ok(hit) => hit,
        Err(error) => {
            return Evidence {
                ax_error: error
                    .diagnostic
                    .as_deref()
                    .and_then(|v| v.get("ax_error"))
                    .and_then(serde_json::Value::as_i64),
                ..Evidence::unavailable("system_hit_unavailable")
            };
        }
    };
    let Ok(bounds) = ax_rect(&hit) else {
        return Evidence::unavailable("system_hit_geometry_unavailable");
    };
    // Identical native reads and conjunction as the original admission check.
    let predicates = Predicates {
        owner: hit.pid().ok() == Some(pid) && window.pid().ok() == Some(pid),
        exact_object: unsafe { ffi::CFEqual(hit.0, window.0) != 0 },
        exact_number: ffi::window_number_for_ax_element(&hit) == Some(number)
            && ffi::window_number_for_ax_element(window) == Some(number),
        point_geometry: application_hit_geometry(bounds, point),
        display_mapping: true,
    };
    Evidence {
        matched: Some(matches(
            predicates.owner,
            predicates.exact_object,
            predicates.exact_number,
            predicates.point_geometry,
            predicates.display_mapping,
        )),
        stage: "system_hit_predicates_observed",
        ax_error: None,
        predicates: Some(predicates),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{WindowCanvasProof, local_pixel_canvas};
    use super::*;
    #[test]
    fn f406_system_unknown_diagnostics_do_not_become_false_or_positive_proof() {
        let value = serde_json::to_value(Evidence::unavailable("system_hit_unavailable")).unwrap();
        assert!(value["matched"].is_null());
        assert!(value["predicates"].is_null());
        assert!(value.get("ax_error").is_none());
        assert!(value.get("occluded").is_none());
    }
    fn sparse() -> WindowCanvasProof {
        WindowCanvasProof {
            application_hit: false,
            exact_window_object: true,
            same_pid: true,
            exact_window_number: true,
            children_count: Some(3),
            system_window_hit: Some(true),
        }
    }
    #[test]
    fn f401_sparse_root_requires_positive_independent_hit() {
        assert!(local_pixel_canvas(Some("AXWindow"), sparse()));
        for system_window_hit in [None, Some(false)] {
            assert!(!local_pixel_canvas(
                Some("AXWindow"),
                WindowCanvasProof {
                    system_window_hit,
                    ..sparse()
                }
            ));
        }
    }
    #[test]
    fn f401_system_root_cannot_borrow_pid_object_or_window_number() {
        for proof in [
            WindowCanvasProof {
                same_pid: false,
                ..sparse()
            },
            WindowCanvasProof {
                exact_window_object: false,
                ..sparse()
            },
            WindowCanvasProof {
                exact_window_number: false,
                ..sparse()
            },
        ] {
            assert!(!local_pixel_canvas(Some("AXWindow"), proof));
        }
        for i in 0..5 {
            let mut v = [true; 5];
            v[i] = false;
            assert!(!matches(v[0], v[1], v[2], v[3], v[4]));
        }
    }
    #[test]
    fn f401_existing_childless_and_scroll_contract_is_preserved() {
        assert!(local_pixel_canvas(
            Some("AXWindow"),
            WindowCanvasProof {
                application_hit: true,
                children_count: Some(0),
                system_window_hit: None,
                ..sparse()
            }
        ));
        assert!(local_pixel_canvas(
            Some("AXScrollArea"),
            WindowCanvasProof::default()
        ));
        for role in [None, Some("AXGroup"), Some("AXButton"), Some("AXTextField")] {
            assert!(!local_pixel_canvas(role, sparse()));
        }
    }
    #[test]
    fn f401_geometry_and_partial_children_are_not_implicit_authority() {
        assert!(!local_pixel_canvas(
            Some("AXWindow"),
            WindowCanvasProof {
                application_hit: true,
                system_window_hit: None,
                ..sparse()
            }
        ));
        assert!(!local_pixel_canvas(
            Some("AXWindow"),
            WindowCanvasProof {
                application_hit: false,
                children_count: Some(0),
                system_window_hit: None,
                ..sparse()
            }
        ));
        assert!(matches(true, true, true, true, true));
    }
}
