//! Live-only native sheet layers. Exact visual/input APIs retain their scope.
use super::*;
use crate::catalog::WindowRecord;
use rwi_core::{VisualCapture, VisualOptions, VisualRect};
use serde_json::json;
mod composite;

const MAX_SHEETS: usize = 3;
const SCAN_BUDGET: Duration = Duration::from_millis(200);
const RELAY_COOLDOWN: Duration = Duration::from_secs(1);

fn temporary_explicit_source_loss(code: &str) -> bool {
    matches!(
        code,
        "window_missing"
            | "process_identity_unknown"
            | "session_or_process_changed"
            | "window_identity_changed"
    )
}
/// CGWindowListCopyWindowInfo returns front-to-back rows. Render back-to-front,
/// and reject a missing/ambiguous layer instead of inventing a caller z-order.
fn visual_layer_order(windows: &[String], catalog: &[Window]) -> Result<Vec<usize>> {
    let mut order = Vec::new();
    for (index, window) in windows.iter().enumerate() {
        let positions = catalog
            .iter()
            .enumerate()
            .filter(|(_, row)| format!("cgwindow:{}", row.id) == *window)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if positions.len() != 1 || windows[..index].contains(window) {
            return Err(fault(
                "live_layer_order_unknown",
                "A selected visual layer is absent, duplicated or ambiguous in the exact native inventory.",
            ));
        }
        order.push((positions[0], index));
    }
    order.sort_by_key(|(position, _)| std::cmp::Reverse(*position));
    Ok(order.into_iter().map(|(_, index)| index).collect())
}

/// One exact captured offscreen root has no inter-window z-order to infer.
/// Multiple layers always retain native on-screen ordering checks.
fn capture_layer_order(
    windows: &[String],
    catalog: &[Window],
    single_offscreen_root: bool,
) -> Result<Vec<usize>> {
    if single_offscreen_root && windows.len() == 1 {
        return Ok(vec![0]);
    }
    visual_layer_order(windows, catalog)
}

