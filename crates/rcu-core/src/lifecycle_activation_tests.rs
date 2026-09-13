//! Offline scripted completion; pauses advance the script, not wall-clock sleeps.
use super::super::test_support::{presentation, window};
use super::*;
use std::collections::VecDeque;

#[path = "lifecycle_activation_boundary_tests.rs"]
mod boundaries;
#[path = "lifecycle_activation_restore_tests.rs"]
mod restoration;

include!("lifecycle_activation_test_support.rs");
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
