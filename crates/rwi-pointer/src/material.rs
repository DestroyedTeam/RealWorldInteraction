//! A1 diamond material. Cached premultiplied layers; no input or screen reads.
//! Native blur uses a bounded three-box approximation, not browser pixel parity.
use crate::{CENTER, SIZE, motion::Frame, raster_size, scale_for_dpi};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    #[default]
    Light,
    Dark,
    HighContrast,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub theme: Theme,
    pub foreground: [u8; 3],
    pub background: [u8; 3],
}

#[derive(Clone, Copy, Default)]
struct Pixel([f32; 4]);
impl Pixel {
    fn rgba(c: u32) -> Self {
        let a = (c & 255) as f32 / 255.;
        Self([
            ((c >> 24) & 255) as f32 / 255. * a,
            ((c >> 16) & 255) as f32 / 255. * a,
            ((c >> 8) & 255) as f32 / 255. * a,
            a,
        ])
    }
    fn opacity(self, a: f64) -> Self {
        let a = a.clamp(0., 1.) as f32;
        Self(self.0.map(|v| v * a))
    }
    fn over(self, under: Self) -> Self {
        let k = 1. - self.0[3];
        Self(std::array::from_fn(|i| self.0[i] + under.0[i] * k))
    }
    fn mix(self, other: Self, t: f64) -> Self {
        let t = t.clamp(0., 1.) as f32;
        Self(std::array::from_fn(|i| {
            self.0[i] * (1. - t) + other.0[i] * t
        }))
    }
}
fn radial(t: f64, stops: &[(f64, u32)]) -> Pixel {
    for pair in stops.windows(2) {
        if t <= pair[1].0 {
            return Pixel::rgba(pair[0].1).mix(
                Pixel::rgba(pair[1].1),
                (t - pair[0].0) / (pair[1].0 - pair[0].0),
            );
        }
    }
    Pixel::rgba(stops.last().unwrap().1)
}
fn coverage(distance: f64, scale: f64) -> f64 {
    (0.5 - distance * scale).clamp(0., 1.)
}
fn disk(x: f64, y: f64, r: f64, scale: f64) -> f64 {
    coverage(x.hypot(y) - r, scale)
}
fn ring(x: f64, y: f64, r: f64, width: f64, scale: f64) -> f64 {
    coverage((x.hypot(y) - r).abs() - width / 2., scale)
}

struct Texture {
    size: usize,
    scale: f64,
    origin: [f64; 2],
    pixels: Vec<Pixel>,
}
impl Texture {
    fn new(extent: usize, dpi: u32, mut f: impl FnMut(f64, f64, f64) -> Pixel) -> Self {
        let scale = scale_for_dpi(dpi);
        let size = (extent as f64 * scale).ceil() as usize;
        let origin = [-(size as f64) / scale / 2.; 2];
        let mut pixels = Vec::with_capacity(size * size);
        for y in 0..size {
            for x in 0..size {
                pixels.push(f(
                    origin[0] + (x as f64 + 0.5) / scale,
                    origin[1] + (y as f64 + 0.5) / scale,
                    scale,
                ));
            }
        }
        Self {
            size,
            scale,
            origin,
            pixels,
        }
    }
    fn sample(&self, x: f64, y: f64) -> Pixel {
        let x = (x - self.origin[0]) * self.scale - 0.5;
        let y = (y - self.origin[1]) * self.scale - 0.5;
        let ix = x.floor() as isize;
        let iy = y.floor() as isize;
        let at = |a: isize, b: isize| {
            if a >= 0 && b >= 0 && (a as usize) < self.size && (b as usize) < self.size {
                self.pixels[b as usize * self.size + a as usize]
            } else {
                Pixel::default()
            }
        };
        at(ix, iy).mix(at(ix + 1, iy), x - ix as f64).mix(
            at(ix, iy + 1).mix(at(ix + 1, iy + 1), x - ix as f64),
            y - iy as f64,
        )
    }
    fn blur(&mut self, sigma: f64) {
        let radius = (sigma * self.scale - 0.5).round().max(1.) as isize;
        let n = self.size;
        let divisor = (radius * 2 + 1) as f32;
        let mut tmp = vec![Pixel::default(); n * n];
        // Three bounded box passes approximate a Gaussian with O(pixels) work.
        for _ in 0..3 {
            for vertical in [false, true] {
                for row in 0..n {
                    let index = |col: isize| -> Option<usize> {
                        (col >= 0 && (col as usize) < n).then(|| {
                            if vertical {
                                col as usize * n + row
                            } else {
                                row * n + col as usize
                            }
                        })
                    };
                    let mut sum = [0.; 4];
                    for c in -radius..=radius {
                        if let Some(i) = index(c) {
                            for (s, v) in sum.iter_mut().zip(self.pixels[i].0) {
                                *s += v;
                            }
                        }
                    }
                    for col in 0..n {
                        tmp[index(col as isize).unwrap()] =
                            Pixel(sum.map(|v| (v / divisor).clamp(0., 1.)));
                        if let Some(i) = index(col as isize - radius) {
                            for (s, v) in sum.iter_mut().zip(self.pixels[i].0) {
                                *s -= v;
                            }
                        }
                        if let Some(i) = index(col as isize + radius + 1) {
                            for (s, v) in sum.iter_mut().zip(self.pixels[i].0) {
                                *s += v;
                            }
                        }
                    }
                }
                std::mem::swap(&mut self.pixels, &mut tmp);
            }
        }
    }
}

