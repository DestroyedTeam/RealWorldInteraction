//! Bounded, context-local on-screen catalog. Fingerprints are not window birth IDs.
#[cfg(test)]
use crate::geometry::Rect;
use crate::native::Window;
use rwi_core::{DiscoveryQuery, Identity, Result, WindowRelation, WindowSelection, digest, fault};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const TTL: Duration = Duration::from_secs(10);
const PAGE_ROWS: usize = 8;
const PAGE_BYTES: usize = 12 * 1024;
const MAX_SNAPSHOTS: usize = 8;
const MAX_TOTAL_ROWS: usize = 2048;
static NEXT_CONTEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct WindowRecord {
    pub window: String,
    pub pid: u32,
    pub process_start: String,
    pub native_epoch: String,
    pub bounds: [f64; 4],
    pub title: Option<String>,
    pub title_truncated: bool,
    /// The single unrotated active display containing this window, when it is
    /// unambiguous. `built_in` distinguishes the laptop panel from external
    /// displays so callers can honor a screen allocation exactly.
    pub display: Option<DisplayRecord>,
    /// No capture/pixel input authority; only advertised AX actions.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub semantic_only: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(crate) struct DisplayRecord {
    pub id: u32,
    /// ColorSync UUID when available; unlike CGDirectDisplayID this can be
    /// used to rediscover a BetterDisplay virtual display after reconnect.
    pub uuid: Option<String>,
    pub built_in: bool,
    pub bounds: [f64; 4],
    pub pixels: [u64; 2],
    pub rotation: f64,
}
/// Optional call-site policy: a window may only be bound when its containing
/// display matches. Windows without an unambiguous display never match.
#[derive(Clone, Copy, Debug)]
pub struct AuthorizedDisplay {
    pub built_in: bool,
    pub id: Option<u32>,
    /// Whether binding may implicitly move a window onto the authorized
    /// display. Off by default: placement is a side effect and must be
    /// requested explicitly (incident 2026-09-15).
    pub place_on_bind: bool,
}
impl AuthorizedDisplay {
    pub(crate) fn allows(&self, display: Option<&DisplayRecord>) -> bool {
        display.is_some_and(|display| {
            display.built_in == self.built_in && self.id.is_none_or(|id| display.id == id)
        })
    }
}
impl DisplayRecord {
    pub(crate) fn from_native(display: &crate::native::Display) -> Self {
        Self {
            id: display.id,
            uuid: display.uuid.clone(),
            built_in: display.built_in,
            bounds: [
                display.rect.x,
                display.rect.y,
                display.rect.w,
                display.rect.h,
            ],
            pixels: display.pixels,
            rotation: display.rotation,
        }
    }
}
impl WindowRecord {
    pub(crate) fn from_native(
        identity: &Identity,
        w: &Window,
        process: String,
        display: Option<&crate::native::Display>,
    ) -> Result<Self> {
        if !w.rect.valid() || !w.alpha.is_finite() {
            return Err(fault(
                "window_geometry_unknown",
                "Window geometry cannot be safely catalogued.",
            ));
        }
        let display = display.map(DisplayRecord::from_native);
        // Display reconnection, scale or placement changes invalidate the same
        // observed window just like geometry/title/process changes do.
        let native_epoch = digest(
            format!(
                "{}:{process}:{}:{:?}:{}:{:?}:{display:?}",
                identity.key(),
                w.id,
                w.rect,
                w.alpha,
                w.title
            )
            .as_bytes(),
        );
        let title = w.title.as_ref().map(|title| {
            let mut end = title.len().min(1024);
            while !title.is_char_boundary(end) {
                end -= 1;
            }
            title[..end].to_owned()
        });
        Ok(Self {
            window: format!("cgwindow:{}", w.id),
            pid: w.pid,
            process_start: process,
            native_epoch,
            bounds: [w.rect.x, w.rect.y, w.rect.w, w.rect.h],
            title_truncated: w.title.as_ref().is_some_and(|t| t.len() > 1024),
            title,
            display,
            semantic_only: false,
        })
    }
    pub(crate) fn logical(mut self) -> Self {
        self.semantic_only = true;
        self.native_epoch = digest(format!("{}:focused_ax_dialog", self.native_epoch).as_bytes());
        self
    }
    pub(crate) fn require_same(&self, current: &Self) -> Result<()> {
        if self != current {
            return Err(fault(
                "window_epoch_changed",
                "Observed CGWindowID/PID/process/bounds/title/display fingerprint changed; discover and select explicitly.",
            ));
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn rect(&self) -> Rect {
        let [x, y, w, h] = self.bounds;
        Rect { x, y, w, h }
    }
}

struct Snapshot {
    rows: Vec<WindowRecord>,
    at: Instant,
    id: String,
    header: Value,
}
impl Snapshot {
    fn fresh(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.at) <= TTL
    }
    fn page(&self, offset: usize) -> Result<Value> {
        if offset > self.rows.len() || (offset == self.rows.len() && offset != 0) {
            return Err(fault(
                "invalid_catalog_cursor",
                "Cursor is outside this bounded catalog.",
            ));
        }
        let mut count = (self.rows.len() - offset).min(PAGE_ROWS);
        loop {
            let mut page = self.header.clone();
            let end = offset + count;
            page["windows"] = json!(&self.rows[offset..end]);
            page["catalog_id"] = json!(self.id);
            page["next_cursor"] = if end < self.rows.len() {
                json!(format!("{}/{end}", self.id))
            } else {
                Value::Null
            };
            page["matching_windows"] = json!(self.rows.len());
            page["complete"] = json!(
                offset == 0 && end == self.rows.len() && page["enumeration_complete"] == true
            );
            page["catalog_ttl_ms"] = json!(TTL.as_millis() as u64);
            page["catalog_cache"] = json!({
                "mode": "multi_snapshot",
                "max_snapshots": MAX_SNAPSHOTS,
                "max_total_rows": MAX_TOTAL_ROWS,
                "per_snapshot_ttl_ms": TTL.as_millis() as u64,
            });
            if serde_json::to_vec(&page)
                .map_err(|_| fault("catalog_encoding", "Invalid native catalog metadata."))?
                .len()
                <= PAGE_BYTES
            {
                return Ok(page);
            }
            if count <= 1 {
                return Err(fault(
                    "catalog_page_budget",
                    "One catalog record/header exceeds the public 12 KiB budget.",
                ));
            }
            count -= 1;
        }
    }
}

/// Discovery queries retain separate snapshots in this desktop context. Neither
/// a later query nor a page read refreshes the original snapshot's authority.
pub(crate) struct Catalog {
    snapshots: VecDeque<Snapshot>,
    context: String,
    sequence: u64,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            snapshots: VecDeque::new(),
            context: digest(
                format!(
                    "{:?}:{}",
                    std::time::SystemTime::now(),
                    NEXT_CONTEXT.fetch_add(1, Ordering::Relaxed)
                )
                .as_bytes(),
            ),
            sequence: 0,
        }
    }
}
impl Catalog {
    pub(crate) fn replace(&mut self, rows: Vec<WindowRecord>, header: Value) -> Result<Value> {
        if rows.len() > MAX_TOTAL_ROWS
            || rows
                .iter()
                .enumerate()
                .any(|(i, w)| rows[..i].iter().any(|p| p.window == w.window))
        {
            return Err(fault(
                "catalog_identity_ambiguous",
                "Native catalog exceeds its bound or repeats a CGWindowID; no binding is accepted.",
            ));
        }
        if !header.is_object() {
            return Err(fault(
                "catalog_encoding",
                "Native catalog metadata must be an object.",
            ));
        }
        if serde_json::to_vec(&header)
            .map_err(|_| fault("catalog_encoding", "Invalid native catalog metadata."))?
            .len()
            > PAGE_BYTES
        {
            return Err(fault(
                "catalog_page_budget",
                "Native catalog metadata exceeds the public 12 KiB budget.",
            ));
        }
        let sequence = self.sequence.checked_add(1).ok_or_else(|| {
            fault(
                "catalog_sequence_exhausted",
                "Create a new desktop context before discovering again.",
            )
        })?;
        let now = Instant::now();
        let candidate = Snapshot {
            rows,
            at: now,
            id: digest(format!("{}:{sequence}", self.context).as_bytes()),
            header,
        };
        // Validate before pruning/eviction so refused metadata never destroys
        // a previously returned cursor or selectable observation.
        let page = candidate.page(0)?;
        self.snapshots.retain(|snapshot| snapshot.fresh(now));
        let mut retained_rows: usize = self.snapshots.iter().map(|s| s.rows.len()).sum();
        while self.snapshots.len() >= MAX_SNAPSHOTS
            || retained_rows + candidate.rows.len() > MAX_TOTAL_ROWS
        {
            if let Some(oldest) = self.snapshots.pop_front() {
                retained_rows -= oldest.rows.len();
            }
        }
        self.snapshots.push_back(candidate);
        self.sequence = sequence;
        Ok(page)
    }
    pub(crate) fn page(&self, cursor: Option<&str>) -> Result<Value> {
        let now = Instant::now();
        let (snapshot, offset) = if let Some(cursor) = cursor {
            let (id, offset) = cursor
                .split_once('/')
                .ok_or_else(|| fault("invalid_catalog_cursor", "Use the returned next_cursor."))?;
            let offset = offset
                .parse::<usize>()
                .map_err(|_| fault("invalid_catalog_cursor", "Use the returned next_cursor."))?;
            let snapshot = self
                .snapshots
                .iter()
                .find(|snapshot| snapshot.id == id && snapshot.fresh(now))
                .ok_or_else(|| {
                    fault(
                        "cursor_expired",
                        "This discovery cursor expired, was evicted, or belongs to another context; discover again.",
                    )
                })?;
            (snapshot, offset)
        } else {
            let snapshot = self
                .snapshots
                .back()
                .filter(|snapshot| snapshot.fresh(now))
                .ok_or_else(Self::expired)?;
            (snapshot, 0)
        };
        snapshot.page(offset)
    }
    fn expired() -> rwi_core::Fault {
        fault(
            "catalog_expired",
            "Discover again before selecting; cached window rows are not current facts.",
        )
    }
    pub(crate) fn select(&self, selection: &WindowSelection) -> Result<WindowRecord> {
        let now = Instant::now();
        if !self.snapshots.iter().any(|snapshot| snapshot.fresh(now)) {
            return Err(Self::expired());
        }
        if !matches!(selection.relation, WindowRelation::Initial) {
            return Err(fault(
                "capability_unavailable",
                "Native macOS owner-chain transitions are not implemented. Use a fresh explicit initial selection under the same authorized desktop; this does not activate it.",
            ));
        }
        let mut matches = self
            .snapshots
            .iter()
            .filter(|snapshot| snapshot.fresh(now))
            .flat_map(|snapshot| &snapshot.rows)
            .filter(|w| {
                w.window == selection.window
                    && w.pid == selection.pid
                    && w.native_epoch == selection.native_epoch
            });
        let found = matches.next().ok_or_else(|| {
            fault(
                "window_not_discovered",
                "Select an exact fresh catalog entry, never a title/PID substitute.",
            )
        })?;
        if matches.any(|other| other != found) {
            return Err(fault(
                "catalog_identity_ambiguous",
                "Matching CGWindowID/PID/epoch observations disagree; wait for expiry or open a fresh context, then discover and select explicitly.",
            ));
        }
        Ok(found.clone())
    }
}

pub(crate) fn matches_query(window: &Window, query: &DiscoveryQuery) -> bool {
    query.pid.is_none_or(|pid| pid == window.pid)
        && query.title_contains.as_ref().is_none_or(|title| {
            window
                .title
                .as_ref()
                .is_some_and(|v| v.to_lowercase().contains(&title.to_lowercase()))
        })
}

#[cfg(test)]
mod registry_tests;
#[cfg(test)]
mod tests;
