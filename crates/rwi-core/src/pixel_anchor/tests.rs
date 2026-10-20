use super::*;

mod context;
mod creation;
fn png(rgb: &[u8], size: [u32; 2]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, size[0], size[1]);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(rgb).unwrap();
    }
    out
}
fn fixture() -> (Vec<u8>, [u32; 2]) {
    let size = [160, 160];
    let mut rgb = vec![255; 160 * 160 * 3];
    for y in 61..99 {
        for x in 61..99 {
            let i = (y * 160 + x) * 3;
            rgb[i..i + 3].copy_from_slice(&[20, 50, 90]);
        }
    }
    (rgb, size)
}
fn red_map(phase: u8) -> (Vec<u8>, [u32; 2]) {
    let size = [160, 160];
    let background = if phase == 1 {
        [104, 132, 52]
    } else {
        [72, 118, 60]
    };
    let mut rgb = vec![0; 160 * 160 * 3];
    for pixel in rgb.chunks_exact_mut(3) {
        pixel.copy_from_slice(&background);
    }
    for y in 32usize..128 {
        for x in 32usize..128 {
            if (x / 12 + y / 12) % 2 == 0 {
                let i = (y * 160 + x) * 3;
                rgb[i..i + 3].copy_from_slice(&[220, 220, 220]);
            }
        }
    }
    let mut markers = vec![
        (80i32, 80i32),
        (64, 80),
        (96, 80),
        (80, 64),
        (80, 96),
        (69, 69),
        (91, 69),
        (69, 91),
        (91, 91),
    ];
    if phase == 2 {
        markers[0] = (84, 80);
    } else if phase == 3 {
        markers.remove(0);
    }
    for (cx, cy) in markers {
        for dy in -3..=3 {
            for dx in -3..=3 {
                if dx * dx + dy * dy > 9 {
                    continue;
                }
                let x = (cx + dx) as usize;
                let y = (cy + dy) as usize;
                let i = (y * 160 + x) * 3;
                rgb[i..i + 3].copy_from_slice(&[250, 18, 20]);
            }
        }
    }
    if phase == 4 {
        // A non-red translucent/damage overlay occludes only the target dot.
        for y in 76..85 {
            for x in 76..85 {
                let i = (y * 160 + x) * 3;
                rgb[i..i + 3].copy_from_slice(&[132, 145, 150]);
            }
        }
    }
    if phase == 1 {
        // Moving terrain/health text and a separate red damage glyph should
        // not invalidate the saved map-marker cluster.
        for y in 40..56 {
            for x in 100..132 {
                let i = (y * 160 + x) * 3;
                rgb[i..i + 3].copy_from_slice(&[232, 238, 235]);
            }
        }
        for y in 40..45 {
            for x in 38..45 {
                let i = (y * 160 + x) * 3;
                rgb[i..i + 3].copy_from_slice(&[250, 18, 20]);
            }
        }
    }
    (png(&rgb, size), size)
}
#[test]
fn animated_small_chrome_is_tolerated_but_local_move_is_not() {
    let (base, size) = fixture();
    let original = png(&base, size);
    let anchor = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
    let mut animation = base.clone();
    for x in 65..81 {
        let i = (55 * 160 + x) * 3;
        animation[i..i + 3].copy_from_slice(&[0, 0, 0]);
    }
    assert!(anchor.matches(
        &png(&animation, size),
        size,
        [80., 80.],
        Some([1., 0., 0., 1., 0., 0.])
    ));
    let mut moved = vec![255; base.len()];
    for y in 61..99 {
        for x in 63..101 {
            let i = (y * 160 + x) * 3;
            moved[i..i + 3].copy_from_slice(&[20, 50, 90]);
        }
    }
    assert!(!anchor.matches(
        &png(&moved, size),
        size,
        [80., 80.],
        Some([1., 0., 0., 1., 0., 0.])
    ));
    assert!(!anchor.matches(&original, size, [81., 80.], Some([1., 0., 0., 1., 0., 0.])));
    assert!(!anchor.matches(&original, size, [80., 80.], None));
}
#[test]
fn bounded_animated_cells_do_not_outvote_a_stable_hit_patch() {
    let (base, size) = fixture();
    let original = png(&base, size);
    let anchor = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
    let mut animated = base;
    let origin = [32usize, 32usize];
    for cell in 0..21usize {
        let gx = cell % GRID;
        let gy = cell / GRID;
        let value = if cell == 0 { 214 } else { 223 };
        for dy in 0..CELL {
            for dx in 0..CELL {
                let x = origin[0] + gx * CELL + dx;
                let y = origin[1] + gy * CELL + dy;
                let i = (y * size[0] as usize + x) * CHANNELS;
                animated[i..i + CHANNELS].fill(value);
            }
        }
    }
    let report = anchor.diagnostic(
        &png(&animated, size),
        size,
        [80., 80.],
        Some([1., 0., 0., 1., 0., 0.]),
    );
    // This reproduces the reported 21 changed cells and raw total delta 681.
    assert_eq!(report["changed_cells"], 21);
    assert_eq!(report["total_delta"], 681);
    assert_eq!(report["central_max_delta"], 0);
    assert_eq!(report["stable_context_delta"], 0);
    assert_eq!(report["matched"], true);
}

