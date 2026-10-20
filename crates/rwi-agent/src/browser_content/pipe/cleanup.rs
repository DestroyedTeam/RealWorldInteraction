//! Typed wait/signal policy used by the actual owned-child close and offline tests.
//! A wait error permanently removes signal authority, never becoming a running receipt.
use serde_json::{Value, json};
use std::io;
pub(super) enum ExitObservation {
    Exited(Value),
    Running,
    WaitUnknown(String),
}
impl ExitObservation {
    pub(super) fn value(&self) -> Value {
        match self {
            Self::Exited(value) => value.clone(),
            Self::Running => {
                json!({"process_exit_observed":false,"process_state":"running","os_exit":"unknown"})
            }
            Self::WaitUnknown(error) => {
                json!({"process_exit_observed":false,"process_state":"wait_unknown","os_exit":"unknown","error":error})
            }
        }
    }
}
#[derive(Default)]
pub(super) struct SignalState {
    pub(super) attempted: bool,
    pub(super) sent: bool,
    pub(super) authority_lost: bool,
}
impl SignalState {
    pub(super) fn try_once(
        &mut self,
        observed: &ExitObservation,
        identity_guard: impl FnOnce() -> io::Result<()>,
        signal: impl FnOnce() -> io::Result<()>,
    ) -> Option<String> {
        if let ExitObservation::WaitUnknown(error) = observed {
            self.authority_lost = true;
            return Some(format!("browser_wait_unknown_no_signal:{error}"));
        }
        if !matches!(observed, ExitObservation::Running) || self.attempted || self.authority_lost {
            return None;
        }
        if let Err(error) = identity_guard() {
            self.authority_lost = true;
            return Some(format!("browser_signal_identity_refused:{error}"));
        }
        self.attempted = true;
        match signal() {
            Ok(()) => {
                self.sent = true;
                None
            }
            Err(error) => Some(format!("browser_own_child_signal_failed:{error}")),
        }
    }
}
pub(super) fn graceful_exit_budget(first_close: bool) -> std::time::Duration {
    if first_close {
        std::time::Duration::from_secs(2)
    } else {
        std::time::Duration::ZERO
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn f14_browser_graceful_budget_only_first_close_waits() {
        assert_eq!(
            graceful_exit_budget(true),
            std::time::Duration::from_secs(2)
        );
        assert_eq!(graceful_exit_budget(false), std::time::Duration::ZERO);
    }
    use std::cell::Cell;
    #[test]
    fn f14_browser_wait_unknown_never_signals_or_becomes_exit_zero() {
        let mut state = SignalState::default();
        let unknown = ExitObservation::WaitUnknown("injected ECHILD".into());
        assert!(
            state
                .try_once(
                    &unknown,
                    || panic!("unknown wait cannot attest identity"),
                    || panic!("unknown wait must never signal")
                )
                .is_some()
        );
        assert!(state.authority_lost && !state.attempted && !state.sent);
        assert_eq!(unknown.value()["process_exit_observed"], false);
        assert_eq!(unknown.value()["os_exit"], "unknown");
        assert!(unknown.value().get("code").is_none());
        assert!(
            state
                .try_once(
                    &ExitObservation::Running,
                    || panic!("authority loss is sticky"),
                    || panic!("no deferred signal")
                )
                .is_none()
        );
        let mut exited = SignalState::default();
        assert!(
            exited
                .try_once(
                    &ExitObservation::Exited(json!({"process_exit_observed":true,"code":0})),
                    || panic!("already reaped"),
                    || panic!("never signal reaped PID")
                )
                .is_none()
        );
    }
    #[test]
    fn f14_browser_running_signal_needs_identity_and_attempts_only_once() {
        let calls = Cell::new(0);
        let mut refused = SignalState::default();
        assert!(
            refused
                .try_once(
                    &ExitObservation::Running,
                    || Err(io::Error::other("birth or UID mismatch")),
                    || {
                        calls.set(calls.get() + 1);
                        Ok(())
                    }
                )
                .is_some()
        );
        assert!(refused.authority_lost && !refused.attempted && !refused.sent);
        assert_eq!(calls.get(), 0);
        assert!(
            refused
                .try_once(
                    &ExitObservation::Running,
                    || panic!("identity refusal is sticky"),
                    || panic!("no retry")
                )
                .is_none()
        );
        let mut allowed = SignalState::default();
        assert!(
            allowed
                .try_once(
                    &ExitObservation::Running,
                    || Ok(()),
                    || {
                        calls.set(calls.get() + 1);
                        Ok(())
                    }
                )
                .is_none()
        );
        assert!(allowed.attempted && allowed.sent && !allowed.authority_lost);
        assert_eq!(calls.get(), 1);
        assert!(
            allowed
                .try_once(
                    &ExitObservation::Running,
                    || panic!("no second identity read after attempt"),
                    || panic!("no second signal")
                )
                .is_none()
        );
        let mut failed = SignalState::default();
        assert!(
            failed
                .try_once(
                    &ExitObservation::Running,
                    || Ok(()),
                    || Err(io::Error::other("injected signal failure"))
                )
                .is_some()
        );
        assert!(failed.attempted && !failed.sent);
        assert!(
            failed
                .try_once(
                    &ExitObservation::Running,
                    || panic!("no retry after failed attempt"),
                    || panic!("no repeated signal")
                )
                .is_none()
        );
    }
}