/// One worker-local object, never an input reference or app-wide window cache.
/// Revalidate native type/PID/window/role/geometry on every pre/post owner walk.
#[derive(Default)]
pub(crate) struct LiveFamilyCapture {
    root: Option<CachedRoot>,
    next_relay: Option<Instant>,
    relay_attempts: u64,
    retained_root_checks: u64,
}
struct CachedRoot {
    record: WindowRecord,
    element: Cf,
}
struct RootFacts<'a> {
    native_type: bool,
    pid: Option<u32>,
    window: Option<u32>,
    rect: Option<Rect>,
    role: Option<&'a str>,
}
fn root_matches(pid: u32, window: u32, rect: Rect, facts: RootFacts<'_>) -> bool {
    facts.native_type
        && facts.pid == Some(pid)
        && facts.window == Some(window)
        && facts.rect.is_some_and(|actual| actual.same(rect))
        && matches!(facts.role, Some("AXWindow" | "AXSheet"))
}
fn relay_ready(now: Instant, next: Option<Instant>) -> bool {
    next.is_none_or(|deadline| now >= deadline)
}
impl LiveFamilyCapture {
    fn root(&mut self, source: &NativeMac, native: &visual::VisualWindow) -> Result<Cf> {
        if let Some(cached) = &self.root {
            if cached.record == native.record {
                self.retained_root_checks = self.retained_root_checks.saturating_add(1);
                if source.live_root_matches(&cached.element, native.rect) {
                    return Ok(cached.element.retained());
                }
                self.root = None;
                return Err(fault(
                    "live_owner_root_changed",
                    "Retained exact owner root no longer matches; no same-frame alias substitution.",
                ));
            }
            self.root = None;
        }
        let now = Instant::now();
        if !relay_ready(now, self.next_relay) {
            return Err(fault(
                "live_owner_relay_cooldown",
                "Owner identity is unavailable; bounded read-only inventory recovery is cooling down.",
            ));
        }
        self.next_relay = Some(now + RELAY_COOLDOWN);
        self.relay_attempts = self.relay_attempts.saturating_add(1);
        let app = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(source.pid as i32))? };
        unsafe { ffi::AXUIElementSetMessagingTimeout(app.0, 0.05) };
        let root = source.resolve_window_element(&app, native.rect)?;
        if !source.live_root_matches(&root, native.rect) {
            return Err(fault(
                "live_owner_root_changed",
                "Resolved owner root lacks exact native type/PID/window/role/geometry.",
            ));
        }
        self.root = Some(CachedRoot {
            record: native.record.clone(),
            element: root.retained(),
        });
        Ok(root)
    }
}
struct Sheet {
    id: u32,
    rect: Rect,
    element: Cf,
}
struct Family {
    sheets: Vec<Sheet>,
    complete: bool,
    reasons: Vec<&'static str>,
}
impl Family {
    fn same(&self, other: &Self) -> bool {
        self.complete == other.complete
            && self.reasons == other.reasons
            && self.sheets.len() == other.sheets.len()
            && self.sheets.iter().zip(&other.sheets).all(|(a, b)| {
                a.id == b.id
                    && a.rect.same(b.rect)
                    && unsafe { ffi::CFEqual(a.element.0, b.element.0) != 0 }
            })
    }
}
impl NativeMac {
    fn live_root_matches(&self, root: &Cf, expected: Rect) -> bool {
        let native_type = unsafe { ffi::CFGetTypeID(root.0) == ffi::AXUIElementGetTypeID() };
        if !native_type {
            return false;
        }
        let role = root.attr("AXRole").ok().and_then(|v| v.string().ok());
        root_matches(
            self.pid,
            self.window,
            expected,
            RootFacts {
                native_type,
                pid: root.pid().ok(),
                window: ffi::window_number_for_ax_element(root),
                rect: ax_rect(root).ok(),
                role: role.as_deref(),
            },
        )
    }
    fn live_sheets(&self, native: &visual::VisualWindow, memo: &mut LiveFamilyCapture) -> Family {
        let mut family = Family {
            sheets: Vec::new(),
            complete: true,
            reasons: Vec::new(),
        };
        let scan = (|| -> Result<()> {
            operation::checkpoint("live_family_owner_begin")?;
            let root = memo.root(self, native)?;
            let catalog = windows()?;
            let mut parents = VecDeque::from([root]);
            let start = Instant::now();
            while let Some(parent) = parents.pop_front() {
                operation::checkpoint("live_family_owner_children")?;
                if start.elapsed() >= SCAN_BUDGET {
                    family.complete = false;
                    family.reasons.push("owner_time_budget");
                    break;
                }
                let value = parent.attr("AXChildren")?;
                let children = unsafe { ffi::array(value.0, 64)? };
                for child in children {
                    operation::checkpoint("live_family_owner_child")?;
                    if start.elapsed() >= SCAN_BUDGET {
                        family.complete = false;
                        family.reasons.push("owner_time_budget");
                        break;
                    }
                    if unsafe { ffi::CFGetTypeID(child.0) != ffi::AXUIElementGetTypeID() } {
                        continue;
                    }
                    let role = child.attr("AXRole").ok().and_then(|v| v.string().ok());
                    if role.as_deref() != Some("AXSheet") {
                        continue;
                    }
                    if child.attr("AXHidden").ok().and_then(|v| v.boolean()) == Some(true) {
                        continue;
                    }
                    if child.pid().ok() != Some(self.pid) {
                        family.complete = false;
                        family
                            .reasons
                            .push("foreign_sheet_requires_explicit_selection");
                        continue;
                    }
                    let linked = child.attr("AXParent").ok().is_some_and(|p| unsafe {
                        ffi::CFEqual(p.0, parent.0) != 0 && ffi::CFEqual(child.0, parent.0) == 0
                    });
                    let id = ffi::window_number_for_ax_element(&child);
                    let rect = ax_rect(&child).ok();
                    let found = catalog
                        .iter()
                        .filter(|w| Some(w.id) == id && w.pid == self.pid)
                        .collect::<Vec<_>>();
                    if !linked
                        || id == Some(self.window)
                        || found.len() != 1
                        || !rect.is_some_and(|r| r.same(found[0].rect))
                    {
                        family.complete = false;
                        family.reasons.push("sheet_identity_or_owner_unknown");
                        continue;
                    }
                    let id = found[0].id;
                    if family.sheets.iter().any(|s| s.id == id) {
                        continue;
                    }
                    if family.sheets.len() >= MAX_SHEETS {
                        family.complete = false;
                        family.reasons.push("sheet_count_budget");
                        continue;
                    }
                    family.sheets.push(Sheet {
                        id,
                        rect: found[0].rect,
                        element: child.retained(),
                    });
                    parents.push_back(child);
                }
            }
            Ok(())
        })();
        if scan.is_err() {
            family.complete = false;
            family.reasons.push("owner_graph_read_unavailable");
        }
        family.reasons.sort_unstable();
        family.reasons.dedup();
        family
    }

