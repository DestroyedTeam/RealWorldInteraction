mod observation;
mod selection;
mod service_policy;
mod visual;
use crate::{
    InputGrant, WindowRecord, capture, check_relation,
    desktop::{self, DpiScope, hwnd},
    input, input_plan, uia,
};
use rwi_core::{
    Backend, Binding, Identity, Intent, MutationContext, Oracle, Predicate, Resolved, Result,
    Scene, Scope, WindowSelection, digest, fault, grounding_matches, operation::checkpoint,
};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub struct NativeWindows {
    desktop_policy: Option<service_policy::DesktopPolicy>,
    identity: Identity,
    selected: Option<WindowRecord>,
    catalog: Vec<WindowRecord>,
    catalog_at: Option<Instant>,
    catalog_id: String,
    catalog_header: Value,
    catalog_complete: bool,
    root_runtime: Option<Vec<i32>>,
    foreground_anchor: Option<crate::foreground::Anchor>,
    diagnostics: Value,
    input_grant: Option<InputGrant>,
    grant_deadline: Option<Instant>,
    input_counters: input::Counters,
    lifecycle_grant: Option<rwi_core::lifecycle::LifecycleGrant>,
    lifecycle_deadline: Option<Instant>,
    allocated_root: Option<WindowRecord>,
    agent_input: Option<Arc<crate::agent_binding::AgentInputAuthority>>,
    selected_catalog_id: String,
    revalidation_timings: crate::revalidation::Timings,
}
impl NativeWindows {
    /// Caller MUST allocate this execution desktop. No focus/input/capture here.
    pub fn new() -> Result<Self> {
        Self::new_with_grant(None)
    }
    pub fn new_with_grant(input_grant: Option<InputGrant>) -> Result<Self> {
        Self::new_with_grants(input_grant, None)
    }
    pub fn new_with_grants(
        input_grant: Option<InputGrant>,
        lifecycle_grant: Option<rwi_core::lifecycle::LifecycleGrant>,
    ) -> Result<Self> {
        let identity = desktop::identity()?;
        let grant_deadline = input_grant.as_ref().map(|g| {
            Instant::now()
                + Duration::from_millis(
                    g.expires_unix_ms
                        .saturating_sub(input::now_ms())
                        .min(3_600_000),
                )
        });
        let allocated_root = if let Some(grant) = &input_grant {
            grant.validate(&identity, input::now_ms())?;
            let root = desktop::record(hwnd(&grant.root.window)?, 0)?;
            if root.key().as_ref() != Some(&grant.root) {
                return Err(fault(
                    "input_grant_window_mismatch",
                    "Allocated root HWND/PID/process creation no longer exists; do not select a replacement by title.",
                ));
            }
            Some(root)
        } else {
            None
        };
        if let Some(g) = &lifecycle_grant {
            g.validate(&identity, input::now_ms())?;
        }
        let lifecycle_deadline = lifecycle_grant.as_ref().map(|g| {
            Instant::now()
                + Duration::from_millis(
                    g.expires_unix_ms
                        .saturating_sub(input::now_ms())
                        .min(3_600_000),
                )
        });
        Ok(Self {
            desktop_policy: None,
            identity,
            selected: None,
            catalog: vec![],
            catalog_at: None,
            catalog_id: String::new(),
            catalog_header: Value::Null,
            catalog_complete: false,
            root_runtime: None,
            foreground_anchor: None,
            diagnostics: Value::Null,
            input_grant,
            grant_deadline,
            input_counters: input::Counters::default(),
            lifecycle_grant,
            lifecycle_deadline,
            allocated_root,
            agent_input: None,
            selected_catalog_id: String::new(),
            revalidation_timings: crate::revalidation::Timings::default(),
        })
    }
    /// Private native-agent route configuration; it cannot replace InputGrant.root.
    /// Initial UIA root pinning waits for an explicit guarded initial root bind.
    pub fn configure_agent_input_scope(
        &mut self,
        session: crate::agent_contract::SessionBinding,
        root_native_epoch: Option<String>,
        dispatch_check: Arc<dyn Fn() -> Result<()> + Send + Sync>,
    ) -> Result<()> {
        if self.agent_input.is_some() || session.native != self.identity {
            return Err(fault(
                "agent_input_configuration_mismatch",
                "Agent native input authority is immutable and must match this backend's native session.",
            ));
        }
        let root = self.allocated_root.clone().ok_or_else(|| {
            fault(
                "agent_input_root_missing",
                "No original native input grant/root was constructed.",
            )
        })?;
        if root_native_epoch
            .as_ref()
            .is_none_or(|epoch| root.native_epoch.as_ref() != Some(epoch))
        {
            return Err(fault(
                "agent_input_root_epoch_changed",
                "Allocated root native epoch differs from the committed activation proof.",
            ));
        }
        self.agent_input = Some(Arc::new(crate::agent_binding::AgentInputAuthority::new(
            session,
            root,
            dispatch_check,
        )?));
        Ok(())
    }
    /// Read-only native/UIA facts for the exact selected window. A title/document
    /// precondition is not a filesystem path proof and is not returned as one.
    pub fn fresh_agent_selected_proof(
        &mut self,
        expected: &Binding,
        target: &Resolved,
    ) -> Result<crate::agent_contract::SelectedInputProof> {
        let authority = self.agent_input.clone().ok_or_else(|| {
            fault(
                "agent_input_scope_unconfigured",
                "Native agent input requires its configured immutable scope.",
            )
        })?;
        authority.verify_root_runtime()?;
        let scene = self.revalidate(target)?;
        if !grounding_matches(expected, &scene, target)
            || !scene.mutation_ready
            || scene.blocked != Some(false)
        {
            return Err(fault(
                "agent_selected_input_stale",
                "Fresh native/UIA scene no longer matches this exact grounded input target.",
            ));
        }
        let selected = self.current()?;
        let selection = authority.retain_selection(&selected)?;
        let native = selection.proof(
            self.root_runtime.clone().ok_or_else(|| {
                fault(
                    "agent_selected_uia_unknown",
                    "No fresh selected UIA root identity.",
                )
            })?,
            self.selected_catalog_id.clone(),
        )?;
        let proof = crate::agent_contract::SelectedInputProof {
            native,
            binding: scene.binding.ok_or_else(|| {
                fault(
                    "agent_selected_binding_unknown",
                    "No fresh native scene binding.",
                )
            })?,
            target: target.scope.clone(),
            observed_unix_ms: input::now_ms(),
        };
        proof.require_target(expected, target)?;
        Ok(proof)
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
    fn lifecycle_active(&self) -> bool {
        self.lifecycle_grant
            .as_ref()
            .is_some_and(|g| g.validate(&self.identity, input::now_ms()).is_ok())
            && self.lifecycle_deadline.is_some_and(|d| Instant::now() < d)
    }
    fn check_discovery_identity(&self) -> Result<()> {
        if desktop::identity()? != self.identity {
            return Err(fault(
                "session_mismatch",
                "Native discovery crossed sessions.",
            ));
        }
        // A service principal's desktop observation authority is independent
        // of the last selected window's narrower/expiring input grant.
        if self.desktop_policy.is_some() || self.lifecycle_active() {
            Ok(())
        } else {
            self.check_identity()
        }
    }
    fn discovery_allows(&self, w: &WindowRecord) -> bool {
        if self.desktop_policy.is_some() {
            return true;
        }
        let matches = |key: &rwi_core::lifecycle::WindowIdentity| {
            w.window == key.window
                && w.pid == key.pid
                && w.process_created == Some(key.process_created)
                && w.native_epoch.as_ref() == Some(&key.native_epoch)
        };
        self.input_grant.as_ref().is_none_or(|g| {
            (g.validate(&self.identity, input::now_ms()).is_ok()
                && self.grant_deadline.is_some_and(|d| Instant::now() < d)
                && g.allows(w))
                || (self.lifecycle_active()
                    && self.lifecycle_grant.as_ref().is_some_and(|l| {
                        l.activations.iter().any(|p| matches(&p.window))
                            || l.launches
                                .iter()
                                .any(|p| p.allowed_existing_windows.iter().any(matches))
                    }))
        })
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
}
impl Backend for NativeWindows {
    fn capture_visual(
        &mut self,
        options: rwi_core::VisualOptions,
    ) -> Result<rwi_core::VisualCapture> {
        self.visual_capture(options)
    }
    fn runtime_status(&self) -> Value {
        json!({"source":"device_native_runtime","sampled_unix_ms":input::now_ms(),"pointer_feedback":crate::pointer_feedback::status()})
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        Some(self.input_counters.sendinput_calls)
    }
    fn identity(&self) -> Identity {
        self.identity.clone()
    }
    fn capabilities(&self) -> Value {
        let lifecycle_ready = self
            .lifecycle_grant
            .as_ref()
            .is_some_and(|g| g.validate(&self.identity, input::now_ms()).is_ok())
            && self.lifecycle_deadline.is_some_and(|d| Instant::now() < d);
        json!({"checked_context_policy":if self.desktop_policy.is_some(){"native_element"}else{"whole_document"},"activation_authorization":if self.desktop_policy.is_some(){Some("selected-window")}else{None},"lifecycle_preflight":lifecycle_ready,"lifecycle_preflight_contract":"read-only bounded inventory, coverage failure counts/examples and explicit cleanup state; no reservation, launch, activation or input", "application_launch":lifecycle_ready && self.lifecycle_grant.as_ref().is_some_and(|g|!g.launches.is_empty()),"window_activation":lifecycle_ready && self.lifecycle_grant.as_ref().is_some_and(|g|!g.activations.is_empty()),"lifecycle_grant":self.lifecycle_grant,"lifecycle_contract":"exact argv/image/file SHA, native identity, monotonic deadline and lease; launch authorization once per run; complete bounded inventories and native events; no implicit input grants; explicit once-only activation; bind remains read-only","lifecycle_epoch_assurance":"HWND/PID/process creation/class plus scoped WinEvent history; no persistent HWND generation counter; destroy/recreate ambiguity remains fail-closed","backend":"windows-native-rust","execution_arch":std::env::consts::ARCH,"observe":true,"capture_visual":visual::capability(),"discover_windows":true,"bind_owned_popup":true,"semantic_source":"bounded UIA control-view plus authoritative GetFocusedElementBuildCache/root ancestry; cached focus flags diagnostic only","input":self.input_grant.as_ref().is_some_and(|g|g.validate(&self.identity,input::now_ms()).is_ok()) && self.grant_deadline.is_some_and(|d|Instant::now()<d),"input_dispatch":self.input_counters,"revalidation_timings":self.revalidation_timings,"input_grant":self.input_grant,"input_contract":"authorized desktop selection or allocated root/owned dialogs + exact native identity + expiry + lease + foreground (selected window or proven native owner)/runtime-ID checks; SendInput count is not goal verification; no auto-focus, clipboard or UIA mutation","pointer_feedback":crate::pointer_feedback::status(),"pointer_drag":{"implemented":true,"button":"left","coordinate_space":"offset_from_grounded_reference_in_device_physical_pixels","max_waypoints":rwi_core::trajectory::MAX_WAYPOINTS,"max_duration_ms":rwi_core::trajectory::MAX_DURATION_MS,"sample_interval_ms":rwi_core::input_trajectory::NATIVE_SAMPLE_INTERVAL_MS,"sampling_policy":"bounded optional interpolation; all requested vertices and times retained","max_lateness_ms":rwi_core::input_trajectory::MAX_LATENESS_MS,"scope":"same native element ancestry and fixed-geometry foreground window; not cross-window drag","physical_motion_takeover_detection":false},"text_limit_utf16":1024,"text_control_characters":false,"scroll_units":"signed wheel units, multiples of 120, +/-1200","verify":["window_title","focused_window","element_value (ValuePattern or bounded TextPattern)","modal"],"capture":"visible single-monitor-contained native window, GDI SDR RGB PNG","spanning_capture":false,"capture_hdr":false,"monitor_effective_dpi":"unavailable; awareness-qualified window DPI only","native_strong_cancellation":false,"native_provider_allocator_hard_limit":false,"cancellation":"cooperative per balanced gesture; no subsequent batch after observed cancel, lease expiry, grant expiry or focus loss; release-only cleanup may follow partial insertion; in-flight native call retains slot/resources","uia_connection_timeout_ms":250,"uia_transaction_timeout_ms":250,"tree_budget":{"nodes":128,"depth":8,"cooperative_ms":1500},"artifact_limits":{"frame_bytes":rwi_core::MAX_FRAME_BYTES,"total_bytes":rwi_core::ARTIFACT_BUDGET,"inline_bytes":rwi_core::INLINE_BYTES},"native_details":self.diagnostics})
    }
    fn lifecycle_grant(&self) -> Result<rwi_core::lifecycle::LifecycleGrant> {
        if desktop::identity()? != self.identity {
            return Err(fault(
                "session_mismatch",
                "Lifecycle receipt/operation belongs to another native session.",
            ));
        }
        self.lifecycle_grant.clone().ok_or_else(||fault("capability_unavailable", "Provide --windows-lifecycle-grant for exact launch/activation scope; input grant is separate."))
    }
    fn lifecycle_deadline(&self) -> Result<Instant> {
        let g = self.lifecycle_grant()?;
        g.validate(&self.identity, input::now_ms())?;
        self.lifecycle_deadline
            .filter(|d| Instant::now() < *d)
            .ok_or_else(|| {
                fault(
                    "lifecycle_grant_expired",
                    "Monotonic owner deadline passed; obtain new authorization.",
                )
            })
    }
    fn lifecycle_driver(&mut self) -> Result<Box<dyn rwi_core::lifecycle::LifecycleDriver>> {
        self.lifecycle_deadline()?;
        Ok(Box::new(crate::WindowsLifecycleDriver::new()?))
    }
    fn validate_activation(&self, window: &rwi_core::lifecycle::WindowIdentity) -> Result<()> {
        self.lifecycle_deadline()?;
        if self
            .catalog_at
            .is_none_or(|t| t.elapsed() > Duration::from_secs(10))
            || !self.catalog.iter().any(|w| {
                w.window == window.window
                    && w.pid == window.pid
                    && w.process_created == Some(window.process_created)
                    && w.native_epoch.as_ref() == Some(&window.native_epoch)
            })
        {
            return Err(fault(
                "stale_activation_window",
                "Discover the exact native HWND/PID/birth/epoch within 10 seconds; read-only binding does not activate it.",
            ));
        }
        Ok(())
    }
    fn discover(&mut self) -> Result<Value> {
        self.discover_with(&rwi_core::DiscoveryQuery::default())
    }
    fn discover_with(&mut self, query: &rwi_core::DiscoveryQuery) -> Result<Value> {
        query.validate()?;
        self.check_discovery_identity()?;
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
        self.check_discovery_identity()?;
        let foreground = if self.desktop_policy.is_some() {
            match desktop::foreground_identity() {
                Ok(identity) => json!({"status":"known","identity":identity}),
                Err(error) => json!({"status":"unavailable","identity":null,"error":error}),
            }
        } else {
            json!({"status":"outside_legacy_discovery_scope"})
        };
        let title = query.title_contains.as_ref().map(|s| s.to_lowercase());
        self.catalog = windows
            .into_iter()
            .filter(|w| {
                query.pid.is_none_or(|p| p == w.pid)
                    && title
                        .as_ref()
                        .is_none_or(|s| w.title.to_lowercase().contains(s))
                    && self.discovery_allows(w)
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
        self.catalog_header = json!({"identity":self.identity,"foreground":foreground,"captured_unix_ms":input::now_ms(),"enumeration_complete":complete,"native_windows_enumerated":native_count,"catalog_ttl_ms":10000,"displays":displays,"selected":self.selected,"filter":query,"input_grant_scoped":self.desktop_policy.is_none() && self.input_grant.is_some(),"native_api":"EnumWindows, GetWindow(GW_OWNER), process creation FILETIME, DWM, PMv2","coverage_note":"complete is true only if this response includes every matching native window and enumeration completed; pagination/partial is not absence. Prefer pid/title filtering."});
        crate::catalog_page(
            self.catalog_header.clone(),
            &self.catalog,
            &self.catalog_id,
            0,
            complete,
        )
    }
    fn bind_window(&mut self, selection: &WindowSelection) -> Result<Value> {
        self.bind_selected(selection)
    }
    fn observe(&mut self) -> Result<Scene> {
        self.scene(true, None, crate::revalidation::Scope::FullTree, None)
    }
    fn observe_region(&mut self, region: [u32; 4]) -> Result<Scene> {
        self.scene(
            true,
            Some(region),
            crate::revalidation::Scope::FullTree,
            None,
        )
    }
    fn revalidate(&mut self, r: &Resolved) -> Result<Scene> {
        let scope = crate::revalidation::Scope::for_reference(
            self.desktop_policy.is_some(),
            r.scope.element.is_some(),
        );
        let started = Instant::now();
        let result = self.scene(r.scope.element.is_none(), r.source_region, scope, Some(r));
        self.revalidation_timings
            .record(scope, started.elapsed(), result.is_ok());
        result
    }
    fn act(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        intent: &Intent,
        value: Option<&str>,
    ) -> Result<Option<bool>> {
        if self.agent_input.is_some() {
            self.fresh_agent_selected_proof(expected, target)?;
        }
        let grant = self.input_grant.clone().ok_or_else(|| fault("input_not_enabled", "Use a fresh allocated --windows-input-grant with --allow-input; default is read-only."))?;
        let plan = input_plan::plan_with_policy(intent, value, self.desktop_policy.is_some())?;
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
        if self.desktop_policy.is_some()
            && matches!(intent, Intent::Text)
            && !self.diagnostics["uia"]["walk"]["details"]
                .as_array()
                .is_some_and(|rows| {
                    rows.iter().any(|r| {
                        r["element_id"].as_str() == target.scope.element.as_deref()
                            && r["value_revision"].is_string()
                    })
                })
        {
            return Err(fault(
                "text_value_precondition_unavailable",
                "The exact editable element needs a bounded native value revision; unrelated document trees are not required.",
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
            self.foreground_anchor.as_ref(),
            target,
            expected.mutation_context.as_ref(),
            &plan,
            &mut self.input_counters,
            self.agent_input.as_deref(),
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
