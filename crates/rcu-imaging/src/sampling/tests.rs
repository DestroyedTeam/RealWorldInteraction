use super::*;

fn full(size: [u32; 2]) -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: size[0],
        height: size[1],
    }
}
fn gray(values: &[u8]) -> Vec<u8> {
    values.iter().flat_map(|&v| [v; 3]).collect()
}

#[test]
fn fractional_crop_gives_every_source_pixel_its_exact_area_weight() {
    let size = [7, 6];
    let crop = Rect {
        x: 1,
        y: 1,
        width: 5,
        height: 4,
    };
    // Hand-derived intersections for 5->3 and 4->3, in thirds of a source pixel.
    let wx = [[3u64, 0, 0], [2, 1, 0], [0, 3, 0], [0, 1, 2], [0, 0, 3]];
    let wy = [[3u64, 0, 0], [1, 2, 0], [0, 2, 1], [0, 0, 3]];
    for (iy, y_weights) in wy.iter().enumerate() {
        for (ix, x_weights) in wx.iter().enumerate() {
            let mut rgb = [247, 37, 11].repeat(7 * 6); // Outside crop must not bleed in.
            for y in 1..5 {
                for x in 1..6 {
                    rgb[(y * 7 + x) * 3..(y * 7 + x) * 3 + 3].fill(0);
                }
            }
            let i = ((iy + 1) * 7 + ix + 1) * 3;
            rgb[i..i + 3].fill(255);
            let output = area_rgb(size, &rgb, crop, [3, 3]).unwrap();
            let expected: Vec<u8> = (0..3)
                .flat_map(|y| {
                    (0..3).flat_map(move |x| {
                        // Each output footprint has area 5*4 integer grid units.
                        [((255 * x_weights[x] * y_weights[y] + 10) / 20) as u8; 3]
                    })
                })
                .collect();
            assert_eq!(
                output, expected,
                "source ({ix},{iy}) including boundary pixels"
            );
            assert!(output.iter().any(|&v| v > 0), "source pixel was dropped");
        }
    }
}

#[test]
fn rgb_channels_half_up_rounding_and_single_axis_reductions_are_exact() {
    let rgb = [0, 30, 255, 60, 90, 0, 240, 210, 90];
    assert_eq!(
        area_rgb([3, 1], &rgb, full([3, 1]), [2, 1]).unwrap(),
        [20, 50, 170, 180, 170, 60]
    );
    assert_eq!(
        area_rgb([1, 3], &rgb, full([1, 3]), [1, 2]).unwrap(),
        [20, 50, 170, 180, 170, 60]
    );
    for (source, expected) in [(vec![0, 1], 1), (vec![0, 0, 1], 0), (vec![0, 1, 1], 1)] {
        let size = [source.len() as u32, 1];
        assert_eq!(
            area_rgb(size, &gray(&source), full(size), [1, 1]).unwrap(),
            [expected; 3]
        );
    }
    let image = gray(&[0, 30, 60, 90, 120, 150, 180, 210, 240]);
    assert_eq!(
        area_rgb([3, 3], &image, full([3, 3]), [2, 2]).unwrap(),
        gray(&[40, 80, 160, 200])
    );
    assert_eq!(
        area_rgb([7, 5], &[255; 7 * 5 * 3], full([7, 5]), [3, 2]).unwrap(),
        [255; 3 * 2 * 3]
    );
}

#[test]
fn thin_row_and_column_include_the_first_and_last_source_pixels() {
    for (size, output) in [([4096, 1], [64, 1]), ([1, 4096], [1, 64])] {
        let mut rgb = vec![255; 4096 * 3];
        rgb[..3].fill(0);
        rgb[4095 * 3..].fill(0);
        let sampled = area_rgb(size, &rgb, full(size), output).unwrap();
        // 63 white pixels plus one black: round(63*255/64) == 251.
        assert_eq!(&sampled[..3], &[251; 3]);
        assert_eq!(&sampled[63 * 3..], &[251; 3]);
        assert!(sampled[3..63 * 3].iter().all(|&v| v == 255));
    }
}

