//! Context-owned bounded capture worker; presenter stays on the main thread.
use super::*;

pub(super) fn run(shared: Arc<Shared>) {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_loop(&shared)));
    loop {
        let mut state = shared
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.capture_pending = false;
        if let Some(operation) = &state.native_operation {
            operation.finish();
        }
        if state.mode != Mode::Stopped {
            let reason = if outcome.is_err() {
                "worker_panicked"
            } else {
                "worker_ended"
            };
            state.last_error = Some(reason.into());
            state.stop(reason);
        }
        // A timed-out callback still owns its capture slot. Keep the bounded
        // session reservation until the native scope has actually released.
        if !state.native_pending() {
            state.worker_done = true;
            return;
        }
        drop(state);
        std::thread::sleep(Duration::from_millis(100));
    }
}
fn run_loop(shared: &Arc<Shared>) {
    loop {
        let Ok(mut state) = shared.state.lock() else {
            return;
        };
        state.expire();
        if state.mode == Mode::Stopped {
            return;
        }
        if matches!(state.mode, Mode::Paused | Mode::Hidden)
            || !state.presentation_allowed()
            || state.native_pending()
            || !state.source_retry_ready(Instant::now())
        {
            let _ = shared.wake.wait_timeout(state, Duration::from_millis(100));
            continue;
        }
        let (source, generation, options, display) = (
            state.source.clone(),
            state.generation,
            state.options.clone(),
            state.display.clone(),
        );
        state.capture_pending = true;
        let operation = match rwi_core::operation::Registry::default().begin(None) {
            Ok(operation) => operation,
            Err(error) => {
                state.last_error = Some(error.code);
                state.stop("operation_unavailable");
                return;
            }
        };
        state.native_operation = Some(operation.clone());
        drop(state);
        let start = Instant::now();
        let capture = rwi_core::operation::run(&operation, || {
            displays().and_then(|list| {
                if !list.contains(&display) {
                    return Err(fault(
                        "live_display_changed",
                        "Preview display topology changed; restart with a fresh display selection.",
                    ));
                }
                source.visual_capture(VisualOptions {
                    max_edge: options.max_edge,
                    ..Default::default()
                })
            })
        });
        operation.finish();
        let Ok(mut state) = shared.state.lock() else {
            return;
        };
        state.capture_pending = false;
        state.expire();
        if state.accepts(generation) {
            state.last_capture_ms = Some(start.elapsed().as_millis());
            match capture {
                Ok(frame) => state.capture_succeeded(frame),
                Err(error) => state.capture_failed(error),
            }
        }
        // Host-view heartbeat updates wake the worker to clear a hidden panel,
        // but are not capture ticks. Keep the requested interval anchored to
        // the prior capture start rather than treating every wake as permission
        // for another ScreenCaptureKit frame.
        let next = (start + Duration::from_millis(options.interval_ms))
            .max(Instant::now() + Duration::from_millis(20));
        wait_for_next_frame(shared, state, next, generation);
    }
}

pub(super) fn wait_for_next_frame(
    shared: &Shared,
    mut state: std::sync::MutexGuard<'_, State>,
    deadline: Instant,
    generation: u64,
) {
    loop {
        state.expire();
        if state.mode != Mode::Running
            || state.generation != generation
            || !state.presentation_allowed()
        {
            return;
        }
        let Some(wait) = deadline.checked_duration_since(Instant::now()) else {
            return;
        };
        state = shared
            .wake
            .wait_timeout(state, wait)
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0;
    }
}
