//! A single retained-object read, never a retry of the foreground request.
use rwi_core::{Fault, Result, fault};
use serde_json::{Value, json};
use std::time::Duration;

const PAUSE: Duration = Duration::from_millis(20);
const BUDGET: Duration = Duration::from_millis(100);

#[derive(Clone, Copy)]
pub(crate) struct ReadSite {
    pub phase: &'static str,
    pub site: &'static str,
    pub attribute: &'static str,
}

#[derive(Default)]
pub(crate) struct Settlement {
    record: Option<Value>,
}

fn eligible(error: &Fault, attribute: &str) -> bool {
    let Some(d) = error.diagnostic.as_ref() else {
        return false;
    };
    error.code == "ax_attribute_unavailable"
        && matches!(attribute, "AXFocusedWindow" | "AXPosition" | "AXSize")
        && d["api"] == "AXUIElementCopyAttributeValue"
        && d["operation_kind"] == "read"
        && d["attribute"] == attribute
        && d["ax_error"] == -25204
}

fn budget_error() -> Fault {
    fault(
        "foreground_readback_budget",
        "Read-only settlement exceeded its soft budget; observe fresh state, never replay foreground input.",
    )
}

impl Settlement {
    /// `read` must address the SAME retained native object and attribute twice.
    /// One policy is shared by the entire activation, not reset per attribute.
    pub(crate) fn read<T>(
        &mut self,
        site: ReadSite,
        mut read: impl FnMut() -> Result<T>,
        check: impl Fn() -> Result<()>,
        wait: impl FnOnce(Duration) -> Result<()>,
        mut clock: impl FnMut() -> Duration,
    ) -> Result<T> {
        check()?;
        let original = match read() {
            Ok(value) => return Ok(value),
            Err(error) if self.record.is_none() && eligible(&error, site.attribute) => error,
            Err(error) => return Err(error),
        };
        let start = clock();
        // Reserve before any pause: another read failure cannot extend this call.
        self.record = Some(json!({
            "scope":"retained_attribute_read_only", "phase":site.phase,
            "site":site.site, "attribute":site.attribute, "original_fault":original,
            "max_extra_attribute_reads":1, "extra_attribute_reads":0,
            "configured_pause_ms":20, "pause_requested":false, "soft_budget_ms":100,
            "input_replayed":false
        }));
        let mut extra_reads = 0;
        let mut pause_requested = false;
        let result = (|| {
            check()?;
            if clock()
                .saturating_sub(start)
                .checked_add(PAUSE)
                .is_none_or(|spent| spent >= BUDGET)
            {
                return Err(budget_error());
            }
            pause_requested = true;
            wait(PAUSE)?;
            check()?;
            if clock().saturating_sub(start) >= BUDGET {
                return Err(budget_error());
            }
            extra_reads = 1;
            let value = read();
            check()?;
            if clock().saturating_sub(start) >= BUDGET {
                return Err(budget_error());
            }
            value
        })();
        let record = self.record.as_mut().expect("reserved above");
        record["extra_attribute_reads"] = extra_reads.into();
        record["pause_requested"] = pause_requested.into();
        record["elapsed_since_initial_fault_ms"] =
            (clock().saturating_sub(start).as_secs_f64() * 1000.0).into();
        record["attribute_outcome"] = if result.is_ok() {
            "observed_after_extra_read"
        } else {
            "unconfirmed"
        }
        .into();
        if let Err(error) = &result {
            record["outcome_error"] = json!(error);
        }
        result
    }

