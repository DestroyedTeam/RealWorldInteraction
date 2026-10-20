//! One device generation covers both lanes; no lock is held across network I/O.
use super::*;

#[derive(Default)]
pub(super) struct Generation {
    // Pointer identity cannot wrap/reuse while an older checkout retains its Arc.
    token: Arc<()>,
    instance: Option<String>,
    // At most one provisional origin. First binding must not cancel a healthy
    // original request whose TLS handshake is still completing. This token may
    // start its original request, but can never put into or retire the bound pool.
    first_bind_origin: Option<Arc<()>>,
}
impl Shared {
    pub(super) fn checkout(&self, lane: &Lane, fresh: bool) -> (Option<Connection>, Arc<()>) {
        let generation = self.generation.lock().unwrap_or_else(|e| e.into_inner());
        let connection = if fresh {
            None
        } else {
            let mut idle = lane.idle.lock().unwrap_or_else(|e| e.into_inner());
            idle.retain(|c| {
                Arc::ptr_eq(&c.generation, &generation.token)
                    && c.idle_since.elapsed() < CLIENT_IDLE_TIMEOUT
            });
            idle.pop()
        };
        (connection, generation.token.clone())
    }
    #[cfg(all(test, unix))]
    pub(super) fn is_current(&self, token: &Arc<()>) -> bool {
        let generation = self.generation.lock().unwrap_or_else(|e| e.into_inner());
        Arc::ptr_eq(token, &generation.token)
    }
    pub(super) fn send_permitted(&self, token: &Arc<()>) -> bool {
        let generation = self.generation.lock().unwrap_or_else(|e| e.into_inner());
        Arc::ptr_eq(token, &generation.token)
            || generation
                .first_bind_origin
                .as_ref()
                .is_some_and(|origin| Arc::ptr_eq(token, origin))
    }
    pub(super) fn retire(&self, token: &Arc<()>) {
        let mut generation = self.generation.lock().unwrap_or_else(|e| e.into_inner());
        if Arc::ptr_eq(token, &generation.token) {
            self.retire_locked(&mut generation);
        } else if generation
            .first_bind_origin
            .as_ref()
            .is_some_and(|origin| Arc::ptr_eq(token, origin))
        {
            // A failed provisional connection cannot clear the winning pool,
            // but its remaining provisional siblings lose their send permit.
            generation.first_bind_origin = None;
        }
    }
    fn retire_locked(&self, generation: &mut Generation) {
        *generation = Generation::default();
        self.work
            .idle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.control
            .idle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.pool_invalidations.fetch_add(1, Ordering::Relaxed);
    }
    pub(super) fn release(&self, lane: &Lane, mut connection: Connection, instance: &str) {
        let mut generation = self.generation.lock().unwrap_or_else(|e| e.into_inner());
        if !Arc::ptr_eq(&connection.generation, &generation.token) {
            if generation
                .first_bind_origin
                .as_ref()
                .is_some_and(|origin| Arc::ptr_eq(&connection.generation, origin))
                && generation.instance.as_deref() != Some(instance)
            {
                // An observed conflicting provisional instance also ends the
                // no-fault send lineage, without replacing the winning pool.
                generation.first_bind_origin = None;
            }
            self.retired_returns_discarded
                .fetch_add(1, Ordering::Relaxed);
            return;
        }
        if generation.instance.is_none() {
            // First binding is also a barrier: other fresh connections may
            // still be negotiating or returning their first reply for an older
            // agent. Only this winning connection receives the bound token.
            // Their eventual success/error must neither replace nor retire it.
            generation.first_bind_origin = Some(generation.token.clone());
            generation.token = Arc::new(());
            connection.generation = generation.token.clone();
        } else if generation
            .instance
            .as_deref()
            .is_some_and(|old| old != instance)
        {
            // A new authenticated stream may legitimately observe a restarted
            // agent; all other streams from the old service are now retired.
            self.retire_locked(&mut generation);
            connection.generation = generation.token.clone();
        }
        generation.instance = Some(instance.into());
        connection.instance = Some(instance.into());
        if connection.persistent {
            connection.idle_since = Instant::now();
            lane.idle
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(connection);
        }
    }
}