#[test]
fn broad_low_level_patch_drift_still_fails_stable_context_budget() {
    let (base, size) = fixture();
    let original = png(&base, size);
    let anchor = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
    let mut drift = base;
    for y in 32..128usize {
        for x in 32..128usize {
            let i = (y * size[0] as usize + x) * CHANNELS;
            for channel in 0..CHANNELS {
                drift[i + channel] = drift[i + channel].saturating_sub(3);
            }
        }
    }
    let report = anchor.diagnostic(
        &png(&drift, size),
        size,
        [80., 80.],
        Some([1., 0., 0., 1., 0., 0.]),
    );
    assert_eq!(report["changed_cells"], 0);
    assert_eq!(report["total_delta"], 768);
    assert_eq!(report["hit_region_delta"], 192);
    assert_eq!(report["stable_context_delta"], 480);
    assert_eq!(report["reason"], "stable_context_delta_budget_exceeded");
    assert_eq!(report["matched"], false);
}

#[test]
fn opt_in_red_marker_cluster_ignores_unrelated_animation_but_not_marker_changes() {
    let (original, size) = red_map(0);
    let anchor = RedMarkerAnchor::from_png(&original, size, [80., 80.], [80., 80.])
        .expect("target is centered on a compact red cluster");
    assert_eq!(anchor.markers.len(), 9);

    let (animated, _) = red_map(1);
    assert!(anchor.matches(&animated, size, [80., 80.], Some([1., 0., 0., 1., 0., 0.])));
    let strict = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
    assert!(!strict.matches(&animated, size, [80., 80.], Some([1., 0., 0., 1., 0., 0.])));

    for (phase, reason) in [
        (2, "target_marker_missing_or_moved"),
        (3, "target_marker_missing_or_moved"),
        (4, "target_marker_missing_or_moved"),
    ] {
        let (changed, _) = red_map(phase);
        let report = anchor.diagnostic(&changed, size, [80., 80.], Some([1., 0., 0., 1., 0., 0.]));
        assert_eq!(report["matched"], false);
        assert_eq!(report["reason"], reason);
        assert_eq!(report["target_marker_matched"], false);
        assert_eq!(report["input_authority"], false);
    }
}

