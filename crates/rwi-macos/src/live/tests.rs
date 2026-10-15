//! Pure lifecycle/race tests: no AppKit, native enumeration, screen or input.
use super::*;
fn state() -> State {
    let options: LiveOptions =
        serde_json::from_value(json!({"display":{"id":1},"duration_ms":1000})).unwrap();
    State {
        source: Arc::new(NativeMac::fixture_for_live()),
        additional_sources: vec![],
        generation: 1,
        mode: Mode::Running,
        deadline: Instant::now() + Duration::from_secs(1),
        options,
        display: Display {
            id: 1,
            uuid: None,
            built_in: true,
            rect: crate::geometry::Rect {
                x: 0.,
                y: 0.,
                w: 1200.,
                h: 800.,
            },
            pixels: [1200, 800],
            rotation: 0.,
        },
        latest: Some(serde_json::from_value(json!({
            "identity":{"host":"fixture","guest":null,"session":"s","seat":"s","desktop":"d"},
            "window":{"window":"w","pid":1,"process_created":1,"native_epoch":"e"},
            "source_rect":{"x":0,"y":0,"width":1,"height":1},"source_pixel_size":[1,1],"image_size":[1,1],
            "visual_to_desktop":[1,0,0,1,0,0],"captured_unix_ms":1,
            "assurance":"fixture","atomicity":"fixture","normalization":"fixture",
            "feedback":{"requested":"exclude","suspension_ticket":null,"exclusion":"fixture","before_copy":null,"after_copy":null,"after_release":null},
            "details":{}
        })).unwrap()),
        metadata: json!({"window":"old"}),
        captured: 0,
        presented: 0,
        capture_pending: true,
        native_operation: None,
        worker_done: false,
        visible: true,
        image_present: true,
        window_id: Some(99),
        last_capture_ms: None,
        last_error: None,
        last_error_detail: None,
        source_retry_count: 0,
        source_retry_at: None,
        presenter_failure: None,
        stop_reason: None,
        host_view: None,
    }
}
pub(super) fn session() -> Session {
    Session {
        controls: Arc::new(Controls {
            id: 999,
            shared: Arc::new(Shared {
                state: Mutex::new(state()),
                wake: Condvar::new(),
            }),
        }),
    }
}
#[test]
fn dropping_owned_session_revokes_exported_controls_without_resurrecting_capture() {
    let session = session();
    let controls = session.controller();
    drop(session);
    assert_eq!(controls.status()["status"], "stopped");
    assert_eq!(controls.status()["stop_reason"], "context_closed");
    controls.control(LiveAction::Resume);
    assert_eq!(controls.status()["status"], "stopped");
    assert!(!controls.shared.state.lock().unwrap().accepts(1));
}
#[test]
fn paused_and_resumed_sessions_never_accept_an_in_flight_old_frame() {
    let mut state = state();
    let generation = state.generation;
    assert!(state.accepts(generation));
    assert!(state.latest.is_some());
    state.control(LiveAction::Pause);
    assert!(!state.accepts(generation));
    assert!(state.latest.is_none());
    state.control(LiveAction::Resume);
    assert!(!state.accepts(generation));
    assert!(state.accepts(state.generation));
}
#[test]
fn hidden_session_clears_frames_stays_hidden_on_rebind_and_needs_explicit_resume() {
    let session = session();
    let old_generation = session.shared.state.lock().unwrap().generation;
    session.control(LiveAction::Hide);
    {
        let s = session.shared.state.lock().unwrap();
        assert_eq!(s.mode, Mode::Hidden);
        assert!(s.latest.is_none());
        assert!(!s.accepts(old_generation));
    }
    assert_eq!(session.status()["user_hidden"], true);
    session.source(NativeMac::fixture_for_live());
    session.source_failed("source_changed".into());
    session.control(LiveAction::Pause);
    assert_eq!(session.status()["status"], "hidden");
    session.control(LiveAction::Resume);
    let s = session.shared.state.lock().unwrap();
    assert_eq!(s.mode, Mode::Running);
    assert!(s.accepts(s.generation));
    assert!(!s.accepts(old_generation));
}
#[test]
fn stop_cancels_waiter_but_retains_native_callback_ownership() {
    let mut state = state();
    let operation = rwi_core::operation::Registry::default()
        .begin(None)
        .unwrap();
    let native = rwi_core::operation::run(&operation, || {
        rwi_core::operation::native_call("fixture_live_capture").unwrap()
    });
    state.native_operation = Some(operation.clone());
    state.control(LiveAction::Stop);
    operation.finish();
    assert!(operation.status().cancel_requested);
    assert!(state.native_pending());
    assert!(state.latest.is_none());
    drop(native);
    assert!(!state.native_pending());
}
#[test]
fn explicit_rebind_invalidates_old_source_but_does_not_override_user_pause_or_stop() {
    let session = session();
    session.control(LiveAction::Pause);
    let generation = session.shared.state.lock().unwrap().generation;
    session.source(NativeMac::fixture_for_live());
    {
        let state = session.shared.state.lock().unwrap();
        assert_eq!(state.mode, Mode::Paused);
        assert!(!state.accepts(generation));
        assert!(state.metadata.is_null());
    }
    session.control(LiveAction::Stop);
    session.source_failed("late_source_error".into());
    session.source(NativeMac::fixture_for_live());
    assert_eq!(session.status()["status"], "stopped");
}
#[test]
fn root_rebind_drops_explicit_extra_sources_without_resurrecting_a_stopped_session() {
    let session = session();
    session
        .shared
        .state
        .lock()
        .unwrap()
        .additional_sources
        .push(Arc::new(NativeMac::fixture_for_live()));
    session.control(LiveAction::Pause);
    session.source(NativeMac::fixture_for_live());
    assert!(
        session
            .shared
            .state
            .lock()
            .unwrap()
            .additional_sources
            .is_empty()
    );
    assert_eq!(session.status()["status"], "paused");
    session.control(LiveAction::Stop);
    session.source(NativeMac::fixture_for_live());
    assert_eq!(session.status()["status"], "stopped");
}
#[test]
fn expiry_and_context_drop_clear_frames_without_claiming_pending_capture_is_released() {
    let session = session();
    let shared = session.shared.clone();
    {
        let mut state = shared.state.lock().unwrap();
        state.deadline = Instant::now() - Duration::from_millis(1);
        state.control(LiveAction::Resume);
        assert_eq!(state.mode, Mode::Stopped);
        assert_eq!(state.stop_reason.as_deref(), Some("duration_expired"));
    }
    drop(session);
    let state = shared.state.lock().unwrap();
    assert_eq!(state.stop_reason.as_deref(), Some("duration_expired"));
    assert!(state.latest.is_none());
    assert!(state.capture_pending);
    assert!(!state.worker_done);
}

