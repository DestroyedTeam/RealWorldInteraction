//! One bounded shared PNG retained after native presenter consumption.
//! Reads never capture, renew Host presence, select/focus a window or send input.
use super::*;
use rwi_core::live_image::{
    CachedLiveImage, LiveImageCursor, LiveImageQuery, LiveImageReply, MAX_LIVE_IMAGE_AGE_MS,
    MAX_LIVE_IMAGE_BYTES,
};

pub(super) struct CachedFrame {
    cursor: LiveImageCursor,
    image: Arc<VisualCapture>,
    received_at: Instant,
}
impl CachedFrame {
    pub(super) fn new(generation: u64, sequence: u64, image: Arc<VisualCapture>) -> Option<Self> {
        let png = &image.png;
        let [w, h] = image.image_size;
        // Producer is the existing native encoder; this is a bounded header check,
        // not a new untrusted PNG decoder or a claim of complete CRC validation.
        if png.len() < 24
            || png.len() > MAX_LIVE_IMAGE_BYTES
            || !png.starts_with(b"\x89PNG\r\n\x1a\n")
            || &png[12..16] != b"IHDR"
            || w == 0
            || h == 0
            || w > 2048
            || h > 2048
            || u32::from_be_bytes(png[16..20].try_into().ok()?) != w
            || u32::from_be_bytes(png[20..24].try_into().ok()?) != h
        {
            return None;
        }
        Some(Self {
            cursor: LiveImageCursor {
                generation,
                sequence,
            },
            image,
            received_at: Instant::now(),
        })
    }
}
impl Controls {
    pub(crate) fn latest_image(&self, query: LiveImageQuery) -> Result<LiveImageReply> {
        query.validate()?;
        let mut s = self
            .shared
            .state
            .lock()
            .map_err(|_| fault("live_state_unavailable", "Preview state unavailable."))?;
        s.expire();
        let gate = s.host_view.as_ref().ok_or_else(|| {
            fault(
                "live_image_unbound",
                "Cached images require an existing Host view binding.",
            )
        })?;
        if query.view_id != gate.id {
            return Err(fault(
                "live_view_mismatch",
                "The image request belongs to another Host view.",
            ));
        }
        if !s.accepts(s.generation) {
            return Err(fault(
                "live_image_unavailable",
                "Preview is hidden, paused, stopped, expired or awaiting real Host presence.",
            ));
        }
        let cache = s.export_latest.as_ref().ok_or_else(|| {
            fault(
                "live_image_pending",
                "No bounded current PNG is retained; no capture was started.",
            )
        })?;
        if cache.received_at.elapsed() > Duration::from_millis(MAX_LIVE_IMAGE_AGE_MS)
            || cache.cursor.generation != s.generation
            || cache.cursor.sequence != s.captured
        {
            s.export_latest = None;
            return Err(fault(
                "live_image_stale",
                "Cached PNG expired; no capture, retry or old-frame substitute.",
            ));
        }
        if query.after == Some(cache.cursor) {
            return Ok(LiveImageReply::Unchanged(cache.cursor));
        }
        Ok(LiveImageReply::Ready(CachedLiveImage {
            cursor: cache.cursor,
            image: cache.image.clone(),
        }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn query() -> LiveImageQuery {
        LiveImageQuery {
            view_id: "a".repeat(32),
            after: None,
        }
    }
    fn frame(s: &State) -> VisualCapture {
        let mut v = (**s.latest.as_ref().unwrap()).clone();
        let mut png = Vec::new();
        {
            let mut e = png::Encoder::new(&mut png, 1, 1);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header()
                .unwrap()
                .write_image_data(&[20, 50, 90, 255])
                .unwrap();
        }
        v.png = png;
        v
    }
    fn owned() -> Session {
        let session = super::super::tests::session();
        {
            let mut s = session.shared.state.lock().unwrap();
            s.deadline = Instant::now() + Duration::from_secs(30);
            s.host_view = Some(ViewGate::new("a".repeat(32)));
            s.host_view
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
            let v = frame(&s);
            s.capture_succeeded(v);
        }
        session
    }
    #[test]
    fn f30_live_image_survives_presenter_take_and_shares_bytes() {
        let session = owned();
        let controls = session.controller();
        let pending = session.shared.state.lock().unwrap().latest.take().unwrap();
        let LiveImageReply::Ready(reply) = controls.latest_image(query()).unwrap() else {
            panic!("ready")
        };
        assert!(Arc::ptr_eq(&pending, &reply.image));
        assert_eq!(reply.cursor.sequence, 1);
        assert_eq!(session.shared.state.lock().unwrap().captured, 1);
    }
    #[test]
    fn f30_live_image_wrong_or_unbound_view_refuses_without_state_change() {
        let session = owned();
        let controls = session.controller();
        let mut q = query();
        q.view_id = "b".repeat(32);
        assert_eq!(
            controls.latest_image(q).unwrap_err().code,
            "live_view_mismatch"
        );
        {
            let mut s = session.shared.state.lock().unwrap();
            assert_eq!(s.captured, 1);
            s.host_view = None;
        }
        assert_eq!(
            controls.latest_image(query()).unwrap_err().code,
            "live_image_unbound"
        );
    }
    #[test]
    fn f30_live_image_reads_do_not_renew_presence_or_capture() {
        let session = owned();
        let controls = session.controller();
        let until = session
            .shared
            .state
            .lock()
            .unwrap()
            .host_view
            .as_ref()
            .unwrap()
            .visible_until;
        for _ in 0..3 {
            assert!(matches!(
                controls.latest_image(query()),
                Ok(LiveImageReply::Ready(_))
            ));
        }
        let s = session.shared.state.lock().unwrap();
        assert_eq!(s.host_view.as_ref().unwrap().visible_until, until);
        assert_eq!(s.host_view.as_ref().unwrap().sequence, 1);
        assert_eq!(s.captured, 1);
        assert_eq!(s.presented, 0);
    }
    #[test]
    fn f30_live_image_same_cursor_has_no_png_reply() {
        let session = owned();
        let controls = session.controller();
        let LiveImageReply::Ready(first) = controls.latest_image(query()).unwrap() else {
            panic!("ready")
        };
        let mut q = query();
        q.after = Some(first.cursor);
        assert!(
            matches!(controls.latest_image(q).unwrap(),LiveImageReply::Unchanged(c)if c==first.cursor)
        );
    }
    #[test]
    fn f30_live_image_hide_pause_stop_and_expired_presence_revoke() {
        for action in [LiveAction::Hide, LiveAction::Pause, LiveAction::Stop] {
            let session = owned();
            session.shared.state.lock().unwrap().control(action);
            assert!(session.controller().latest_image(query()).is_err());
            assert!(session.shared.state.lock().unwrap().export_latest.is_none());
        }
        let session = owned();
        session
            .shared
            .state
            .lock()
            .unwrap()
            .host_view
            .as_mut()
            .unwrap()
            .visible_until = Some(Instant::now() - Duration::from_millis(1));
        assert!(session.controller().latest_image(query()).is_err());
        assert!(session.shared.state.lock().unwrap().export_latest.is_none());
    }
    #[test]
    fn f30_live_image_stale_cache_clears_without_capture_or_renewal() {
        let session = owned();
        session
            .shared
            .state
            .lock()
            .unwrap()
            .export_latest
            .as_mut()
            .unwrap()
            .received_at = Instant::now() - Duration::from_millis(MAX_LIVE_IMAGE_AGE_MS + 1);
        assert_eq!(
            session.controller().latest_image(query()).unwrap_err().code,
            "live_image_stale"
        );
        let s = session.shared.state.lock().unwrap();
        assert!(s.export_latest.is_none());
        assert_eq!(s.captured, 1);
        assert_eq!(s.host_view.as_ref().unwrap().sequence, 1);
    }
    #[test]
    fn f30_live_image_overbudget_or_malformed_next_frame_never_returns_old_png() {
        for malformed in [false, true] {
            let session = owned();
            {
                let mut s = session.shared.state.lock().unwrap();
                let mut next = frame(&s);
                if malformed {
                    next.png[12] = b'X';
                } else {
                    next.png.resize(MAX_LIVE_IMAGE_BYTES + 1, 0);
                }
                s.capture_succeeded(next);
                assert!(s.latest.is_some());
                assert!(s.export_latest.is_none());
            }
            assert_eq!(
                session.controller().latest_image(query()).unwrap_err().code,
                "live_image_pending"
            );
        }
    }
    #[test]
    fn f30_live_image_source_generation_rebind_clears_export() {
        let session = owned();
        let LiveImageReply::Ready(old) = session.controller().latest_image(query()).unwrap() else {
            panic!("ready")
        };
        session.source(NativeMac::fixture_for_live());
        let s = session.shared.state.lock().unwrap();
        assert!(s.generation > old.cursor.generation);
        assert!(s.export_latest.is_none());
        assert!(s.latest.is_none());
    }
}
