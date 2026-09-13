//! A capture can reserve exclusion before the presenter worker exists.
use super::*;
use crate::pointer_feedback::ReleaseOnce;
use rcu_core::operation::checkpoint;

pub(crate) struct CaptureGuard {
    hold: ReleaseOnce<Arc<Shared>>,
    pub(crate) ticket: u64,
}
impl CaptureGuard {
    pub(crate) fn evidence(&self) -> serde_json::Value {
        snapshot_shared(self.hold.get().expect("live capture guard"))
    }
    pub(crate) fn ensure_hidden(&self) -> Result<()> {
        let model = self.hold.get().expect("live capture guard").lock();
        clean_presenter(&model)?;
        if model.worker_requested && !model.stopped && !model.gate.capture_ready(self.ticket) {
            return Err(fault(
                "pointer_feedback_capture_changed",
                "Presenter hidden-state acknowledgement no longer covers this capture ticket.",
            ));
        }
        Ok(())
    }
}
fn release(shared: Arc<Shared>) {
    let mut model = shared.lock();
    model.gate.resume();
    shared.changed.notify_all();
}
pub(crate) fn suspend_capture() -> Result<CaptureGuard> {
    checkpoint("feedback_capture_suspend")?;
    // Initializing this shared Rust state does not start any thread or window.
    // A later presenter() uses the same gate, and Window::new starts hidden.
    let shared = shared().clone();
    let ticket = {
        let mut model = shared.lock();
        if model.gate.suspended >= 64 {
            return Err(fault(
                "pointer_feedback_capture_limit",
                "At most 64 overlapping capture guards.",
            ));
        }
        model.capture_suspensions += 1;
        model.gate.suspend()
    };
    let guard = CaptureGuard {
        hold: ReleaseOnce::new(shared.clone(), release),
        ticket,
    };
    shared.changed.notify_all();
    wait_hidden(&shared, ticket)?;
    Ok(guard)
}
fn wait_hidden(shared: &Shared, ticket: u64) -> Result<()> {
    let deadline = Instant::now() + Duration::from_millis(250);
    let mut model = shared.lock();
    clean_presenter(&model)?;
    while model.worker_requested && !model.stopped && !model.gate.capture_ready(ticket) {
        checkpoint("feedback_capture_wait")?;
        let now = Instant::now();
        if now >= deadline {
            return Err(fault(
                "pointer_feedback_capture_busy",
                "Presenter did not acknowledge this hidden-state ticket; no potentially contaminated capture returned.",
            ));
        }
        model = shared
            .changed
            .wait_timeout(model, (deadline - now).min(Duration::from_millis(10)))
            .unwrap_or_else(|e| e.into_inner())
            .0;
    }
    clean_presenter(&model)?;
    checkpoint("feedback_capture_hidden")
}
fn clean_presenter(model: &Model) -> Result<()> {
    if model.cleanup_uncertain {
        Err(fault(
            "pointer_feedback_resource_release_uncertain",
            "Presenter cleanup is unproven; exclude cannot attest hidden state. Inspect device feedback status.",
        ))
    } else {
        Ok(())
    }
}
fn snapshot_shared(shared: &Shared) -> serde_json::Value {
    let model = shared.lock();
    serde_json::json!({
        "presenter":if model.stopped {"stopped"} else if model.started {"running"} else if model.worker_requested {"starting"} else {"not_started"},
        "visible":model.gate.visible,
        "visibility_known":!model.cleanup_uncertain,
        "resource_release_uncertain":model.cleanup_uncertain,
        "active_suspensions":model.gate.suspended,
        "revision":model.gate.revision,
        "applied_revision":model.gate.applied_revision,
        "error":model.error.as_ref().map(|s|s.chars().take(512).collect::<String>()),
        "measurement":"presenter model/last native submission, not proof of visible pixels or display latency"
    })
}
pub(crate) fn snapshot() -> serde_json::Value {
    SHARED.get().map_or_else(|| serde_json::json!({"presenter":"not_started","visible":false,"active_suspensions":0,"revision":0,"applied_revision":0}), |s| snapshot_shared(s))
}
