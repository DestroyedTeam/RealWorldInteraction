//! Explicit paired background gesture. Ordinary drag stays window-contained.
use super::*;
use rwi_core::{
    drag_between::{Endpoint, Pair, Surface},
    input_trajectory::{self, TrajectorySink},
};
use std::time::Instant;

impl NativeMac {
    pub(crate) fn paired_drag_surface(
        &mut self,
        endpoint: &Endpoint,
        initial: bool,
    ) -> Result<Surface> {
        operation::checkpoint("macos_paired_drag_endpoint")?;
        if !self.allow_input
            || self.semantic_only
            || endpoint.identity != self.identity
            || endpoint.resolved.scope.pid != self.pid
            || endpoint.resolved.scope.window != self.window_name()
        {
            return Err(fault(
                "drag_endpoint_mismatch",
                "Exact input-authorized visual native endpoint required.",
            ));
        }
        if initial && let Some(seconds) = crate::activity::user_active(1.2) {
            let (pid, focused) = self.foreground_input_window();
            if crate::activity::same_input_resource(self.pid, self.window, pid, focused, false) {
                return Err(fault(
                    "user_active",
                    &format!(
                        "Recent native activity ({seconds:.2}s) conflicts with a paired endpoint; yield, no replay."
                    ),
                ));
            }
        }
        if initial {
            self.drag_anchor_facts(&endpoint.resolved)?.validate(
                &endpoint.binding,
                &endpoint.resolved,
                Kind::Pointer,
            )?;
        }
        let meta = self.capture_guard_metadata()?;
        if meta.identity != endpoint.identity
            || !meta.mapping_ready
            || meta.blocked != Some(false)
            || meta.binding.native_epoch != endpoint.binding.native_epoch
            || meta.binding.layout != endpoint.binding.layout
            || !unsafe { ffi::CGPreflightPostEventAccess() }
            || !unsafe { ffi::CGPreflightListenEventAccess() }
        {
            return Err(fault(
                "drag_endpoint_changed",
                "Endpoint context/process/window/title/geometry/modal/layout/TCC changed; stop and release.",
            ));
        }
        Ok(Surface {
            identity: meta.identity,
            scope: meta.binding.scope,
            process_start: self.process.clone(),
            native_epoch: meta.binding.native_epoch,
            layout: meta.binding.layout,
            bounds: [meta.rect.x, meta.rect.y, meta.rect.w, meta.rect.h],
        })
    }
    pub(crate) fn dispatch_drag_between(
        &mut self,
        pair: &Pair,
        destination: &mut dyn Backend,
    ) -> Result<Option<bool>> {
        if !crate::background::pointer_delivery_available() {
            return Err(fault(
                "background_delivery_unavailable",
                "Paired route has no native window addressing; no activation/global fallback.",
            ));
        }
        let source = self.paired_drag_surface(&pair.source, true)?;
        let peer = destination.drag_surface(&pair.destination, true)?;
        if source.identity != peer.identity || source.layout != peer.layout {
            return Err(fault(
                "drag_domain_mismatch",
                "Paired endpoints require one unchanged native desktop/layout.",
            ));
        }
        let all_displays = displays()?;
        let delivery = crate::background::Delivery {
            pid: self.pid,
            window: self.window,
            frame: Rect {
                x: source.bounds[0],
                y: source.bounds[1],
                w: source.bounds[2],
                h: source.bounds[3],
            },
        };
        let gesture = MouseGesture::new(pair.source.resolved.point, CGMouseButton::Left)?;
        let mut sink = Sink {
            native: self,
            destination,
            pair,
            source,
            peer,
            all_displays,
            delivery,
            gesture,
            started: Instant::now(),
        };
        input_trajectory::execute(&pair.path, pair.source.resolved.point, &mut sink)?;
        Ok(None)
    }
}

struct Sink<'a> {
    native: &'a mut NativeMac,
    destination: &'a mut dyn Backend,
    pair: &'a Pair,
    source: Surface,
    peer: Surface,
    all_displays: Vec<Display>,
    delivery: crate::background::Delivery,
    gesture: MouseGesture,
    started: Instant,
}
impl TrajectorySink for Sink<'_> {
    fn sample_interval_ms(&self) -> u64 {
        drag::NATIVE_SAMPLE_INTERVAL_MS
    }
    fn now_ms(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
    fn sleep_ms(&mut self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
    fn validate_point(&mut self, point: [f64; 2]) -> Result<()> {
        if !point.iter().all(|n| n.is_finite())
            || self
                .all_displays
                .iter()
                .filter(|d| d.rotation.abs() < 0.001 && d.rect.contains(point))
                .count()
                != 1
        {
            return Err(fault(
                "drag_point_outside_desktop",
                "Every paired sample requires exactly one active unrotated display; no clamp.",
            ));
        }
        Ok(())
    }
    fn guard_point(&mut self, point: [f64; 2], _held: bool) -> Result<()> {
        self.validate_point(point)?;
        // Initial endpoint admission already checked recent activity and full
        // anchors. Our primer/held events reset the combined OS activity clock;
        // do not misclassify them as another human interaction. Native identity,
        // geometry, modal state, permissions and cancellation remain guarded.
        if displays()? != self.all_displays
            || self.native.paired_drag_surface(&self.pair.source, false)? != self.source
            || self
                .destination
                .drag_surface(&self.pair.destination, false)?
                != self.peer
        {
            return Err(fault(
                "drag_pair_changed",
                "A paired endpoint or display changed; release once without switching/replay.",
            ));
        }
        Ok(())
    }
    fn press_left(&mut self, point: [f64; 2]) -> Result<()> {
        let moved = self.gesture.tracking_event()?;
        dispatch::count(&self.native.dispatches);
        crate::background::deliver_pointer(&moved, &self.delivery, point);
        std::thread::sleep(Duration::from_millis(12));
        self.guard_point(point, false)?;
        let down = self.gesture.down_event()?;
        dispatch::count(&self.native.dispatches);
        crate::background::deliver_pointer(&down, &self.delivery, point);
        Ok(())
    }
    fn move_held(&mut self, point: [f64; 2]) -> Result<()> {
        let (event, motion) = self.gesture.motion_event(point)?;
        dispatch::count(&self.native.dispatches);
        // Keep the initiating receiver/window. AppKit's native drag manager, not
        // a fabricated event sent to the destination, owns pasteboard routing.
        crate::background::deliver_pointer(&event, &self.delivery, point);
        self.gesture.commit_motion(motion);
        Ok(())
    }
    fn release_left(&mut self) -> Result<()> {
        let event = self.gesture.release_event()?;
        dispatch::count(&self.native.dispatches);
        crate::background::deliver_pointer(&event, &self.delivery, self.gesture.last_point());
        Ok(())
    }
}
