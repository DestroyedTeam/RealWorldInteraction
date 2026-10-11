//! Headless per-user desktop backend; selection is an explicit later read-only call.
use crate::{
    capture::Capturer,
    catalog::{Catalog, WindowRecord, matches_query},
    geometry::Rect,
    native::{Display, NativeMac, process_epoch, window_inventory, windows},
    session,
};
use rwi_core::*;
mod backend;
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

const PLACEMENT_SETTLE_BUDGET: Duration = Duration::from_millis(950);
const PLACEMENT_SETTLE_INTERVAL: Duration = Duration::from_millis(50);

/// Two distinct native inventory samples must agree. A first sample merely
/// showing the destination display can still be an in-flight window animation.
fn placement_stable(previous: &mut Option<Rect>, current: Option<Rect>) -> bool {
    let Some(current) = current else {
        *previous = None;
        return false;
    };
    let stable = previous.is_some_and(|last| last.same(current));
    *previous = Some(current);
    stable
}

fn placement_at_requested_origin(rect: Rect, origin: [f64; 2]) -> bool {
    (rect.x - origin[0]).abs() <= 1.0 && (rect.y - origin[1]).abs() <= 1.0
}

pub struct NativeMacDesktop {
    identity: Identity,
    uid: u32,
    allow_input: bool,
    selected: Option<NativeMac>,
    catalog: Catalog,
    capturer: Capturer,
    dispatches: Arc<AtomicU64>,
    authorized_display: Option<crate::catalog::AuthorizedDisplay>,
    live: Option<crate::live::Session>,
    fixed_attachment: bool,
}
struct DesktopLiveControl {
    identity: Identity,
    uid: u32,
    session: Option<Arc<crate::live::Controls>>,
}
impl LiveController for DesktopLiveControl {
    fn control(&self, action: LiveAction) -> Result<Value> {
        if action == LiveAction::Start {
            return Err(fault(
                "live_start_requires_executor",
                "Preview creation stays on the native work lane.",
            ));
        }
        Ok(self.session.as_ref().map(|session| session.control_native(action))
            .unwrap_or_else(|| json!({"status":"stopped","worker_done":true,"capture_pending":false,"presenter_visible":false,"presenter_image_present":false,"options":null,"input_authority":false,"input_sent":false})))
    }
    fn view_update(&self, update: LiveViewUpdate) -> Result<Value> {
        if session::identity_for_uid(self.uid)? != self.identity {
            return Err(fault(
                "session_mismatch",
                "The native macOS user/security session/boot/seat changed; open a new context.",
            ));
        }
        self.session
            .as_ref()
            .ok_or_else(|| {
                fault(
                    "live_not_started",
                    "Start a bound preview before publishing presence.",
                )
            })?
            .view_update(update)
    }
}
impl NativeMacDesktop {
    /// Reads only native process/console identity, never windows, AX, capture or input.
    pub fn new(
        uid: u32,
        allow_input: bool,
        authorized_display: Option<crate::AuthorizedDisplay>,
    ) -> Result<Self> {
        Ok(Self::from_identity(
            session::identity_for_uid(uid)?,
            uid,
            allow_input,
            authorized_display,
        ))
    }
    fn from_identity(
        identity: Identity,
        uid: u32,
        allow_input: bool,
        authorized_display: Option<crate::AuthorizedDisplay>,
    ) -> Self {
        Self {
            identity,
            uid,
            allow_input,
            selected: None,
            catalog: Catalog::default(),
            capturer: Capturer::default(),
            dispatches: Arc::new(AtomicU64::new(0)),
            authorized_display,
            live: None,
            fixed_attachment: false,
        }
    }
    /// Same exact catalog/window/process validation for legacy binding and
    /// side-effect-free attachment. Never selects by title or foreground.
    fn validate_selection(
        &self,
        selection: &WindowSelection,
    ) -> Result<(WindowRecord, crate::native::Window, Option<Display>)> {
        self.check_identity()?;
        let selected = self.catalog.select(selection)?;
        let all = if selected.semantic_only {
            crate::native::focused_dialog_window(selected.pid)?
                .into_iter()
                .collect()
        } else {
            windows()?
        };
        let candidates = all
            .iter()
            .filter(|w| format!("cgwindow:{}", w.id) == selected.window)
            .collect::<Vec<_>>();
        if candidates.len() != 1 || candidates[0].pid != selected.pid {
            return Err(fault(
                "window_epoch_changed",
                "The exact discovered window/PID is absent or ambiguous; no replacement by title.",
            ));
        }
        let current = candidates[0].clone();
        let display = crate::native::displays()
            .ok()
            .and_then(|list| crate::native::containing_display(&list, current.rect).cloned());
        let current_record = WindowRecord::from_native(
            &self.identity,
            &current,
            process_epoch(current.pid)?,
            display.as_ref(),
        )?;
        selected.require_same(&if selected.semantic_only {
            current_record.logical()
        } else {
            current_record
        })?;
        Ok((selected, current, display))
    }
    fn selected_backend(
        &self,
        selected: &WindowRecord,
        current: &crate::native::Window,
    ) -> Result<NativeMac> {
        let mut backend = NativeMac::with_shared_resources(
            current.pid,
            current.id,
            self.allow_input,
            self.capturer.clone(),
            self.dispatches.clone(),
        )?;
        backend.semantic_only = selected.semantic_only;
        backend.require_discovered_process(&selected.process_start)?;
        if backend.identity() != self.identity {
            return Err(fault(
                "session_mismatch",
                "Native context changed while selecting a window.",
            ));
        }
        self.check_identity()?;
        Ok(backend)
    }
    fn check_identity(&self) -> Result<()> {
        if session::identity_for_uid(self.uid)? != self.identity {
            return Err(fault(
                "session_mismatch",
                "The native macOS user/security session/boot/seat changed; open a new context, never reuse its lease or frames.",
            ));
        }
        Ok(())
    }
    fn selected(&mut self) -> Result<&mut NativeMac> {
        self.selected.as_mut().ok_or_else(||fault("window_selection_required","Discover and explicitly bind an exact CGWindowID/PID/process fingerprint. No foreground window is chosen automatically."))
    }
    /// Read-only bounded settle after exactly one AXPosition mutation. A
    /// timeout reports unconfirmed placement rather than replaying the move.
    fn settle_window_placement(
        &mut self,
        pid: u32,
        window_name: &str,
        target: &Display,
        requested_origin: [f64; 2],
    ) -> Result<Value> {
        let started = Instant::now();
        let semantic_only = self.selected()?.semantic_only;
        let mut previous = None;
        let mut checks = 0u32;
        loop {
            rwi_core::operation::checkpoint("macos_window_placement_readback")?;
            self.check_identity()?;
            self.selected()?
                .require_discovered_process(&process_epoch(pid)?)?;
            let current_displays = crate::native::displays()?;
            let topology_same = current_displays.iter().any(|display| display == target);
            let window = if semantic_only {
                // The logical AX dialog can have its own off-screen CG ID.
                // Its independently grounded focused record is the only valid
                // readback; a separately numbered rendered panel is not an alias.
                self.selected()?.window_record().ok()
            } else {
                windows()?.into_iter().find(|window| {
                    window.pid == pid && format!("cgwindow:{}", window.id) == window_name
                })
            };
            checks += 1;
            let bounds = window.as_ref().map(|window| window.rect);
            let current_display = bounds.and_then(|rect| {
                crate::native::containing_display(&current_displays, rect)
                    .map(crate::catalog::DisplayRecord::from_native)
            });
            let on_target = topology_same
                && current_display
                    .as_ref()
                    .is_some_and(|display| display.id == target.id);
            let confirmed = placement_stable(
                &mut previous,
                bounds.filter(|rect| {
                    on_target && placement_at_requested_origin(*rect, requested_origin)
                }),
            );
            if confirmed || started.elapsed() >= PLACEMENT_SETTLE_BUDGET {
                return Ok(json!({
                    "window": window_name,
                    "bounds": bounds.map(|rect| [rect.x, rect.y, rect.w, rect.h]),
                    "display": current_display,
                    "requested_origin": requested_origin,
                    "placement_confirmed": confirmed,
                    "semantic_only": semantic_only,
                    "rendered_surface_confirmed": if semantic_only {Value::Null} else {json!(confirmed)},
                    "settle_checks": checks,
                    "settle_elapsed_ms": started.elapsed().as_millis(),
                    "native_move_sent": true,
                    "input_sent": false,
                    "activated": false,
                    "next": if confirmed && semantic_only {
                        "Logical AX dialog placement is confirmed. Discover the rendered popup separately before claiming its visual surface moved; old references are invalid."
                    } else if confirmed {
                        "Reobserve the moved window before input; old frames and coordinates are invalid."
                    } else {
                        "Placement was not confirmed within the bounded readback. Discover the exact window and display; do not replay the move."
                    },
                }));
            }
            std::thread::sleep(PLACEMENT_SETTLE_INTERVAL);
        }
    }
}