fn diamond() -> (Vec<u8>, Vec<[u32; 2]>) {
    let mut rgb = vec![255; 512 * 512 * 3];
    let mut line = vec![];
    for y in 0..512i32 {
        for x in 0..512i32 {
            if (x - 256).abs() + (y - 256).abs() == 139 {
                let i = (y as usize * 512 + x as usize) * 3;
                rgb[i..i + 3].fill(0);
                line.push([x as u32, y as u32]);
            }
        }
    }
    (rgb, line)
}
fn assert_ring_stays_closed(raster: &Raster) {
    let [w, h] = raster.size.map(|n| n as usize);
    let mut reached = vec![false; w * h];
    let mut stack = vec![0usize];
    while let Some(i) = stack.pop() {
        if reached[i] || raster.rgb[i * 3] != 255 {
            continue;
        }
        reached[i] = true;
        let (x, y) = (i % w, i / w);
        if x > 0 {
            stack.push(i - 1);
        }
        if x + 1 < w {
            stack.push(i + 1);
        }
        if y > 0 {
            stack.push(i - w);
        }
        if y + 1 < h {
            stack.push(i + w);
        }
    }
    assert!(reached[w - 1] && reached[w * h - 1]);
    assert!(
        !reached[(h / 2) * w + w / 2],
        "a preview gap opened the ring"
    );
}

#[test]
fn one_pixel_closed_diagonal_survives_integer_and_fractional_preview_reduction() {
    let (rgb, line) = diamond();
    // The old 4x center-pick misses this entire one-pixel diagonal: its sampled
    // x/y parity is even while every line pixel has odd x+y. Not a visual oracle.
    for y in 0..128 {
        for x in 0..128 {
            assert_eq!(rgb[((y * 4 + 2) * 512 + x * 4 + 2) * 3], 255);
        }
    }
    let original = encode_rgb([512, 512], &rgb).unwrap();
    let hash = rcu_core::digest(&original);
    for edge in [128, 153] {
        let (preview, mapping) = view(&original, None, edge, None).unwrap();
        let decoded = decode_original(&preview).unwrap();
        assert_eq!(mapping.kind, "area_average_preview");
        assert_eq!(decoded.size, [edge, edge]);
        for &[x, y] in &line {
            let ox = ((2 * x + 1) * edge / 1024) as usize;
            let oy = ((2 * y + 1) * edge / 1024) as usize;
            assert!(decoded.rgb[(oy * edge as usize + ox) * 3] < 255);
        }
        assert_ring_stays_closed(&decoded);
        let (visual, size) = encode_visual_rgb([512, 512], &rgb, edge).unwrap();
        assert_eq!(size, decoded.size);
        assert_eq!(decode_original(&visual).unwrap().rgb, decoded.rgb);
    }
    assert_eq!(rcu_core::digest(&original), hash);
    assert_eq!(decode_original(&original).unwrap().rgb, rgb);
}

#[test]
fn mapping_and_native_crop_pixels_are_unchanged() {
    let size = [199, 113];
    let mut rgb = vec![];
    for y in 0..size[1] {
        for x in 0..size[0] {
            rgb.extend_from_slice(&[x as u8, y as u8, 37]);
        }
    }
    let original = encode_rgb(size, &rgb).unwrap();
    let original_hash = rcu_core::digest(&original);
    let crop = Rect {
        x: 11,
        y: 7,
        width: 131,
        height: 97,
    };
    let desktop = Some([1., 0., 0., 1., -1920., -300.]);
    let (_, m) = view(&original, Some(crop), 64, desktop).unwrap();
    let (sx, sy) = (131. / 64., 97. / 47.);
    let affine = [sx, 0., 0., sy, 11. + (sx - 1.) / 2., 7. + (sy - 1.) / 2.];
    assert_eq!(m.view_size, [64, 47]);
    assert_eq!(m.view_to_original, affine);
    assert_eq!(m.original_to_desktop, desktop);
    assert_eq!(m.crop, crop);
    assert_eq!(m.original_size, size);
    assert_eq!(
        m.original_point([63., 46.]).unwrap(),
        [sx * 63. + affine[4], sy * 46. + affine[5]]
    );
    assert!(m.original_point([64., 0.]).is_err());
    let (tile, m) = view(&original, Some(crop), 2048, desktop).unwrap();
    assert_eq!(m.kind, "native_resolution_tile");
    assert_eq!(m.view_to_original, [1., 0., 0., 1., 11., 7.]);
    let decoded = decode_original(&tile).unwrap();
    for y in 0..97usize {
        assert_eq!(
            &decoded.rgb[y * 131 * 3..(y + 1) * 131 * 3],
            &rgb[((y + 7) * 199 + 11) * 3..((y + 7) * 199 + 142) * 3]
        );
    }
    assert_eq!(rcu_core::digest(&original), original_hash);
}

