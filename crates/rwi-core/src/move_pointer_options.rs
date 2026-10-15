//! Explicit delivery selection; never promote background movement implicitly.
use crate::{ForegroundState, Result, fault};
use serde::{Deserialize, Serialize};

/// Only public NSEvent tracking kinds; never a CGEventType enum conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrackingPhase {
    Entered,
    Exited,
}
impl TrackingPhase {
    pub fn native_kind(self) -> u32 {
        match self {
            Self::Entered => 8,
            Self::Exited => 9,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "delivery", rename_all = "snake_case", deny_unknown_fields)]
pub enum MovePointerOptions {
    BackgroundWindow {},
    /// macOS private SPI experiment; never the default or an automatic retry.
    BackgroundSkylightExperimental {},
    /// Public AppKit constructor experiment; unchanged public PID transport.
    #[serde(rename = "background_appkit_experimental")]
    BackgroundAppKitExperimental {
        /// Caller-declared CG integer deltas, not inferred physical motion.
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_explicit_delta"
        )]
        delta: Option<[i32; 2]>,
    },
    /// Registered public legacy tag carrier, not modern-area pointer discovery.
    #[serde(rename = "background_appkit_tracking_experimental")]
    BackgroundAppKitTrackingExperimental {
        phase: TrackingPhase,
        /// Full-width opaque NSInteger in canonical signed decimal; no JS Number.
        tracking_number: String,
    },
    /// Authenticated cooperative owner only; no public tag or trusted witness input.
    BackgroundCooperativeTrackingExperimental {
        phase: TrackingPhase,
        owner_endpoint: String,
        registration_id: String,
    },
    ForegroundSystem {
        expected_foreground: ForegroundState,
    },
}
fn deserialize_explicit_delta<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<[i32; 2]>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    <[i32; 2]>::deserialize(deserializer).map(Some)
}

