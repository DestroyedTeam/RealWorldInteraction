use super::*;

#[test]
fn api_refusal_does_not_override_independent_exact_foreground_observation() {
    for during_cleanup in [false, true] {
        let (r, d) = once(Script {
            accepted: false,
            on_activate: if during_cleanup {
                Step::default()
            } else {
                Step::foreground(Some(1))
            },
            on_end: if during_cleanup {
                Step::foreground(Some(1))
            } else {
                Step::default()
            },
            ..Default::default()
        });
        assert_eq!(r.api_accepted, Some(false));
        assert_eq!(r.state, "foreground_observed");
        assert_eq!(r.after_foreground, Some(window(1).key));
        assert_eq!(d.activations, 1);
        assert!(d.pauses.is_empty());
    }
}

#[test]
fn errors_remain_uncertain_even_when_final_cleanup_also_observes_interference() {
    let (r, _) = once(Script {
        on_activate: Step {
            incomplete: true,
            ..Default::default()
        },
        on_end: Step::foreground(Some(3)),
        ..Default::default()
    });
    assert_eq!(r.state, "uncertain");
    assert_eq!(r.error.unwrap().code, "incomplete_native_events");
    assert!(r.user_intervened);
}

#[test]
fn elapsed_observation_and_lease_deadlines_reject_after_a_native_effect_without_sleeping() {
    let (mut r, mut d) = once(Script::default());
    let past = Instant::now() - Duration::from_millis(1);
    let e = observe_completion(&mut d, &grant(), &mut r, 100, past).unwrap_err();
    assert_eq!(e.code, "activation_observation_timeout");
    let e = operation::with_dispatch_deadline(past, || {
        sample(&mut d, &grant(), &mut r, 100, "activation_after_cleanup")
    })
    .unwrap_err();
    assert_eq!(e.code, "lease_expired_during_dispatch");
    assert_eq!(d.activations, 1);
    assert!(d.pauses.is_empty());
}
