use crate::{
    InputGrant, WindowRecord, capture, check_relation,
    desktop::{self, DpiScope, hwnd},
    input, input_plan, uia,
};
use rcu_core::{
    Backend, Binding, Identity, Intent, Oracle, Predicate, Resolved, Result, Scene, Scope,
    WindowSelection, digest, fault, grounding_matches, operation::checkpoint,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct NativeWindows {
    identity: Identity,
    selected: Option<WindowRecord>,
    catalog: Vec<WindowRecord>,
    catalog_at: Option<Instant>,
    catalog_id: String,
    catalog_header: Value,
    catalog_complete: bool,
    root_runtime: Option<Vec<i32>>,
    diagnostics: Value,
    input_grant: Option<InputGrant>,
    grant_deadline: Option<Instant>,
    input_counters: input::Counters,
}
impl NativeWindows {
    /// Caller MUST allocate this execution desktop. No focus/input/capture here.
    pub fn new() -> Result<Self> {
        Self::new_with_grant(None)
    }
    pub fn new_with_grant(input_grant: Option<InputGrant>) -> Result<Self> {
        let identity = desktop::identity()?;
        let grant_deadline = input_grant.as_ref().map(|g| {
            Instant::now()
                + Duration::from_millis(
                    g.expires_unix_ms
                        .saturating_sub(input::now_ms())
                        .min(3_600_000),
                )
        });
        if let Some(grant) = &input_grant {
            grant.validate(&identity, input::now_ms())?;
            let root = desktop::record(hwnd(&grant.root.window)?, 0)?;
            if root.key().as_ref() != Some(&grant.root) {
                return Err(fault(
                    "input_grant_window_mismatch",
                    "Allocated root HWND/PID/process creation no longer exists; do not select a replacement by title.",
                ));
            }
        }
        Ok(Self {
            identity,
            selected: None,
            catalog: vec![],
            catalog_at: None,
            catalog_id: String::new(),
            catalog_header: Value::Null,
            catalog_complete: false,
            root_runtime: None,
            diagnostics: Value::Null,
            input_grant,
            grant_deadline,
            input_counters: input::Counters::default(),
        })
    }
    fn check_identity(&self) -> Result<()> {
        if desktop::identity()? != self.identity {
            return Err(fault(
                "session_mismatch",
                "Native logon/desktop identity changed; open a new explicitly allocated connection.",
            ));
        }
        if let Some(grant) = &self.input_grant {
            grant.validate(&self.identity, input::now_ms())?;
        }
        if self.grant_deadline.is_some_and(|d| Instant::now() >= d) {
            return Err(fault(
                "input_grant_expired",
                "Allocated monotonic deadline passed; a clock rollback never extends the input slot.",
            ));
        }
        Ok(())
    }
    fn selected(&self) -> Result<WindowRecord> {
        self.selected.clone().ok_or_else(||fault("window_selection_required","Discover windows, then bind one exact native_epoch/PID/HWND. No default foreground window is chosen."))
    }
    fn current(&self) -> Result<WindowRecord> {
        let old = self.selected()?;
        let now = desktop::record(hwnd(&old.window)?, 0)?;
        if old.native_epoch.is_none() || old.native_epoch != now.native_epoch || old.pid != now.pid
        {
            return Err(fault(
                "window_epoch_changed",
                "Selected process/window no longer matches. Do not bind by title.",
            ));
        }
        Ok(now)
    }
    fn scene(&mut self, with_capture: bool, region: Option<[u32; 4]>) -> Result<Scene> {
        self.check_identity()?;
        let _dpi = DpiScope::enter()?;
        let before = self.current()?;
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
        let (mut elements, modal, uia_details) =
            match uia::walk(hwnd(&before.window)?, before.pid, epoch) {
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
                        json!({"status":"available","walk":walk}),
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
        if before.visible != after.visible
            || before.minimized != after.minimized
            || before.cloaked != after.cloaked
            || before.window_dpi != after.window_dpi
            || before.rectangle != after.rectangle
            || before.dwm_visible_frame != after.dwm_visible_frame
            || before.foreground != after.foreground
            || before.owner_chain != after.owner_chain
            || before.enabled != after.enabled
            || serde_json::to_value(&displays).ok() != serde_json::to_value(&after_displays).ok()
        {
            return Err(fault(
                "native_scene_changed",
                "Focus, geometry, display or owner relation changed during observation; discard and reobserve.",
            ));
        }
        let binding = Binding {
            scope: Scope {
                window: before.window.clone(),
                pid: before.pid,
                element: None,
            },
            focus_window: desktop::window_id(unsafe {
                windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow()
            }),
            layout: digest(
                serde_json::to_string(&(&displays, rectangle, before.window_dpi))
                    .unwrap()
                    .as_bytes(),
            ),
            content: digest(&png),
            native_epoch: digest(
                serde_json::to_string(&(
                    epoch,
                    &self.root_runtime,
                    &before.owner_chain,
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
        let mutation_ready = self.input_grant.as_ref().is_some_and(|g| g.allows(&before))
            && before.foreground
            && before.enabled
            && uia_details["status"] == "available";
        self.diagnostics = json!({"window":before,"displays":displays,"focused_element_ids":focused_element_ids,"uia":uia_details,"capture_timing":timing,"capture_backend":"GetDC desktop / CreateDIBSection / BitBlt SRCCOPY|CAPTUREBLT / GdiFlush","capture_scope":"visible desktop pixels; occluding windows remain visible", "capture_bounds_source":if before.dwm_visible_frame.is_some(){"DWM_EXTENDED_FRAME_BOUNDS in global physical pixels; NOT a PrintWindow buffer"}else{"GetWindowRect in PMv2 physical pixels"},"content_fidelity":"8-bit SDR; HDR/protected-content/cursor fidelity unavailable","native_window_epoch_assurance":"HWND + PID + process creation + class; UIA root runtime when available. No WinEvent generation tracking","physical_host_guest_mapping":"not attested inside Windows; external input grant records configured VM route separately","input_grant":self.input_grant});
        Ok(Scene{identity:self.identity.clone(),assurance:"native-attested".into(),binding:Some(binding),mutation_ready,blocked:Some(!before.enabled || !before.foreground),image_size:[rectangle.width,rectangle.height],pixel_to_desktop:Some([1.,0.,0.,1.,f64::from(rectangle.x),f64::from(rectangle.y)]),elements,captured_unix_ms:SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,atomicity:"bounded UIA walk/GDI pre-post checks; input rechecks native focus/identity per balanced gesture; no atomic window-scoped SendInput or compositor transaction".into(),image_normalization:format!("top-down BGRA BI_RGB to RGB PNG; alpha discarded; physical pixels, no scaling; modal={modal:?}"),png})
    }
}
impl Backend for NativeWindows {
    fn identity(&self) -> Identity {
        self.identity.clone()
    }
    fn capabilities(&self) -> Value {
        json!({"backend":"windows-native-rust","execution_arch":std::env::consts::ARCH,"observe":true,"discover_windows":true,"bind_owned_popup":true,"semantic_source":"bounded native UIA control-view cache","input":self.input_grant.as_ref().is_some_and(|g|g.validate(&self.identity,input::now_ms()).is_ok()) && self.grant_deadline.is_some_and(|d|Instant::now()<d),"input_dispatch":self.input_counters,"input_grant":self.input_grant,"input_contract":"opt-in allocated root/owned dialogs + exact native identity + expiry + lease + foreground/runtime-ID checks; SendInput count is not goal verification; no auto-focus, clipboard or UIA mutation","text_limit_utf16":1024,"text_control_characters":false,"scroll_units":"signed wheel units, multiples of 120, +/-1200","verify":["window_title","focused_window","element_value (ValuePattern or bounded TextPattern)","modal"],"capture":"visible single-monitor-contained native window, GDI SDR RGB PNG","spanning_capture":false,"capture_hdr":false,"monitor_effective_dpi":"unavailable; awareness-qualified window DPI only","native_strong_cancellation":false,"native_provider_allocator_hard_limit":false,"cancellation":"cooperative per balanced gesture; no subsequent batch after observed cancel, lease expiry, grant expiry or focus loss; release-only cleanup may follow partial insertion; in-flight native call retains slot/resources","uia_connection_timeout_ms":250,"uia_transaction_timeout_ms":250,"tree_budget":{"nodes":128,"depth":8,"cooperative_ms":1500},"artifact_limits":{"frame_bytes":rcu_core::MAX_FRAME_BYTES,"total_bytes":rcu_core::ARTIFACT_BUDGET,"inline_bytes":rcu_core::INLINE_BYTES},"native_details":self.diagnostics})
    }
    fn discover(&mut self) -> Result<Value> {
        self.discover_with(&rcu_core::DiscoveryQuery::default())
    }
    fn discover_with(&mut self, query: &rcu_core::DiscoveryQuery) -> Result<Value> {
        query.validate()?;
        self.check_identity()?;
        if let Some(cursor) = &query.cursor {
            let (id, offset) = cursor.split_once('/').ok_or_else(|| {
                fault(
                    "invalid_catalog_cursor",
                    "Use the returned opaque next_cursor.",
                )
            })?;
            if id != self.catalog_id
                || self
                    .catalog_at
                    .is_none_or(|t| t.elapsed() > Duration::from_secs(10))
            {
                return Err(fault(
                    "catalog_expired",
                    "Repeat discovery with an exact pid/title filter; do not manually traverse unrelated windows.",
                ));
            }
            let offset = offset
                .parse()
                .map_err(|_| fault("invalid_catalog_cursor", "Use the returned cursor."))?;
            return crate::catalog_page(
                self.catalog_header.clone(),
                &self.catalog,
                &self.catalog_id,
                offset,
                self.catalog_complete,
            );
        }
        let _dpi = DpiScope::enter()?;
        let (windows, complete) = desktop::windows()?;
        let native_count = windows.len();
        let displays = desktop::displays()?;
        self.check_identity()?;
        let title = query.title_contains.as_ref().map(|s| s.to_lowercase());
        self.catalog = windows
            .into_iter()
            .filter(|w| {
                query.pid.is_none_or(|p| p == w.pid)
                    && title
                        .as_ref()
                        .is_none_or(|s| w.title.to_lowercase().contains(s))
                    && self.input_grant.as_ref().is_none_or(|g| g.allows(w))
            })
            .collect();
        self.catalog_at = Some(Instant::now());
        self.catalog_complete = complete;
        self.catalog_id = digest(
            format!(
                "{:?}:{}:{}",
                SystemTime::now(),
                self.identity.key(),
                self.catalog.len()
            )
            .as_bytes(),
        );
        self.catalog_header = json!({"identity":self.identity,"enumeration_complete":complete,"native_windows_enumerated":native_count,"catalog_ttl_ms":10000,"displays":displays,"selected":self.selected,"filter":query,"input_grant_scoped":self.input_grant.is_some(),"native_api":"EnumWindows, GetWindow(GW_OWNER), process creation FILETIME, DWM, PMv2","coverage_note":"complete is true only if this response includes every matching native window and enumeration completed; pagination/partial is not absence. Prefer pid/title filtering."});
        crate::catalog_page(
            self.catalog_header.clone(),
            &self.catalog,
            &self.catalog_id,
            0,
            complete,
        )
    }
    fn bind_window(&mut self, s: &WindowSelection) -> Result<Value> {
        self.check_identity()?;
        let _dpi = DpiScope::enter()?;
        if self
            .catalog_at
            .is_none_or(|t| t.elapsed() > Duration::from_secs(10))
        {
            return Err(fault("catalog_expired", "Discover again before binding."));
        }
        let selected = self
            .catalog
            .iter()
            .find(|w| {
                w.window == s.window
                    && w.pid == s.pid
                    && w.native_epoch.as_deref() == Some(&s.native_epoch)
            })
            .ok_or_else(|| {
                fault(
                    "window_not_discovered",
                    "Use an exact native catalog entry; unknown process epochs cannot bind.",
                )
            })?;
        let current = desktop::record(hwnd(&s.window)?, selected.z_index)?;
        if current.native_epoch != selected.native_epoch
            || current.owner_chain != selected.owner_chain
        {
            return Err(fault(
                "window_epoch_changed",
                "Window or owner relation changed since discovery.",
            ));
        }
        check_relation(self.selected.as_ref(), &current, &s.relation)?;
        if self
            .input_grant
            .as_ref()
            .is_some_and(|g| !g.allows(&current))
        {
            return Err(fault(
                "input_window_not_allocated",
                "Input-enabled connection may bind only its allocated root or native-owned dialogs.",
            ));
        }
        self.root_runtime = None;
        self.selected = Some(current.clone());
        self.diagnostics = Value::Null;
        Ok(
            json!({"bound":current,"relation":s.relation,"input":self.input_grant.is_some(),"next":"Observe this exact window; prior frames/references invalidated. Keyboard input requires the exact focused UIA element."}),
        )
    }
    fn observe(&mut self) -> Result<Scene> {
        self.scene(true, None)
    }
    fn observe_region(&mut self, region: [u32; 4]) -> Result<Scene> {
        self.scene(true, Some(region))
    }
    fn revalidate(&mut self, r: &Resolved) -> Result<Scene> {
        self.scene(r.scope.element.is_none(), None)
    }
    fn act(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        intent: &Intent,
        value: Option<&str>,
    ) -> Result<Option<bool>> {
        let grant = self.input_grant.clone().ok_or_else(|| fault("input_not_enabled", "Use a fresh allocated --windows-input-grant with --allow-input; default is read-only."))?;
        let plan = input_plan::plan(intent, value)?;
        let _dpi = DpiScope::enter()?;
        let now = self.revalidate(target)?;
        if !grounding_matches(expected, &now, target)
            || !now.mutation_ready
            || now.blocked != Some(false)
        {
            return Err(fault(
                "stale_reference",
                "Immediate native input binding/visibility recheck failed; no input.",
            ));
        }
        let window = self.current()?;
        if target.scope.window != window.window || target.scope.pid != window.pid {
            return Err(fault(
                "input_target_mismatch",
                "Reference belongs to another native window.",
            ));
        }
        input::dispatch(
            &grant,
            self.grant_deadline.expect("input grant deadline"),
            &self.identity,
            &window,
            target,
            &plan,
            &mut self.input_counters,
        )
    }
    fn inspect(&mut self, scope: &Scope, predicate: &Predicate, expected: &str) -> Result<Oracle> {
        self.check_identity()?;
        let _dpi = DpiScope::enter()?;
        let current = self.current()?;
        if current.window != scope.window || current.pid != scope.pid {
            return Err(fault(
                "verification_scope_mismatch",
                "Native oracle must match selected HWND/PID.",
            ));
        }
        let w = hwnd(&current.window)?;
        let matched = match predicate {
            Predicate::WindowTitle => {
                if current.title_truncated {
                    return Err(fault(
                        "native_title_truncated",
                        "Window title exceeds the metadata bound; do not verify a truncated prefix.",
                    ));
                }
                current.title == expected
            }
            Predicate::FocusedWindow => current.foreground && expected == current.window,
            Predicate::Modal => {
                let walk = uia::walk(w, current.pid, current.native_epoch.as_deref().unwrap())?;
                walk.modal
                    .map(|m| m.to_string() == expected)
                    .ok_or_else(|| {
                        fault(
                            "modal_state_unknown",
                            "UIA WindowPattern did not attest modality.",
                        )
                    })?
            }
            Predicate::ElementValue => uia::value_matches(
                w,
                current.pid,
                current.native_epoch.as_deref().unwrap(),
                scope.element.as_deref().ok_or_else(|| {
                    fault("element_required", "Use exact observed UIA runtime ID.")
                })?,
                expected,
            )?,
        };
        self.check_identity()?;
        Ok(Oracle {
            identity: self.identity.clone(),
            scope: scope.clone(),
            assurance: "native-attested".into(),
            matched,
            source: "independent Win32/UIA read; no input acknowledgement or screenshot echo"
                .into(),
        })
    }
}