impl MovePointerOptions {
    /// A tag is an opaque API scalar, potentially address-shaped, never an ObjC
    /// object to decode. Parsing does NOT prove registration or current ownership.
    pub fn parse_tracking_number(value: &str) -> Result<i64> {
        let number = value.parse::<i64>().map_err(|_| {
            fault(
                "invalid_input_value",
                "Tracking tag must be a nonzero canonical signed i64 decimal string; no input.",
            )
        })?;
        if number == 0 || number.to_string() != value {
            return Err(fault(
                "invalid_input_value",
                "Tracking tag must be canonical, nonzero and lossless; no input.",
            ));
        }
        Ok(number)
    }
    pub fn validate_cooperative_selector(endpoint: &str, registration: &str) -> Result<()> {
        // macOS peer path syntax, independent of the MCP Host's platform.
        if endpoint.len() > 100
            || endpoint.as_bytes().contains(&0)
            || !endpoint.starts_with('/')
            || endpoint
                .split('/')
                .skip(1)
                .any(|part| part.is_empty() || part == "." || part == "..")
            || !crate::device::valid_id(registration)
        {
            return Err(fault(
                "invalid_input_value",
                "Use a short absolute owner socket and exact opaque registration_id; no tag/pointer/witness input.",
            ));
        }
        Ok(())
    }
    /// Policy bound for this opt-in experiment, not a measured motion history.
    pub fn validate_explicit_delta(delta: [i32; 2]) -> Result<()> {
        if delta == [0, 0] || delta.iter().any(|v| !(-128..=128).contains(v)) {
            return Err(fault(
                "invalid_input_value",
                "Explicit AppKit delta requires two signed integers, each -128..128, with at least one nonzero; no input.",
            ));
        }
        Ok(())
    }
    pub fn parse(value: Option<&str>) -> Result<Self> {
        let Some(value) = value else {
            return Ok(Self::BackgroundWindow {});
        };
        if value.len() > 4096 {
            return Err(fault(
                "invalid_input_value",
                "Bound pointer delivery options to 4096 bytes.",
            ));
        }
        let options: Self = serde_json::from_str(value).map_err(|_| {
            fault(
                "invalid_input_value",
                "Use explicit pointer delivery options, never text, a button or an action.",
            )
        })?;
        if let Self::ForegroundSystem {
            expected_foreground,
        } = &options
        {
            expected_foreground.validate()?;
        }
        if let Self::BackgroundAppKitExperimental { delta: Some(delta) } = &options {
            Self::validate_explicit_delta(*delta)?;
        }
        if let Self::BackgroundAppKitTrackingExperimental {
            tracking_number, ..
        } = &options
        {
            Self::parse_tracking_number(tracking_number)?;
        }
        if let Self::BackgroundCooperativeTrackingExperimental {
            owner_endpoint,
            registration_id,
            ..
        } = &options
        {
            Self::validate_cooperative_selector(owner_endpoint, registration_id)?;
        }
        Ok(options)
    }
    pub fn requires_foreground(&self) -> bool {
        matches!(self, Self::ForegroundSystem { .. })
    }
    pub fn require_capability(&self, capabilities: &serde_json::Value) -> Result<()> {
        if let Self::BackgroundCooperativeTrackingExperimental {
            owner_endpoint,
            registration_id,
            ..
        } = self
        {
            Self::validate_cooperative_selector(owner_endpoint, registration_id)?;
            let cap = &capabilities["pointer_move"]["background_cooperative_tracking_experimental"];
            if cap["implemented"] != true || cap["available"] != true {
                return Err(fault(
                    "capability_unavailable",
                    "Authenticated cooperative tracking is unavailable; no foreign-tag/direct-post fallback.",
                ));
            }
        }
        if let Self::BackgroundAppKitTrackingExperimental {
            tracking_number, ..
        } = self
        {
            Self::parse_tracking_number(tracking_number)?;
            let route = &capabilities["pointer_move"]["background_appkit_tracking_experimental"];
            if route["implemented"] != true || route["available"] != true {
                return Err(fault(
                    "capability_unavailable",
                    "Registered legacy AppKit tracking experiment is unavailable; no fallback.",
                ));
            }
        }
        if matches!(self, Self::BackgroundSkylightExperimental {}) {
            let route = &capabilities["pointer_move"]["background_skylight_experimental"];
            if route["implemented"] != true || route["available"] != true {
                return Err(fault(
                    "capability_unavailable",
                    "The experimental macOS SkyLight pointer route is unavailable; no fallback.",
                ));
            }
        }
        if matches!(self, Self::BackgroundAppKitExperimental { .. }) {
            let route = &capabilities["pointer_move"]["background_appkit_experimental"];
            if route["implemented"] != true || route["available"] != true {
                return Err(fault(
                    "capability_unavailable",
                    "The experimental macOS AppKit pointer constructor is unavailable; no fallback.",
                ));
            }
        }
        if let Self::BackgroundAppKitExperimental { delta: Some(delta) } = self {
            Self::validate_explicit_delta(*delta)?;
            let feature =
                &capabilities["pointer_move"]["background_appkit_experimental"]["explicit_delta"];
            if feature["implemented"] != true || feature["available"] != true {
                return Err(fault(
                    "capability_unavailable",
                    "This backend does not advertise explicit AppKit delta; no fallback.",
                ));
            }
        }
        // Explicit background selection is not the same as omitting options.
        // An unadvertised device must never reinterpret it as its native route.
        if matches!(self, Self::BackgroundWindow {})
            && (capabilities["pointer_move"]["implemented"] != true
                || capabilities["pointer_move"]["default_delivery"] != "background_window")
        {
            return Err(fault(
                "capability_unavailable",
                "This backend does not advertise window-addressed background pointer delivery; no fallback.",
            ));
        }
        if self.requires_foreground()
            && capabilities["pointer_move"]["foreground_system"]["implemented"] != true
        {
            return Err(fault(
                "capability_unavailable",
                "This backend does not advertise explicit foreground pointer delivery; no fallback.",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod appkit_move_option_tests;

#[cfg(test)]
mod explicit_delta_tests;