fn png_pixels(size: [u32; 2], color: png::ColorType, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = vec![];
    let mut encoder = png::Encoder::new(&mut bytes, size[0], size[1]);
    encoder.set_color(color);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(pixels).unwrap();
    drop(writer);
    bytes
}

#[test]
fn alpha_is_composited_on_white_before_rgb_area_averaging() {
    let mut rgba = vec![];
    let mut ga = vec![];
    let mut rgb = vec![];
    for _ in 0..64 {
        for x in 0..128 {
            if x % 2 == 0 {
                rgba.extend_from_slice(&[200, 20, 99, 0]);
                ga.extend_from_slice(&[20, 0]);
                rgb.extend_from_slice(&[255; 3]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 128]);
                ga.extend_from_slice(&[0, 128]);
                rgb.extend_from_slice(&[127; 3]);
            }
        }
    }
    for (color, pixels) in [
        (png::ColorType::Rgba, rgba),
        (png::ColorType::GrayscaleAlpha, ga),
        (png::ColorType::Rgb, rgb),
    ] {
        let original = png_pixels([128, 64], color, &pixels);
        let hash = rcu_core::digest(&original);
        let (preview, m) = view(&original, None, 64, None).unwrap();
        assert_eq!(m.view_size, [64, 32]);
        assert_eq!(
            decode_original(&preview).unwrap().rgb,
            vec![191; 64 * 32 * 3]
        );
        assert_eq!(rcu_core::digest(&original), hash);
    }
}

#[test]
fn invalid_layout_bounds_upscale_output_budget_and_cancellation_remain_strict() {
    assert_eq!(
        area_rgb([16_000_001, 1], &[], full([16_000_001, 1]), [64, 1])
            .unwrap_err()
            .code,
        "image_dimension_limit"
    );
    assert_eq!(
        area_rgb([2, 2], &[0; 11], full([2, 2]), [1, 1])
            .unwrap_err()
            .code,
        "encoding_source_layout"
    );
    for crop in [
        Rect {
            x: -1,
            y: 0,
            width: 1,
            height: 1,
        },
        Rect {
            x: 2,
            y: 0,
            width: 1,
            height: 1,
        },
    ] {
        assert_eq!(
            area_rgb([2, 2], &[0; 12], crop, [1, 1]).unwrap_err().code,
            "invalid_crop"
        );
    }
    for output in [[0, 1], [1, 0], [3, 1], [1, 3]] {
        assert_eq!(
            area_rgb([2, 2], &[0; 12], full([2, 2]), output)
                .unwrap_err()
                .code,
            "invalid_sample_size"
        );
    }
    let sampled = area_rgb([2, 2], &[127; 12], full([2, 2]), [1, 1]).unwrap();
    assert_eq!(
        encode_rgb_limited([1, 1], &sampled, 16).unwrap_err().code,
        "png_size_limit"
    );
    let op = rcu_core::operation::Registry::default()
        .begin(None)
        .unwrap();
    op.cancel();
    for output in [[2, 2], [1, 1]] {
        assert_eq!(
            rcu_core::operation::run(&op, || area_rgb([2, 2], &[0; 12], full([2, 2]), output))
                .unwrap_err()
                .code,
            "operation_cancelled"
        );
    }
}

#[test]
#[ignore = "explicit bounded offscreen RGB conversion timing; not capture, transport, GUI or task latency"]
fn measure_bounded_area_conversion() {
    use std::{hint::black_box, time::Instant};
    for (size, output) in [
        ([3600, 2186], [1024, 621]),
        ([4000, 4000], [1024, 1024]),
        ([512, 512], [512, 512]),
    ] {
        let rgb: Vec<u8> = (0..size[0] as usize * size[1] as usize * 3)
            .map(|i| (i.wrapping_mul(37) ^ (i >> 9)) as u8)
            .collect();
        black_box(area_rgb(size, &rgb, full(size), output).unwrap());
        let mut ms = vec![];
        for _ in 0..5 {
            let started = Instant::now();
            black_box(area_rgb(size, black_box(&rgb), full(size), output).unwrap());
            ms.push(started.elapsed().as_secs_f64() * 1000.);
        }
        ms.sort_by(f64::total_cmp);
        println!(
            "scope=pure_rgb_area_conversion build={} source={size:?} output={output:?} n=5 min_ms={:.3} median_ms={:.3} max_ms={:.3} source_rgb_bytes={} output_rgb_bytes={} extra_scratch=constant no_png_or_capture_or_transport=true",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            },
            ms[0],
            ms[2],
            ms[4],
            rgb.len(),
            output[0] as usize * output[1] as usize * 3
        );
    }
}
