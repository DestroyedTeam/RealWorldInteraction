#[derive(Default)]
struct Step {
    presentation: Option<WindowPresentation>,
    foreground: Option<Option<WindowIdentity>>,
    tick: Option<u64>,
    events: Vec<NativeEvent>,
    incomplete: bool,
    invalid_window: bool,
    new_session: bool,
    cancel: bool,
}
impl Step {
    fn foreground(n: Option<u32>) -> Self {
        Self {
            foreground: Some(n.map(|n| window(n).key)),
            ..Self::default()
        }
    }
    fn events(events: Vec<NativeEvent>) -> Self {
        Self {
            events,
            ..Self::default()
        }
    }
}
fn event(name: &str, n: u32) -> NativeEvent {
    NativeEvent {
        event: name.into(),
        window: format!("hwnd:{n:x}"),
        event_time_ms: 1,
        thread_id: 3,
    }
}
struct Script {
    presentation: WindowPresentation,
    foreground: Option<WindowIdentity>,
    tick: u64,
    events: Vec<NativeEvent>,
    complete: bool,
    invalid_window: bool,
    new_session: bool,
    op: Option<operation::Operation>,
    before_dispatch: Step,
    on_activate: Step,
    on_restore: Step,
    before_focus: Step,
    restore_observations: u64,
    on_pause: VecDeque<Step>,
    on_end: Step,
    accepted: bool,
    hook_error: bool,
    effect_error: Option<bool>,
    restore_error: Option<bool>,
    restore_accepted: bool,
    released: bool,
    began: usize,
    ended: usize,
    reads: usize,
    activations: u64,
    restores: u64,
    pauses: Vec<u64>,
}
impl Default for Script {
    fn default() -> Self {
        Self {
            presentation: presentation(),
            foreground: Some(window(2).key),
            tick: 100,
            events: vec![],
            complete: true,
            invalid_window: false,
            new_session: false,
            op: None,
            before_dispatch: Step::default(),
            on_activate: Step::foreground(Some(1)),
            on_restore: Step {
                presentation: Some(presentation()),
                ..Step::foreground(Some(1))
            },
            before_focus: Step::default(),
            restore_observations: 0,
            on_pause: VecDeque::new(),
            on_end: Step::default(),
            accepted: true,
            hook_error: false,
            effect_error: None,
            restore_error: None,
            restore_accepted: true,
            released: true,
            began: 0,
            ended: 0,
            reads: 0,
            activations: 0,
            restores: 0,
            pauses: vec![],
        }
    }
}
impl Script {
    fn apply(&mut self, step: Step) {
        if let Some(presentation) = step.presentation {
            self.presentation = presentation;
        }
        if let Some(foreground) = step.foreground {
            self.foreground = foreground;
        }
        if let Some(tick) = step.tick {
            self.tick = tick;
        }
        self.events.extend(step.events);
        self.complete &= !step.incomplete;
        self.invalid_window |= step.invalid_window;
        self.new_session |= step.new_session;
        if step.cancel {
            self.op.as_ref().unwrap().cancel();
        }
    }
    fn batch(&self, hooks_released: bool) -> EventBatch {
        EventBatch {
            events: self.events.clone(),
            complete: self.complete,
            hooks_released,
        }
    }
}
fn identity() -> Identity {
    Identity {
        host: "offline".into(),
        guest: Some("fixture".into()),
        session: "session".into(),
        seat: "seat".into(),
        desktop: "desktop".into(),
    }
}
fn grant() -> LifecycleGrant {
    LifecycleGrant {
        run_id: "activation-offline".into(),
        route_vm_id: String::new(),
        expected_identity: identity(),
        expires_unix_ms: now_ms() + 60_000,
        launches: vec![],
        activations: vec![ActivationPermission {
            authorization_id: "activate".into(),
            window: window(1).key,
        }],
    }
}
impl LifecycleDriver for Script {
    fn identity(&mut self) -> Result<Identity> {
        let mut identity = identity();
        if self.new_session {
            identity.session = "replacement".into();
        }
        Ok(identity)
    }
    fn verify_application(&mut self, _: &ApplicationIdentity) -> Result<()> {
        unreachable!()
    }
    fn verify_file(&mut self, _: &AllowedFile) -> Result<()> {
        unreachable!()
    }
    fn inventory(&mut self, _: &[ApplicationIdentity]) -> Result<Inventory> {
        unreachable!()
    }
    fn begin_events(&mut self) -> Result<()> {
        panic!("activation must request its scoped events")
    }
    fn begin_activation_events(&mut self, target: &WindowIdentity) -> Result<()> {
        assert_eq!(target, &window(1).key);
        self.began += 1;
        if self.hook_error {
            return Err(fault("fixture_hook_failed", "No hooks."));
        }
        Ok(())
    }
    fn events(&mut self) -> Result<EventBatch> {
        assert_eq!(self.began, 1);
        self.reads += 1;
        if self.reads == 1 {
            let step = std::mem::take(&mut self.before_dispatch);
            self.apply(step);
        }
        if self.restores != 0 {
            self.restore_observations += 1;
            if self.restore_observations == 2 {
                let step = std::mem::take(&mut self.before_focus);
                self.apply(step);
            }
        }
        Ok(self.batch(false))
    }
    fn end_events(&mut self) -> Result<EventBatch> {
        self.ended += 1;
        let step = std::mem::take(&mut self.on_end);
        self.apply(step);
        Ok(self.batch(self.released))
    }
    fn launch(&mut self, _: &LaunchPermission) -> Result<NativeLaunch> {
        unreachable!()
    }
    fn foreground(&mut self) -> Result<Option<WindowIdentity>> {
        Ok(self.foreground.clone())
    }
    fn user_input_tick(&mut self) -> Result<u64> {
        Ok(self.tick)
    }
    fn check_window(&mut self, _: &WindowIdentity) -> Result<()> {
        if self.invalid_window {
            Err(fault(
                "application_window_identity_changed",
                "Fixture target replaced.",
            ))
        } else {
            Ok(())
        }
    }
    fn activate(&mut self, _: &WindowIdentity) -> Result<bool> {
        if self.effect_error == Some(false) {
            return Err(fault("fixture_predispatch", "Rejected before API."));
        }
        self.activations += 1;
        let step = std::mem::take(&mut self.on_activate);
        self.apply(step);
        if self.effect_error == Some(true) {
            return Err(fault("fixture_effect_error", "Uncertain after API."));
        }
        Ok(self.accepted)
    }
    fn window_presentation(&mut self, _: &WindowIdentity) -> Result<WindowPresentation> {
        Ok(self.presentation.clone())
    }
    fn restore(&mut self, _: &WindowIdentity) -> Result<bool> {
        if self.restore_error == Some(false) {
            return Err(fault(
                "fixture_restore_predispatch",
                "Rejected before restore API.",
            ));
        }
        self.restores += 1;
        let step = std::mem::take(&mut self.on_restore);
        self.apply(step);
        if self.restore_error == Some(true) {
            return Err(fault(
                "fixture_restore_effect_error",
                "Uncertain after restore API.",
            ));
        }
        Ok(self.restore_accepted)
    }
    fn native_effect_count(&self) -> Option<u64> {
        Some(self.activations + self.restores)
    }
    fn pause(&mut self, ms: u64) {
        assert!(ms <= OBSERVATION_SLICE_MS);
        self.pauses.push(ms);
        if let Some(step) = self.on_pause.pop_front() {
            self.apply(step);
        }
    }
}
fn run(
    dir: &Path,
    g: &LifecycleGrant,
    d: &mut Script,
    expected: &Option<WindowIdentity>,
) -> Result<ActivationReceipt> {
    activate(
        dir,
        g,
        d,
        ActivateRequest {
            authorization_id: "activate",
            request_id: "once",
            expected_foreground: expected,
        },
    )
}
fn check_receipt(r: &ActivationReceipt) {
    assert!(!r.goal_verified && !r.input_authority_granted);
    assert!(r.replay.starts_with("forbidden"));
}
fn once(mut d: Script) -> (ActivationReceipt, Script) {
    let dir = tempfile::tempdir().unwrap();
    let expected = d.foreground.clone();
    let g = grant();
    let r = if let Some(op) = &d.op.clone() {
        operation::run(op, || run(dir.path(), &g, &mut d, &expected)).unwrap()
    } else {
        run(dir.path(), &g, &mut d, &expected).unwrap()
    };
    check_receipt(&r);
    let count = d.activations;
    let restores = d.restores;
    let reads = d.reads;
    let replay = run(dir.path(), &g, &mut d, &expected).unwrap();
    assert_eq!(replay.state, r.state);
    assert_eq!((d.activations, d.reads, d.ended), (count, reads, 1));
    assert_eq!(d.restores, restores);
    (r, d)
}
