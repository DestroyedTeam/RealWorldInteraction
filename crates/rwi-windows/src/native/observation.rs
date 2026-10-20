//! Native scene acquisition and pre/post identity checks.
use super::*;

impl NativeWindows {
    pub(super) fn scene(
        &mut self,
        with_capture: bool,
        region: Option<[u32; 4]>,
        semantic_scope: crate::revalidation::Scope,
        target: Option<&Resolved>,
    ) -> Result<Scene> {
        self.check_identity()?;
        let _dpi = DpiScope::enter()?;
        let before = self.current()?;
        let foreground = desktop::input_foreground(&before, self.desktop_policy.is_some())?;
        let displays = desktop::displays()?;
        let rectangle = before
            .dwm_visible_frame
            .or(before.rectangle)
            .ok_or_else(|| {
                fault(
                    "window_geometry_unavailable",
                    "Window bounds unavailable or exceed image budget.",
                )
            })?;
        if !displays.iter().any(|d| d.physical_rect.contains(rectangle)) {
            return Err(fault(
                "capture_spanning_or_offscreen_unavailable",
                "M1 requires the entire window on one physical monitor. No silent clipping or resolution changes.",
            ));
        }
        if !before.visible || before.minimized || before.cloaked != Some(false) {
            return Err(fault(
                "window_not_visible",
                "Window hidden, minimized, or cloak state unknown. No restore/focus side effect.",
            ));
        }
        let rectangle = match region {
            Some(r) => crate::scoped_region(rectangle, r)?,
            None => rectangle,
        };
        let epoch = before.native_epoch.as_deref().unwrap();
        let semantic_read = match semantic_scope {
            crate::revalidation::Scope::FullTree => {
                uia::walk(hwnd(&before.window)?, before.pid, epoch)
            }
            crate::revalidation::Scope::RootIdentity => uia::root_identity(hwnd(&before.window)?),
            crate::revalidation::Scope::TargetElement => uia::revalidate_target(
                hwnd(&before.window)?,
                before.pid,
                epoch,
                target.ok_or_else(|| {
                    fault(
                        "exact_element_required",
                        "Targeted scene requires the original reference.",
                    )
                })?,
            ),
        };
        let (mut elements, modal, uia_details) = match semantic_read {
            Ok(walk) => {
                if self
                    .root_runtime
                    .as_ref()
                    .is_some_and(|r| r != &walk.root_runtime_id)
                {
                    return Err(fault(
                        "uia_root_epoch_changed",
                        "Native UIA root changed; old references are invalid.",
                    ));
                }
                self.root_runtime = Some(walk.root_runtime_id.clone());
                (
                    walk.elements.clone(),
                    walk.modal,
                    json!({"status":"available","read_scope":semantic_scope,"walk":walk}),
                )
            }
            Err(e) if e.code == "operation_cancelled" || e.code == "operation_deadline" => {
                return Err(e);
            }
            Err(e) => (vec![], None, json!({"status":"unavailable","error":e})),
        };
        elements.retain(|e| {
            e.point[0] >= f64::from(rectangle.x)
                && e.point[1] >= f64::from(rectangle.y)
                && e.point[0] < f64::from(rectangle.x) + f64::from(rectangle.width)
                && e.point[1] < f64::from(rectangle.y) + f64::from(rectangle.height)
        });
        let (png, timing) = if with_capture {
            let (p, t) = capture::capture(rectangle)?;
            (p, Some(t))
        } else {
            (vec![], None)
        };
        checkpoint("windows_post_capture_validation")?;
        self.check_identity()?;
        let after = self.current()?;
        let after_displays = desktop::displays()?;
        if foreground != desktop::input_foreground(&after, self.desktop_policy.is_some())?
            || before.visible != after.visible
            || before.minimized != after.minimized
            || before.cloaked != after.cloaked
            || before.window_dpi != after.window_dpi
            || before.rectangle != after.rectangle
            || before.dwm_visible_frame != after.dwm_visible_frame
            || before.foreground != after.foreground
            || before.owner_chain != after.owner_chain
            || before.owner_chain_complete != after.owner_chain_complete
            || before.enabled != after.enabled
            || before.title != after.title
            || before.title_truncated != after.title_truncated
            || serde_json::to_value(&displays).ok() != serde_json::to_value(&after_displays).ok()
        {
            return Err(fault(
                "native_scene_changed",
                "Focus, geometry, display or owner relation changed during observation; discard and reobserve.",
            ));
        }
        let binding = Binding {
            mutation_context: if self.desktop_policy.is_some() && !before.title_truncated {
                Some(MutationContext {
                    window_title: before.title.clone(),
                    document_state: None,
                })
            } else if !before.title_truncated {
                uia_details["walk"]["document_state"]
                    .as_str()
                    .map(|state| MutationContext {
                        window_title: before.title.clone(),
                        document_state: Some(state.into()),
                    })
            } else {
                None
            },
            scope: Scope {
                window: before.window.clone(),
                pid: before.pid,
                element: None,
            },
            focus_window: foreground.window.clone(),
            layout: crate::revalidation::layout_fingerprint(&displays, rectangle, &before)?,
            content: digest(&png),
            native_epoch: digest(
                serde_json::to_string(&(
                    epoch,
                    &self.root_runtime,
                    &before.owner_chain,
                    &foreground,
                    before.enabled,
                ))
                .unwrap()
                .as_bytes(),
            ),
        };
        let focused_element_ids = uia_details["walk"]["details"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|x| x["keyboard_focus"] == true)
                    .filter_map(|x| x["element_id"].as_str())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.foreground_anchor = foreground.anchor.clone();
        let mutation_ready = self.input_grant.as_ref().is_some_and(|g| g.allows(&before))
            && foreground.anchor.is_some()
            && before.enabled
            && uia_details["status"] == "available";
        self.diagnostics = json!({"window":before,"input_foreground":foreground,"displays":displays,"focused_element_ids":focused_element_ids,"uia":uia_details,"capture_timing":timing,"capture_backend":"GetDC desktop / CreateDIBSection / BitBlt SRCCOPY|CAPTUREBLT / GdiFlush","capture_scope":"visible desktop pixels; other occlusion remains visible; RWI feedback is quiesced during grounding capture", "capture_bounds_source":if before.dwm_visible_frame.is_some(){"DWM_EXTENDED_FRAME_BOUNDS in global physical pixels; NOT a PrintWindow buffer"}else{"GetWindowRect in PMv2 physical pixels"},"content_fidelity":"8-bit SDR; HDR/protected-content/cursor fidelity unavailable","native_window_epoch_assurance":"HWND + PID + process creation + class; UIA root runtime when available. No WinEvent generation tracking","physical_host_guest_mapping":"not attested inside Windows; external input grant records configured VM route separately","input_grant":self.input_grant});
        Ok(Scene{identity:self.identity.clone(),assurance:"native-attested".into(),binding:Some(binding),mutation_ready,blocked:Some(!before.enabled || foreground.anchor.is_none()),image_size:[rectangle.width,rectangle.height],pixel_to_desktop:Some([1.,0.,0.,1.,f64::from(rectangle.x),f64::from(rectangle.y)]),elements,captured_unix_ms:SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,atomicity:"bounded UIA walk/GDI pre-post checks; input rechecks native focus/identity per balanced gesture; no atomic window-scoped SendInput or compositor transaction".into(),image_normalization:format!("top-down BGRA BI_RGB to RGB PNG; alpha discarded; physical pixels, no scaling; modal={modal:?}"),png})
    }
}
