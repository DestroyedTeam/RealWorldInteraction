//! Shared A1 pointer material and motion expression. Every platform's native
//! presenter renders from this crate so the marker cannot drift between them.
//! Nothing here dispatches input or reads a cursor position.
pub mod material;
pub mod motion;

pub const SIZE: usize = 160;
pub const CENTER: usize = SIZE / 2;
pub const MAX_DPI: u32 = 288;
pub const HIGHLIGHT_MS: u64 = 800; // Activity validity, not idle lifetime.

pub fn normalize_dpi(dpi: u32) -> u32 {
    if dpi == 0 { 96 } else { dpi.clamp(96, MAX_DPI) }
}
pub fn scale_for_dpi(dpi: u32) -> f64 {
    f64::from(normalize_dpi(dpi)) / 96.
}
pub fn raster_size(dpi: u32) -> usize {
    2 * (CENTER as f64 * scale_for_dpi(dpi)).ceil() as usize
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Idle,
    Pointer,
    Pressed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Anchor {
    pub point: [i32; 2],
    pub dpi: u32,
}
impl Anchor {
    pub fn with_dpi(self, dpi: u32) -> Self {
        Self {
            dpi: normalize_dpi(dpi),
            ..self
        }
    }
}