#[test]
fn red_marker_cluster_requires_a_unique_target_and_neighboring_markers() {
    let size = [160, 160];
    let mut rgb = vec![80; 160 * 160 * 3];
    for (cx, cy) in [(80, 80), (64, 80)] {
        for dy in -3..=3 {
            for dx in -3..=3 {
                if dx * dx + dy * dy <= 9 {
                    let i = ((cy + dy) as usize * 160 + (cx + dx) as usize) * 3;
                    rgb[i..i + 3].copy_from_slice(&[250, 18, 20]);
                }
            }
        }
    }
    let two_markers = png(&rgb, size);
    assert!(RedMarkerAnchor::from_png(&two_markers, size, [80., 80.], [80., 80.]).is_none());
    assert!(RedMarkerAnchor::from_png(&two_markers, size, [48., 48.], [48., 48.]).is_none());
}
#[test]
fn blank_or_invalid_source_does_not_weaken_full_content_guard() {
    let size = [160, 160];
    assert!(
        PixelAnchor::from_png(
            &png(&vec![255; 160 * 160 * 3], size),
            size,
            [80., 80.],
            [80., 80.]
        )
        .is_none()
    );
    assert!(PixelAnchor::from_png(b"not-png", size, [80., 80.], [80., 80.]).is_none());
    let (textured, size) = fixture();
    assert!(PixelAnchor::from_png(&png(&textured, size), size, [51., 51.], [51., 51.]).is_none());
}
#[test]
fn diagnostic_reports_central_and_outer_change_without_changing_admission() {
    let (base, size) = fixture();
    let original = png(&base, size);
    let anchor = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
    let transform = Some([1., 0., 0., 1., 0., 0.]);
    let mut outer = base.clone();
    // Static central button, many moving pixels in the surrounding 96px
    // patch: this must remain refused rather than inventing target identity.
    for y in 32..58 {
        for x in 32..128 {
            let i = (y * 160 + x) * 3;
            outer[i..i + 3].copy_from_slice(&[0, 0, 0]);
        }
    }
    let report = anchor.diagnostic(&png(&outer, size), size, [80., 80.], transform);
    assert_eq!(report["reason"], "changed_cell_budget_exceeded");
    assert_eq!(report["central_max_delta"], 0);
    assert_eq!(report["matched"], false);
    assert_eq!(report["input_authority"], false);
    let mut center = base;
    for y in 74..86 {
        for x in 74..86 {
            let i = (y * 160 + x) * 3;
            center[i..i + 3].copy_from_slice(&[255, 255, 255]);
        }
    }
    let report = anchor.diagnostic(&png(&center, size), size, [80., 80.], transform);
    assert_eq!(report["reason"], "central_patch_changed");
    assert!(report["central_max_delta"].as_u64().unwrap() > 16);
    assert!(!anchor.matches(&png(&center, size), size, [80., 80.], transform));
    assert_eq!(
        anchor.diagnostic(&original, size, [80., 80.], transform)["reason"],
        "matched"
    );
}
#[test]
fn diagnostics_distinguish_mapping_dimensions_and_decode_failure() {
    let (base, size) = fixture();
    let original = png(&base, size);
    let anchor = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
    for (image, dimensions, point, transform, reason) in [
        (
            &original[..],
            [161, 160],
            [80., 80.],
            Some([1., 0., 0., 1., 0., 0.]),
            "image_dimensions_changed",
        ),
        (
            &original[..],
            size,
            [81., 80.],
            Some([1., 0., 0., 1., 0., 0.]),
            "desktop_point_changed",
        ),
        (&original[..], size, [80., 80.], None, "transform_missing"),
        (
            &original[..],
            size,
            [80., 80.],
            Some([1., 0., 0., 1., 1., 0.]),
            "transform_changed",
        ),
        (
            &b"bad-png"[..],
            size,
            [80., 80.],
            Some([1., 0., 0., 1., 0., 0.]),
            "patch_decode_unavailable",
        ),
    ] {
        let report = anchor.diagnostic(image, dimensions, point, transform);
        assert_eq!(report["reason"], reason);
        assert_eq!(
            report["matched"],
            anchor.matches(image, dimensions, point, transform)
        );
        assert!(report["changed_cells"].is_null());
    }
}

// Synthetic source-pixel cells, not a capture or reconstruction of the
// CreatureCurios before/after PNG. These tests exercise the same comparator.
fn dynamic_anchor_diagnostic_fixture(cell_deltas: &[(usize, u8)]) -> serde_json::Value {
    let (base, size) = fixture();
    let original = png(&base, size);
    let anchor = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
    let mut current = base;
    for &(cell, delta) in cell_deltas {
        for dy in 0..CELL {
            for dx in 0..CELL {
                let x = 32 + (cell % GRID) * CELL + dx;
                let y = 32 + (cell / GRID) * CELL + dy;
                let i = (y * size[0] as usize + x) * CHANNELS;
                for channel in 0..CHANNELS {
                    current[i + channel] = current[i + channel].checked_sub(delta).unwrap();
                }
            }
        }
    }
    anchor.diagnostic(
        &png(&current, size),
        size,
        [80., 80.],
        Some([1., 0., 0., 1., 0., 0.]),
    )
}

