//! Bounded read-only native RGBA composition. Never a pixel/input mapping grant.
use super::*;
use std::io::Cursor;
const MAX_PIXELS: usize = 2048 * 2048;
pub(super) struct Tile<'a> {
    pub png: &'a [u8],
    pub rect: Rect,
    pub size: [u32; 2],
}
pub(super) struct Composed {
    pub png: Vec<u8>,
    pub rect: Rect,
    pub size: [u32; 2],
}

fn decode(tile: &Tile<'_>) -> Result<Vec<u8>> {
    let mut decoder = png::Decoder::new(Cursor::new(tile.png));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|_| fault("live_family_png", "Native PNG header unavailable."))?;
    if [reader.info().width, reader.info().height] != tile.size
        || tile.size.contains(&0)
        || u64::from(tile.size[0]) * u64::from(tile.size[1]) > MAX_PIXELS as u64
    {
        return Err(fault(
            "live_family_png",
            "Native tile dimensions changed or exceed budget.",
        ));
    }
    let n = reader
        .output_buffer_size()
        .ok_or_else(|| fault("live_family_png", "Native PNG decode size unavailable."))?;
    if n > MAX_PIXELS * 4 {
        return Err(fault(
            "live_family_png",
            "Native PNG decode exceeds budget.",
        ));
    }
    let mut bytes = vec![0; n];
    let info = reader
        .next_frame(&mut bytes)
        .map_err(|_| fault("live_family_png", "Native PNG frame unavailable."))?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(fault(
            "live_family_png",
            "Native RGBA8 required; no implicit color substitute.",
        ));
    }
    bytes.truncate(info.buffer_size());
    Ok(bytes)
}
fn over(dst: &mut [u8], src: [u8; 4]) {
    let a = u32::from(src[3]);
    let b = u32::from(dst[3]);
    let alpha = a * 255 + b * (255 - a);
    if alpha == 0 {
        dst.fill(0);
        return;
    }
    for i in 0..3 {
        dst[i] = ((u32::from(src[i]) * a * 255 + u32::from(dst[i]) * b * (255 - a) + alpha / 2)
            / alpha)
            .min(255) as u8;
    }
    dst[3] = ((alpha + 127) / 255) as u8;
}
// Area-weighted premultiplied sampling, then straight RGBA source-over. At an
// identical native grid this is byte-exact; downscaling never drops alpha.
fn sample(bytes: &[u8], size: [u32; 2], box_: [f64; 4]) -> [u8; 4] {
    let [x0, y0, x1, y1] = box_;
    let mut sums = [0.; 4];
    let mut area = 0.;
    for y in (y0.floor().max(0.) as u32)..(y1.ceil().min(f64::from(size[1])) as u32) {
        for x in (x0.floor().max(0.) as u32)..(x1.ceil().min(f64::from(size[0])) as u32) {
            let weight = (x1.min(f64::from(x + 1)) - x0.max(f64::from(x))).max(0.)
                * (y1.min(f64::from(y + 1)) - y0.max(f64::from(y))).max(0.);
            let at = ((y as usize) * (size[0] as usize) + x as usize) * 4;
            let p = &bytes[at..at + 4];
            let alpha = f64::from(p[3]);
            for i in 0..3 {
                sums[i] += f64::from(p[i]) * alpha * weight;
            }
            sums[3] += alpha * weight;
            area += weight;
        }
    }
    if area == 0. || sums[3] == 0. {
        return [0; 4];
    }
    [
        (sums[0] / sums[3]).round() as u8,
        (sums[1] / sums[3]).round() as u8,
        (sums[2] / sums[3]).round() as u8,
        (sums[3] / area).round() as u8,
    ]
}
pub(super) fn compose(tiles: &[Tile<'_>], max_edge: u32) -> Result<Composed> {
    if tiles.is_empty()
        || tiles.len() > 4
        || !(64..=2048).contains(&max_edge)
        || tiles.iter().any(|t| !t.rect.valid() || t.size.contains(&0))
    {
        return Err(fault(
            "live_family_budget",
            "Finite 1..4 native tiles and an explicit bounded grid required.",
        ));
    }
    let x = tiles.iter().map(|t| t.rect.x).fold(f64::INFINITY, f64::min);
    let y = tiles.iter().map(|t| t.rect.y).fold(f64::INFINITY, f64::min);
    let right = tiles
        .iter()
        .map(|t| t.rect.x + t.rect.w)
        .fold(f64::NEG_INFINITY, f64::max);
    let bottom = tiles
        .iter()
        .map(|t| t.rect.y + t.rect.h)
        .fold(f64::NEG_INFINITY, f64::max);
    let rect = Rect {
        x,
        y,
        w: right - x,
        h: bottom - y,
    };
    if !rect.valid()
        || [x, y, right, bottom]
            .iter()
            .any(|v| v.abs() > f64::from(i32::MAX / 2))
    {
        return Err(fault(
            "live_family_budget",
            "Native family geometry is invalid.",
        ));
    }
    let native = tiles
        .iter()
        .map(|t| (f64::from(t.size[0]) / t.rect.w).min(f64::from(t.size[1]) / t.rect.h))
        .fold(f64::INFINITY, f64::min);
    let scale = native.min(f64::from(max_edge) / rect.w.max(rect.h));
    let size = [
        (rect.w * scale).floor().max(1.) as u32,
        (rect.h * scale).floor().max(1.) as u32,
    ];
    let count = (size[0] as usize)
        .checked_mul(size[1] as usize)
        .filter(|n| *n <= MAX_PIXELS)
        .ok_or_else(|| {
            fault(
                "live_family_budget",
                "Native family allocation exceeds budget.",
            )
        })?;
    let mut pixels = vec![0; count * 4];
    for (tile_index, tile) in tiles.iter().enumerate() {
        operation::checkpoint("live_family_composite_tile")?;
        let bytes = decode(tile)?;
        // Exact native grids use a fast byte-preserving source-over path.
        // This is common for Retina parent/sheet captures and avoids floating
        // point area resampling across the unchanged parent on every Live tick.
        let gx = f64::from(size[0]) / rect.w;
        let gy = f64::from(size[1]) / rect.h;
        let ox = (tile.rect.x - rect.x) * gx;
        let oy = (tile.rect.y - rect.y) * gy;
        if (tile.rect.w * gx - f64::from(tile.size[0])).abs() < 0.000001
            && (tile.rect.h * gy - f64::from(tile.size[1])).abs() < 0.000001
            && (ox - ox.round()).abs() < 0.000001
            && (oy - oy.round()).abs() < 0.000001
        {
            let x = ox.round() as usize;
            let y = oy.round() as usize;
            if x + tile.size[0] as usize <= size[0] as usize
                && y + tile.size[1] as usize <= size[1] as usize
            {
                if tile_index == 0 && tile.rect.same(rect) && tile.size == size {
                    pixels.copy_from_slice(&bytes);
                } else {
                    for row in 0..tile.size[1] as usize {
                        operation::checkpoint("live_family_native_row")?;
                        for col in 0..tile.size[0] as usize {
                            let src = (row * tile.size[0] as usize + col) * 4;
                            let dst = ((y + row) * size[0] as usize + x + col) * 4;
                            over(
                                &mut pixels[dst..dst + 4],
                                bytes[src..src + 4].try_into().unwrap(),
                            );
                        }
                    }
                }
                continue;
            }
        }
        let sx = f64::from(tile.size[0]) / tile.rect.w;
        let sy = f64::from(tile.size[1]) / tile.rect.h;
        for py in 0..size[1] {
            operation::checkpoint("live_family_composite_row")?;
            let wy0 = rect.y + f64::from(py) * rect.h / f64::from(size[1]);
            let wy1 = rect.y + f64::from(py + 1) * rect.h / f64::from(size[1]);
            if wy1 <= tile.rect.y || wy0 >= tile.rect.y + tile.rect.h {
                continue;
            }
            for px in 0..size[0] {
                let wx0 = rect.x + f64::from(px) * rect.w / f64::from(size[0]);
                let wx1 = rect.x + f64::from(px + 1) * rect.w / f64::from(size[0]);
                if wx1 <= tile.rect.x || wx0 >= tile.rect.x + tile.rect.w {
                    continue;
                }
                let mut p = sample(
                    &bytes,
                    tile.size,
                    [
                        (wx0 - tile.rect.x).max(0.) * sx,
                        (wy0 - tile.rect.y).max(0.) * sy,
                        (wx1 - tile.rect.x).min(tile.rect.w) * sx,
                        (wy1 - tile.rect.y).min(tile.rect.h) * sy,
                    ],
                );
                let coverage = (wx1.min(tile.rect.x + tile.rect.w) - wx0.max(tile.rect.x))
                    * (wy1.min(tile.rect.y + tile.rect.h) - wy0.max(tile.rect.y))
                    / ((wx1 - wx0) * (wy1 - wy0));
                p[3] = (f64::from(p[3]) * coverage).round() as u8;
                let at = ((py as usize) * (size[0] as usize) + px as usize) * 4;
                over(&mut pixels[at..at + 4], p);
            }
        }
    }
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, size[0], size[1]);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    {
        let mut w = encoder
            .write_header()
            .map_err(|_| fault("live_family_png", "Composite header failed."))?;
        w.write_image_data(&pixels)
            .map_err(|_| fault("live_family_png", "Composite PNG failed."))?;
        w.finish()
            .map_err(|_| fault("live_family_png", "Composite PNG finish failed."))?;
    }
    if out.len() > 16 * 1024 * 1024 {
        return Err(fault(
            "live_family_budget",
            "Composite PNG exceeds byte budget.",
        ));
    }
    Ok(Composed {
        png: out,
        rect,
        size,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn png(width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, width, height);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            let mut w = e.write_header().unwrap();
            w.write_image_data(pixels).unwrap();
            w.finish().unwrap();
        }
        out
    }
    #[test]
    fn native_overlay_keeps_parent_outside_the_exact_sheet_rectangle() {
        let parent = png(
            2,
            2,
            &[
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
            ],
        );
        let child = png(1, 1, &[0, 255, 0, 255]);
        let result = compose(
            &[
                Tile {
                    png: &parent,
                    rect: Rect {
                        x: 10.,
                        y: 20.,
                        w: 2.,
                        h: 2.,
                    },
                    size: [2, 2],
                },
                Tile {
                    png: &child,
                    rect: Rect {
                        x: 11.,
                        y: 21.,
                        w: 1.,
                        h: 1.,
                    },
                    size: [1, 1],
                },
            ],
            64,
        )
        .unwrap();
        assert_eq!(result.size, [2, 2]);
        assert_eq!(
            decode(&Tile {
                png: &result.png,
                rect: result.rect,
                size: result.size
            })
            .unwrap(),
            [
                255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255
            ]
        );
    }
    #[test]
    fn full_extent_transparent_sheet_overlays_instead_of_replacing_parent() {
        let parent = png(1, 1, &[12, 34, 56, 255]);
        let child = png(1, 1, &[255, 0, 0, 0]);
        let r = Rect {
            x: 0.,
            y: 0.,
            w: 1.,
            h: 1.,
        };
        let result = compose(
            &[
                Tile {
                    png: &parent,
                    rect: r,
                    size: [1, 1],
                },
                Tile {
                    png: &child,
                    rect: r,
                    size: [1, 1],
                },
            ],
            64,
        )
        .unwrap();
        assert_eq!(
            decode(&Tile {
                png: &result.png,
                rect: result.rect,
                size: result.size
            })
            .unwrap(),
            [12, 34, 56, 255]
        );
    }
    #[test]
    fn composition_rejects_declared_size_mismatch_before_copying() {
        let bytes = png(1, 1, &[9, 8, 7, 255]);
        assert!(
            compose(
                &[Tile {
                    png: &bytes,
                    rect: Rect {
                        x: 0.,
                        y: 0.,
                        w: 2.,
                        h: 2.
                    },
                    size: [2, 2]
                }],
                64
            )
            .is_err()
        );
    }
    #[test]
    fn straight_alpha_over_preserves_transparent_and_opaque_pixels() {
        let mut dst = [12, 34, 56, 255];
        over(&mut dst, [90, 90, 90, 0]);
        assert_eq!(dst, [12, 34, 56, 255]);
        over(&mut dst, [255, 0, 0, 128]);
        assert_eq!(dst, [134, 17, 28, 255]);
        over(&mut dst, [0, 255, 0, 255]);
        assert_eq!(dst, [0, 255, 0, 255]);
    }
    #[test]
    fn identical_grid_sample_is_exact_and_alpha_downsample_is_premultiplied() {
        let pixels = [255, 0, 0, 255, 0, 0, 255, 0];
        assert_eq!(sample(&pixels, [2, 1], [0., 0., 1., 1.]), [255, 0, 0, 255]);
        assert_eq!(sample(&pixels, [2, 1], [0., 0., 2., 1.]), [255, 0, 0, 128]);
    }
    #[test]
    fn empty_or_invalid_family_refuses_before_png_decode() {
        assert!(compose(&[], 2048).is_err());
        let t = Tile {
            png: &[],
            rect: Rect {
                x: 0.,
                y: 0.,
                w: f64::NAN,
                h: 1.,
            },
            size: [1, 1],
        };
        assert!(compose(&[t], 2048).is_err());
    }
}
