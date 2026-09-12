//! Device-paced execution with release-only cleanup on every post-down exit.
//! This module is portable so cancellation/partial insertion can be tested offline.
use rcu_core::{Result, fault, operation::checkpoint, trajectory::DragPath};

pub(crate) const MAX_LATENESS_MS: u64 = 250;
// Measured synchronous UIA guards cannot sustain a 16ms interpolation grid in
// real applications. Coarsen optional interpolation, never caller waypoints or
// safety checks. This is a development cadence, not an FPS/latency guarantee.
pub(crate) const NATIVE_SAMPLE_INTERVAL_MS: u64 = 64;
pub(crate) trait TrajectorySink {
    fn sample_interval_ms(&self) -> u64 {
        rcu_core::trajectory::SAMPLE_INTERVAL_MS
    }
    fn now_ms(&self) -> u64;
    fn sleep_ms(&mut self, ms: u64);
    fn validate_point(&mut self, point: [f64; 2]) -> Result<()>;
    fn guard_point(&mut self, point: [f64; 2], owned_left: bool) -> Result<()>;
    fn press_left(&mut self, point: [f64; 2]) -> Result<()>;
    fn move_held(&mut self, point: [f64; 2]) -> Result<()>;
    /// No cancellation gate: releases only the attempted left button, never replays down.
    fn release_left(&mut self) -> Result<()>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct DragStats {
    pub interpolation_interval_ms: u64,
    pub samples_planned: usize,
    pub moves_inserted: usize,
    pub elapsed_ms: u64,
    pub release_inserted: bool,
}

struct Held<'a, S: TrajectorySink> {
    sink: &'a mut S,
    active: bool,
}
impl<S: TrajectorySink> Held<'_, S> {
    fn release(&mut self) -> Result<()> {
        self.active = false;
        let result = self.sink.release_left();
        if result.is_err() {
            rcu_core::operation::resource_release_uncertain();
        }
        result
    }
}
impl<S: TrajectorySink> Drop for Held<'_, S> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.release();
        }
    }
}