#[test]
fn dynamic_anchor_diagnostic_reports_spatial_split_for_synthetic_46_cell_rejection() {
    let mut deltas = Vec::new();
    let hit = (0..GRID * GRID).filter(|cell| {
        let (row, column) = (cell / GRID, cell % GRID);
        (4..12).contains(&row)
            && (4..12).contains(&column)
            && !((7..=8).contains(&row) && (7..=8).contains(&column))
    });
    deltas.extend(hit.take(24).map(|cell| (cell, 13)));
    let context = (0..GRID * GRID)
        .filter(|cell| !((4..12).contains(&(cell / GRID)) && (4..12).contains(&(cell % GRID))))
        .collect::<Vec<_>>();
    for (index, &cell) in context.iter().take(22).enumerate() {
        deltas.push((cell, if index == 0 { 145 } else { 142 }));
    }
    deltas.extend(context.iter().skip(22).take(36).map(|&cell| (cell, 3)));
    let report = dynamic_anchor_diagnostic_fixture(&deltas);
    assert_eq!(report["changed_cells"], 46);
    assert_eq!(report["central_max_delta"], 0);
    assert_eq!(report["hit_region_delta"], 312);
    assert_eq!(report["stable_context_delta"], 78);
    assert_eq!(report["total_delta"], 3547);
    assert_eq!(report["reason"], "changed_cell_budget_exceeded");
    assert_eq!(report["matched"], false);
    assert_eq!(report["hit_changed_cells"], 24);
    assert_eq!(report["context_changed_cells"], 22);
    assert_eq!(report["context_trimmed_changed_cells"], 22);
    assert_eq!(report["stable_context_changed_cells"], 0);
    assert_eq!(
        report["failed_checks"],
        serde_json::json!(["changed_cell_budget_exceeded"])
    );
    assert_eq!(report["input_authority"], false);
    println!("synthetic_46_cell_report={report}");
}

#[test]
fn dynamic_anchor_diagnostic_lists_all_failed_budgets_without_changing_reason_priority() {
    let report = dynamic_anchor_diagnostic_fixture(
        &(0..GRID * GRID).map(|cell| (cell, 20)).collect::<Vec<_>>(),
    );
    assert_eq!(report["reason"], "central_patch_changed");
    assert_eq!(report["matched"], false);
    assert_eq!(report["changed_cells"], 256);
    assert_eq!(report["hit_changed_cells"], 64);
    assert_eq!(report["context_changed_cells"], 192);
    assert_eq!(report["context_trimmed_changed_cells"], 32);
    assert_eq!(report["stable_context_changed_cells"], 160);
    assert_eq!(
        report["failed_checks"],
        serde_json::json!([
            "central_patch_changed",
            "changed_cell_budget_exceeded",
            "hit_region_delta_budget_exceeded",
            "stable_context_delta_budget_exceeded",
        ])
    );
}

#[test]
fn dynamic_anchor_diagnostic_distinguishes_stable_context_drift_from_changed_cell_budget() {
    let report = dynamic_anchor_diagnostic_fixture(
        &(0..GRID * GRID).map(|cell| (cell, 3)).collect::<Vec<_>>(),
    );
    assert_eq!(report["reason"], "stable_context_delta_budget_exceeded");
    assert_eq!(report["changed_cells"], 0);
    assert_eq!(report["hit_changed_cells"], 0);
    assert_eq!(report["context_changed_cells"], 0);
    assert_eq!(report["context_trimmed_changed_cells"], 0);
    assert_eq!(report["stable_context_changed_cells"], 0);
    assert_eq!(
        report["failed_checks"],
        serde_json::json!(["stable_context_delta_budget_exceeded"])
    );
    assert_eq!(report["matched"], false);
}

#[test]
fn dynamic_anchor_diagnostic_stable_patch_has_no_failures_or_changed_cells() {
    let report = dynamic_anchor_diagnostic_fixture(&[]);
    assert_eq!(report["reason"], "matched");
    assert_eq!(report["matched"], true);
    assert_eq!(report["hit_changed_cells"], 0);
    assert_eq!(report["context_changed_cells"], 0);
    assert_eq!(report["context_trimmed_changed_cells"], 0);
    assert_eq!(report["stable_context_changed_cells"], 0);
    assert_eq!(report["failed_checks"], serde_json::json!([]));
}

#[test]
fn dynamic_anchor_diagnostic_unavailable_patch_does_not_invent_spatial_metrics() {
    let (base, size) = fixture();
    let original = png(&base, size);
    let anchor = PixelAnchor::from_png(&original, size, [80., 80.], [80., 80.]).unwrap();
    let report = anchor.diagnostic(
        &original,
        [161, 160],
        [80., 80.],
        Some([1., 0., 0., 1., 0., 0.]),
    );
    assert_eq!(report["matched"], false);
    assert_eq!(report["reason"], "image_dimensions_changed");
    for field in [
        "hit_changed_cells",
        "context_changed_cells",
        "context_trimmed_changed_cells",
        "stable_context_changed_cells",
    ] {
        assert!(report.as_object().unwrap().contains_key(field));
        assert!(report[field].is_null());
    }
    assert_eq!(
        report["failed_checks"],
        serde_json::json!(["image_dimensions_changed"])
    );
}
