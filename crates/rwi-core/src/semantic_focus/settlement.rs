//! One bounded, read-only settling pair after an acknowledged exact focus write.
//! The callback cannot resend a setter. Native retains all identity/custody guards.
use super::*;
use std::time::{Duration, Instant};

const BUDGET: Duration = Duration::from_millis(100);
const DELAY: Duration = Duration::from_millis(25);

#[derive(Debug, Clone, Serialize)]
pub struct FocusReadbackSettlement {
    pub version: u8,
    pub strategy: &'static str,
    pub budget_ms: u64,
    pub delay_ms: u64,
    pub additional_readback_pairs: u8,
    pub elapsed_ms: f64,
    pub stop_reason: &'static str,
    pub final_pair_usable: bool,
    /// The original summary AND original literal paired probes, never rewritten.
    pub initial: Box<FocusReadbackSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation_error: Option<serde_json::Value>,
}

fn known_pair(p: &FocusReadbackAxisPair) -> bool {
    [&p.first, &p.second].iter().all(|p| {
        p.raw.native_status == 0
            && p.raw.value.is_some()
            && p.unknown_cause.is_none()
            && p.validation_error_code.is_none()
    })
}
fn custody(r: &FocusReadback) -> bool {
    match (&r.os_foreground_before, &r.os_foreground_after) {
        (Some(a), Some(b)) => a.validate().is_ok() && b.validate().is_ok() && a == b,
        (None, None) => r
            .readback_diagnostics
            .as_ref()
            .and_then(|d| d.public_application_custody.as_ref())
            .is_some_and(|p| {
                p.before_pid != 0
                    && p.before_pid == p.after_pid
                    && !p.before_process_start.is_empty()
                    && p.before_process_start == p.after_process_start
                    && p.retained_application_equal == Some(true)
                    && p.process_lifetime_equal == Some(true)
                    && !p.full_foreground_window_known
            }),
        _ => false,
    }
}
fn eligible(r: &FocusReadback) -> bool {
    if r.write_plan != FocusWritePlan::SetAXFocusedTrueOnce
        || r.setter_attempt != FocusSetterAttempt::Attempted
        || r.setter_status != Some(0)
        || !custody(r)
        || summarize_focus_readback(r.clone()).state == FocusReadbackState::Ready
    {
        return false;
    }
    r.readback_diagnostics.as_ref().is_some_and(|d| {
        d.settlement.is_none()
            && [
                &d.element_focused,
                &d.focused_ui_element_cf_match,
                &d.application_focused_window_cf_match,
            ]
            .iter()
            .all(|p| known_pair(p))
            && d.application_focused_window_cf_match.first.raw.value == Some(true)
            && d.application_focused_window_cf_match.second.raw.value == Some(true)
    })
}
fn same_phase_custody(a: &FocusReadback, b: &FocusReadback) -> bool {
    fn public(r: &FocusReadback) -> &FocusPublicApplicationReadback {
        r.readback_diagnostics
            .as_ref()
            .unwrap()
            .public_application_custody
            .as_ref()
            .unwrap()
    }
    a.write_plan == b.write_plan
        && a.setter_attempt == b.setter_attempt
        && a.setter_status == b.setter_status
        && custody(b)
        && a.os_foreground_before == b.os_foreground_before
        && a.os_foreground_after == b.os_foreground_after
        && b.readback_diagnostics
            .as_ref()
            .is_some_and(|d| d.settlement.is_none())
        && match (&a.os_foreground_before, &b.os_foreground_before) {
            (None, None) => {
                let x = public(a);
                let y = public(b);
                x.before_pid == y.before_pid && x.before_process_start == y.before_process_start
            }
            _ => true,
        }
}