pub(crate) fn native_capabilities(
    allow_input: bool,
    permissions: Value,
    console_set: Option<i64>,
) -> Value {
    let mut capabilities = json!({
        "adapter":"rust-macos-native","capture":"ScreenCaptureKit.SCScreenshotManager",
        "input":"CoreGraphics.CGEvent","oracle":"Accessibility.AXUIElement",
        "native_semantic_actions":["AXPress","AXOpen","AXConfirm","AXScrollToVisible","AXScrollDownByPage","AXScrollUpByPage","AXScrollLeftByPage","AXScrollRightByPage"],
        "checked_context_policy":"native_element","input_opt_in":allow_input,
        "semantic_object_identity":{"implemented":true,"scope":"current selected native backend","identity":"retained type-checked AXUIElement plus CFEqual window/object identity and non-reused serial; not attribute hash alone","max_retained_objects":crate::native::AX_TREE_MAX_NODES*2,"revalidation":"selected object only; exact native window membership, process/provider, geometry, role, value, enabled/actions and existing core binding guards","ancestry_max_nodes":32,"ancestry_read_budget_ms":500,"replacement_or_eviction":"fresh observation required; no attribute-search fallback"},
        "permissions":permissions,"permission_prompts":false,
        "evidence":"native_api_observed","hardware_attestation":false,
        "seat_kind":"logical_on_console_domain","native_console_set_id":console_set,"native_hardware_seat_observed":console_set.is_some(),
        "window_discovery":{"implemented":true,"scope":"on_screen_same_native_user; exact-PID queries may additionally expose one independently grounded focused semantic-only AX dialog","max_native_rows":2048,"max_page_rows":8,"max_page_bytes":12288,"catalog_ttl_ms":10000},
        "binding_identity":"fresh exact CGWindowID (including a logical AX dialog's own ID) + PID + process start + context + observed bounds/title fingerprint; no window creation-time or continuous lifetime attestation",
        "semantic_dialog_discovery":{"implemented":true,"exact_pid_required":true,"visual_available":false,"input":"advertised native AX actions only"},
        "visual_only_snapshot":{"implemented":true,"trigger":"AXWindows explicitly unsupported/no-value or complete unambiguous AX search with no match","capture":"exact bound CGWindowID/PID via ScreenCaptureKit","controls":false,"input_reference":false,"native_tile":true,"live_source_unchanged":true},
        "local_pixel_grounding":{"implemented":true,"evidence":"stable textured local bitmap patch plus native input checks","native_surfaces":["AXScrollArea","exact application-hit AXWindow with explicitly empty AXChildren"],"window_canvas_requires_exact_pid_object_and_window_number":true,"blank_patch":"full_frame_match_required","visual_only_input":false},
        "modal_policy":"the exact focused selected dialog may receive its advertised AX action; a different window blocked by it or an unknown modal state fails closed",
        "ax_tree":"bounded_partial","ax_tree_budget":{"max_nodes":crate::native::AX_TREE_MAX_NODES,"max_queue":crate::native::AX_TREE_MAX_QUEUE,"max_duration_ms":3000,"max_depth":crate::native::AX_TREE_MAX_DEPTH},"mapping":"single containing unrotated display only",
        "pointer_move":{"implemented":true,"event":"mouse_moved","button_transition":false,"activation_requested":false,"cursor_warp_requested":false,"global_fallback":false,"motion_history":"unknown","goal_verification":"independent","scope":"one grounded exact-window point; native prerequisites checked per operation"},
        "pointer_drag":{"implemented":true,"button":"left","coordinate_space":"offset_from_grounded_reference_in_desktop_points","max_waypoints":rwi_core::trajectory::MAX_WAYPOINTS,"max_duration_ms":rwi_core::trajectory::MAX_DURATION_MS,"sample_interval_ms":crate::native::NATIVE_SAMPLE_INTERVAL_MS,"sampling_policy":"bounded interpolation; all requested vertices and times retained","max_lateness_ms":rwi_core::input_trajectory::MAX_LATENESS_MS,"scope":"pinned window; every sample must stay inside the observed frame; background postToPid attempt, no cursor movement or effect acknowledgement"},"pointer_feedback":{"implemented":true},
        "capture_visual":{"implemented":true,"selected_window_required":true,"source_space":"macos_desktop_points","max_edge":{"minimum":64,"maximum":2048,"default":1024},"capture":"exact_window_ScreenCaptureKit","input_authority":false,"frame_created":false,"ax_walk":false,"retention":"response_only"},"native_roi":{"implemented":false},
        "live":{"implemented":true,"ownership":"MCP_device_context","source":"explicit_bound_window","automatic_owner_inference":false,"presentation":"RWI_Agent_native_panel","max_sessions":4,"default_duration_ms":120000,"max_duration_ms":600000},
        "lifecycle":{"implemented":false},"recovery":"read-only HID + combined session resource state, requiring listen-event preflight and agent idle writer proof",
        "unsupported":["legacy_grant_activation","launch","native owner-chain transition","minimized/hidden window discovery","mixed-display windows","rotated displays","native ROI","clipboard","unattended unlock"],
        "text_input":"only the exact focused, explicitly classified non-secure AX text element; unknown classifications remain unavailable",
        "background_operation":"exact-window observation; process/window-addressed input without system-cursor movement or explicit system-frontmost request; target AppKit active/key/order may change, client-dependent; independently verify effect and receiver state",
        "background_input":{"delivery":"CGEventPostToPid/window-addressed","foreground_transition":"none","native_acknowledgement":false,"application_effect_attested":false,"readback_required":true},
        "interruption_coverage":"per-batch focus, hit-test, layout, session, permissions, lease and cancellation; not a complete global human-input observer",
        "lock_state":"unknown","live_acceptance":"per_session_status_required",
        "atomicity":"pre/post validation; neither CGEventPost nor property equality proves the business goal"
    });
    capabilities["background_input"]["foreground_transition_scope"] =
        json!("no explicit system-frontmost request; not target AppKit key/order preservation");
    capabilities["background_input"]["target_appkit_state_unchanged_attested"] = json!(false);
    capabilities["background_input"]["window_order_unchanged_attested"] = json!(false);
    capabilities["background_input"]["receiver_preparation"] = json!({
        "pointer":"none; requested addressed sequence only",
        "keyboard":"legacy AppKit activation when inactive after payload/focus validation",
        "application_effect_attested":false,
    });
    capabilities["foreground_activation"] = json!({
        "implemented": true,
        "route": "explicit public AXRaise/AXFrontmost once; on-screen exact visual window",
        "fallback": false,
    });
    capabilities["native_semantic_action_policy"] = json!("advertised_element_action");
    capabilities["pointer_move"]["default_delivery"] = json!("background_window");
    capabilities["pointer_move"]["foreground_system"] = json!({
        "implemented":true,"route":"explicit CGEventPost(HID)","moves_system_cursor":true,
        "activation_requested":false,"fallback":false,"native_acknowledgement":false,
        "value":{"delivery":"foreground_system","expected_foreground":"fresh exact selected-window tuple from current_foreground"},
        "requires":"fresh grounded reference + shared input lease + exact current foreground + idle HID + existing post/listen permissions",
        "goal_verification":"independent; not guaranteed tracking or tooltip"
    });
    capabilities
}

