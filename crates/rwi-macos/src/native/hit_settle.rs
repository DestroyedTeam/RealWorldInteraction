//! Bounded read-only hit convergence. This neither dispatches nor changes focus.
use rwi_core::{Result, fault};
use std::time::Duration;

pub(super) const BUDGET: Duration = Duration::from_millis(120);
const PROBE_BUDGET: Duration = Duration::from_millis(20);
const INTERVAL: Duration = Duration::from_millis(20);
const MAX_SAMPLES: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Verdict {
    Exact,
    SameWindowMismatch,
    ForeignWindow,
    Unknown,
}
pub(super) struct Sample<T> {
    pub verdict: Verdict,
    pub detail: T,
}
pub(super) struct Outcome<T> {
    pub accepted: bool,
    pub samples: Vec<Sample<T>>,
    pub elapsed: Duration,
}

/// Every probe receives the identical requested point and a remaining native
/// read budget. Only two consecutive strict matches can authorize visibility.
/// A foreign-window cache result is negative evidence, never a replacement.
pub(super) fn settle<T>(
    point: [f64; 2],
    mut probe: impl FnMut([f64; 2], Duration) -> Result<Sample<T>>,
    mut elapsed: impl FnMut() -> Duration,
    mut wait: impl FnMut(Duration),
    mut checkpoint: impl FnMut() -> Result<()>,
) -> Result<Outcome<T>> {
    let mut samples = Vec::with_capacity(MAX_SAMPLES);
    let mut consecutive = 0;
    for index in 0..MAX_SAMPLES {
        checkpoint()?;
        let remaining = BUDGET
            .checked_sub(elapsed())
            .filter(|value| !value.is_zero())
            .ok_or_else(|| {
                fault(
                    "ax_hit_settle_deadline",
                    "Read-only hit convergence exhausted its 120 ms budget; no input.",
                )
            })?;
        let sample = probe(point, remaining.min(PROBE_BUDGET))?;
        checkpoint()?;
        let duration = elapsed();
        if duration >= BUDGET {
            return Err(fault(
                "ax_hit_settle_deadline",
                "Native hit read exceeded its bounded convergence budget; no input.",
            ));
        }
        consecutive = if sample.verdict == Verdict::Exact {
            consecutive + 1
        } else {
            0
        };
        samples.push(sample);
        if consecutive == 2 {
            return Ok(Outcome {
                accepted: true,
                samples,
                elapsed: duration,
            });
        }
        if index + 1 < MAX_SAMPLES {
            if BUDGET.saturating_sub(duration) <= INTERVAL {
                return Err(fault(
                    "ax_hit_settle_deadline",
                    "No remaining budget for another hit sample; no input.",
                ));
            }
            wait(INTERVAL);
        }
    }
    Ok(Outcome {
        accepted: false,
        samples,
        elapsed: elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    fn sequence(values: &[Verdict]) -> Outcome<usize> {
        let clock = Cell::new(Duration::ZERO);
        let mut index = 0;
        settle(
            [4., 8.],
            |point, budget| {
                assert_eq!(point, [4., 8.]);
                assert!(budget <= PROBE_BUDGET);
                let sample = Sample {
                    verdict: values[index],
                    detail: index,
                };
                index += 1;
                Ok(sample)
            },
            || clock.get(),
            |wait| clock.set(clock.get() + wait),
            || Ok(()),
        )
        .unwrap()
    }
    #[test]
    fn wrong_cache_then_two_exact_samples_accepts_only_original_point() {
        for first in [
            Verdict::SameWindowMismatch,
            Verdict::ForeignWindow,
            Verdict::Unknown,
        ] {
            let outcome = sequence(&[first, Verdict::Exact, Verdict::Exact]);
            assert!(outcome.accepted);
            assert_eq!(outcome.samples.len(), 3);
            assert_eq!(outcome.elapsed, Duration::from_millis(40));
        }
    }
    #[test]
    fn initial_exact_requires_a_second_exact_sample() {
        let outcome = sequence(&[Verdict::Exact, Verdict::Exact]);
        assert!(outcome.accepted);
        assert_eq!(outcome.samples.len(), 2);
    }
    #[test]
    fn permanent_wrong_window_unknown_and_flicker_never_authorize() {
        for values in [
            [Verdict::ForeignWindow; 3],
            [Verdict::Unknown; 3],
            [Verdict::SameWindowMismatch; 3],
            [Verdict::Exact, Verdict::SameWindowMismatch, Verdict::Exact],
            [Verdict::ForeignWindow, Verdict::Exact, Verdict::Unknown],
        ] {
            let outcome = sequence(&values);
            assert!(!outcome.accepted);
            assert_eq!(outcome.samples.len(), 3);
        }
    }
    #[test]
    fn native_timeout_cancellation_and_deadline_stop_without_success() {
        for code in ["hit_test_unknown", "operation_cancelled"] {
            let error = settle::<()>(
                [1., 1.],
                |_, _| Err(fault(code, "probe stopped")),
                || Duration::ZERO,
                |_| panic!("no wait after failure"),
                || Ok(()),
            )
            .err()
            .unwrap();
            assert_eq!(error.code, code);
        }
        let error = settle::<()>(
            [1., 1.],
            |_, _| panic!("expired read must not start"),
            || BUDGET,
            |_| panic!("expired read must not wait"),
            || Ok(()),
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "ax_hit_settle_deadline");
        let error = settle::<()>(
            [1., 1.],
            |_, _| panic!("cancelled read must not start"),
            || Duration::ZERO,
            |_| panic!("cancelled read must not wait"),
            || Err(fault("operation_cancelled", "cancelled")),
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "operation_cancelled");
    }
    #[test]
    fn slow_native_return_is_rejected_even_if_exact() {
        let clock = Cell::new(Duration::ZERO);
        let error = settle(
            [1., 1.],
            |_, _| {
                clock.set(BUDGET);
                Ok(Sample {
                    verdict: Verdict::Exact,
                    detail: (),
                })
            },
            || clock.get(),
            |_| panic!("no wait after deadline"),
            || Ok(()),
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "ax_hit_settle_deadline");
    }
    #[test]
    fn cancellation_between_samples_never_probes_again() {
        let clock = Cell::new(Duration::ZERO);
        let probes = Cell::new(0);
        let error = settle(
            [1., 1.],
            |_, _| {
                probes.set(probes.get() + 1);
                Ok(Sample {
                    verdict: Verdict::Exact,
                    detail: (),
                })
            },
            || clock.get(),
            |duration| clock.set(duration),
            || {
                if clock.get() > Duration::ZERO {
                    Err(fault("operation_cancelled", "cancelled during wait"))
                } else {
                    Ok(())
                }
            },
        )
        .err()
        .unwrap();
        assert_eq!(error.code, "operation_cancelled");
        assert_eq!(probes.get(), 1);
    }
}
