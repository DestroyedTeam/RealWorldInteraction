use super::panel_size;
#[test]
fn detailed_capture_keeps_native_preview_compact() {
    let (width, height) = panel_size(2048, 1800., 1169., Some([1320, 1104]));
    assert!(
        width <= 220.,
        "high-detail capture must not occupy most of the screen: {width}"
    );
    assert!(
        height <= super::MAX_PREVIEW_HEIGHT_PT,
        "source-aspect overview must remain compact: {height}"
    );
    let wide = panel_size(2048, 1920., 1080., Some([2048, 942]));
    assert!(wide.0 <= 220. && wide.1 <= 150.);
}
#[test]
fn display_only_bound_does_not_depend_on_high_detail_capture_edge() {
    assert_eq!(super::MAX_PREVIEW_WIDTH_PT, 220.);
    assert_eq!(super::MAX_PREVIEW_HEIGHT_PT, 220.);
    for max_edge in [640, 1024, 2048, 4096] {
        let (width, height) = panel_size(max_edge, 1800., 1169., Some([1320, 1104]));
        assert_eq!(width, 220.);
        assert!((height - 218.61818181818182).abs() < 0.000001);
    }
}
#[test]
fn portrait_preview_shrinks_with_proportional_fit_and_clear_gutters() {
    let (width, height) = panel_size(2048, 1800., 1169., Some([252, 420]));
    assert_eq!((width, height), (160., 220.));
    let (fit_width, fit_height) = proportional_fit(width, height, [252, 420]);
    assert!((fit_width - 103.2).abs() < 0.000001);
    assert!((fit_height - 172.).abs() < 0.000001);
    assert!((fit_width / fit_height - 0.6).abs() < 0.000001);
}
fn proportional_fit(width: f64, height: f64, source: [u64; 2]) -> (f64, f64) {
    let [source_width, source_height] = source.map(|n| n as f64);
    let scale = ((width - 16.) / source_width).min((height - super::CHROME_HEIGHT) / source_height);
    (source_width * scale, source_height * scale)
}
#[test]
fn dual_axis_cap_fixed_geometry_matrix_keeps_complete_source_in_viewport() {
    for (display, source, expected) in [
        ([1920., 1080.], [1600, 1920], [160., 220.]),
        ([1800., 1169.], [1320, 1104], [220., 218.61818181818182]),
        ([1920., 1080.], [2048, 942], [220., 141.83203125]),
        ([1920., 1080.], [2, 1], [220., 150.]),
        ([1800., 1169.], [252, 420], [160., 220.]),
        ([1920., 1080.], [320, 3200], [160., 220.]),
        ([1920., 1080.], [3840, 320], [220., 65.]),
        ([300., 300.], [1600, 1920], [160., 204.]),
        ([500., 200.], [1600, 1920], [160., 104.]),
        ([208., 316.], [1600, 1920], [160., 220.]),
    ] {
        let (width, height) = panel_size(2048, display[0], display[1], Some(source));
        assert!((width - expected[0]).abs() < 0.000001);
        assert!((height - expected[1]).abs() < 0.000001);
        assert!(width.is_finite() && height.is_finite());
        assert!(width <= super::MAX_PREVIEW_WIDTH_PT);
        assert!(height <= super::MAX_PREVIEW_HEIGHT_PT);
        let (fit_width, fit_height) = proportional_fit(width, height, source);
        assert!(fit_width <= width - 16. + 0.000001);
        assert!(fit_height <= height - super::CHROME_HEIGHT + 0.000001);
        assert!((fit_width / fit_height - source[0] as f64 / source[1] as f64).abs() < 0.000001);
    }
    for source in [None, Some([0, 1920]), Some([1600, 0])] {
        assert_eq!(panel_size(2048, 1920., 1080., source), (220., 150.));
    }
}
#[test]
fn preview_buttons_distinguish_hide_resume_and_close_without_minimize() {
    assert_eq!(super::preview_control_icon("隐藏预览"), "eye.slash");
    assert_eq!(super::preview_control_icon("继续预览"), "play.fill");
    assert_eq!(super::preview_control_icon("关闭预览"), "xmark");
}
