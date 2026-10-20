use super::*;

fn assert_creation_failure(
    encoded: &[u8],
    size: [u32; 2],
    pixel: [f64; 2],
    expected: CreationFailure,
    feature_calls: usize,
) {
    reset_feature_decode_count();
    assert_eq!(
        PixelAnchor::from_png_diagnosed(encoded, size, pixel, [80., 80.]).err(),
        Some(expected)
    );
    assert_eq!(feature_decode_count(), feature_calls);
    reset_feature_decode_count();
    assert!(PixelAnchor::from_png(encoded, size, pixel, [80., 80.]).is_none());
    assert_eq!(feature_decode_count(), feature_calls);
}

#[test]
fn creation_preconditions_preserve_option_admission_without_decode() {
    let (rgb, size) = fixture();
    let encoded = png(&rgb, size);
    assert_creation_failure(
        &encoded,
        [95, 160],
        [80., 80.],
        CreationFailure::ImageTooSmall,
        0,
    );
    assert_creation_failure(
        &encoded,
        size,
        [f64::NAN, 80.],
        CreationFailure::PixelNotFinite,
        0,
    );
    for pixel in [[47.99, 80.], [80., 47.99], [112., 80.], [80., 112.]] {
        assert_creation_failure(
            &encoded,
            size,
            pixel,
            CreationFailure::PixelEdgeClearance,
            0,
        );
    }
    assert_eq!(CreationFailure::ImageTooSmall.as_str(), "image_too_small");
    assert_eq!(CreationFailure::PixelNotFinite.as_str(), "pixel_not_finite");
    assert_eq!(
        CreationFailure::PixelEdgeClearance.as_str(),
        "pixel_edge_clearance"
    );
}

#[test]
fn creation_png_failures_preserve_option_and_record_the_actual_decode_stage() {
    let (rgb, size) = fixture();
    let encoded = png(&rgb, size);
    assert_creation_failure(
        b"not-a-png",
        size,
        [80., 80.],
        CreationFailure::PngHeaderDecodeFailed,
        1,
    );
    assert_creation_failure(
        &encoded,
        [161, 160],
        [80., 80.],
        CreationFailure::PngDimensionsMismatch,
        1,
    );
    let idat = encoded
        .windows(4)
        .position(|bytes| bytes == b"IDAT")
        .unwrap();
    assert_creation_failure(
        &encoded[..idat + 5],
        size,
        [80., 80.],
        CreationFailure::PngFrameDecodeFailed,
        1,
    );
    assert_eq!(
        CreationFailure::PngHeaderDecodeFailed.as_str(),
        "png_header_decode_failed"
    );
    assert_eq!(
        CreationFailure::PngDimensionsMismatch.as_str(),
        "png_dimensions_mismatch"
    );
    assert_eq!(
        CreationFailure::PngFrameDecodeFailed.as_str(),
        "png_frame_decode_failed"
    );
}

#[test]
fn creation_input_budgets_preserve_option_without_extra_feature_calls() {
    let oversized = vec![0; crate::MAX_FRAME_BYTES + 1];
    assert_creation_failure(
        &oversized,
        [160, 160],
        [80., 80.],
        CreationFailure::PngByteBudgetExceeded,
        1,
    );
    assert_creation_failure(
        b"not-decoded-when-pixel-budget-is-exceeded",
        [4001, 4000],
        [80., 80.],
        CreationFailure::ImagePixelBudgetExceeded,
        1,
    );
    assert_eq!(
        CreationFailure::PngByteBudgetExceeded.as_str(),
        "png_byte_budget_exceeded"
    );
    assert_eq!(
        CreationFailure::ImagePixelBudgetExceeded.as_str(),
        "image_pixel_budget_exceeded"
    );
}

#[test]
fn creation_luma_reason_does_not_infer_a_visually_blank_patch() {
    let size = [160, 160];
    let uniform = png(&vec![90; 160 * 160 * 3], size);
    assert_creation_failure(
        &uniform,
        size,
        [80., 80.],
        CreationFailure::LumaSpanBelowMinimum,
        1,
    );
    // Different RGB colors may still have the same existing average luma.
    // The reason describes that actual feature gate, not visual uniformity.
    let mut colored = vec![0; 160 * 160 * 3];
    for y in 0..160 {
        for x in 0..160 {
            let color = if (x / 6 + y / 6) % 2 == 0 {
                [255, 0, 0]
            } else {
                [0, 255, 0]
            };
            let at = (y * 160 + x) * 3;
            colored[at..at + 3].copy_from_slice(&color);
        }
    }
    assert_creation_failure(
        &png(&colored, size),
        size,
        [80., 80.],
        CreationFailure::LumaSpanBelowMinimum,
        1,
    );
    assert_eq!(
        CreationFailure::LumaSpanBelowMinimum.as_str(),
        "luma_span_below_minimum"
    );
}

#[test]
fn diagnosed_creation_success_preserves_features_and_decodes_once() {
    let (rgb, size) = fixture();
    let encoded = png(&rgb, size);
    reset_feature_decode_count();
    let diagnosed =
        PixelAnchor::from_png_diagnosed(&encoded, size, [80., 80.], [80., 80.]).unwrap();
    assert_eq!(feature_decode_count(), 1);
    reset_feature_decode_count();
    let compatible = PixelAnchor::from_png(&encoded, size, [80., 80.], [80., 80.]).unwrap();
    assert_eq!(feature_decode_count(), 1);
    assert_eq!(diagnosed.image_size, compatible.image_size);
    assert_eq!(diagnosed.pixel, compatible.pixel);
    assert_eq!(diagnosed.desktop, compatible.desktop);
    assert_eq!(diagnosed.origin, compatible.origin);
    assert_eq!(diagnosed.features, compatible.features);
    // The old constructor did not validate desktop coordinates. Resolution
    // and later native mapping checks own that gate; do not add one here.
    for desktop in [[f64::NAN, 80.], [80., f64::INFINITY]] {
        reset_feature_decode_count();
        assert!(PixelAnchor::from_png_diagnosed(&encoded, size, [80., 80.], desktop).is_ok());
        assert_eq!(feature_decode_count(), 1);
        reset_feature_decode_count();
        assert!(PixelAnchor::from_png(&encoded, size, [80., 80.], desktop).is_some());
        assert_eq!(feature_decode_count(), 1);
    }
    println!(
        "anchor_creation_success: result=some; compatibility=some; feature_extractions=1; input_authority=false"
    );
}

#[test]
fn feature_decode_counter_is_thread_local_and_resettable() {
    reset_feature_decode_count();
    let (rgb, size) = fixture();
    assert!(features(&png(&rgb, size), size, [32, 32]).is_some());
    assert_eq!(feature_decode_count(), 1);
    std::thread::spawn(|| {
        reset_feature_decode_count();
        assert_eq!(feature_decode_count(), 0);
        let (rgb, size) = fixture();
        assert!(features(&png(&rgb, size), size, [32, 32]).is_some());
        assert_eq!(feature_decode_count(), 1);
    })
    .join()
    .unwrap();
    assert_eq!(feature_decode_count(), 1);
    reset_feature_decode_count();
    assert_eq!(feature_decode_count(), 0);
}