pub(crate) fn execute(
    path: &DragPath,
    origin: [f64; 2],
    sink: &mut impl TrajectorySink,
) -> Result<DragStats> {
    let interpolation_interval_ms = sink.sample_interval_ms();
    let samples = path.samples_with_interval(interpolation_interval_ms)?;
    let absolute = |offset: [f64; 2]| [origin[0] + offset[0], origin[1] + offset[1]];
    // Reject out-of-desktop waypoints/samples before any pointer movement/down.
    for sample in &samples {
        sink.validate_point(absolute(sample.offset))?;
    }
    checkpoint("drag_before_guard")?;
    sink.guard_point(origin, false)?;
    checkpoint("drag_before_down")?;
    let mut held = Held { sink, active: true };
    let start = held.sink.now_ms();
    let mut stats = DragStats {
        interpolation_interval_ms,
        samples_planned: samples.len(),
        moves_inserted: 0,
        elapsed_ms: 0,
        release_inserted: false,
    };
    let outcome = (|| {
        held.sink.press_left(origin)?;
        for sample in samples.iter().skip(1) {
            while held.sink.now_ms().saturating_sub(start) < sample.at_ms {
                checkpoint("drag_wait")?;
                let remaining = sample
                    .at_ms
                    .saturating_sub(held.sink.now_ms().saturating_sub(start));
                held.sink.sleep_ms(remaining.min(8));
            }
            checkpoint("drag_before_move_guard")?;
            held.sink.guard_point(absolute(sample.offset), true)?;
            checkpoint("drag_before_move")?;
            if held.sink.now_ms().saturating_sub(start) > sample.at_ms + MAX_LATENESS_MS {
                return Err(fault(
                    "drag_schedule_late",
                    "Native guards exceeded the bounded schedule; stop and release, never jump ahead or replay the path.",
                ));
            }
            held.sink.move_held(absolute(sample.offset))?;
            stats.moves_inserted += 1;
        }
        Ok(())
    })();
    let cleanup = held.release();
    stats.elapsed_ms = held.sink.now_ms().saturating_sub(start);
    stats.release_inserted = cleanup.is_ok();
    match (outcome, cleanup) {
        (Ok(()), Ok(())) => Ok(stats),
        (result, cleanup) => {
            let mut error=result.err().unwrap_or_else(|| fault("drag_release_unconfirmed", "Left-button release was not acknowledged; inspect native input state, never replay."));
            error.diagnostic = Some(Box::new(
                serde_json::json!({"trajectory":stats,"release_error":cleanup.err(),"original_diagnostic":error.diagnostic}),
            ));
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcu_core::{
        operation::{self, Operation, Registry},
        trajectory::Waypoint,
    };
    #[derive(Default)]
    struct Fake {
        interval_ms: Option<u64>,
        now: u64,
        log: Vec<&'static str>,
        guard_fail: bool,
        press_fail: bool,
        move_fail: bool,
        release_fail: bool,
        late: bool,
        cancel: Option<Operation>,
    }
    impl TrajectorySink for Fake {
        fn sample_interval_ms(&self) -> u64 {
            self.interval_ms
                .unwrap_or(rcu_core::trajectory::SAMPLE_INTERVAL_MS)
        }
        fn now_ms(&self) -> u64 {
            self.now
        }
        fn sleep_ms(&mut self, ms: u64) {
            self.now += ms;
        }
        fn validate_point(&mut self, p: [f64; 2]) -> Result<()> {
            if p.iter().any(|n| !n.is_finite() || n.abs() > 1000.) {
                Err(fault("outside", "test"))
            } else {
                Ok(())
            }
        }
        fn guard_point(&mut self, _: [f64; 2], held: bool) -> Result<()> {
            self.log
                .push(if held { "guard_held" } else { "guard_idle" });
            if self.late && held {
                self.now += 251;
            }
            if self.guard_fail {
                Err(fault("guard", "test"))
            } else {
                Ok(())
            }
        }
        fn press_left(&mut self, _: [f64; 2]) -> Result<()> {
            self.log.push("down");
            if let Some(op) = &self.cancel {
                op.cancel();
            }
            if self.press_fail {
                Err(fault("partial_down", "test"))
            } else {
                Ok(())
            }
        }
        fn move_held(&mut self, _: [f64; 2]) -> Result<()> {
            self.log.push("move");
            if self.move_fail {
                Err(fault("partial_move", "test"))
            } else {
                Ok(())
            }
        }
        fn release_left(&mut self) -> Result<()> {
            self.log.push("up");
            if self.release_fail {
                Err(fault("partial_up", "test"))
            } else {
                Ok(())
            }
        }
    }
    fn path() -> DragPath {
        DragPath {
            waypoints: vec![
                Waypoint {
                    at_ms: 0,
                    offset: [0., 0.],
                },
                Waypoint {
                    at_ms: 32,
                    offset: [32., 0.],
                },
            ],
        }
    }
    #[test]
    fn paced_drag_sends_one_down_and_one_up() {
        let mut f = Fake::default();
        let s = execute(&path(), [10., 20.], &mut f).unwrap();
        assert_eq!(
            f.log,
            [
                "guard_idle",
                "down",
                "guard_held",
                "move",
                "guard_held",
                "move",
                "up"
            ]
        );
        assert_eq!(s.moves_inserted, 2);
        assert_eq!(s.elapsed_ms, 32);
        assert!(s.release_inserted);
    }
    #[test]
    fn native_cadence_keeps_dense_required_vertices_and_release_contract() {
        let p = DragPath {
            waypoints: vec![
                Waypoint {
                    at_ms: 0,
                    offset: [0., 0.],
                },
                Waypoint {
                    at_ms: 16,
                    offset: [16., 0.],
                },
                Waypoint {
                    at_ms: 32,
                    offset: [16., 16.],
                },
                Waypoint {
                    at_ms: 160,
                    offset: [0., 0.],
                },
            ],
        };
        let mut f = Fake {
            interval_ms: Some(NATIVE_SAMPLE_INTERVAL_MS),
            ..Fake::default()
        };
        let s = execute(&p, [0., 0.], &mut f).unwrap();
        assert_eq!(s.interpolation_interval_ms, 64);
        assert_eq!(s.samples_planned, 5); // 0,16,32,96,160 -- no caller vertex skipped
        assert_eq!(s.moves_inserted, 4);
        assert_eq!(s.elapsed_ms, 160);
        assert!(s.release_inserted);
    }
    #[test]
    fn pre_dispatch_rejection_does_not_release_someone_elses_button() {
        let mut f = Fake {
            guard_fail: true,
            ..Fake::default()
        };
        assert!(execute(&path(), [0., 0.], &mut f).is_err());
        assert_eq!(f.log, ["guard_idle"]);
        let mut f = Fake::default();
        assert!(execute(&path(), [1000., 0.], &mut f).is_err());
        assert!(f.log.is_empty());
    }
    #[test]
    fn every_post_down_failure_releases_once_without_retry() {
        for mut f in [
            Fake {
                press_fail: true,
                ..Fake::default()
            },
            Fake {
                move_fail: true,
                ..Fake::default()
            },
            Fake {
                release_fail: true,
                ..Fake::default()
            },
            Fake {
                late: true,
                ..Fake::default()
            },
        ] {
            let err = execute(&path(), [0., 0.], &mut f).unwrap_err();
            assert!(err.diagnostic.is_some());
            assert_eq!(f.log.iter().filter(|s| **s == "down").count(), 1);
            assert_eq!(f.log.iter().filter(|s| **s == "up").count(), 1);
            assert_eq!(f.log.last(), Some(&"up"));
        }
    }
    #[test]
    fn cancellation_after_down_stops_moves_but_not_release() {
        let registry = Registry::default();
        let op = registry.begin(Some("drag-cancel".into())).unwrap();
        let mut f = Fake {
            cancel: Some(op.clone()),
            ..Fake::default()
        };
        operation::run(&op, || {
            assert!(execute(&path(), [0., 0.], &mut f).is_err());
        });
        assert_eq!(f.log, ["guard_idle", "down", "up"]);
    }
    #[test]
    fn failed_release_remains_uncertain_after_operation_finishes() {
        let op = Registry::default().begin(None).unwrap();
        operation::run(&op, || {
            let mut f = Fake {
                release_fail: true,
                ..Fake::default()
            };
            assert!(execute(&path(), [0., 0.], &mut f).is_err());
        });
        op.finish();
        assert!(op.status().terminal);
        assert!(op.status().resource_release_uncertain);
        assert!(!op.status().resources_released);
    }
    #[test]
    fn unwind_attempts_release_without_retrying_down() {
        let mut f = Fake::default();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _held = Held {
                sink: &mut f,
                active: true,
            };
            panic!("fixture");
        }));
        assert_eq!(f.log, ["up"]);
    }
}
