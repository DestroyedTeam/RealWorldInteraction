//! Public, offline restore-before-focus regressions. No OS effects or real sleeps.
use super::*;

fn minimized() -> WindowPresentation {
    WindowPresentation {
        minimized: true,
        usable_geometry: false,
        ..presentation()
    }
}

#[test]
fn minimized_target_restores_even_if_it_was_already_foreground() {
    for already_foreground in [false, true] {
        let (r, d) = once(Script {
            presentation: minimized(),
            foreground: Some(window(if already_foreground { 1 } else { 2 }).key),
            ..Default::default()
        });
        assert_eq!(r.state, "foreground_observed");
        assert_eq!((d.restores, d.activations), (1, 0));
        assert_eq!(r.api_accepted, None);
        assert_eq!(r.restore_api_accepted, Some(true));
        assert!(r.before_presentation.unwrap().minimized);
        assert!(r.after_presentation.unwrap().presented());
    }
}

#[test]
fn delayed_restore_then_focus_use_one_request_each_and_independent_observation() {
    let (r, d) = once(Script {
        presentation: minimized(),
        on_restore: Step::default(),
        on_pause: [
            Step::default(),
            Step {
                presentation: Some(presentation()),
                ..Default::default()
            },
        ]
        .into(),
        ..Default::default()
    });
    assert_eq!(r.state, "foreground_observed");
    assert_eq!((d.restores, d.activations, d.pauses.len()), (1, 1, 2));
    assert_eq!(
        (r.restore_api_accepted, r.api_accepted),
        (Some(true), Some(true))
    );
}

#[test]
fn restoring_and_focusing_share_the_original_slice_bound() {
    let mut pauses: VecDeque<_> = (0..MAX_OBSERVATION_SLICES - 1)
        .map(|_| Step::default())
        .collect();
    pauses.push_back(Step {
        presentation: Some(presentation()),
        ..Default::default()
    });
    let (r, d) = once(Script {
        presentation: minimized(),
        on_restore: Step::default(),
        on_activate: Step::default(),
        on_pause: pauses,
        ..Default::default()
    });
    assert_eq!(r.state, "uncertain");
    assert_eq!(r.error.unwrap().code, "activation_observation_timeout");
    assert_eq!((d.restores, d.activations), (1, 1));
    assert_eq!(d.pauses.len(), MAX_OBSERVATION_SLICES as usize);
}

#[test]
fn restore_timeout_and_late_cleanup_arrival_cannot_dispatch_focus_or_claim_success() {
    for late in [false, true] {
        let (r, d) = once(Script {
            presentation: minimized(),
            on_restore: Step::default(),
            on_end: if late {
                Step {
                    presentation: Some(presentation()),
                    ..Step::foreground(Some(1))
                }
            } else {
                Step::default()
            },
            ..Default::default()
        });
        assert_eq!(r.state, "uncertain");
        assert_eq!(r.error.unwrap().code, "activation_observation_timeout");
        assert_eq!(r.restore_api_accepted, Some(true));
        assert_eq!((d.restores, d.activations), (1, 0));
        assert_eq!(d.pauses.len(), MAX_OBSERVATION_SLICES as usize);
    }
}

#[test]
fn hidden_cloaked_unknown_and_unusable_nonminimized_targets_are_not_dispatched() {
    for p in [
        WindowPresentation {
            visible: false,
            ..minimized()
        },
        WindowPresentation {
            cloaked: Some(true),
            ..minimized()
        },
        WindowPresentation {
            cloaked: None,
            ..minimized()
        },
        WindowPresentation {
            usable_geometry: false,
            ..presentation()
        },
    ] {
        let (r, d) = once(Script {
            presentation: p,
            foreground: Some(window(1).key),
            ..Default::default()
        });
        assert_eq!(r.state, "not_dispatched");
        assert_eq!(r.error.unwrap().code, "activation_presentation_unavailable");
        assert_eq!((d.restores, d.activations), (0, 0));
    }
}

#[test]
fn restore_refusal_is_separate_from_independent_presentation() {
    for independently_restored in [false, true] {
        let (r, d) = once(Script {
            presentation: minimized(),
            restore_accepted: false,
            on_restore: if independently_restored {
                Step {
                    presentation: Some(presentation()),
                    ..Step::foreground(Some(1))
                }
            } else {
                Step::default()
            },
            ..Default::default()
        });
        assert_eq!(r.restore_api_accepted, Some(false));
        assert_eq!(
            r.state,
            if independently_restored {
                "foreground_observed"
            } else {
                "uncertain"
            }
        );
        assert_eq!(
            r.error.map(|e| e.code),
            (!independently_restored).then(|| "window_restore_not_accepted".into())
        );
        assert_eq!((d.restores, d.activations), (1, 0));
        assert!(d.pauses.is_empty());
    }
}