    pub(crate) fn annotate(&self, result: Result<Value>, requests: [u8; 2]) -> Result<Value> {
        let counts = json!({"AXRaise":requests[0], "AXFrontmost":requests[1]});
        let annotate = |value: &mut Value| {
            value["native_request_counts"] = counts.clone();
            if let Some(record) = &self.record {
                value["readback_settlement"] = record.clone();
            }
        };
        match result {
            Ok(mut value) => {
                annotate(&mut value);
                Ok(value)
            }
            Err(mut error) => {
                let mut diagnostic = error
                    .diagnostic
                    .take()
                    .map(|value| *value)
                    .unwrap_or_else(|| json!({}));
                if !diagnostic.is_object() {
                    diagnostic = json!({"original_diagnostic":diagnostic});
                }
                annotate(&mut diagnostic);
                error.diagnostic = Some(Box::new(diagnostic));
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    fn site() -> ReadSite {
        ReadSite {
            phase: "after_raise",
            site: "initial_application",
            attribute: "AXFocusedWindow",
        }
    }
    fn failure() -> Fault {
        let mut e = fault("ax_attribute_unavailable", "original literal error");
        e.diagnostic = Some(Box::new(
            json!({"api":"AXUIElementCopyAttributeValue", "operation_kind":"read", "attribute":"AXFocusedWindow", "ax_error":-25204, "replay_allowed":false}),
        ));
        e
    }
    #[test]
    fn f113_foreground_read_success_has_no_extra_clock_wait_or_record() {
        let mut s = Settlement::default();
        assert_eq!(
            s.read(
                site(),
                || Ok(7),
                || Ok(()),
                |_| panic!("no wait"),
                || panic!("no clock")
            )
            .unwrap(),
            7
        );
        assert!(s.record.is_none());
    }
    #[test]
    fn f113_foreground_read_three_allowed_attributes_keep_original_fault_and_one_read() {
        for attribute in ["AXFocusedWindow", "AXPosition", "AXSize"] {
            let mut s = Settlement::default();
            let calls = Cell::new(0);
            let time = Cell::new(Duration::ZERO);
            let mut original = failure();
            original.diagnostic.as_mut().unwrap()["attribute"] = attribute.into();
            assert_eq!(
                s.read(
                    ReadSite {
                        attribute,
                        ..site()
                    },
                    || {
                        calls.set(calls.get() + 1);
                        if calls.get() == 1 {
                            Err(original.clone())
                        } else {
                            Ok(42)
                        }
                    },
                    || Ok(()),
                    |d| {
                        assert_eq!(d, PAUSE);
                        time.set(time.get() + d);
                        Ok(())
                    },
                    || time.get()
                )
                .unwrap(),
                42
            );
            assert_eq!(calls.get(), 2);
            let record = s.record.unwrap();
            assert_eq!(record["original_fault"], json!(original));
            assert_eq!(record["extra_attribute_reads"], 1);
            assert_eq!(record["elapsed_since_initial_fault_ms"], 20.0);
        }
    }
    #[test]
    fn f113_foreground_read_other_errors_never_settle() {
        for n in 0..6 {
            let mut e = failure();
            match n {
                0 => e.code = "other".into(),
                1 => e.diagnostic.as_mut().unwrap()["ax_error"] = (-25205).into(),
                2 => e.diagnostic.as_mut().unwrap()["api"] = "setter".into(),
                3 => e.diagnostic.as_mut().unwrap()["operation_kind"] = "write".into(),
                4 => e.diagnostic.as_mut().unwrap()["attribute"] = "AXTitle".into(),
                _ => e.diagnostic = None,
            }
            let mut s = Settlement::default();
            let calls = Cell::new(0);
            let actual = s
                .read::<()>(
                    site(),
                    || {
                        calls.set(calls.get() + 1);
                        Err(e.clone())
                    },
                    || Ok(()),
                    |_| panic!("no wait"),
                    || panic!("no clock"),
                )
                .unwrap_err();
            assert_eq!(json!(actual), json!(e));
            assert_eq!(calls.get(), 1);
            assert!(s.record.is_none());
        }
    }
    #[test]
    fn f113_foreground_read_shared_policy_does_not_retry_second_failure() {
        let mut s = Settlement::default();
        let calls = Cell::new(0);
        s.read(
            site(),
            || {
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    Err(failure())
                } else {
                    Ok(())
                }
            },
            || Ok(()),
            |_| Ok(()),
            || Duration::ZERO,
        )
        .unwrap();
        let old = s.record.clone();
        s.read::<()>(
            site(),
            || {
                calls.set(calls.get() + 1);
                Err(failure())
            },
            || Ok(()),
            |_| panic!("consumed"),
            || panic!("consumed"),
        )
        .unwrap_err();
        assert_eq!(calls.get(), 3);
        assert_eq!(s.record, old);
    }
    #[test]
    fn f113_foreground_read_initial_guard_refusal_has_zero_reads() {
        let mut s = Settlement::default();
        let e = s
            .read::<()>(
                site(),
                || panic!("no read"),
                || Err(fault("cancelled", "stop")),
                |_| panic!("no wait"),
                || panic!("no clock"),
            )
            .unwrap_err();
        assert_eq!(e.code, "cancelled");
        assert!(s.record.is_none());
    }
    #[test]
    fn f113_foreground_read_guard_before_pause_has_no_extra_read() {
        let mut s = Settlement::default();
        let checks = Cell::new(0);
        let e = s
            .read::<()>(
                site(),
                || Err(failure()),
                || {
                    checks.set(checks.get() + 1);
                    if checks.get() == 2 {
                        Err(fault("cancelled", "stop"))
                    } else {
                        Ok(())
                    }
                },
                |_| panic!("no wait"),
                || Duration::ZERO,
            )
            .unwrap_err();
        assert_eq!(e.code, "cancelled");
        assert_eq!(s.record.unwrap()["extra_attribute_reads"], 0);
    }
    #[test]
    fn f113_foreground_read_guard_after_pause_has_no_extra_read() {
        let mut s = Settlement::default();
        let checks = Cell::new(0);
        let calls = Cell::new(0);
        let e = s
            .read::<()>(
                site(),
                || {
                    calls.set(calls.get() + 1);
                    Err(failure())
                },
                || {
                    checks.set(checks.get() + 1);
                    if checks.get() == 3 {
                        Err(fault("session_changed", "stop"))
                    } else {
                        Ok(())
                    }
                },
                |_| Ok(()),
                || Duration::ZERO,
            )
            .unwrap_err();
        assert_eq!(e.code, "session_changed");
        assert_eq!(calls.get(), 1);
        assert_eq!(s.record.unwrap()["pause_requested"], true);
    }
    #[test]
    fn f113_foreground_read_expired_budget_does_not_pause() {
        let mut s = Settlement::default();
        let ticks = Cell::new(0);
        let e = s
            .read::<()>(
                site(),
                || Err(failure()),
                || Ok(()),
                |_| panic!("no wait"),
                || {
                    ticks.set(ticks.get() + 1);
                    if ticks.get() == 1 {
                        Duration::ZERO
                    } else {
                        BUDGET
                    }
                },
            )
            .unwrap_err();
        assert_eq!(e.code, "foreground_readback_budget");
        assert_eq!(s.record.unwrap()["pause_requested"], false);
    }
    #[test]
    fn f113_foreground_read_overslept_pause_has_no_extra_read() {
        let mut s = Settlement::default();
        let time = Cell::new(Duration::ZERO);
        let calls = Cell::new(0);
        let e = s
            .read::<()>(
                site(),
                || {
                    calls.set(calls.get() + 1);
                    Err(failure())
                },
                || Ok(()),
                |_| {
                    time.set(BUDGET);
                    Ok(())
                },
                || time.get(),
            )
            .unwrap_err();
        assert_eq!(e.code, "foreground_readback_budget");
        assert_eq!(calls.get(), 1);
        assert_eq!(s.record.unwrap()["extra_attribute_reads"], 0);
    }
    #[test]
    fn f113_foreground_read_second_error_preserves_both_and_request_counts() {
        let mut s = Settlement::default();
        let calls = Cell::new(0);
        let original = failure();
        let e = s
            .read::<()>(
                site(),
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == 1 {
                        Err(original.clone())
                    } else {
                        Err(fault("second_error", "second literal"))
                    }
                },
                || Ok(()),
                |_| Ok(()),
                || Duration::ZERO,
            )
            .unwrap_err();
        let e = s.annotate(Err(e), [1, 0]).unwrap_err();
        assert_eq!(e.code, "second_error");
        assert_eq!(e.next_step, "second literal");
        let d = e.diagnostic.unwrap();
        assert_eq!(d["readback_settlement"]["original_fault"], json!(original));
        assert_eq!(
            d["native_request_counts"],
            json!({"AXRaise":1,"AXFrontmost":0})
        );
    }
    #[test]
    fn f113_foreground_read_late_success_is_not_accepted() {
        let mut s = Settlement::default();
        let time = Cell::new(Duration::ZERO);
        let calls = Cell::new(0);
        let e = s
            .read(
                site(),
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == 1 {
                        Err(failure())
                    } else {
                        time.set(BUDGET);
                        Ok(42)
                    }
                },
                || Ok(()),
                |_| Ok(()),
                || time.get(),
            )
            .unwrap_err();
        assert_eq!(e.code, "foreground_readback_budget");
        assert_eq!(s.record.unwrap()["attribute_outcome"], "unconfirmed");
    }
    #[test]
    fn f113_foreground_read_guard_after_second_read_rejects_changed_context() {
        let mut s = Settlement::default();
        let calls = Cell::new(0);
        let checks = Cell::new(0);
        let e = s
            .read(
                site(),
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == 1 {
                        Err(failure())
                    } else {
                        Ok(42)
                    }
                },
                || {
                    checks.set(checks.get() + 1);
                    if checks.get() == 4 {
                        Err(fault("foreground_changed", "stop"))
                    } else {
                        Ok(())
                    }
                },
                |_| Ok(()),
                || Duration::ZERO,
            )
            .unwrap_err();
        assert_eq!(e.code, "foreground_changed");
        assert_eq!(calls.get(), 2);
        assert_eq!(s.record.unwrap()["extra_attribute_reads"], 1);
    }
}
