use super::*;

const SIZE: [u32; 2] = [768, 768];
fn panel() -> Vec<u8> {
    let mut rgb = vec![255; 768 * 768 * 3];
    // Blank hit patch, but a visible surrounding border outside its 96px extent.
    for y in 300..468 {
        for x in 300..468 {
            if !(306..462).contains(&x) || !(306..462).contains(&y) {
                rgb[(y * 768 + x) * 3..(y * 768 + x) * 3 + 3].fill(20);
            }
        }
    }
    rgb
}
fn anchor(rgb: &[u8]) -> PixelAnchor {
    PixelAnchor::from_png(&png(rgb, SIZE), SIZE, [384., 384.], [384., 384.]).unwrap()
}
fn matches(a: &PixelAnchor, rgb: &[u8]) -> bool {
    a.matches(
        &png(rgb, SIZE),
        SIZE,
        [384., 384.],
        Some([1., 0., 0., 1., 0., 0.]),
    )
}
#[test]
fn f400_blank_center_matches_exact_context_despite_remote_update() {
    let rgb = panel();
    let a = anchor(&rgb);
    let mut remote = rgb;
    remote[..3].fill(0);
    assert!(a.context.is_some());
    assert!(matches(&a, &remote));
    let d = a.diagnostic(
        &png(&remote, SIZE),
        SIZE,
        [384., 384.],
        Some([1., 0., 0., 1., 0., 0.]),
    );
    assert_eq!(d["anchor_kind"], "exact_textured_context");
    assert_eq!(d["context"]["edge"], 192);
    assert_eq!(d["input_authority"], false);
}
#[test]
fn f400_context_or_small_subject_replacement_is_not_trimmed() {
    let rgb = panel();
    let a = anchor(&rgb);
    for (x, y) in [(384, 384), (301, 330), (470, 330)] {
        let mut changed = rgb.clone();
        let i = (y * 768 + x) * 3;
        changed[i..i + 3].fill(0);
        assert!(
            !matches(&a, &changed),
            "changed pixel in context must not be animation-trimmed"
        );
    }
}
#[test]
fn f400_wholly_uniform_surface_retains_original_failure() {
    let result = PixelAnchor::from_png_diagnosed(
        &png(&vec![255; 768 * 768 * 3], SIZE),
        SIZE,
        [384., 384.],
        [384., 384.],
    );
    assert_eq!(result.unwrap_err(), CreationFailure::LumaSpanBelowMinimum);
}
#[test]
fn f400_context_does_not_substitute_geometry_or_transform() {
    let rgb = panel();
    let a = anchor(&rgb);
    let p = png(&rgb, SIZE);
    assert!(!a.matches(&p, SIZE, [385., 384.], Some([1., 0., 0., 1., 0., 0.])));
    assert!(!a.matches(&p, SIZE, [384., 384.], None));
    assert!(!a.matches(&p, SIZE, [384., 384.], Some([1., 0., 0., 1., 1., 0.])));
    assert!(!a.matches(&p, [769, 768], [384., 384.], Some([1., 0., 0., 1., 0., 0.])));
}
#[test]
fn f400_context_creation_and_compare_decode_once_each() {
    let rgb = panel();
    reset_feature_decode_count();
    let a = anchor(&rgb);
    assert_eq!(feature_decode_count(), 1);
    reset_feature_decode_count();
    assert!(matches(&a, &rgb));
    assert_eq!(feature_decode_count(), 1);
}
#[test]
fn f400_context_hashes_visible_normalized_rgba_not_png_encoding() {
    let rgb = panel();
    let a = anchor(&rgb);
    let mut rgba = Vec::with_capacity(768 * 768 * 4);
    for px in rgb.chunks_exact(3) {
        rgba.extend_from_slice(px);
        rgba.push(255);
    }
    // Invisible RGB is normalized to white, exactly like the original decoder.
    let i = (384 * 768 + 384) * 4;
    rgba[i..i + 4].copy_from_slice(&[77, 88, 99, 0]);
    let mut p = Vec::new();
    {
        let mut e = png::Encoder::new(&mut p, 768, 768);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&rgba).unwrap();
    }
    assert!(a.matches(&p, SIZE, [384., 384.], Some([1., 0., 0., 1., 0., 0.])));
}
#[test]
fn f400_context_expands_only_to_fixed_bound_and_near_edges_stays_strict() {
    let mut rgb = vec![255; 768 * 768 * 3];
    for y in 250..450 {
        let i = (y * 768 + 230) * 3;
        rgb[i..i + 3].fill(20);
    }
    let a = anchor(&rgb);
    assert_eq!(a.context.as_ref().unwrap().diagnostic()["edge"], 384);
    assert!(PixelAnchor::from_png(&png(&rgb, SIZE), SIZE, [65., 65.], [65., 65.]).is_none());
}
#[test]
fn f400_a_single_noisy_pixel_is_not_context_texture() {
    let mut rgb = vec![255; 768 * 768 * 3];
    let i = (300 * 768 + 300) * 3;
    rgb[i..i + 3].fill(0);
    assert!(PixelAnchor::from_png(&png(&rgb, SIZE), SIZE, [384., 384.], [384., 384.]).is_none());
}
