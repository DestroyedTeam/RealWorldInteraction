//! Ephemeral page-scoped image admission. Reads never renew presence or lifetime.
use super::*;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
const PRESENCE_TTL: Duration = Duration::from_millis(1500);
const INTERVAL: Duration = Duration::from_millis(250);
#[derive(Default)]
pub(super) struct State {
    sequence: u64,
    epoch: u64,
    busy: Option<u64>,
    next_read: Option<Instant>,
    visible_until: Option<Instant>,
}
impl State {
    pub(super) fn presence(&mut self, sequence: u64, shown: bool, now: Instant) {
        if !shown || !self.visible(now) {
            self.epoch = sequence;
        }
        self.visible_until = shown.then_some(now + PRESENCE_TTL);
    }
    fn visible(&self, now: Instant) -> bool {
        self.visible_until.is_some_and(|until| now < until)
    }
}
pub(super) struct Readers(Arc<Semaphore>);
impl Default for Readers {
    fn default() -> Self {
        Self(Arc::new(Semaphore::new(2)))
    }
}
pub(in crate::gateway) struct Ticket {
    views: Arc<Views>,
    key: String,
    page: String,
    epoch: u64,
    sequence: u64,
    pub(in crate::gateway) target: String,
    pub(in crate::gateway) binding: String,
    _permit: OwnedSemaphorePermit,
}
impl Views {
    pub(in crate::gateway) fn image_read(
        self: &Arc<Self>,
        key: &str,
        page: &str,
        sequence: u64,
    ) -> Result<Ticket, &'static str> {
        let permit = self
            .image_readers
            .0
            .clone()
            .try_acquire_owned()
            .map_err(|_| "live_image_busy")?;
        let mut entries = self.entries.lock().map_err(|_| "live_view_unavailable")?;
        let v = entries.get_mut(key).ok_or("live_view_ended")?;
        let now = Instant::now();
        if v.expires <= now || v.page.as_deref() != Some(page) {
            return Err("live_view_ended");
        }
        if !v.image.visible(now) {
            return Err("live_image_hidden");
        }
        if sequence == 0 || sequence > 9_007_199_254_740_991 || sequence <= v.image.sequence {
            return Err("stale_image_request");
        }
        if v.image.busy.is_some() {
            return Err("live_image_busy");
        }
        if v.image.next_read.is_some_and(|until| until > now) {
            return Err("live_image_rate_limited");
        }
        v.image.sequence = sequence;
        v.image.busy = Some(sequence);
        v.image.next_read = Some(now + INTERVAL);
        Ok(Ticket {
            views: self.clone(),
            key: key.into(),
            page: page.into(),
            epoch: v.image.epoch,
            sequence,
            target: v.target.clone(),
            binding: v.binding.clone(),
            _permit: permit,
        })
    }
}
impl Ticket {
    pub(in crate::gateway) fn current(&self) -> bool {
        self.views.entries.lock().is_ok_and(|entries| {
            entries.get(&self.key).is_some_and(|v| {
                let now = Instant::now();
                v.expires > now
                    && v.page.as_deref() == Some(&self.page)
                    && v.image.visible(now)
                    && v.image.epoch == self.epoch
                    && v.image.busy == Some(self.sequence)
            })
        })
    }
}
impl Drop for Ticket {
    fn drop(&mut self) {
        if let Ok(mut entries) = self.views.entries.lock()
            && let Some(v) = entries.get_mut(&self.key)
            && v.page.as_deref() == Some(&self.page)
            && v.image.busy == Some(self.sequence)
        {
            v.image.busy = None;
        }
    }
}
#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;