fn feedback_overlay(visible: bool, point: Option<[f64; 2]>) -> Result<Value> {
    if visible {
        let point = match point {
            Some(point) => point,
            None => crate::overlay::last_point().ok_or_else(|| {
                fault(
                    "invalid_arguments",
                    "Provide point on the first visible request.",
                )
            })?,
        };
        if point.iter().any(|v| !v.is_finite()) {
            return Err(fault("invalid_arguments", "Point must be finite."));
        }
        crate::overlay::show_at(point);
    } else {
        crate::overlay::hide();
    }
    Ok(crate::overlay::status_json())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn backend() -> NativeMacDesktop {
        NativeMacDesktop::from_identity(
            Identity {
                host: "synthetic".into(),
                guest: None,
                session: "uid:501:security:100:boot:1".into(),
                seat: "console:1".into(),
                desktop: "quartz-console".into(),
            },
            501,
            false,
            None,
        )
    }
    #[test]
    fn unselected_backend_constructs_and_rejects_observation_without_reading_native_ui() {
        let mut backend = backend();
        assert_eq!(backend.input_dispatch_count(), Some(0));
        assert_eq!(
            backend.observe().unwrap_err().code,
            "window_selection_required"
        );
        assert!(!backend.allow_input);
        assert!(backend.selected.is_none());
        assert_eq!(
            backend.live(LiveAction::Status, None).unwrap()["status"],
            "stopped"
        );
    }
    #[test]
    fn placement_needs_two_matching_target_samples_and_resets_on_gap_or_animation() {
        let first = Rect {
            x: 1536.0,
            y: 24.0,
            w: 586.0,
            h: 488.0,
        };
        let shifted = Rect { x: 1537.0, ..first };
        let mut previous = None;
        assert!(!placement_stable(&mut previous, Some(first)));
        assert!(!placement_stable(&mut previous, Some(shifted)));
        assert!(placement_stable(&mut previous, Some(shifted)));
        assert!(!placement_stable(&mut previous, None));
        assert!(!placement_stable(&mut previous, Some(first)));
        assert!(placement_stable(&mut previous, Some(first)));
    }
    #[test]
    fn placement_on_target_display_is_not_confirmation_if_ax_ignored_the_origin() {
        let requested = [1536.0, 24.0];
        let unchanged_sheet = Rect {
            x: 2016.0,
            y: 158.0,
            w: 260.0,
            h: 258.0,
        };
        assert!(!placement_at_requested_origin(unchanged_sheet, requested));
        assert!(placement_at_requested_origin(
            Rect {
                x: 1536.5,
                y: 24.5,
                ..unchanged_sheet
            },
            requested
        ));
    }

    #[test]
    fn background_claim_does_not_attest_appkit_key_or_window_order_preservation() {
        let cap = native_capabilities(false, json!({}), None);
        assert_eq!(
            cap["background_input"]["target_appkit_state_unchanged_attested"],
            false
        );
        assert_eq!(
            cap["background_input"]["window_order_unchanged_attested"],
            false
        );
        assert_eq!(cap["background_input"]["foreground_transition"], "none");
        assert!(
            cap["background_input"]["foreground_transition_scope"]
                .as_str()
                .unwrap()
                .contains("system")
        );
    }
    #[test]
    fn capabilities_separate_native_implementation_permissions_and_unsupported_tools() {
        let cap = native_capabilities(
            false,
            json!({"accessibility":false,"screen_capture":false,"post_event":false,"listen_event":false}),
            None,
        );
        assert_eq!(cap["window_discovery"]["implemented"], true);
        assert!(
            cap["native_semantic_actions"]
                .as_array()
                .is_some_and(|actions| actions.iter().any(|action| action == "AXScrollToVisible"))
        );
        assert_eq!(cap["input_opt_in"], false);
        assert_eq!(cap["pointer_drag"]["implemented"], true);
        assert_eq!(cap["pointer_drag"]["button"], "left");
        assert_eq!(cap["background_input"]["foreground_transition"], "none");
        assert_eq!(cap["background_input"]["native_acknowledgement"], false);
        assert_eq!(
            cap["background_input"]["application_effect_attested"],
            false
        );
        assert_eq!(cap["background_input"]["readback_required"], true);
        assert_eq!(cap["capture_visual"]["implemented"], true);
        assert_eq!(cap["capture_visual"]["input_authority"], false);
        assert_eq!(cap["visual_only_snapshot"]["implemented"], true);
        assert_eq!(cap["visual_only_snapshot"]["input_reference"], false);
        assert_eq!(cap["local_pixel_grounding"]["implemented"], true);
        assert_eq!(cap["local_pixel_grounding"]["visual_only_input"], false);
        assert_eq!(
            cap["local_pixel_grounding"]["window_canvas_requires_exact_pid_object_and_window_number"],
            true
        );
        assert_eq!(
            cap["pointer_drag"]["max_duration_ms"],
            rwi_core::trajectory::MAX_DURATION_MS
        );
        for key in ["native_roi", "lifecycle"] {
            assert_eq!(cap[key]["implemented"], false);
        }
        assert_eq!(cap["pointer_feedback"]["implemented"], true);
        assert_eq!(cap["semantic_dialog_discovery"]["visual_available"], false);
        assert!(
            cap["unsupported"]
                .as_array()
                .unwrap()
                .iter()
                .all(|item| item != "Pointer feedback")
        );
        assert_eq!(cap["lock_state"], "unknown");
        assert_eq!(cap["hardware_attestation"], false);
        assert_eq!(cap["seat_kind"], "logical_on_console_domain");
        assert_eq!(cap["native_hardware_seat_observed"], false);
        assert!(cap["native_console_set_id"].is_null());
        assert_eq!(cap["live_acceptance"], "per_session_status_required");
    }
    #[test]
    #[ignore = "Explicit process-only native diagnostic: identity and non-prompting permission booleans; no window or UI access"]
    fn process_only_native_diagnostic_without_windows_or_ui_access() {
        let uid = unsafe { libc::getuid() };
        let status = session::process_status(uid);
        let mut report = json!({"diagnostic":"process_only_no_ui","process":status});
        match NativeMacDesktop::new(uid, false, None) {
            Ok(mut backend) => {
                assert!(backend.selected.is_none());
                assert_eq!(
                    backend.observe().unwrap_err().code,
                    "window_selection_required"
                );
                let dir = tempfile::tempdir().unwrap();
                let mut engine = Engine::new(dir.path().into()).unwrap();
                engine
                    .register("process-only-native-desktop".into(), Box::new(backend))
                    .unwrap();
                report["targets"] = engine.targets();
                report["unselected_constructor"] = json!("available");
            }
            Err(error) => {
                report["unselected_constructor"] = json!("unavailable");
                report["error"] = json!(error);
            }
        }
        assert_eq!(report["process"]["screen_captured"], false);
        assert_eq!(report["process"]["window_list_read"], false);
        assert_eq!(report["process"]["input_sent"], false);
        println!("{}", serde_json::to_string(&report).unwrap());
    }
}