/// Native GDI supplies coverage only. Product text and colors remain fixed RWI.
pub struct Label {
    pub width: usize,
    pub height: usize,
    pub coverage: Vec<u8>,
    pub padding: f64,
}
impl Label {
    pub fn fallback(dpi: u32) -> Self {
        let scale = scale_for_dpi(dpi);
        let width = (24. * scale).ceil() as usize;
        let height = (15. * scale).ceil() as usize;
        const LETTERS: [[u8; 7]; 3] = [
            [30, 17, 17, 30, 20, 18, 17],
            [17, 17, 17, 21, 21, 21, 10],
            [31, 4, 4, 4, 4, 4, 31],
        ];
        let mut values = vec![0; width * height];
        for y in 0..height {
            for x in 0..width {
                let px = ((x as f64 + 0.5) / scale - 3.).floor() as isize;
                let py = ((y as f64 + 0.5) / scale - 3.).floor() as isize;
                if (0..17).contains(&px)
                    && (0..7).contains(&py)
                    && px % 6 < 5
                    && (LETTERS[px as usize / 6][py as usize] & (1 << (4 - px % 6))) != 0
                {
                    values[y * width + x] = 255;
                }
            }
        }
        Self {
            width,
            height,
            coverage: values,
            padding: 3.,
        }
    }
}