#[test]
fn active_context_drop_records_context_closed_and_keeps_pending_capture_owned() {
    let session = session();
    let shared = session.shared.clone();
    let generation = shared.state.lock().unwrap().generation;
    drop(session);
    let state = shared.state.lock().unwrap();
    assert_eq!(state.mode, Mode::Stopped);
    assert_eq!(state.stop_reason.as_deref(), Some("context_closed"));
    assert_eq!(state.generation, generation.wrapping_add(1));
    assert!(state.latest.is_none());
    assert!(state.capture_pending);
    assert!(!state.worker_done);
}

#[test]
fn window_server_readback_requires_exact_owner_unique_id_and_complete_absence() {
    let rect = crate::geometry::Rect {
        x: 10.,
        y: 20.,
        w: 420.,
        h: 284.,
    };
    let row = crate::native::Window {
        id: 99,
        pid: 42,
        rect,
        alpha: 1.,
        title: None,
    };
    let mut inventory = WindowInventory {
        windows: vec![row.clone()],
        native_rows: 1,
        invalid_rows: 0,
    };
    assert_eq!(
        presenter_readback(&inventory, Some(99), 42).unwrap(),
        Some(rect)
    );
    assert!(presenter_readback(&inventory, Some(99), 43).is_err());
    inventory.windows.push(row);
    assert!(presenter_readback(&inventory, Some(99), 42).is_err());
    inventory.windows.clear();
    assert_eq!(presenter_readback(&inventory, Some(99), 42).unwrap(), None);
    inventory.invalid_rows = 1;
    assert!(presenter_readback(&inventory, Some(99), 42).is_err());
    assert_eq!(presenter_readback(&inventory, None, 42).unwrap(), None);
}

