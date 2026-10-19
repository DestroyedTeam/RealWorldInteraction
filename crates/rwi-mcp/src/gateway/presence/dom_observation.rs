//! Bounded, page-scoped client measurements. These never authorize Native work.
use super::*;
use std::collections::VecDeque;

const SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_EVENTS: usize = 8;

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::gateway) struct DomObservation {
    observed_unix_ms: u64,
    document_visible: bool,
    intersecting: bool,
    image_hidden: bool,
    image_src_present: bool,
    natural_size: [u32; 2],
    panel_size: [u32; 2],
    cursor: Option<Cursor>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct Cursor {
    generation: u64,
    sequence: u64,
}

impl DomObservation {
    pub(in crate::gateway) fn valid(&self) -> bool {
        (1..=SAFE_INTEGER).contains(&self.observed_unix_ms)
            && self.natural_size.iter().all(|value| *value <= 2048)
            && self.panel_size.iter().all(|value| *value <= 4096)
            && self.cursor.as_ref().is_none_or(|cursor| {
                (1..=SAFE_INTEGER).contains(&cursor.generation)
                    && (1..=SAFE_INTEGER).contains(&cursor.sequence)
            })
    }
}

struct Sample {
    observation: DomObservation,
    presence_sequence: u64,
    received_unix_ms: u128,
    received: Instant,
}

#[derive(Default)]
pub(super) struct Log(VecDeque<Sample>);

impl Views {
    /// Called only after the existing app-event capability/sequence check.
    /// Recheck under the registry lock: an intervening page replacement or
    /// newer heartbeat cannot borrow this observation. No presence/life update.
    pub(in crate::gateway) fn observe_dom(
        &self,
        key: &str,
        page: &str,
        sequence: u64,
        observation: DomObservation,
    ) -> bool {
        if !observation.valid() {
            return false;
        }
        let Ok(mut entries) = self.entries.lock() else {
            return false;
        };
        let Some(view) = entries.get_mut(key) else {
            return false;
        };
        if view.expires <= Instant::now()
            || view.page.as_deref() != Some(page)
            || sequence == 0
            || view.ui_sequence != sequence
            || view
                .dom
                .0
                .back()
                .is_some_and(|sample| sample.presence_sequence >= sequence)
        {
            return false;
        }
        if view.dom.0.len() == MAX_EVENTS {
            view.dom.0.pop_front();
        }
        view.dom.0.push_back(Sample {
            observation,
            presence_sequence: sequence,
            received_unix_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            received: Instant::now(),
        });
        true
    }

    /// Status-only evidence. Missing/expired/replaced clients remain unknown.
    pub(in crate::gateway) fn decorate_dom_observation(&self, target: &str, data: &mut Value) {
        let Some(binding) = data["host_view_binding"].as_str() else {
            return;
        };
        let Ok(entries) = self.entries.lock() else {
            return;
        };
        let Some(view) = entries.values().find(|view| {
            view.target == target && view.binding == binding && view.expires > Instant::now()
        }) else {
            return;
        };
        let Some(last) = view.dom.0.back() else {
            return;
        };
        let events: Vec<_> = view
            .dom
            .0
            .iter()
            .map(|sample| {
                json!({
                    "presence_sequence":sample.presence_sequence,
                    "received_unix_ms":sample.received_unix_ms,
                    "dom":sample.observation
                })
            })
            .collect();
        data["component_observation"] = json!({
            "evidence":"client_reported_own_DOM; not Host attestation or Native authority",
            "clock":"client timestamps are untrusted and not synchronized to Native sampling",
            "read_only":true,"capture_or_presence_renewed":false,
            "age_ms":last.received.elapsed().as_millis(),"max_events":MAX_EVENTS,"events":events
        });
    }
}

#[cfg(test)]
mod tests;