pub struct Material {
    dpi: u32,
    style: Style,
    ambient: Texture,
    contact: Texture,
    body: Texture,
    rim: Texture,
    pressure: Texture,
    hotspot: Texture,
    label: Label,
}
impl Material {
    pub fn new(dpi: u32, style: Style, label: Label) -> Self {
        debug_assert_eq!(CENTER * 2, SIZE);
        let mut ambient = Texture::new(SIZE, dpi, |x, y, s| {
            radial(
                x.hypot(y) / (56. * 2_f64.sqrt()),
                &[
                    (0., 0x53d5ff35),
                    (0.22, 0x4abdfd30),
                    (0.39, 0x50c4fa24),
                    (0.55, 0x77d5fb12),
                    (0.72, 0x77d5fb00),
                ],
            )
            .opacity(disk(x, y, 56., s))
        });
        ambient.blur(3.);
        let mut contact = Texture::new(80, dpi, |x, y, _| {
            radial(
                x.hypot(y - 3.25) / (12.5 * 2_f64.sqrt()),
                &[(0., 0x163b5e65), (0.32, 0x183d5830), (0.72, 0x183d5800)],
            )
        });
        contact.blur(2.4);
        let mut glow = Texture::new(80, dpi, |x, y, s| {
            Pixel::rgba(0x33c4ff40).opacity(disk(x, y, 21., s))
        });
        glow.blur(7.5);
        let mut shadow = Texture::new(80, dpi, |x, y, s| {
            Pixel::rgba(0x12496275).opacity(disk(x, y - 5., 16., s))
        });
        shadow.blur(4.);
        let body = Texture::new(80, dpi, |x, y, s| {
            let inside = disk(x, y, 21., s);
            let d = (x + 6.72).hypot(y + 10.5) / 27.72_f64.hypot(31.5);
            let stops = if style.theme == Theme::Dark {
                &[
                    (0., 0xe5feff96),
                    (0.24, 0x8becff35),
                    (0.48, 0x40c7f835),
                    (0.64, 0x52bef74a),
                    (0.76, 0x8ce9ff08),
                ]
            } else {
                &[
                    (0., 0xe5feffc2),
                    (0.20, 0xa1f9fd45),
                    (0.48, 0x40c7f848),
                    (0.64, 0x52bef746),
                    (0.75, 0x8ce9ff16),
                ]
            };
            let p = radial(d, stops)
                .opacity(inside)
                .over(shadow.sample(x, y).over(glow.sample(x, y)));
            // Local inset definition; no source input/cursor shadow is sampled.
            let top = ((y.abs().min(30.) - 18.) / 3.).clamp(0., 1.)
                * inside
                * if y < 0. { 1. } else { 0. };
            let bottom = ((y - 16.) / 5.).clamp(0., 1.) * inside;
            Pixel::rgba(0x2388c13a)
                .opacity(bottom)
                .over(Pixel::rgba(0xefffffaa).opacity(top).over(p))
        });
        let rim = Texture::new(80, dpi, |x, y, s| {
            let color = if y < -x.abs() {
                0xe5ffffa3
            } else if x < -y.abs() {
                0xbcf6ff40
            } else if y > x.abs() {
                0x2195bd2b
            } else {
                0
            };
            Pixel::rgba(color).opacity(ring(x, y, 21.5, 1., s))
        });
        let mut pressure = Texture::new(64, dpi, |x, y, s| {
            Pixel::rgba(0x75efff80).opacity(ring(x, y, 10.5, 1., s))
        });
        pressure.blur(4.5);
        for y in 0..pressure.size {
            for x in 0..pressure.size {
                let px = pressure.origin[0] + (x as f64 + 0.5) / pressure.scale;
                let py = pressure.origin[1] + (y as f64 + 0.5) / pressure.scale;
                let i = y * pressure.size + x;
                pressure.pixels[i] = Pixel::rgba(0xeaffffe6)
                    .opacity(ring(px, py, 10., 1., pressure.scale))
                    .over(
                        Pixel::rgba(0x1d87a378)
                            .opacity(ring(px, py, 11., 1., pressure.scale))
                            .over(pressure.pixels[i]),
                    );
            }
        }
        let mut hotspot = Texture::new(28, dpi, |x, y, s| {
            Pixel::rgba(if style.theme == Theme::HighContrast {
                0
            } else {
                0x173b5266
            })
            .opacity(coverage((x.abs() + (y - 1.).abs() - 6.) / 2_f64.sqrt(), s))
        });
        hotspot.blur(1.);
        for y in 0..hotspot.size {
            for x in 0..hotspot.size {
                let px = hotspot.origin[0] + (x as f64 + 0.5) / hotspot.scale;
                let py = hotspot.origin[1] + (y as f64 + 0.5) / hotspot.scale;
                let d = (px.abs() + py.abs() - 6.) / 2_f64.sqrt();
                let i = y * hotspot.size + x;
                let ink = if style.theme == Theme::HighContrast {
                    rgb(style.background)
                } else {
                    0x113b48ff
                };
                let edge = if style.theme == Theme::HighContrast {
                    rgb(style.foreground)
                } else {
                    0xffffffff
                };
                let p = Pixel::rgba(ink)
                    .opacity(coverage(d, hotspot.scale))
                    .over(hotspot.pixels[i]);
                let p = Pixel::rgba(edge)
                    .opacity(coverage(d.abs() - 0.9, hotspot.scale))
                    .over(p);
                hotspot.pixels[i] = Pixel::rgba(if style.theme == Theme::HighContrast {
                    edge
                } else {
                    0xb6f7ffff
                })
                .opacity(coverage(
                    (px.abs() + py.abs() - 3.) / 2_f64.sqrt(),
                    hotspot.scale,
                ))
                .over(p);
            }
        }
        Self {
            dpi,
            style,
            ambient,
            contact,
            body,
            rim,
            pressure,
            hotspot,
            label,
        }
    }
    pub fn raster(&self, frame: Frame, label_offset: [f64; 2]) -> Vec<u8> {
        let size = raster_size(self.dpi);
        let scale = scale_for_dpi(self.dpi);
        let center = (size / 2) as f64;
        let (sin, cos) = frame.angle.sin_cos();
        let mut out = vec![0; size * size * 4];
        for y in 0..size {
            for x in 0..size {
                let px = (x as f64 + 0.5 - center) / scale;
                let py = (y as f64 + 0.5 - center) / scale;
                let ex = (cos * px + sin * py) / frame.scale_x;
                let ey = (-sin * px + cos * py) / frame.scale_y;
                let mut p = if self.style.theme == Theme::HighContrast {
                    let base =
                        Pixel::rgba(rgb(self.style.background)).opacity(disk(px, py, 21., scale));
                    let edge = Pixel::rgba(rgb(self.style.foreground))
                        .opacity(ring(px, py, 21., 1.5, scale));
                    let press = Pixel::rgba(rgb(self.style.foreground))
                        .opacity(ring(px, py, 10.5, 2., scale) * frame.pressure);
                    press.over(edge.over(base))
                } else {
                    let ambient = self
                        .ambient
                        .sample(px / frame.scale_x, py / frame.scale_x)
                        .opacity(frame.light);
                    let contact = self
                        .contact
                        .sample(ex, ey)
                        .opacity(0.66 + frame.pressure * 0.24);
                    let body = self
                        .body
                        .sample(ex, ey)
                        .opacity(0.84 + frame.pressure * 0.14);
                    self.pressure.sample(ex, ey).opacity(frame.pressure).over(
                        self.rim
                            .sample(ex, ey)
                            .over(body.over(contact.over(ambient))),
                    )
                };
                p = self.hotspot.sample(px, py).over(p);
                let lx = ((px - label_offset[0] + self.label.padding) * scale).floor() as isize;
                let ly = ((py - label_offset[1] + self.label.padding) * scale).floor() as isize;
                let mask = |a: isize, b: isize| {
                    if a >= 0
                        && b >= 0
                        && (a as usize) < self.label.width
                        && (b as usize) < self.label.height
                    {
                        self.label.coverage[b as usize * self.label.width + a as usize] as f64
                            / 255.
                    } else {
                        0.
                    }
                };
                let coverage = mask(lx, ly);
                let dark = self.style.theme == Theme::Dark;
                let (ink, shadow) = if self.style.theme == Theme::HighContrast {
                    (rgb(self.style.foreground), rgb(self.style.background))
                } else if dark {
                    (0xc1f3fcff, 0x061b24ff)
                } else {
                    (0x194e62ff, 0xffffffed)
                };
                let d = (scale.round() as isize).max(1);
                let halo = mask(lx - d, ly)
                    .max(mask(lx + d, ly))
                    .max(mask(lx, ly - d))
                    .max(mask(lx, ly + d));
                p = Pixel::rgba(ink)
                    .opacity(coverage)
                    .over(Pixel::rgba(shadow).opacity(halo).over(p));
                let alpha = (p.0[3] * 255.).round().clamp(0., 255.) as u8;
                let ch = |i: usize| (p.0[i] * 255.).round().clamp(0., alpha as f32) as u8;
                out[(y * size + x) * 4..(y * size + x) * 4 + 4].copy_from_slice(&[
                    ch(2),
                    ch(1),
                    ch(0),
                    alpha,
                ]);
            }
        }
        out
    }
}
fn rgb(c: [u8; 3]) -> u32 {
    u32::from_be_bytes([c[0], c[1], c[2], 255])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diamond_material_is_premultiplied_bounded_and_not_an_arrow() {
        let m = Material::new(96, Style::default(), Label::fallback(96));
        let out = m.raster(Frame::idle(), [14., 14.]);
        let n = raster_size(96);
        assert_eq!(out.len(), n * n * 4);
        assert!(
            out.chunks_exact(4)
                .all(|p| p[..3].iter().all(|v| *v <= p[3]))
        );
        assert_eq!(&out[..4], &[0; 4]);
        let at = |x: usize, y: usize| &out[(y * n + x) * 4..(y * n + x) * 4 + 4];
        assert!(at(CENTER, CENTER)[3] > 240);
        assert!(at(CENTER - 30, CENTER)[3] > 0);
        let mut pressed = Frame::idle();
        pressed.pressure = 1.;
        assert_ne!(out, m.raster(pressed, [14., 14.]));
    }
    #[test]
    fn motion_disabled_click_has_a_static_contact_cue_without_a_held_state() {
        let mut motion = super::super::motion::Motion::default();
        motion.sample([100., 100.], 96, false, 0.);
        motion.click(0.);
        let clicked = motion.frame(0., false);
        assert_eq!(clicked.phase, super::super::Phase::Pointer);
        for theme in [Theme::Light, Theme::HighContrast] {
            let style = Style {
                theme,
                foreground: [255; 3],
                background: [0; 3],
            };
            let m = Material::new(96, style, Label::fallback(96));
            assert_ne!(
                m.raster(clicked, [14., 14.]),
                m.raster(Frame::idle(), [14., 14.])
            );
            assert_eq!(
                m.raster(motion.frame(200., false), [14., 14.]),
                m.raster(Frame::idle(), [14., 14.])
            );
        }
    }
    #[test]
    #[ignore = "offscreen product-raster study; requires RCU_POINTER_PREVIEW_DIR, no native display"]
    fn write_offscreen_study() {
        let dir = std::path::PathBuf::from(
            std::env::var_os("RCU_POINTER_PREVIEW_DIR").expect("explicit output directory"),
        );
        std::fs::create_dir_all(&dir).unwrap();
        for (name, theme, pressure) in [
            ("idle", Theme::Light, 0.),
            ("pressed", Theme::Light, 1.),
            ("dark", Theme::Dark, 0.),
            ("high-contrast", Theme::HighContrast, 1.),
        ] {
            let style = Style {
                theme,
                foreground: [255; 3],
                background: [0; 3],
            };
            let m = Material::new(192, style, Label::fallback(192));
            let mut f = Frame::idle();
            f.pressure = pressure;
            let rgba = m.raster(f, [14., 14.]);
            let bg: [u8; 3] = if theme == Theme::Dark {
                [17, 30, 29]
            } else {
                [255; 3]
            };
            let rgb: Vec<_> = rgba
                .chunks_exact(4)
                .flat_map(|p| {
                    [2, 1, 0].into_iter().enumerate().map(move |(j, i)| {
                        p[i].saturating_add(
                            ((u16::from(bg[j]) * (255 - u16::from(p[3]))) / 255) as u8,
                        )
                    })
                })
                .collect();
            let n = raster_size(192) as u32;
            let png = rwi_imaging::encode_rgb([n, n], &rgb).unwrap();
            std::fs::write(dir.join(format!("a1-{name}.png")), png).unwrap();
        }
    }
}
