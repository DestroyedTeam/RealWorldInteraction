//! A bounded, device-paced drag proposal; no timers, OS calls or business logic.
use crate::{Result, fault};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_WAYPOINTS: usize = 128;
pub const MAX_DURATION_MS: u64 = 5_000;
pub const SAMPLE_INTERVAL_MS: u64 = 16;
pub const MAX_SAMPLE_INTERVAL_MS: u64 = 64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Waypoint {
    /// Milliseconds since button-down. Strictly increasing, starting at zero.
    pub at_ms: u64,
    /// Offset from the grounded reference in DEVICE physical desktop pixels.
    /// These are not controller, preview or original-image coordinates.
    pub offset: [f64; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DragPath {
    /// Left button only; first point must be {at_ms:0, offset:[0,0]}.
    pub waypoints: Vec<Waypoint>,
}

impl DragPath {
    pub fn validate(&self) -> Result<()> {
        let points = &self.waypoints;
        if !(2..=MAX_WAYPOINTS).contains(&points.len())
            || points[0].at_ms != 0
            || points[0].offset != [0., 0.]
            || points
                .last()
                .is_none_or(|p| !(16..=MAX_DURATION_MS).contains(&p.at_ms))
            || points.windows(2).any(|p| p[0].at_ms >= p[1].at_ms)
            || points
                .iter()
                .any(|p| p.offset.iter().any(|v| !v.is_finite() || v.abs() > 16_384.))
        {
            return Err(fault(
                "invalid_drag_path",
                "Use 2..128 finite waypoints, first {at_ms:0,offset:[0,0]}, strictly increasing times ending at 16..5000ms, offsets bounded to +/-16384 device physical pixels. No input or replay.",
            ));
        }
        Ok(())
    }

    /// Piecewise linear samples, retaining every supplied vertex and endpoint.
    /// At most 440 samples, independent of wall-clock scheduling or OS latency.
    pub fn samples(&self) -> Result<Vec<Waypoint>> {
        self.samples_with_interval(SAMPLE_INTERVAL_MS)
    }

    /// Device-selected interpolation density. Required vertices and their times
    /// are never removed or shifted, including vertices closer than the cadence.
    pub fn samples_with_interval(&self, interval_ms: u64) -> Result<Vec<Waypoint>> {
        self.validate()?;
        if !(SAMPLE_INTERVAL_MS..=MAX_SAMPLE_INTERVAL_MS).contains(&interval_ms) {
            return Err(fault(
                "invalid_drag_cadence",
                "Interpolation cadence must be within 16..64 ms; no input.",
            ));
        }
        let mut samples = vec![self.waypoints[0].clone()];
        for pair in self.waypoints.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let mut at = a.at_ms + interval_ms;
            while at < b.at_ms {
                let ratio = (at - a.at_ms) as f64 / (b.at_ms - a.at_ms) as f64;
                samples.push(Waypoint {
                    at_ms: at,
                    offset: std::array::from_fn(|i| {
                        a.offset[i] + (b.offset[i] - a.offset[i]) * ratio
                    }),
                });
                at += interval_ms;
            }
            samples.push(b.clone());
        }
        Ok(samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_cadence_never_drops_or_retimes_requested_vertices() {
        let p = path();
        for interval in 16..=64 {
            let samples = p.samples_with_interval(interval).unwrap();
            assert!(p.waypoints.iter().all(|v| samples.contains(v)));
            assert!(samples.windows(2).all(|p| p[0].at_ms < p[1].at_ms));
            assert_eq!(samples.last(), p.waypoints.last());
        }
        for interval in [0, 15, 65, u64::MAX] {
            assert!(p.samples_with_interval(interval).is_err());
        }
    }
    fn path() -> DragPath {
        DragPath {
            waypoints: vec![
                Waypoint {
                    at_ms: 0,
                    offset: [0., 0.],
                },
                Waypoint {
                    at_ms: 33,
                    offset: [33., -33.],
                },
                Waypoint {
                    at_ms: 50,
                    offset: [0., 0.],
                },
            ],
        }
    }
    #[test]
    fn interpolation_keeps_vertices_and_device_relative_units() {
        let p = path();
        let s = p.samples().unwrap();
        assert_eq!(
            s.iter().map(|p| p.at_ms).collect::<Vec<_>>(),
            [0, 16, 32, 33, 49, 50]
        );
        assert_eq!(s[1].offset, [16., -16.]);
        assert_eq!(s[3], p.waypoints[1]);
        assert_eq!(s.last(), p.waypoints.last());
    }
    #[test]
    fn invalid_or_unbounded_paths_fail_before_sampling() {
        let original = path();
        for points in [
            vec![],
            vec![original.waypoints[0].clone()],
            vec![original.waypoints[0].clone(); 129],
        ] {
            assert!(DragPath { waypoints: points }.samples().is_err());
        }
        for (at, offset) in [
            (0, [1., 0.]),
            (15, [1., 0.]),
            (5001, [1., 0.]),
            (u64::MAX, [1., 0.]),
            (50, [f64::NAN, 0.]),
            (50, [f64::INFINITY, 0.]),
            (50, [16385., 0.]),
        ] {
            let mut p = original.clone();
            p.waypoints[2] = Waypoint { at_ms: at, offset };
            assert!(p.validate().is_err());
        }
        let mut p = original;
        p.waypoints[0].offset = [1., 0.];
        assert!(p.validate().is_err());
    }
    #[test]
    fn maximum_sample_count_is_bounded() {
        let mut p = path();
        p.waypoints = (0..128)
            .map(|i| Waypoint {
                at_ms: i * 5000 / 127,
                offset: [i as f64, 0.],
            })
            .collect();
        assert!(p.samples().unwrap().len() <= 440);
        let json = serde_json::to_value(p).unwrap();
        let mut bad = json.clone();
        bad["shell"] = true.into();
        assert!(serde_json::from_value::<DragPath>(bad).is_err());
        assert!(
            serde_json::from_value::<DragPath>(json)
                .unwrap()
                .validate()
                .is_ok()
        );
    }
}
