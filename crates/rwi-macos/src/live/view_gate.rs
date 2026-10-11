//! Short-lived Host-view presence, independent of user pause and worker lifetime.
use super::*;
pub(super) const VIEW_LEASE: Duration = Duration::from_millis(1500);
pub(super) struct ViewGate {
    pub id: String,
    pub sequence: u64,
    pub visible_until: Option<Instant>,
}
impl ViewGate {
    pub fn new(id: String) -> Self {
        Self {
            id,
            sequence: 0,
            visible_until: None,
        }
    }
    pub fn visible(&self, now: Instant) -> bool {
        self.visible_until.is_some_and(|until| now < until)
    }
    pub fn expire(&mut self, now: Instant) -> bool {
        if self.visible_until.is_some_and(|until| now >= until) {
            self.visible_until = None;
            true
        } else {
            false
        }
    }
    pub fn update(&mut self, update: &rwi_core::LiveViewUpdate, now: Instant) -> Result<bool> {
        update.validate()?;
        if update.view_id != self.id {
            return Err(fault(
                "live_view_mismatch",
                "The visibility event belongs to another preview generation.",
            ));
        }
        if update.sequence <= self.sequence {
            return Ok(false);
        }
        self.sequence = update.sequence;
        self.visible_until = update.visible.then_some(now + VIEW_LEASE);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn update(sequence: u64, visible: bool) -> rwi_core::LiveViewUpdate {
        rwi_core::LiveViewUpdate {
            view_id: "a".repeat(32),
            sequence,
            visible,
        }
    }
    #[test]
    fn late_visible_cannot_override_hide_or_extend_an_expired_lease() {
        let now = Instant::now();
        let mut g = ViewGate::new("a".repeat(32));
        assert!(!g.visible(now));
        assert!(g.update(&update(1, true), now).unwrap());
        assert!(g.visible(now));
        assert!(g.update(&update(3, false), now).unwrap());
        assert!(!g.update(&update(2, true), now).unwrap());
        assert!(!g.visible(now));
        g.update(&update(4, true), now).unwrap();
        assert!(g.expire(now + VIEW_LEASE));
        assert!(!g.update(&update(4, true), now + VIEW_LEASE).unwrap());
        assert!(!g.visible(now + VIEW_LEASE));
        assert!(g.update(&update(5, true), now + VIEW_LEASE).unwrap());
        assert!(g.visible(now + VIEW_LEASE));
    }
    #[test]
    fn another_binding_never_mutates_the_gate() {
        let mut g = ViewGate::new("b".repeat(32));
        assert!(g.update(&update(1, true), Instant::now()).is_err());
        assert_eq!(g.sequence, 0);
        assert!(g.visible_until.is_none());
    }
}
