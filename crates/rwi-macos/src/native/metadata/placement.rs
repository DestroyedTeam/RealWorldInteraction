//! Pure bounded popup placement geometry, never input authority.
use super::*;

/// Bounded best-effort placement. All same-application windows remain obstacles,
/// even when only the first 64 contribute candidate edges. Native input still
/// checks actual hit ownership after the move; free geometry is not authority.
pub(super) fn panel_placement_position(
    window: Rect,
    display: Rect,
    obstacles: &[Rect],
) -> (f64, f64) {
    let base = crate::native::placement_position(window, display);
    let max_x = display.x + display.w - window.w;
    let max_y = display.y + display.h - window.h;
    let mut candidates = vec![base, (max_x, base.1), (base.0, max_y), (max_x, max_y)];
    for rect in obstacles.iter().filter(|rect| rect.valid()).take(64) {
        candidates.extend([
            (rect.x + rect.w + 24.0, base.1),
            (base.0, rect.y + rect.h + 24.0),
            (rect.x - window.w - 24.0, base.1),
            (base.0, rect.y - window.h - 24.0),
        ]);
    }
    candidates
        .into_iter()
        .filter(|&(x, y)| {
            x >= display.x
                && x <= max_x
                && y >= display.y
                && y <= max_y
                && obstacles.iter().filter(|rect| rect.valid()).all(|rect| {
                    x + window.w <= rect.x
                        || x >= rect.x + rect.w
                        || y + window.h <= rect.y
                        || y >= rect.y + rect.h
                })
        })
        .min_by(|a, b| {
            let distance = |p: &(f64, f64)| (p.0 - base.0).powi(2) + (p.1 - base.1).powi(2);
            distance(a).total_cmp(&distance(b))
        })
        .unwrap_or(base)
}

#[cfg(test)]
#[test]
fn panel_placement_avoids_covered_origin_without_moving_other_windows() {
    let rect = |x, y, w, h| Rect { x, y, w, h };
    let display = rect(1512.0, 0.0, 1920.0, 1080.0);
    let panel = rect(18.0, 52.0, 341.0, 367.0);
    let obstacles = [
        rect(1536.0, 24.0, 1434.0, 660.0),
        rect(1536.0, 24.0, 1220.0, 784.0),
    ];
    assert_eq!(
        panel_placement_position(panel, display, &obstacles),
        (2994.0, 24.0)
    );
    assert_eq!(
        panel_placement_position(panel, display, &[]),
        (1536.0, 24.0)
    );
    assert_eq!(
        panel_placement_position(panel, display, &[display]),
        (1536.0, 24.0)
    );
    let negative = rect(-1920.0, -1080.0, 1920.0, 1080.0);
    assert_eq!(
        panel_placement_position(panel, negative, &[rect(-1896.0, -1056.0, 1434.0, 784.0)]),
        (-438.0, -1056.0)
    );
}
