//! Independent positive exact-window hit for sparse AX canvases. Read-only;
//! no global timeout mutation, cursor, activation, event, or permission prompt.
use super::{Cf, application_hit_geometry, ax_rect, displays, ffi, hit_test};

fn matches(owner: bool, object: bool, number: bool, geometry: bool, display: bool) -> bool {
    owner && object && number && geometry && display
}
pub(super) fn read(window: &Cf, pid: u32, number: u32, point: [f64; 2]) -> Option<bool> {
    let screens = displays().ok()?;
    let containing = screens
        .iter()
        .filter(|d| d.rect.contains(point))
        .collect::<Vec<_>>();
    if containing.len() != 1 || containing[0].rotation != 0. {
        return Some(false);
    }
    let system = unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide()).ok()? };
    // NEVER AXUIElementSetMessagingTimeout on SystemWide: it changes all objects.
    let hit = hit_test(&system, point).ok()?;
    let bounds = ax_rect(&hit).ok()?;
    Some(matches(
        hit.pid().ok() == Some(pid) && window.pid().ok() == Some(pid),
        unsafe { ffi::CFEqual(hit.0, window.0) != 0 },
        ffi::window_number_for_ax_element(&hit) == Some(number)
            && ffi::window_number_for_ax_element(window) == Some(number),
        application_hit_geometry(bounds, point),
        true,
    ))
}

#[cfg(test)]
mod tests {
    use super::super::{WindowCanvasProof, local_pixel_canvas};
    use super::*;
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