    /// No new binding/reference or permission. Only exact, parent-linked sheets;
    /// unrelated same-PID windows and unverified foreign renderers are excluded.
    pub(crate) fn live_visual_capture_with_additional(
        &self,
        options: VisualOptions,
        memo: &mut LiveFamilyCapture,
        additional: &[std::sync::Arc<NativeMac>],
    ) -> Result<VisualCapture> {
        let before = self.visual_window()?;
        let family = self.live_sheets(&before, memo);
        let order_before = windows()?;
        let mut root = self.visual_capture(options)?;
        let mut frames = Vec::new();
        let mut explicit_windows = Vec::new();
        let mut explicit_omissions = Vec::new();
        for sheet in &family.sheets {
            operation::checkpoint("live_family_capture_sheet")?;
            let mut child = self.visual_copy()?;
            child.window = sheet.id;
            child.allow_input = false;
            child.semantic_only = false;
            let frame = child.visual_capture(options)?;
            let actual = Rect {
                x: f64::from(frame.source_rect.x),
                y: f64::from(frame.source_rect.y),
                w: f64::from(frame.source_rect.width),
                h: f64::from(frame.source_rect.height),
            };
            if !actual.same(sheet.rect) {
                return Err(fault(
                    "visual_window_changed",
                    "Owned sheet geometry changed; discard this read-only frame.",
                ));
            }
            frames.push(frame);
        }
        for source in additional {
            operation::checkpoint("live_explicit_source_capture")?;
            match source.visual_capture(options) {
                Ok(frame) => {
                    // A caller may explicitly select an already verified sheet.
                    // Do not composite one window twice or call it a new edge.
                    if frame.window.window != root.window.window
                        && !frames
                            .iter()
                            .any(|other| other.window.window == frame.window.window)
                    {
                        if frames.len() >= MAX_SHEETS {
                            return Err(fault(
                                "live_family_budget",
                                "At most four combined root/sheet/explicit visual layers; select fewer sources.",
                            ));
                        }
                        explicit_windows.push(frame.window.window.clone());
                        frames.push(frame);
                    }
                }
                Err(error) if temporary_explicit_source_loss(&error.code) => {
                    explicit_omissions.push(json!({"pid":source.pid,"window":format!("cgwindow:{}",source.window),"reason":error.code,"replacement_selected":false}));
                }
                Err(error) => return Err(error),
            }
        }
        let ids = std::iter::once(&root)
            .chain(frames.iter())
            .map(|frame| frame.window.window.clone())
            .collect::<Vec<_>>();
        let order = capture_layer_order(&ids, &order_before, !before.on_screen_inventory_match)?;
        if order != capture_layer_order(&ids, &windows()?, !before.on_screen_inventory_match)? {
            return Err(fault(
                "visual_window_changed",
                "Selected visual layer order changed during capture; discard the composite.",
            ));
        }
        let after = self.visual_window()?;
        let rechecked = self.live_sheets(&after, memo);
        if before.record != after.record
            || before.on_screen_inventory_match != after.on_screen_inventory_match
            || before.displays != after.displays
            || !family.same(&rechecked)
        {
            return Err(fault(
                "visual_window_changed",
                "Exact Live root or parent-linked sheet set changed; discard this read-only composite.",
            ));
        }
        for source in additional {
            let id = format!("cgwindow:{}", source.window);
            if explicit_windows.contains(&id) {
                let expected = frames
                    .iter()
                    .find(|frame| frame.window.window == id)
                    .unwrap();
                let current = source.visual_window()?;
                let rect = expected.source_rect;
                if !current.rect.same(Rect {
                    x: f64::from(rect.x),
                    y: f64::from(rect.y),
                    w: f64::from(rect.width),
                    h: f64::from(rect.height),
                }) {
                    return Err(fault(
                        "visual_window_changed",
                        "An explicit additional visual source moved during composition; discard this frame.",
                    ));
                }
            }
        }
        let layers = std::iter::once(&root).chain(frames.iter()).map(|f| json!({
            "window":f.window, "source_rect":f.source_rect, "image_size":f.image_size,
            "captured_unix_ms":f.captured_unix_ms,
            "relation":if f.window.window == root.window.window {"selected_root"} else if explicit_windows.contains(&f.window.window) {"caller_explicit_visual_selection; ownership_not_inferred"} else {"verified_AXSheet_AXParent"}
        })).collect::<Vec<_>>();
        if !frames.is_empty() {
            let all = std::iter::once(&root)
                .chain(frames.iter())
                .collect::<Vec<_>>();
            let tiles = order
                .iter()
                .map(|index| all[*index])
                .map(|f| composite::Tile {
                    png: &f.png,
                    rect: Rect {
                        x: f64::from(f.source_rect.x),
                        y: f64::from(f.source_rect.y),
                        w: f64::from(f.source_rect.width),
                        h: f64::from(f.source_rect.height),
                    },
                    size: f.image_size,
                })
                .collect::<Vec<_>>();
            let composed = composite::compose(&tiles, options.max_edge)?;
            let native_scale = std::iter::once(&root)
                .chain(frames.iter())
                .map(|f| {
                    (f64::from(f.source_pixel_size[0]) / f64::from(f.source_rect.width))
                        .min(f64::from(f.source_pixel_size[1]) / f64::from(f.source_rect.height))
                })
                .fold(f64::INFINITY, f64::min);
            root.png = composed.png;
            root.image_size = composed.size;
            root.source_rect = VisualRect {
                x: composed.rect.x as i32,
                y: composed.rect.y as i32,
                width: composed.rect.w as u32,
                height: composed.rect.h as u32,
            };
            // The preview grid can be bounded below native size. Keep native
            // source resolution separate from the composed output dimensions.
            root.source_pixel_size = [
                (composed.rect.w * native_scale).round() as u32,
                (composed.rect.h * native_scale).round() as u32,
            ];
            let sx = composed.rect.w / f64::from(composed.size[0]);
            let sy = composed.rect.h / f64::from(composed.size[1]);
            root.visual_to_desktop = [
                sx,
                0.,
                0.,
                sy,
                composed.rect.x + (sx - 1.) / 2.,
                composed.rect.y + (sy - 1.) / 2.,
            ];
            root.atomicity = "non-atomic exact root/owned-sheet captures; native pre/post process/window/topology and AXParent set checks; visual-only, never an input frame".into();
            root.normalization = "bounded straight RGBA source-over at desktop-point positions; common non-upscaling grid; original source PNGs not modified".into();
        }
        root.details["family"] = json!({"scope":"selected_root_verified_attached_sheets_and_explicit_visual_selections",
            "layers":layers,"complete":family.complete && explicit_omissions.is_empty(),"reasons":family.reasons,"input_authority":false,
            "explicit_source_omissions":explicit_omissions,"automatic_foreign_owner_inference":false,
            "composition_order":if !before.on_screen_inventory_match && ids.len() == 1 { "single_exact_offscreen_root_no_inter_window_order" } else { "native_window_inventory_back_to_front_pre_post_checked" },
            "selection_updated":false,"new_reference":false,"all_same_pid_windows_included":false,
            "foreign_pid_requires_explicit_selection":true,"atomic":false,
            "root_relay_attempts":memo.relay_attempts,"retained_root_checks":memo.retained_root_checks});
        Ok(root)
    }
}

