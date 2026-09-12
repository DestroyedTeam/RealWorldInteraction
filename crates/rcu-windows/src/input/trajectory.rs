//! Native sink for one bounded left-button trajectory. No shell or UIA mutation.
use super::*;
use crate::input_trajectory::TrajectorySink;

impl TrajectorySink for NativeSink<'_> {
    fn now_ms(&self) -> u64 {
        self.trajectory_clock.elapsed().as_millis() as u64
    }
    fn sleep_ms(&mut self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
    fn validate_point(&mut self, point: [f64; 2]) -> Result<()> {
        move_to(point)?;
        let r = self
            .window
            .dwm_visible_frame
            .or(self.window.rectangle)
            .ok_or_else(|| {
                fault(
                    "drag_bounds_unknown",
                    "Native fixed-window bounds are required before dragging.",
                )
            })?;
        if point[0] < f64::from(r.x)
            || point[1] < f64::from(r.y)
            || point[0] >= f64::from(r.x) + f64::from(r.width)
            || point[1] >= f64::from(r.y) + f64::from(r.height)
        {
            return Err(fault(
                "drag_outside_window",
                "All samples must stay in the same observed fixed-geometry native window; no input.",
            ));
        }
        Ok(())
    }
    fn guard_point(&mut self, point: [f64; 2], owned_left: bool) -> Result<()> {
        self.current_point = point;
        self.owned_left = owned_left;
        self.scope.set_point(point);
        input_plan::guard_before_batch(self)
    }
    fn press_left(&mut self, point: [f64; 2]) -> Result<()> {
        self.send_drag_events(&[move_to(point)?, mouse(MOUSEEVENTF_LEFTDOWN, 0, 0, 0)])
    }
    fn move_held(&mut self, point: [f64; 2]) -> Result<()> {
        let mut event = move_to(point)?;
        unsafe {
            event.Anonymous.mi.dwFlags |= MOUSEEVENTF_MOVE_NOCOALESCE;
        }
        self.send_drag_events(&[event])
    }
    fn release_left(&mut self) -> Result<()> {
        // Deliberately no cancellation/focus guard and no down/move retry here.
        // A provider stall before this return still retains the operation fence.
        self.counters.release_cleanup_calls += 1;
        let inserted = unsafe {
            SendInput(
                &[mouse(MOUSEEVENTF_LEFTUP, 0, 0, 0)],
                size_of::<INPUT>() as i32,
            )
        };
        self.counters.release_events_inserted += u64::from(inserted);
        self.owned_left = false;
        if inserted == 1 {
            Ok(())
        } else {
            Err(fault(
                "drag_release_unconfirmed",
                "SendInput did not acknowledge the release; inspect native state, never replay.",
            ))
        }
    }
}

impl NativeSink<'_> {
    fn send_drag_events(&mut self, events: &[INPUT]) -> Result<()> {
        checkpoint("before_drag_sendinput")?;
        let call = native_call("sendinput_trajectory_sample")?;
        self.counters.sendinput_calls += 1;
        let inserted = unsafe { SendInput(events, size_of::<INPUT>() as i32) };
        self.counters.events_inserted += u64::from(inserted);
        drop(call);
        self.first_batch = false;
        if inserted == events.len() as u32 {
            Ok(())
        } else {
            Err(fault(
                "input_partial_unconfirmed",
                &format!(
                    "Trajectory inserted {inserted}/{} events; release-only cleanup follows, no replay.",
                    events.len()
                ),
            ))
        }
    }
}