/// The observer must repeat the exact retained Native readback and its guards,
/// not acquire a new target, dispatch input, refresh custody or relax a lease.
/// 100ms is a soft start budget; a synchronous Native call keeps its own timeout.
pub fn settle_focus_readback(
    initial: FocusReadback,
    observe: impl FnMut() -> Result<FocusReadback>,
) -> FocusReadback {
    let start = Instant::now();
    settle_with(
        initial,
        || start.elapsed(),
        std::thread::sleep,
        || crate::operation::checkpoint("semantic_focus_readonly_settlement"),
        observe,
    )
}
fn settle_with(
    initial: FocusReadback,
    mut elapsed: impl FnMut() -> Duration,
    mut pause: impl FnMut(Duration),
    mut checkpoint: impl FnMut() -> Result<()>,
    mut observe: impl FnMut() -> Result<FocusReadback>,
) -> FocusReadback {
    if !eligible(&initial) {
        return initial;
    }
    let mut summary = summarize_focus_readback(initial.clone());
    summary.readback_diagnostics = initial.readback_diagnostics.clone();
    let mut evidence = FocusReadbackSettlement {
        version: 1,
        strategy: "bounded_readonly_pair_v1",
        budget_ms: BUDGET.as_millis() as u64,
        delay_ms: DELAY.as_millis() as u64,
        additional_readback_pairs: 0,
        elapsed_ms: 0.,
        stop_reason: "budget_expired",
        final_pair_usable: false,
        initial: Box::new(summary),
        observation_error: None,
    };
    let mut result = initial.clone();
    let mut run = || -> Result<()> {
        checkpoint()?;
        if elapsed().saturating_add(DELAY) >= BUDGET {
            return Ok(());
        }
        pause(DELAY);
        checkpoint()?;
        if elapsed() >= BUDGET {
            return Ok(());
        }
        evidence.additional_readback_pairs = 1;
        let later = observe()?;
        if elapsed() > BUDGET {
            return Ok(());
        }
        if !same_phase_custody(&initial, &later) {
            evidence.stop_reason = "phase_or_custody_changed";
            return Ok(());
        }
        evidence.stop_reason = match summarize_focus_readback(later.clone()).state {
            FocusReadbackState::Ready => "ready_pair",
            FocusReadbackState::Partial => "partial_pair",
            FocusReadbackState::Unknown => "unknown_pair",
        };
        evidence.final_pair_usable = true;
        result = later;
        Ok(())
    };
    if let Err(error) = run() {
        evidence.stop_reason = "observation_unconfirmed";
        evidence.observation_error = Some(serde_json::to_value(error).expect("Fault serializes"));
    }
    evidence.elapsed_ms = elapsed().as_secs_f64() * 1000.;
    result
        .readback_diagnostics
        .as_mut()
        .expect("eligible/later diagnostics checked")
        .settlement = Some(Box::new(evidence));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    fn read(value: bool) -> FocusBooleanRead {
        FocusBooleanRead {
            native_status: 0,
            value: Some(value),
        }
    }
    fn pair(a: bool, b: bool) -> FocusReadbackAxisPair {
        FocusReadbackAxisPair::new(
            FocusReadbackProbe::known(read(a)),
            FocusReadbackProbe::known(read(b)),
        )
    }
    fn sample(first: bool, second: bool) -> FocusReadback {
        let foreground = crate::ForegroundState {
            pid: 42,
            window: "cgwindow:9".into(),
            process_start: "42:1:2".into(),
            native_epoch: "a".repeat(64),
        };
        let axis = if first == second {
            read(first)
        } else {
            FocusBooleanRead {
                native_status: 0,
                value: None,
            }
        };
        FocusReadback {
            write_plan: FocusWritePlan::SetAXFocusedTrueOnce,
            setter_attempt: FocusSetterAttempt::Attempted,
            setter_status: Some(0),
            element_focused: axis,
            focused_ui_element_cf_match: axis,
            application_focused_window_cf_match: read(true),
            os_foreground_before: Some(foreground.clone()),
            os_foreground_after: Some(foreground),
            readback_diagnostics: Some(FocusReadbackDiagnostics::new(
                pair(first, second),
                pair(first, second),
                pair(true, true),
            )),
        }
    }
    fn run(initial: FocusReadback, later: FocusReadback) -> (FocusReadback, usize, u64) {
        let clock = Cell::new(0u64);
        let calls = Cell::new(0);
        let result = settle_with(
            initial,
            || Duration::from_millis(clock.get()),
            |d| clock.set(clock.get() + d.as_millis() as u64),
            || Ok(()),
            || {
                calls.set(calls.get() + 1);
                Ok(later.clone())
            },
        );
        (result, calls.get(), clock.get())
    }
    fn summary(r: FocusReadback) -> FocusReadbackSummary {
        summarize_focus_readback(r)
    }
    fn public_sample(value: bool) -> FocusReadback {
        let mut r = sample(value, value);
        r.os_foreground_before = None;
        r.os_foreground_after = None;
        r.readback_diagnostics
            .as_mut()
            .unwrap()
            .public_application_custody = Some(FocusPublicApplicationReadback {
            scope: "public_frontmost_application_identity_only",
            before_pid: 42,
            before_process_start: "42:1:2".into(),
            after_pid: 42,
            after_process_start: "42:1:2".into(),
            retained_application_equal: Some(true),
            process_lifetime_equal: Some(true),
            full_foreground_window_known: false,
        });
        r
    }
    #[test]
    fn f96_focus_settlement_public_custody_retains_narrow_identity_not_global_window() {
        let (r, n, ms) = run(public_sample(false), public_sample(true));
        assert_eq!((n, ms), (1, 25));
        let s = summary(r);
        assert!(s.goal_verified);
        assert_eq!(s.os_foreground_unchanged, None);
        assert!(s.os_foreground_before.is_none() && s.os_foreground_after.is_none());
        let d = s.readback_diagnostics.unwrap();
        assert!(
            !d.public_application_custody
                .unwrap()
                .full_foreground_window_known
        );
        assert_eq!(
            d.settlement.unwrap().initial.state,
            FocusReadbackState::Partial
        );
    }
    #[test]
    fn f96_focus_settlement_public_custody_missing_or_changed_identity_never_refreshes() {
        for case in 0..5 {
            let mut initial = public_sample(false);
            let p = initial
                .readback_diagnostics
                .as_mut()
                .unwrap()
                .public_application_custody
                .as_mut()
                .unwrap();
            match case {
                0 => p.before_pid = 0,
                1 => p.before_process_start.clear(),
                2 => p.retained_application_equal = None,
                3 => p.process_lifetime_equal = Some(false),
                _ => p.full_foreground_window_known = true,
            }
            let (r, n, ms) = run(initial, public_sample(true));
            assert_eq!((n, ms), (0, 0));
            assert!(!summary(r).goal_verified);
        }
        for changed_birth in [false, true] {
            let mut later = public_sample(true);
            let p = later
                .readback_diagnostics
                .as_mut()
                .unwrap()
                .public_application_custody
                .as_mut()
                .unwrap();
            if changed_birth {
                p.before_process_start = "42:2:3".into();
                p.after_process_start = "42:2:3".into();
            } else {
                p.before_pid = 43;
                p.after_pid = 43;
            }
            let (r, n, _) = run(public_sample(false), later);
            assert_eq!(n, 1);
            let s = summary(r);
            assert_eq!(s.state, FocusReadbackState::Unknown);
            assert_eq!(
                s.readback_diagnostics
                    .unwrap()
                    .settlement
                    .unwrap()
                    .stop_reason,
                "phase_or_custody_changed"
            );
        }
    }
    #[test]
    fn f96_focus_settlement_ready_fast_path_does_not_wait_or_observe() {
        let (r, n, ms) = run(sample(true, true), sample(false, false));
        assert_eq!((n, ms), (0, 0));
        assert!(summary(r).goal_verified);
    }
    #[test]
    fn f96_focus_settlement_delayed_partial_keeps_original_and_one_final_pair() {
        let (r, n, ms) = run(sample(false, false), sample(true, true));
        assert_eq!((n, ms), (1, 25));
        let s = summary(r);
        assert!(s.goal_verified);
        let e = s.readback_diagnostics.unwrap().settlement.unwrap();
        assert_eq!(e.initial.state, FocusReadbackState::Partial);
        assert_eq!(e.initial.element_focused.value, Some(false));
        assert_eq!(e.additional_readback_pairs, 1);
    }
    #[test]
    fn f96_focus_settlement_drift_preserves_both_original_literal_reads() {
        let (r, n, _) = run(sample(false, true), sample(true, true));
        assert_eq!(n, 1);
        let s = summary(r);
        assert!(s.goal_verified);
        let e = s.readback_diagnostics.unwrap().settlement.unwrap();
        assert_eq!(e.initial.state, FocusReadbackState::Unknown);
        let d = e.initial.readback_diagnostics.unwrap();
        assert_eq!(d.element_focused.first.raw.value, Some(false));
        assert_eq!(d.element_focused.second.raw.value, Some(true));
    }
    #[test]
    fn f96_focus_settlement_still_partial_or_unknown_never_loops() {
        for later in [sample(false, false), sample(false, true)] {
            let (r, n, _) = run(sample(false, false), later);
            assert_eq!(n, 1);
            assert!(!summary(r).goal_verified);
        }
    }
    #[test]
    fn f96_focus_settlement_native_error_missing_diagnostics_and_wrong_window_refuse() {
        let mut error = sample(false, false);
        error
            .readback_diagnostics
            .as_mut()
            .unwrap()
            .element_focused
            .first
            .raw
            .native_status = -25204;
        let mut missing = sample(false, false);
        missing.readback_diagnostics = None;
        let mut window = sample(false, false);
        window
            .readback_diagnostics
            .as_mut()
            .unwrap()
            .application_focused_window_cf_match = pair(false, false);
        for input in [error, missing, window] {
            let (_, n, ms) = run(input, sample(true, true));
            assert_eq!((n, ms), (0, 0));
        }
    }
    #[test]
    fn f96_focus_settlement_no_write_or_failed_setter_never_waits() {
        let mut no = sample(false, false);
        no.write_plan = FocusWritePlan::NoWriteAlreadyFocused;
        no.setter_attempt = FocusSetterAttempt::NotAttempted;
        no.setter_status = None;
        let mut failed = sample(false, false);
        failed.setter_status = Some(-25204);
        for input in [no, failed] {
            let (_, n, ms) = run(input, sample(true, true));
            assert_eq!((n, ms), (0, 0));
        }
    }
    #[test]
    fn f96_focus_settlement_budget_expiry_never_starts_another_native_pair() {
        let clock = Cell::new(0);
        let r = settle_with(
            sample(false, false),
            || Duration::from_millis(clock.get()),
            |_| clock.set(100),
            || Ok(()),
            || panic!("expired pair"),
        );
        let s = summary(r);
        assert_eq!(s.state, FocusReadbackState::Unknown);
        assert_eq!(
            s.readback_diagnostics
                .unwrap()
                .settlement
                .unwrap()
                .additional_readback_pairs,
            0
        );
    }
    #[test]
    fn f96_focus_settlement_error_retains_original_and_literal_fault_not_success() {
        let r = settle_with(
            sample(false, false),
            || Duration::ZERO,
            |_| {},
            || Ok(()),
            || Err(fault("focus_context_changed_after_call", "original fault")),
        );
        let s = summary(r);
        assert_eq!(s.state, FocusReadbackState::Unknown);
        let e = s.readback_diagnostics.unwrap().settlement.unwrap();
        assert_eq!(e.initial.element_focused.value, Some(false));
        assert_eq!(
            e.observation_error.unwrap()["code"],
            "focus_context_changed_after_call"
        );
    }
    #[test]
    fn f96_focus_settlement_foreground_or_phase_change_cannot_promote_ready() {
        let mut fg = sample(true, true);
        fg.os_foreground_after.as_mut().unwrap().pid = 43;
        let mut phase = sample(true, true);
        phase.write_plan = FocusWritePlan::NoWriteAlreadyFocused;
        for later in [fg, phase] {
            let (r, n, _) = run(sample(false, false), later);
            assert_eq!(n, 1);
            assert_eq!(summary(r).state, FocusReadbackState::Unknown);
        }
    }
    #[test]
    fn f96_focus_settlement_cancellation_guard_never_observes_or_retries() {
        let r = settle_with(
            sample(false, false),
            || Duration::ZERO,
            |_| panic!("cancelled wait"),
            || Err(fault("operation_cancelled", "original cancellation")),
            || panic!("cancelled observe"),
        );
        let s = summary(r);
        assert!(!s.goal_verified);
        let e = s.readback_diagnostics.unwrap().settlement.unwrap();
        assert_eq!(e.additional_readback_pairs, 0);
        assert_eq!(e.observation_error.unwrap()["code"], "operation_cancelled");
    }
    #[test]
    fn f96_focus_settlement_synchronous_call_over_budget_cannot_promote_ready() {
        let clock = Cell::new(0);
        let r = settle_with(
            sample(false, false),
            || Duration::from_millis(clock.get()),
            |_| {},
            || Ok(()),
            || {
                clock.set(101);
                Ok(sample(true, true))
            },
        );
        let s = summary(r);
        assert_eq!(s.state, FocusReadbackState::Unknown);
        let e = s.readback_diagnostics.unwrap().settlement.unwrap();
        assert_eq!(e.additional_readback_pairs, 1);
        assert!(!e.final_pair_usable);
    }
}