#[test]
fn appkit_hidden_acknowledgement_does_not_confirm_window_server_removal() {
    let rect = Some(crate::geometry::Rect {
        x: 10.,
        y: 20.,
        w: 420.,
        h: 284.,
    });
    let mut status = json!({
        "status":"stopped", "presenter_visible":false, "presenter_image_present":false,
        "requested_preview_display":{"bounds":[0,0,1200,900]},
    });
    assert!(!presentation_matches(&status, rect));
    assert!(presentation_matches(&status, None));
    status["status"] = json!("running");
    status["presenter_visible"] = json!(true);
    assert!(!presentation_matches(&status, rect));
    status["presenter_image_present"] = json!(true);
    assert!(presentation_matches(&status, rect));
    assert!(!presentation_matches(
        &status,
        Some(crate::geometry::Rect {
            x: 1512.,
            y: 20.,
            w: 420.,
            h: 284.,
        })
    ));
    status["status"] = json!("paused");
    assert!(!presentation_matches(&status, rect));
    status["presenter_image_present"] = json!(false);
    assert!(presentation_matches(&status, rect));
    assert!(!presentation_matches(&status, None));
    status["status"] = json!("hidden");
    status["presenter_visible"] = json!(false);
    assert!(presentation_matches(&status, None));
    assert!(!presentation_matches(&status, rect));
}

#[test]
fn host_visibility_never_overrides_pause_stop_or_accepts_a_late_capture() {
    let session = session();
    session.shared.state.lock().unwrap().host_view = Some(ViewGate::new("a".repeat(32)));
    let update = |sequence, visible| rwi_core::LiveViewUpdate {
        view_id: "a".repeat(32),
        sequence,
        visible,
    };
    let generation = session.shared.state.lock().unwrap().generation;
    assert!(!session.shared.state.lock().unwrap().accepts(generation));
    session.view_update(update(1, true)).unwrap();
    let generation = session.shared.state.lock().unwrap().generation;
    assert!(session.shared.state.lock().unwrap().accepts(generation));
    session.view_update(update(3, false)).unwrap();
    assert!(!session.shared.state.lock().unwrap().accepts(generation));
    assert_eq!(
        session.view_update(update(2, true)).unwrap()["accepted"],
        false
    );
    {
        let mut s = session.shared.state.lock().unwrap();
        s.control(LiveAction::Pause);
    }
    session.view_update(update(4, true)).unwrap();
    assert_eq!(session.shared.state.lock().unwrap().mode, Mode::Paused);
    {
        let mut s = session.shared.state.lock().unwrap();
        s.control(LiveAction::Stop);
    }
    assert_eq!(
        session.view_update(update(5, true)).unwrap()["reason"],
        "preview_stopped"
    );
    assert_eq!(session.shared.state.lock().unwrap().mode, Mode::Stopped);
}

#[test]
fn presence_wakeups_do_not_shorten_requested_capture_interval() {
    let shared = Arc::new(Shared {
        state: Mutex::new(state()),
        wake: Condvar::new(),
    });
    let started = Instant::now();
    let notifier = shared.clone();
    let pulses = std::thread::spawn(move || {
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(25));
            notifier.wake.notify_all();
        }
    });
    wait_for_next_frame(
        &shared,
        shared.state.lock().unwrap(),
        started + Duration::from_millis(130),
        1,
    );
    pulses.join().unwrap();
    assert!(started.elapsed() >= Duration::from_millis(120));
}

#[test]
fn terminal_live_stop_reason_survives_later_requested_stop() {
    let mut state = state();
    state.last_error = Some("live_presenter_wrong_display".into());
    state.stop("presenter_wrong_display");
    let generation = state.generation;
    state.control(LiveAction::Stop);
    assert_eq!(
        state.stop_reason.as_deref(),
        Some("presenter_wrong_display")
    );
    assert_eq!(state.generation, generation);
    assert_eq!(
        state.last_error.as_deref(),
        Some("live_presenter_wrong_display")
    );
    assert!(state.latest.is_none());
}

#[test]
fn native_presenter_failure_evidence_survives_stop_and_context_drop() {
    let session = session();
    let shared = session.shared.clone();
    let evidence = json!({"window":99,"bounds":[1300,20,420,284],"containing_display":null});
    {
        let mut state = shared.state.lock().unwrap();
        state.presenter_failure = Some(evidence.clone());
        state.last_error = Some("live_presenter_wrong_display".into());
        state.stop("presenter_wrong_display");
    }
    session.control(LiveAction::Stop);
    assert_eq!(session.status()["presenter_failure"], evidence);
    drop(session);
    let state = shared.state.lock().unwrap();
    assert_eq!(state.presenter_failure, Some(evidence));
    assert_eq!(
        state.stop_reason.as_deref(),
        Some("presenter_wrong_display")
    );
}