#[cfg(test)]
mod owner_root_tests {
    use super::*;
    #[test]
    fn explicit_visual_layers_follow_native_order_and_never_guess_missing_windows() {
        let rect = Rect {
            x: 0.,
            y: 0.,
            w: 10.,
            h: 10.,
        };
        let rows = vec![
            Window {
                id: 7,
                pid: 12,
                rect,
                alpha: 1.,
                title: None,
            },
            Window {
                id: 8,
                pid: 13,
                rect,
                alpha: 1.,
                title: None,
            },
        ];
        assert_eq!(
            visual_layer_order(&["cgwindow:8".into(), "cgwindow:7".into()], &rows).unwrap(),
            vec![0, 1]
        );
        assert_eq!(
            visual_layer_order(&["cgwindow:7".into(), "cgwindow:8".into()], &rows).unwrap(),
            vec![1, 0]
        );
        assert!(visual_layer_order(&["cgwindow:9".into()], &rows).is_err());
        assert!(visual_layer_order(&["cgwindow:7".into(), "cgwindow:7".into()], &rows).is_err());
        for code in [
            "window_missing",
            "process_identity_unknown",
            "session_or_process_changed",
            "window_identity_changed",
        ] {
            assert!(temporary_explicit_source_loss(code));
        }
        for code in [
            "cancelled",
            "screen_capture_permission_missing",
            "window_epoch_changed",
            "storage_or_transport_error",
        ] {
            assert!(!temporary_explicit_source_loss(code));
        }
    }
    #[test]
    fn retained_owner_root_requires_every_positive_native_fact() {
        let rect = Rect {
            x: 1800.,
            y: 24.,
            w: 660.,
            h: 552.,
        };
        let facts = || RootFacts {
            native_type: true,
            pid: Some(17),
            window: Some(23),
            rect: Some(rect),
            role: Some("AXWindow"),
        };
        assert!(root_matches(17, 23, rect, facts()));
        for facts in [
            RootFacts {
                native_type: false,
                ..facts()
            },
            RootFacts {
                pid: None,
                ..facts()
            },
            RootFacts {
                pid: Some(18),
                ..facts()
            },
            RootFacts {
                window: None,
                ..facts()
            },
            RootFacts {
                window: Some(24),
                ..facts()
            },
            RootFacts {
                rect: None,
                ..facts()
            },
            RootFacts {
                rect: Some(Rect { x: 1801., ..rect }),
                ..facts()
            },
            RootFacts {
                role: None,
                ..facts()
            },
            RootFacts {
                role: Some("AXButton"),
                ..facts()
            },
        ] {
            assert!(!root_matches(17, 23, rect, facts));
        }
    }
    #[test]
    fn failed_owner_inventory_has_a_real_bounded_cooldown() {
        let now = Instant::now();
        assert!(relay_ready(now, None));
        assert!(!relay_ready(now, Some(now + RELAY_COOLDOWN)));
        assert!(relay_ready(
            now + RELAY_COOLDOWN,
            Some(now + RELAY_COOLDOWN)
        ));
    }
}

#[cfg(test)]
mod offscreen_order_tests {
    use super::*;
    #[test]
    fn one_exact_offscreen_root_has_only_one_layer() {
        assert_eq!(
            capture_layer_order(&["cgwindow:8".into()], &[], true).unwrap(),
            vec![0]
        );
    }
    #[test]
    fn on_screen_or_multi_layer_missing_order_still_refuses() {
        assert!(capture_layer_order(&["cgwindow:8".into()], &[], false).is_err());
        assert!(
            capture_layer_order(&["cgwindow:8".into(), "cgwindow:9".into()], &[], true).is_err()
        );
    }
    #[test]
    fn empty_layers_do_not_invent_an_offscreen_root() {
        assert!(capture_layer_order(&[], &[], true).unwrap().is_empty());
    }
}
