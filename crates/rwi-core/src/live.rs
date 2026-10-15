//! Bounded local preview control. No image stream or input authority is implied.
use crate::{DisplayTarget, Engine, Result, WindowRelation, WindowSelection, fault, operation};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LiveAction {
    Start,
    Pause,
    Hide,
    Resume,
    Stop,
    Status,
}

/// An already-owned preview's bounded control plane. No capture, target selection,
/// native input or preview creation is exposed through this handle. The backend
/// must revoke it when its owning session/binding is retired. Presence sequences
/// and expiry remain native checks, not facts inferred from an input operation.
pub trait LiveController: Send + Sync {
    /// Read an already-owned cached PNG only; never capture or renew presence.
    fn latest_image(
        &self,
        _: crate::live_image::LiveImageQuery,
    ) -> Result<crate::live_image::LiveImageReply> {
        Err(fault(
            "capability_unavailable",
            "Cached Live image export is not implemented on this backend.",
        ))
    }
    fn control(&self, action: LiveAction) -> Result<serde_json::Value>;
    fn view_update(&self, update: LiveViewUpdate) -> Result<serde_json::Value>;
}

pub type LiveControllers =
    std::collections::BTreeMap<String, Result<std::sync::Arc<dyn LiveController>>>;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LiveOptions {
    /// Explicit display for the user's preview panel, independent of source display.
    pub display: DisplayTarget,
    #[serde(default = "label")]
    pub label: String,
    /// Capture detail and the preferred size of the native preview panel.
    #[serde(default = "edge")]
    pub max_edge: u32,
    #[serde(default = "interval")]
    pub interval_ms: u64,
    /// Hard lifetime bound, also covering an unclean Host disconnect.
    #[serde(default = "duration")]
    pub duration_ms: u64,
    /// Up to three explicitly discovered additional visual windows. This is
    /// observation-only; it does not assert ownership or grant input authority.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_windows: Vec<WindowSelection>,
    /// Adapter-owned Host view binding. Never supplied by the model-facing tool.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(skip)]
    pub host_view_id: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LiveViewUpdate {
    pub view_id: String,
    pub sequence: u64,
    pub visible: bool,
}
impl LiveViewUpdate {
    pub fn validate(&self) -> Result<()> {
        if !valid_view_id(&self.view_id) || self.sequence == 0 {
            return Err(fault(
                "invalid_live_view",
                "Use the adapter-owned view binding and a positive sequence.",
            ));
        }
        Ok(())
    }
}
fn valid_view_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn label() -> String {
    "RWI Live".into()
}
fn edge() -> u32 {
    2048
}
fn interval() -> u64 {
    500
}
fn duration() -> u64 {
    120_000
}
impl LiveOptions {
    pub fn validate(&self) -> Result<()> {
        self.display.validate()?;
        if self.additional_windows.len() > 3
            || self
                .additional_windows
                .iter()
                .enumerate()
                .any(|(index, window)| {
                    window.pid == 0
                        || window.window.is_empty()
                        || window.window.len() > 128
                        || window.native_epoch.is_empty()
                        || window.native_epoch.len() > 128
                        || !matches!(window.relation, WindowRelation::Initial)
                        || self.additional_windows[..index]
                            .iter()
                            .any(|other| other.window == window.window)
                })
        {
            return Err(fault(
                "invalid_live_sources",
                "Use at most three distinct, explicitly discovered initial window selections; this grants no input or ownership relationship.",
            ));
        }
        if !(64..=2048).contains(&self.max_edge)
            || !(250..=2000).contains(&self.interval_ms)
            || !(1000..=600_000).contains(&self.duration_ms)
            || self.label.is_empty()
            || self.label.chars().count() > 64
            || self.label.chars().any(char::is_control)
            || self
                .host_view_id
                .as_deref()
                .is_some_and(|id| !valid_view_id(id))
        {
            return Err(fault(
                "invalid_live_options",
                "Use max_edge 64..2048, interval_ms 250..2000, duration_ms 1000..600000 and a 1..64 character label.",
            ));
        }
        Ok(())
    }
}
impl Engine {
    /// Metadata-only export under the executor's ordinary serialized lane. The
    /// caller retains exact target ownership; handles cannot create a preview.
    pub fn live_controllers(&self) -> LiveControllers {
        self.targets
            .iter()
            .map(|(target, registered)| (target.clone(), registered.backend.live_controller()))
            .collect()
    }
    pub fn live_view(&mut self, target: &str, update: LiveViewUpdate) -> Result<serde_json::Value> {
        operation::checkpoint("live_view_presence")?;
        update.validate()?;
        self.backend(target)?.live_view(update)
    }
    pub fn live(
        &mut self,
        target: &str,
        action: LiveAction,
        options: Option<LiveOptions>,
    ) -> Result<serde_json::Value> {
        operation::checkpoint("live_control")?;
        if action == LiveAction::Start {
            options
                .as_ref()
                .ok_or_else(|| {
                    fault(
                        "invalid_live_options",
                        "Start requires an explicit preview display and options.",
                    )
                })?
                .validate()?;
        } else if options.is_some() {
            return Err(fault(
                "invalid_live_options",
                "Options are only accepted by start; stop and start to reconfigure.",
            ));
        }
        self.backend(target)?.live(action, options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn additional_live_sources_are_bounded_explicit_and_not_owner_claims() {
        let selection = serde_json::json!({"window":"cgwindow:7","pid":23,"native_epoch":"epoch","relation":"initial"});
        let make = |windows| {
            serde_json::from_value::<LiveOptions>(
                serde_json::json!({"display":{"id":1},"additional_windows":windows}),
            )
            .unwrap()
        };
        make(vec![selection.clone()]).validate().unwrap();
        assert!(
            make(vec![selection.clone(), selection.clone()])
                .validate()
                .is_err()
        );
        let mut bad = selection.clone();
        bad["relation"] = serde_json::json!("owned_popup");
        assert!(make(vec![bad]).validate().is_err());
        let mut bad = selection.clone();
        bad["pid"] = serde_json::json!(0);
        assert!(make(vec![bad]).validate().is_err());
        assert!(
            make(
                (0..4)
                    .map(|i| {
                        let mut s = selection.clone();
                        s["window"] = serde_json::json!(format!("cgwindow:{i}"));
                        s
                    })
                    .collect::<Vec<_>>()
            )
            .validate()
            .is_err()
        );
        let empty = make(vec![]);
        assert!(
            serde_json::to_value(empty)
                .unwrap()
                .get("additional_windows")
                .is_none()
        );
    }
    #[test]
    fn live_options_bound_duration_rate_memory_and_preview_display() {
        let valid: LiveOptions =
            serde_json::from_value(serde_json::json!({"display":{"id":7}})).unwrap();
        valid.validate().unwrap();
        assert_eq!(valid.max_edge, 2048);
        for bad in [
            serde_json::json!({"display":{}}),
            serde_json::json!({"display":{"id":7},"duration_ms":600001}),
            serde_json::json!({"display":{"id":7},"interval_ms":1}),
            serde_json::json!({"display":{"id":7},"max_edge":2049}),
            serde_json::json!({"display":{"id":7},"label":"line\nbreak"}),
        ] {
            assert!(
                serde_json::from_value::<LiveOptions>(bad)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
    }
    #[test]
    fn host_presence_fields_are_internal_and_strictly_validated() {
        let schema =
            serde_json::to_value(schemars::schema_for!(crate::device::api::LiveArgs)).unwrap();
        assert!(!schema.to_string().contains("host_view_id"));
        assert!(schema["properties"].get("view").is_none());
        for (id, sequence) in [("".into(), 1), ("A".repeat(32), 1), ("a".repeat(32), 0)] {
            assert!(
                LiveViewUpdate {
                    view_id: id,
                    sequence,
                    visible: true
                }
                .validate()
                .is_err()
            );
        }
        LiveViewUpdate {
            view_id: "a".repeat(32),
            sequence: 1,
            visible: true,
        }
        .validate()
        .unwrap();
        assert_eq!(
            serde_json::to_value(LiveAction::Hide).unwrap(),
            serde_json::json!("hide")
        );
    }
}
