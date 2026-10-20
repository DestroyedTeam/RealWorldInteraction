//! Read-only cached Live pixels; no capture, selection, input reference or lease renewal.
use crate::{LiveViewUpdate, Result, VisualCapture, fault};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const MAX_LIVE_IMAGE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_LIVE_IMAGE_AGE_MS: u64 = 2000;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LiveImageCursor {
    pub generation: u64,
    pub sequence: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LiveImageQuery {
    pub view_id: String,
    #[serde(default)]
    pub after: Option<LiveImageCursor>,
}
impl LiveImageQuery {
    pub fn validate(&self) -> Result<()> {
        LiveViewUpdate {
            view_id: self.view_id.clone(),
            sequence: 1,
            visible: false,
        }
        .validate()?;
        if self.after.is_some_and(|c| {
            c.generation == 0
                || c.sequence == 0
                || c.generation > MAX_SAFE_INTEGER
                || c.sequence > MAX_SAFE_INTEGER
        }) {
            return Err(fault(
                "invalid_live_image_cursor",
                "Use a positive safe-integer generation/sequence pair.",
            ));
        }
        Ok(())
    }
}
// Deliberately not Serialize: transport must expose pixels and bounded descriptive
// metadata only, never a resolver frame, target-selection or capture capability.
#[derive(Clone, Debug)]
pub struct CachedLiveImage {
    pub cursor: LiveImageCursor,
    pub image: Arc<VisualCapture>,
}
#[derive(Clone, Debug)]
pub enum LiveImageReply {
    Ready(CachedLiveImage),
    Unchanged(LiveImageCursor),
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn f30_live_image_query_is_strict_without_target_or_input_fields() {
        let base = serde_json::json!({"view_id":"a".repeat(32)});
        assert!(
            serde_json::from_value::<LiveImageQuery>(base.clone())
                .unwrap()
                .validate()
                .is_ok()
        );
        for key in ["target", "frame", "visible", "capture", "input", "close"] {
            let mut bad = base.clone();
            bad[key] = serde_json::json!(true);
            assert!(serde_json::from_value::<LiveImageQuery>(bad).is_err());
        }
    }
    #[test]
    fn f30_live_image_cursor_requires_positive_safe_integer_axes() {
        for (generation, sequence) in [(0, 1), (1, 0), (u64::MAX, 1), (1, MAX_SAFE_INTEGER + 1)] {
            let q = LiveImageQuery {
                view_id: "a".repeat(32),
                after: Some(LiveImageCursor {
                    generation,
                    sequence,
                }),
            };
            assert!(q.validate().is_err());
        }
    }
    #[test]
    fn f30_live_image_unsupported_controller_does_not_fall_back() {
        struct Unsupported;
        impl crate::LiveController for Unsupported {
            fn control(&self, _: crate::LiveAction) -> Result<serde_json::Value> {
                panic!("no control fallback")
            }
            fn view_update(&self, _: LiveViewUpdate) -> Result<serde_json::Value> {
                panic!("no presence fallback")
            }
        }
        assert_eq!(
            crate::LiveController::latest_image(
                &Unsupported,
                LiveImageQuery {
                    view_id: "a".repeat(32),
                    after: None
                }
            )
            .unwrap_err()
            .code,
            "capability_unavailable"
        );
    }
}
