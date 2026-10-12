//! Scoped visual selection only; never a Window/input fallback for native metadata.
use super::*;
pub(super) fn read(pid: u32, window: u32) -> Result<(Window, bool)> {
    select(pid, window, windows()?, || {
        let inventory = exact_window_inventory(window)?;
        if inventory.invalid_rows != 0 {
            return Err(fault(
                "visual_record_invalid",
                "Exact native row has unproven geometry/owner; no image.",
            ));
        }
        Ok(inventory.windows)
    })
    .map_err(|error| {
        if error.code == "window_missing" {
            window_record_probe::missing(pid, window, window_record_probe::read(window))
        } else {
            error
        }
    })
}

pub(super) fn select(
    pid: u32,
    window: u32,
    onscreen: Vec<Window>,
    exact: impl FnOnce() -> Result<Vec<Window>>,
) -> Result<(Window, bool)> {
    let matching: Vec<_> = onscreen
        .into_iter()
        .filter(|w| w.pid == pid && w.id == window)
        .collect();
    match matching.len() {
        1 => return Ok((matching.into_iter().next().unwrap(), true)),
        0 => {}
        _ => {
            return Err(fault(
                "window_binding_ambiguous",
                "More than one on-screen row matches; no fallback.",
            ));
        }
    }
    let rows = exact()?;
    if rows.len() > 1 {
        return Err(fault(
            "window_binding_ambiguous",
            "Exact-ID visual query returned more than one row.",
        ));
    }
    let row = rows.into_iter().next().ok_or_else(|| {
        fault(
            "window_missing",
            "Exact-ID visual record absent; no substitution.",
        )
    })?;
    if row.id != window || row.pid != pid {
        return Err(fault(
            "window_owner_or_identity_changed",
            "Exact visual ID/owner does not match the binding.",
        ));
    }
    Ok((row, false))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn row(pid: u32, id: u32) -> Window {
        Window {
            pid,
            id,
            rect: Rect {
                x: 0.,
                y: 0.,
                w: 10.,
                h: 10.,
            },
            alpha: 1.,
            title: None,
        }
    }
    #[test]
    fn onscreen_match_never_queries_offscreen() {
        assert!(
            select(1, 2, vec![row(1, 2)], || panic!("unexpected query"))
                .unwrap()
                .1
        );
    }
    #[test]
    fn exact_offscreen_match_is_observation_only() {
        assert!(!select(1, 2, vec![], || Ok(vec![row(1, 2)])).unwrap().1);
    }
    #[test]
    fn ambiguous_onscreen_never_queries_offscreen() {
        assert_eq!(
            select(1, 2, vec![row(1, 2), row(1, 2)], || panic!(
                "unexpected query"
            ))
            .err()
            .unwrap()
            .code,
            "window_binding_ambiguous"
        );
    }
    #[test]
    fn exact_owner_or_id_substitution_refuses() {
        for w in [row(3, 2), row(1, 3)] {
            assert_eq!(
                select(1, 2, vec![], || Ok(vec![w])).err().unwrap().code,
                "window_owner_or_identity_changed"
            );
        }
    }
    #[test]
    fn missing_or_ambiguous_exact_refuses() {
        for count in [0, 2] {
            assert!(select(1, 2, vec![], || Ok((0..count).map(|_| row(1, 2)).collect())).is_err());
        }
    }
    #[test]
    fn exact_query_failure_keeps_original_error() {
        assert_eq!(
            select(1, 2, vec![], || Err(fault("query_failure", "original")))
                .err()
                .unwrap()
                .code,
            "query_failure"
        );
    }
}