#[test]
fn restoring_effect_is_sticky_after_a_later_focus_predispatch_rejection() {
    let (r, d) = once(Script {
        presentation: minimized(),
        on_restore: Step {
            presentation: Some(presentation()),
            ..Default::default()
        },
        effect_error: Some(false),
        ..Default::default()
    });
    assert_eq!(r.state, "uncertain");
    assert_eq!(r.error.unwrap().code, "fixture_predispatch");
    assert_eq!(r.restore_api_accepted, Some(true));
    assert_eq!((d.restores, d.activations), (1, 0));
    for dispatched in [false, true] {
        let (r, d) = once(Script {
            presentation: minimized(),
            restore_error: Some(dispatched),
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
        assert_eq!((d.restores, d.activations), (u64::from(dispatched), 0));
    }
}

#[test]
fn restore_interference_and_lifetime_loss_never_continue_to_focus() {
    for (step, error) in [
        (
            Step {
                tick: Some(99),
                ..Default::default()
            },
            None,
        ),
        (Step::foreground(Some(3)), None),
        (
            Step::events(vec![event("foreground", 3), event("foreground", 2)]),
            None,
        ),
        (
            Step {
                invalid_window: true,
                ..Default::default()
            },
            Some("application_window_identity_changed"),
        ),
        (
            Step {
                new_session: true,
                ..Default::default()
            },
            Some("session_mismatch"),
        ),
        (
            Step::events(vec![event("destroy", 1), event("create", 1)]),
            Some("application_window_lifetime_changed"),
        ),
        (
            Step {
                incomplete: true,
                ..Default::default()
            },
            Some("incomplete_native_events"),
        ),
    ] {
        let (r, d) = once(Script {
            presentation: minimized(),
            on_restore: Step {
                presentation: Some(presentation()),
                ..Default::default()
            },
            before_focus: step,
            ..Default::default()
        });
        assert_eq!(
            r.state,
            if error.is_some() {
                "uncertain"
            } else {
                "yielded"
            }
        );
        assert_eq!(r.error.as_ref().map(|e| e.code.as_str()), error);
        assert_eq!((d.restores, d.activations), (1, 0));
    }
}

#[test]
fn cancellation_during_restore_wait_and_between_phases_never_dispatches_focus() {
    for phase in 0..3 {
        let op = operation::Registry::default().begin(None).unwrap();
        let mut d = Script {
            op: Some(op.clone()),
            presentation: minimized(),
            on_restore: Step {
                presentation: Some(presentation()),
                ..Default::default()
            },
            ..Default::default()
        };
        match phase {
            0 => d.on_restore.cancel = true,
            1 => {
                d.on_restore = Step::default();
                d.on_pause.push_back(Step {
                    cancel: true,
                    ..Default::default()
                });
            }
            _ => d.before_focus.cancel = true,
        }
        let (r, d) = once(d);
        assert_eq!(r.state, "uncertain");
        assert_eq!(r.error.unwrap().code, "operation_cancelled");
        assert_eq!(r.restore_api_accepted, Some(true));
        assert_eq!((d.restores, d.activations), (1, 0));
        assert!(r.resources_released);
        op.finish();
        assert!(op.status().terminal);
    }
}

#[test]
fn cleanup_presentation_changes_cannot_preserve_success() {
    for p in [
        minimized(),
        WindowPresentation {
            visible: false,
            ..presentation()
        },
        WindowPresentation {
            cloaked: Some(true),
            ..presentation()
        },
        WindowPresentation {
            cloaked: None,
            ..presentation()
        },
        WindowPresentation {
            usable_geometry: false,
            ..presentation()
        },
    ] {
        let (r, d) = once(Script {
            presentation: minimized(),
            on_end: Step {
                presentation: Some(p),
                ..Default::default()
            },
            ..Default::default()
        });
        assert_eq!(r.state, "uncertain");
        assert_eq!(r.error.unwrap().code, "activation_presentation_unavailable");
        assert_eq!((d.restores, d.activations), (1, 0));
    }
}

#[test]
fn restored_phase_observation_enforces_real_grant_and_lease_checks() {
    let (mut r, mut d) = once(Script {
        presentation: minimized(),
        ..Default::default()
    });
    for lease in [false, true] {
        let mut g = grant();
        let mut budget = ObservationBudget {
            deadline: Instant::now() + Duration::from_millis(500),
            pauses: 0,
        };
        let error = if lease {
            operation::with_dispatch_deadline(Instant::now() - Duration::from_millis(1), || {
                observe_restore(&mut d, &g, &mut r, 100, &mut budget)
            })
            .unwrap_err()
        } else {
            g.expires_unix_ms = now_ms() - 1;
            observe_restore(&mut d, &g, &mut r, 100, &mut budget).unwrap_err()
        };
        assert_eq!(
            error.code,
            if lease {
                "lease_expired_during_dispatch"
            } else {
                "lifecycle_grant_expired"
            }
        );
        assert_eq!((d.restores, d.activations), (1, 0));
    }
}