#[test]
fn transient_source_loss_uses_bounded_read_only_recovery() {
    let mut s = state();
    let source = s.source.clone();
    let deadline = s.deadline;
    for attempt in 1..=SOURCE_RECOVERY_ATTEMPTS {
        s.capture_failed(fault(
            "window_missing",
            "Pinned source temporarily absent, never substitute",
        ));
        assert_eq!(s.source_retry_count, attempt);
        assert!(s.latest.is_none() && Arc::ptr_eq(&source, &s.source));
        assert_eq!(s.deadline, deadline);
        if attempt < SOURCE_RECOVERY_ATTEMPTS {
            assert_eq!(s.mode, Mode::Recovering);
            let next = s.source_retry_at.unwrap();
            assert!(!s.source_retry_ready(next - Duration::from_millis(1)));
            assert!(s.source_retry_ready(next));
        } else {
            assert_eq!(s.mode, Mode::Paused);
            assert!(s.source_retry_at.is_none());
        }
    }
}
#[test]
fn explicit_controls_disable_recovery_and_reject_late_frames() {
    for action in [LiveAction::Pause, LiveAction::Hide, LiveAction::Stop] {
        let mut s = state();
        s.capture_failed(fault("window_missing", "temporary"));
        let old = s.generation;
        s.control(action);
        let expected = match action {
            LiveAction::Pause => Mode::Paused,
            LiveAction::Hide => Mode::Hidden,
            _ => Mode::Stopped,
        };
        assert_eq!(s.mode, expected);
        assert!(s.source_retry_at.is_none());
        assert!(!s.accepts(old));
        s.capture_failed(fault("window_missing", "late"));
        assert_eq!(s.mode, expected);
    }
}
#[test]
fn successful_same_source_frame_clears_recovery_after_validation() {
    let mut s = state();
    let frame = s.latest.take().unwrap();
    let source = s.source.clone();
    s.capture_failed(fault(
        "capture_binding_changed",
        "window resized during read",
    ));
    assert!(s.accepts(s.generation));
    s.capture_succeeded(frame);
    assert_eq!(s.mode, Mode::Running);
    assert_eq!(s.source_retry_count, 0);
    assert!(s.last_error.is_none() && s.last_error_detail.is_none() && s.source_retry_at.is_none());
    assert_eq!(s.captured, 1);
    assert!(s.latest.is_some() && Arc::ptr_eq(&source, &s.source));
}
#[test]
fn identity_permissions_timeout_and_display_faults_never_auto_recover() {
    for code in [
        "session_or_process_changed",
        "screen_capture_permission_missing",
        "capture_timeout",
        "window_binding_ambiguous",
        "live_display_changed",
        "visual_display_unknown",
    ] {
        let mut s = state();
        s.capture_failed(fault(code, "retained native failure detail"));
        assert_eq!(s.mode, Mode::Paused);
        assert_eq!(s.source_retry_count, 0);
        assert!(s.source_retry_at.is_none() && s.latest.is_none());
        assert_eq!(s.last_error.as_deref(), Some(code));
        assert_eq!(
            s.last_error_detail.as_deref(),
            Some("retained native failure detail")
        );
    }
}
#[test]
fn recovery_obeys_expiry_and_host_visibility_without_extending_time() {
    let mut s = state();
    s.capture_failed(fault("window_missing", "temporary"));
    let deadline = s.deadline;
    s.host_view = Some(ViewGate::new("owned-recovery-view".into()));
    assert!(!s.presentation_allowed() && !s.accepts(s.generation));
    assert_eq!(s.deadline, deadline);
    s.deadline = Instant::now() - Duration::from_millis(1);
    s.expire();
    assert_eq!(s.mode, Mode::Stopped);
    assert_eq!(s.stop_reason.as_deref(), Some("duration_expired"));
    assert!(s.source_retry_at.is_none());
}
#[test]
fn recovery_never_overlaps_an_outstanding_native_capture() {
    let mut s = state();
    let op = rwi_core::operation::Registry::default()
        .begin(None)
        .unwrap();
    let native = rwi_core::operation::run(&op, || {
        rwi_core::operation::native_call("fixture_source_recovery").unwrap()
    });
    s.native_operation = Some(op.clone());
    s.capture_failed(fault("window_missing", "before callback release"));
    op.finish();
    assert!(s.native_pending());
    s.control(LiveAction::Stop);
    assert!(s.native_pending());
    drop(native);
    assert!(!s.native_pending());
}
