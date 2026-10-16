//! Native overlay versus Host image plumbing; no Host-render attestation or input.
use super::*;
pub(super) const HOST_DESCRIPTION: &str = "Host_component_cached_image; Host_render_unattested";
pub(super) fn description(options: &LiveOptions) -> &'static str {
    if options.host_image_only {
        HOST_DESCRIPTION
    } else {
        "RWI_Agent_native_nonactivating_panel"
    }
}
pub(super) fn preview_display(
    options: &LiveOptions,
    enumerate: impl FnOnce() -> Result<Vec<Display>>,
) -> Result<Option<Display>> {
    options.validate()?;
    if options.host_image_only {
        return Ok(None);
    }
    let wanted = options.display.as_ref().ok_or_else(|| {
        fault(
            "live_display_missing",
            "Native previews need one explicit display.",
        )
    })?;
    let mut matches = enumerate()?
        .into_iter()
        .filter(|d| wanted.matches(d.built_in, d.id, d.uuid.as_deref()))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(fault(
            "live_display_ambiguous",
            "Select one current preview display by UUID or native id.",
        ));
    }
    let display = matches.remove(0);
    if display.rect.w < 300. || display.rect.h < 300. {
        return Err(fault(
            "live_display_too_small",
            "The allocated preview display must fit a 300-point panel.",
        ));
    }
    Ok(Some(display))
}
pub(super) fn check_topology(
    display: Option<&Display>,
    enumerate: impl FnOnce() -> Result<Vec<Display>>,
) -> Result<()> {
    if let Some(display) = display
        && !enumerate()?.contains(display)
    {
        return Err(fault(
            "live_display_changed",
            "Preview display topology changed; restart with a fresh display selection.",
        ));
    }
    Ok(())
}
impl State {
    /// Presenter tick may drop its unused pending slot, never the shared export.
    pub(super) fn release_host_pending_without_overlay(&mut self) -> bool {
        if !self.options.host_image_only {
            return false;
        }
        self.latest = None;
        self.visible = false;
        self.image_present = false;
        self.window_id = None;
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use rwi_core::live_image::{LiveImageQuery, LiveImageReply};
    fn host() -> Session {
        let s = super::super::tests::session();
        {
            let mut v = s.shared.state.lock().unwrap();
            v.options = serde_json::from_value(
                json!({"host_image_only":true,"host_view_id":"a".repeat(32),"duration_ms":1000}),
            )
            .unwrap();
            v.display = None;
            v.host_view = Some(ViewGate::new("a".repeat(32)));
            v.host_view
                .as_mut()
                .unwrap()
                .update(
                    &rwi_core::LiveViewUpdate {
                        view_id: "a".repeat(32),
                        sequence: 1,
                        visible: true,
                    },
                    Instant::now(),
                )
                .unwrap();
            v.visible = false;
            v.image_present = false;
            v.window_id = None;
        }
        s
    }
    fn seed(s: &Session) {
        let mut v = s.shared.state.lock().unwrap();
        let mut frame = Arc::unwrap_or_clone(v.latest.clone().unwrap());
        // Synthetic header for the pure cache guard only, not a decoded PNG/GUI.
        frame.png = vec![0; 24];
        frame.png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        frame.png[12..16].copy_from_slice(b"IHDR");
        frame.png[16..20].copy_from_slice(&1_u32.to_be_bytes());
        frame.png[20..24].copy_from_slice(&1_u32.to_be_bytes());
        v.capture_succeeded(frame);
    }
    fn query() -> LiveImageQuery {
        LiveImageQuery {
            view_id: "a".repeat(32),
            after: None,
        }
    }
    #[test]
    fn f36_host_image_mode_never_enumerates_viewer_displays() {
        let s = host();
        let v = s.shared.state.lock().unwrap();
        assert!(
            preview_display(&v.options, || panic!(
                "Host images must not enumerate viewer displays"
            ))
            .unwrap()
            .is_none()
        );
    }
    #[test]
    fn f36_host_image_legacy_native_display_match_and_size_stay_strict() {
        let s = super::super::tests::session();
        let v = s.shared.state.lock().unwrap();
        let display = v.display.clone().unwrap();
        assert_eq!(
            preview_display(&v.options, || Ok(vec![display.clone()])).unwrap(),
            Some(display.clone())
        );
        assert!(preview_display(&v.options, || Ok(vec![])).is_err());
        assert!(
            preview_display(&v.options, || Ok(vec![display.clone(), display.clone()])).is_err()
        );
        let mut small = display;
        small.rect.w = 299.;
        assert_eq!(
            preview_display(&v.options, || Ok(vec![small]))
                .unwrap_err()
                .code,
            "live_display_too_small"
        );
    }
    #[test]
    fn f36_host_image_topology_check_ignores_viewer_not_source_identity() {
        check_topology(None, || panic!("no viewer topology dependency")).unwrap();
        let s = super::super::tests::session();
        let d = s.shared.state.lock().unwrap().display.clone().unwrap();
        assert_eq!(
            check_topology(Some(&d), || Ok(vec![])).unwrap_err().code,
            "live_display_changed"
        );
        check_topology(Some(&d), || Ok(vec![d.clone()])).unwrap();
    }
    #[test]
    fn f36_host_image_pending_release_keeps_exact_same_export_arc_and_cursor() {
        let s = host();
        seed(&s);
        let LiveImageReply::Ready(before) = s.latest_image(query()).unwrap() else {
            panic!("fixture must be ready")
        };
        {
            let mut v = s.shared.state.lock().unwrap();
            assert!(v.release_host_pending_without_overlay());
            assert!(v.latest.is_none());
            assert!(v.export_latest.is_some());
            assert_eq!(v.mode, Mode::Running);
            assert!(!v.visible && !v.image_present && v.window_id.is_none());
        }
        let LiveImageReply::Ready(after) = s.latest_image(query()).unwrap() else {
            panic!("export lost")
        };
        assert!(Arc::ptr_eq(&before.image, &after.image));
        assert_eq!(before.cursor, after.cursor);
    }
    #[test]
    fn f36_host_image_hide_pause_stop_and_presence_expiry_still_revoke_cache() {
        for action in [
            Some(LiveAction::Hide),
            Some(LiveAction::Pause),
            Some(LiveAction::Stop),
            None,
        ] {
            let s = host();
            seed(&s);
            {
                let mut v = s.shared.state.lock().unwrap();
                v.release_host_pending_without_overlay();
                if let Some(a) = action {
                    v.control(a);
                } else {
                    v.host_view.as_mut().unwrap().visible_until = Some(Instant::now());
                    v.expire();
                }
                assert!(v.export_latest.is_none());
            }
            assert!(s.latest_image(query()).is_err());
        }
    }
    #[test]
    fn f36_host_image_status_has_no_native_viewer_or_false_host_render_claim() {
        let s = host();
        seed(&s);
        s.shared
            .state
            .lock()
            .unwrap()
            .release_host_pending_without_overlay();
        let status = s.status();
        assert_eq!(status["presentation"], HOST_DESCRIPTION);
        assert!(status["requested_preview_display"].is_null());
        assert_eq!(status["presenter_visible"], false);
        assert_eq!(status["presenter_image_present"], false);
        assert!(status["presenter_window"].is_null());
        assert_eq!(status["frames_presented"], 0);
        assert!(status.get("Host_render_verified").is_none());
        assert_eq!(status["input_sent"], false);
    }
    #[test]
    fn f36_host_image_readback_expects_no_native_window_in_all_modes() {
        let s = host();
        for mode in [
            Mode::Running,
            Mode::Recovering,
            Mode::Paused,
            Mode::Hidden,
            Mode::Stopped,
        ] {
            s.shared.state.lock().unwrap().mode = mode;
            let status = s.status();
            assert!(presentation_matches(&status, None));
            assert!(!presentation_matches(
                &status,
                Some(crate::geometry::Rect {
                    x: 0.,
                    y: 0.,
                    w: 10.,
                    h: 10.
                })
            ));
        }
    }
    #[test]
    fn f36_host_image_pending_release_never_changes_legacy_native_state() {
        let s = super::super::tests::session();
        let mut v = s.shared.state.lock().unwrap();
        let pending = v.latest.clone().unwrap();
        assert!(!v.release_host_pending_without_overlay());
        assert!(Arc::ptr_eq(v.latest.as_ref().unwrap(), &pending));
        assert!(v.visible && v.image_present && v.window_id == Some(99));
    }
}
