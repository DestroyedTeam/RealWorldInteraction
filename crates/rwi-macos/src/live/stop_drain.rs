//! Soft bounded Stop drain observation only; never a join or native preemption.
use super::*;
pub(super) fn budget(action: LiveAction) -> Duration {
    Duration::from_millis(if action == LiveAction::Stop { 500 } else { 250 })
}
pub(super) fn deadline(action: LiveAction, started: Instant) -> Instant {
    // Preserve the old post-lock/setter 250ms budget for every other action.
    let base = if action == LiveAction::Stop {
        started
    } else {
        Instant::now()
    };
    base + budget(action)
}
pub(super) fn settled(state: &State, action: LiveAction) -> bool {
    match action {
        LiveAction::Pause => !state.image_present,
        LiveAction::Hide => !state.visible && !state.image_present,
        LiveAction::Stop => {
            state.mode == Mode::Stopped
                && state.worker_done
                && !state.capture_pending
                && !state.native_pending()
                && !state.visible
                && !state.image_present
        }
        LiveAction::Start | LiveAction::Resume => {
            state.mode != Mode::Running
                || !state.presentation_allowed()
                || (state.visible && state.image_present)
        }
        LiveAction::Status => true,
    }
}
pub(super) fn wait_slice(until: Instant) -> Duration {
    // Condvar releases the mutex: the worker must publish its own drain flag.
    until
        .saturating_duration_since(Instant::now())
        .min(Duration::from_millis(10))
}
pub(super) fn annotate(action: LiveAction, started: Instant, mut status: Value) -> Value {
    if action == LiveAction::Stop {
        let known = status["status"] == "stopped"
            && [
                "worker_done",
                "capture_pending",
                "native_call_pending",
                "presenter_visible",
                "presenter_image_present",
            ]
            .iter()
            .all(|field| status[*field].is_boolean());
        let drained = known
            && status["worker_done"] == true
            && status["capture_pending"] == false
            && status["native_call_pending"] == false
            && status["presenter_visible"] == false
            && status["presenter_image_present"] == false;
        status["stop_completion"] = json!({
            "state": if !known { "unknown" } else if drained { "drained" } else { "pending" },
            "soft_wait_budget_ms": budget(action).as_millis(),
            "wait_elapsed_ms": started.elapsed().as_millis(),
        });
    }
    status
}

#[cfg(test)]
mod tests {
    use super::super::tests::session;
    use super::*;
    // Offline state/Condvar tests only, never an old runtime-session oracle.
    #[test]
    fn live_stop_drain_already_done_returns_same_session_terminal_state() {
        let session = session();
        {
            let mut state = session.shared.state.lock().unwrap();
            state.stop("fixture_completed");
            state.worker_done = true;
            state.capture_pending = false;
            state.visible = false;
            state.image_present = false;
        }
        let result = session.control(LiveAction::Stop);
        assert_eq!(result["session_id"], "live:999");
        assert_eq!(result["worker_done"], true);
        assert_eq!(result["stop_reason"], "fixture_completed");
        assert_eq!(result["stop_completion"]["state"], "drained");
        assert_eq!(result["stop_completion"]["soft_wait_budget_ms"], 500);
    }
    #[test]
    fn live_stop_drain_wait_releases_state_lock_for_existing_worker_flag() {
        let session = session();
        let shared = session.shared.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let producer = std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(2);
            let mut state = shared.state.lock().unwrap();
            ready_tx.send(()).unwrap();
            while state.mode != Mode::Stopped {
                if Instant::now() >= until {
                    return false;
                }
                state = shared
                    .wake
                    .wait_timeout(state, Duration::from_millis(10))
                    .unwrap()
                    .0;
            }
            state.capture_pending = false;
            state.visible = false;
            state.image_present = false;
            state.worker_done = true;
            shared.wake.notify_all();
            true
        });
        ready_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        let result = session.control(LiveAction::Stop);
        assert!(producer.join().unwrap());
        assert_eq!(result["session_id"], "live:999");
        assert_eq!(result["stop_completion"]["state"], "drained");
        assert_eq!(result["worker_done"], true);
    }
    #[test]
    fn live_stop_drain_timeout_keeps_pending_not_fabricated_completion() {
        let session = session();
        {
            let mut state = session.shared.state.lock().unwrap();
            state.capture_pending = false;
            state.visible = false;
            state.image_present = false;
        }
        let result = session.control(LiveAction::Stop);
        assert_eq!(result["status"], "stopped");
        assert_eq!(result["worker_done"], false);
        assert_eq!(result["stop_completion"]["state"], "pending");
        assert_eq!(result["stop_completion"]["soft_wait_budget_ms"], 500);
        assert_eq!(result["presenter_visible"], false);
        assert_eq!(result["presenter_image_present"], false);
    }

    #[test]
    fn live_stop_drain_native_wrapper_refresh_preserves_same_session_completion() {
        let session = session();
        {
            let mut state = session.shared.state.lock().unwrap();
            state.stop("fixture_completed");
            state.worker_done = true;
            state.capture_pending = false;
            state.visible = false;
            state.image_present = false;
        }
        let checks = std::cell::Cell::new(0);
        let result = session.control_native_with_readback(LiveAction::Stop, |window| {
            assert_eq!(window, Some(99));
            checks.set(checks.get() + 1);
            Ok(None)
        });
        assert_eq!(checks.get(), 2);
        assert_eq!(result["presenter_readback"]["checks"], 2);
        assert_eq!(result["presenter_readback"]["settled"], true);
        assert_eq!(result["session_id"], "live:999");
        assert_eq!(result["stop_completion"]["state"], "drained");
        assert_eq!(result["worker_done"], true);
    }
    #[test]
    fn live_stop_drain_native_wrapper_timeout_keeps_pending_and_same_call_elapsed() {
        let session = session();
        {
            let mut state = session.shared.state.lock().unwrap();
            state.capture_pending = false;
            state.visible = false;
            state.image_present = false;
        }
        let checks = std::cell::Cell::new(0);
        let result = session.control_native_with_readback(LiveAction::Stop, |_| {
            checks.set(checks.get() + 1);
            Ok(None)
        });
        assert_eq!(checks.get(), 2);
        assert_eq!(result["presenter_readback"]["checks"], 2);
        assert_eq!(result["session_id"], "live:999");
        assert_eq!(result["status"], "stopped");
        assert_eq!(result["worker_done"], false);
        assert_eq!(result["stop_completion"]["state"], "pending");
        assert!(
            result["stop_completion"]["wait_elapsed_ms"]
                .as_u64()
                .unwrap()
                >= 500
        );
        assert_eq!(result["stop_completion"]["soft_wait_budget_ms"], 500);
    }
}
