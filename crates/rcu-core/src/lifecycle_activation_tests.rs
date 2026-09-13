//! Offline scripted completion; pauses advance the script, not wall-clock sleeps.
use super::super::test_support::window;
use super::*;
use std::collections::VecDeque;

#[path = "lifecycle_activation_boundary_tests.rs"]
mod boundaries;

#[derive(Default)]
struct Step {
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
    foreground: Option<WindowIdentity>,
    tick: u64,
    events: Vec<NativeEvent>,
    complete: bool,
    invalid_window: bool,
    new_session: bool,
    op: Option<operation::Operation>,
    before_dispatch: Step,
    on_activate: Step,
    on_pause: VecDeque<Step>,
    on_end: Step,
    accepted: bool,
    hook_error: bool,
    effect_error: Option<bool>,
    released: bool,
    began: usize,
    ended: usize,
    reads: usize,
    activations: u64,
    pauses: Vec<u64>,
}
impl Default for Script {
    fn default() -> Self {
        Self {
            foreground: Some(window(2).key),
            tick: 100,
            events: vec![],
            complete: true,
            invalid_window: false,
            new_session: false,
            op: None,
            before_dispatch: Step::default(),
            on_activate: Step::foreground(Some(1)),
            on_pause: VecDeque::new(),
            on_end: Step::default(),
            accepted: true,
            hook_error: false,
            effect_error: None,
            released: true,
            began: 0,
            ended: 0,
            reads: 0,
            activations: 0,
            pauses: vec![],
        }
    }
}
impl Script {
    fn apply(&mut self, step: Step) {
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
    fn native_effect_count(&self) -> Option<u64> {
        Some(self.activations)
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
    let reads = d.reads;
    let replay = run(dir.path(), &g, &mut d, &expected).unwrap();
    assert_eq!(replay.state, r.state);
    assert_eq!((d.activations, d.reads, d.ended), (count, reads, 1));
    (r, d)
}
#[test]
fn delayed_original_and_null_foreground_complete_without_resending() {
    let (r, d) = once(Script {
        on_activate: Step::default(),
        on_pause: [Step::foreground(None), Step::foreground(Some(1))].into(),
        ..Default::default()
    });
    assert_eq!(r.state, "foreground_observed");
    assert_eq!((d.activations, d.pauses.len(), d.began), (1, 2, 1));
    assert!(r.resources_released && r.native_events.complete && r.native_events.hooks_released);
}
#[test]
fn accepted_but_never_foreground_exhausts_bounded_samples_without_real_sleep() {
    for null in [false, true] {
        let (r, d) = once(Script {
            on_activate: if null {
                Step::foreground(None)
            } else {
                Step::default()
            },
            ..Default::default()
        });
        assert_eq!(r.state, "uncertain");
        assert_eq!(r.error.unwrap().code, "activation_observation_timeout");
        assert_eq!(
            (d.activations, d.pauses.len()),
            (1, MAX_OBSERVATION_SLICES as usize)
        );
    }
}
#[test]
fn api_refusal_is_not_a_timeout_or_a_retry() {
    let (r, d) = once(Script {
        accepted: false,
        on_activate: Step::default(),
        ..Default::default()
    });
    assert_eq!(r.state, "not_foreground");
    assert_eq!(r.api_accepted, Some(false));
    assert!(r.error.is_none() && d.pauses.is_empty());
    assert_eq!(d.activations, 1);
}
#[test]
fn third_foreground_and_transient_round_trip_both_yield() {
    for on_activate in [
        Step::foreground(Some(3)),
        Step {
            foreground: Some(Some(window(1).key)),
            events: vec![event("foreground", 3), event("foreground", 1)],
            ..Default::default()
        },
        Step {
            foreground: Some(Some(window(1).key)),
            events: vec![
                event("foreground", 1),
                event("foreground", 2),
                event("foreground", 1),
            ],
            ..Default::default()
        },
    ] {
        let (r, d) = once(Script {
            on_activate,
            ..Default::default()
        });
        assert_eq!(r.state, "yielded");
        assert!(r.user_intervened && d.pauses.is_empty());
        assert_eq!(d.activations, 1);
    }
}
#[test]
fn input_token_inequality_including_rollback_and_wrap_yields() {
    for (initial, next) in [(100, 101), (100, 99), (u32::MAX as u64, 0)] {
        let (r, d) = once(Script {
            tick: initial,
            on_activate: Step {
                tick: Some(next),
                ..Step::foreground(Some(1))
            },
            ..Default::default()
        });
        assert_eq!(r.state, "yielded");
        assert!(r.user_intervened);
        assert_eq!(d.activations, 1);
    }
    let (r, d) = once(Script {
        before_dispatch: Step {
            tick: Some(99),
            ..Default::default()
        },
        ..Default::default()
    });
    assert_eq!(r.state, "yielded");
    assert_eq!(d.activations, 0);
}
#[test]
fn fresh_foreground_is_request_scoped_not_the_last_tasks_terminal_window() {
    let (r, d) = once(Script {
        foreground: Some(window(7).key),
        ..Default::default()
    });
    assert_eq!(r.state, "foreground_observed");
    assert_eq!(r.before_foreground, Some(window(7).key));
    assert_eq!(d.activations, 1);
    let dir = tempfile::tempdir().unwrap();
    let mut d = Script::default();
    assert_eq!(
        run(dir.path(), &grant(), &mut d, &Some(window(7).key))
            .unwrap_err()
            .code,
        "foreground_changed"
    );
    assert_eq!((d.activations, d.ended), (0, 1));
}
#[test]
fn lifetime_identity_session_and_event_overflow_never_claim_success() {
    for (on_activate, error) in [
        (
            Step::events(vec![event("destroy", 1), event("create", 1)]),
            "application_window_lifetime_changed",
        ),
        (
            Step {
                invalid_window: true,
                ..Default::default()
            },
            "application_window_identity_changed",
        ),
        (
            Step {
                new_session: true,
                ..Default::default()
            },
            "session_mismatch",
        ),
        (
            Step {
                incomplete: true,
                ..Default::default()
            },
            "incomplete_native_events",
        ),
    ] {
        let (r, d) = once(Script {
            on_activate,
            ..Default::default()
        });
        assert_eq!(r.state, "uncertain");
        assert_eq!(r.error.unwrap().code, error);
        assert_eq!(d.activations, 1);
    }
}
#[test]
fn final_drain_and_post_cleanup_intervention_cannot_preserve_success() {
    for on_end in [
        Step::events(vec![event("foreground", 3), event("foreground", 1)]),
        Step::foreground(Some(2)),
        Step::foreground(Some(3)),
        Step::foreground(None),
        Step {
            tick: Some(99),
            ..Default::default()
        },
    ] {
        let (r, d) = once(Script {
            on_end,
            ..Default::default()
        });
        assert_eq!(r.state, "yielded");
        assert!(r.resources_released && r.user_intervened);
        assert_eq!(d.activations, 1);
    }
    for (on_end, error) in [
        (
            Step::events(vec![event("destroy", 1), event("create", 1)]),
            "application_window_lifetime_changed",
        ),
        (
            Step {
                incomplete: true,
                ..Default::default()
            },
            "incomplete_native_events",
        ),
        (
            Step {
                invalid_window: true,
                ..Default::default()
            },
            "application_window_identity_changed",
        ),
        (
            Step {
                new_session: true,
                ..Default::default()
            },
            "session_mismatch",
        ),
    ] {
        let (r, _) = once(Script {
            on_end,
            ..Default::default()
        });
        assert_eq!(r.state, "uncertain");
        assert_eq!(r.error.unwrap().code, error);
    }
}
#[test]
fn cancellation_before_during_wait_and_after_cleanup_never_replays() {
    for phase in 0..3 {
        let op = operation::Registry::default().begin(None).unwrap();
        let mut d = Script {
            op: Some(op.clone()),
            ..Default::default()
        };
        match phase {
            0 => {
                d.on_activate = Step {
                    cancel: true,
                    ..Step::foreground(Some(1))
                }
            }
            1 => {
                d.on_activate = Step::default();
                d.on_pause.push_back(Step {
                    cancel: true,
                    ..Default::default()
                });
            }
            _ => d.on_end.cancel = true,
        }
        let (r, d) = once(d);
        assert_eq!(r.state, "uncertain");
        assert_eq!(r.error.unwrap().code, "operation_cancelled");
        assert_eq!(d.activations, 1);
        assert!(r.resources_released);
        op.finish();
        assert!(op.status().terminal);
    }
    let dir = tempfile::tempdir().unwrap();
    let mut d = Script::default();
    let expected = d.foreground.clone();
    let op = operation::Registry::default().begin(None).unwrap();
    op.cancel();
    let error = operation::run(&op, || run(dir.path(), &grant(), &mut d, &expected)).unwrap_err();
    assert_eq!(error.code, "operation_cancelled");
    assert_eq!(d.activations, 0);
}
#[test]
fn expired_lease_or_grant_prevents_dispatch() {
    for lease in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Script::default();
        let expected = d.foreground.clone();
        let mut g = grant();
        let result = if lease {
            operation::with_dispatch_deadline(Instant::now() - Duration::from_millis(1), || {
                run(dir.path(), &g, &mut d, &expected)
            })
        } else {
            g.expires_unix_ms = now_ms() - 1;
            run(dir.path(), &g, &mut d, &expected)
        };
        assert_eq!(
            result.unwrap_err().code,
            if lease {
                "lease_expired_during_dispatch"
            } else {
                "lifecycle_grant_expired"
            }
        );
        assert_eq!(d.activations, 0);
    }
}
#[test]
fn hook_failures_and_known_native_predispatch_errors_keep_zero_effects() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Script {
        hook_error: true,
        ..Default::default()
    };
    let expected = d.foreground.clone();
    assert_eq!(
        run(dir.path(), &grant(), &mut d, &expected)
            .unwrap_err()
            .code,
        "fixture_hook_failed"
    );
    assert_eq!((d.activations, d.ended), (0, 1));
    for dispatched in [false, true] {
        let (r, d) = once(Script {
            effect_error: Some(dispatched),
            ..Default::default()
        });
        assert_eq!(
            r.state,
            if dispatched {
                "uncertain"
            } else {
                "not_dispatched"
            }
        );
        assert_eq!(d.activations, u64::from(dispatched));
    }
    let (r, d) = once(Script {
        before_dispatch: Step {
            incomplete: true,
            ..Default::default()
        },
        ..Default::default()
    });
    assert_eq!(r.state, "not_dispatched");
    assert_eq!(d.activations, 0);
}
#[test]
fn already_foreground_is_zero_dispatch_and_cleanup_uncertainty_is_sticky() {
    let (r, d) = once(Script {
        foreground: Some(window(1).key),
        ..Default::default()
    });
    assert_eq!(r.state, "already_foreground");
    assert_eq!(d.activations, 0);
    let (r, d) = once(Script {
        released: false,
        ..Default::default()
    });
    assert_eq!(r.state, "uncertain");
    assert_eq!(r.error.unwrap().code, "native_resource_release_uncertain");
    assert!(!r.resources_released && r.resource_release_uncertain);
    assert_eq!(d.activations, 1);
}
