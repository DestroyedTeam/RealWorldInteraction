//! Device-paced expression only. This type never computes a cursor position.
use crate::Phase;

pub const SETTLE_MS: f64 = 180.;
pub const PRESS_MS: f64 = 90.;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct Frame {
    pub phase: Phase,
    pub scale_x: f64,
    pub scale_y: f64,
    pub angle: f64,
    pub pressure: f64,
    pub light: f64,
    pub animating: bool,
}
impl Frame {
    pub fn idle() -> Self {
        Self {
            phase: Phase::Idle,
            scale_x: 1.,
            scale_y: 1.,
            angle: 0.,
            pressure: 0.,
            light: 0.85,
            animating: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Motion {
    last: Option<([f64; 2], f64)>,
    delta: [f64; 2],
    speed: f64,
    reversal: f64,
    angle: f64,
    pressed: bool,
    pressure_from: f64,
    pressure_at: Option<f64>,
    click_at: Option<f64>,
}
impl Motion {
    pub fn sample(&mut self, point: [f64; 2], dpi: u32, pressed: bool, now: f64) {
        if !now.is_finite()
            || !point
                .iter()
                .all(|v| v.is_finite() && (i32::MIN as f64..=i32::MAX as f64).contains(v))
        {
            return;
        }
        let scale = super::scale_for_dpi(dpi);
        self.speed = 0.;
        self.reversal = 0.;
        if let Some((previous, then)) = self.last {
            let age = (now - then).max(0.);
            let delta = [
                (point[0] - previous[0]) / scale,
                (point[1] - previous[1]) / scale,
            ];
            let distance = delta[0].hypot(delta[1]);
            let length = self.delta[0].hypot(self.delta[1]);
            // A gap is not measured velocity. Do not invent a travel envelope.
            if age < SETTLE_MS {
                self.speed = (distance / age.clamp(8., 80.) / 1.4).clamp(0., 1.);
                if distance > 0.01 && length > 0.01 {
                    self.reversal = (-(delta[0] * self.delta[0] + delta[1] * self.delta[1])
                        / (distance * length))
                        .clamp(0., 1.);
                }
            }
            if distance > 0.01 {
                self.angle = delta[1].atan2(delta[0]);
            }
            self.delta = delta;
        }
        self.last = Some((point, now));
        self.set_pressed(pressed, now);
    }
    fn pressure(&self, now: f64) -> f64 {
        let target = f64::from(self.pressed);
        let Some(at) = self.pressure_at else {
            return target;
        };
        let u = ((now - at) / PRESS_MS).clamp(0., 1.);
        self.pressure_from + (target - self.pressure_from) * (1. - (1. - u).powi(3))
    }
    fn set_pressed(&mut self, pressed: bool, now: f64) {
        self.click_at = None;
        if self.pressed != pressed {
            self.pressure_from = self.pressure(now);
            self.pressure_at = Some(now);
            self.pressed = pressed;
        }
    }
    /// The balanced click was acknowledged, including its release. Render a
    /// decaying contact cue, never falsely report a currently held button.
    pub fn click(&mut self, now: f64) {
        self.pressed = false;
        self.pressure_from = 1.;
        self.pressure_at = Some(now);
        self.click_at = Some(now);
    }
    pub fn cancel(&mut self) {
        *self = Self::default();
    }
    pub fn frame(&self, now: f64, animate: bool) -> Frame {
        if !now.is_finite() {
            return Frame::idle();
        }
        let age = self
            .last
            .map(|(_, at)| (now - at).max(0.))
            .unwrap_or(f64::INFINITY);
        let pressure_age = self
            .pressure_at
            .map(|at| (now - at).max(0.))
            .unwrap_or(f64::INFINITY);
        let phase = if self.pressed {
            Phase::Pressed
        } else if age < SETTLE_MS || pressure_age < PRESS_MS {
            Phase::Pointer
        } else {
            Phase::Idle
        };
        let p = if animate {
            self.pressure(now)
        } else {
            // A finite static contact acknowledgement is not a held button.
            f64::from(
                self.pressed
                    || self
                        .click_at
                        .is_some_and(|at| (0. ..PRESS_MS).contains(&(now - at))),
            )
        };
        let v = if animate {
            self.speed * (1. - age / SETTLE_MS).clamp(0., 1.).powi(2)
        } else {
            0.
        };
        let r = if animate {
            self.reversal * (1. - age / 150.).clamp(0., 1.).powi(2)
        } else {
            0.
        };
        Frame {
            phase,
            scale_x: if animate {
                1. + 0.12 * v + 0.03 * r - 0.06 * p
            } else {
                1.
            },
            scale_y: if animate {
                1. - 0.04 * v - 0.03 * p
            } else {
                1.
            },
            angle: if animate { self.angle } else { 0. },
            pressure: p,
            light: 0.85 + 0.14 * v + 0.01 * r,
            animating: age < SETTLE_MS || pressure_age < PRESS_MS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn precise_samples_only_drive_bounded_expression_and_idle_stops() {
        let mut m = Motion::default();
        m.sample([100., 200.], 192, false, 0.);
        m.sample([360., 320.], 192, true, 16.);
        m.sample([100., 200.], 192, true, 32.);
        for t in 32..300 {
            let f = m.frame(t as f64, true);
            assert!((0.94..=1.15).contains(&f.scale_x));
            assert!((0.93..=1.).contains(&f.scale_y));
            assert!((0.85..=1.).contains(&f.light));
        }
        assert!(!m.frame(300., true).animating);
        assert_eq!(m.frame(300., true).phase, Phase::Pressed);
        m.sample([100., 200.], 192, false, 301.);
        assert_eq!(m.frame(500., true).phase, Phase::Idle);
        assert!(!m.frame(500., true).animating);
    }
    #[test]
    fn reduced_motion_preserves_static_hold_and_cancel_preserves_no_history() {
        let mut m = Motion::default();
        m.sample([0., 0.], 96, false, 0.);
        m.sample([100., 60.], 96, true, 10.);
        let f = m.frame(20., false);
        assert_eq!(
            (f.scale_x, f.scale_y, f.angle, f.pressure),
            (1., 1., 0., 1.)
        );
        m.cancel();
        assert_eq!(m.frame(30., true), Frame::idle());
    }
    #[test]
    fn contact_release_and_capture_sampling_do_not_restart_an_envelope() {
        let mut m = Motion::default();
        m.sample([0., 0.], 96, false, 0.);
        m.click(0.);
        assert_eq!(m.frame(0., true).phase, Phase::Pointer);
        assert_eq!(m.frame(0., true).pressure, 1.);
        assert_eq!(m.frame(0., false).pressure, 1.);
        assert_eq!(m.frame(0., false).phase, Phase::Pointer);
        assert_eq!(m.frame(100., false).pressure, 0.);
        assert!(m.frame(40., true).pressure < 1.);
        assert_eq!(m.frame(100., true).pressure, 0.);
        assert_eq!(m.frame(200., true).phase, Phase::Idle);
        m.sample([f64::NAN, 0.], 96, true, 300.);
        assert_eq!(m.frame(300., true).phase, Phase::Idle);
    }
}
