//! Synthetic image-only regression for toolbar points; no capture or input.
use super::*;

#[test]
fn f405_edge_toolbar_keeps_complete_patch_and_all_content_mapping_guards() {
    let size = [200, 160];
    for pixel in [
        [100., 57.8745],
        [57.8745, 80.],
        [142.1255, 80.],
        [100., 102.1255],
    ] {
        let mut rgb = vec![245; size[0] as usize * size[1] as usize * 3];
        let [cx, cy] = pixel.map(|v: f64| v.floor() as usize);
        for y in cy - 18..cy + 18 {
            for x in cx - 18..cx + 18 {
                if (x / 6 + y / 6) % 2 == 0 {
                    let at = (y * size[0] as usize + x) * 3;
                    rgb[at..at + 3].fill(40);
                }
            }
        }
        let original = png(&rgb, size);
        let anchor = PixelAnchor::from_png_diagnosed(&original, size, pixel, pixel).unwrap();
        assert!(anchor.origin.iter().zip(size).all(|(o, n)| o + EDGE <= n));
        let transform = Some([1., 0., 0., 1., 0., 0.]);
        assert!(anchor.matches(&original, size, pixel, transform));
        // Unrelated repaint outside the full patch is not a new local target.
        rgb[..3].fill(0);
        assert!(anchor.matches(&png(&rgb, size), size, pixel, transform));
        for y in cy - 6..cy + 6 {
            for x in cx - 6..cx + 6 {
                let at = (y * size[0] as usize + x) * 3;
                rgb[at..at + 3].fill(0);
            }
        }
        assert!(!anchor.matches(&png(&rgb, size), size, pixel, transform));
        assert!(!anchor.matches(&original, size, [pixel[0] + 1., pixel[1]], transform));
        assert!(!anchor.matches(&original, size, pixel, None));
        assert!(!anchor.matches(&original, [201, 160], pixel, transform));
    }
}

#[test]
fn f405_edge_toolbar_does_not_enable_blank_or_incomplete_patches_or_red_marker_mode() {
    let size = [160, 160];
    let blank = png(&vec![245; 160 * 160 * 3], size);
    assert_eq!(
        PixelAnchor::from_png_diagnosed(&blank, size, [80., 57.87], [80., 57.87]).err(),
        Some(CreationFailure::LumaSpanBelowMinimum)
    );
    for pixel in [[47.99, 80.], [80., 47.99], [112., 80.], [80., 112.]] {
        assert_eq!(
            PixelAnchor::from_png_diagnosed(&blank, size, pixel, pixel).err(),
            Some(CreationFailure::PixelEdgeClearance)
        );
    }
    // The separate opt-in cluster detector retains its original geometry gate.
    assert!(RedMarkerAnchor::from_png(&blank, size, [80., 57.87], [80., 57.87]).is_none());
}
