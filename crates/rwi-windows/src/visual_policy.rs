//! Portable pre/post policy for non-grounding visible-desktop captures.
use crate::WindowRecord;
use rwi_core::{Result, fault};
use rwi_imaging::Rect;
pub(crate) fn rectangle(window: &WindowRecord) -> Result<Rect> {
    if !window.visible || window.minimized || window.cloaked != Some(false) {
        return Err(fault(
            "window_not_visible",
            "Visual capture never restores/activates a hidden, minimized or unknown-cloak window.",
        ));
    }
    if window.pid == 0
        || window.process_created.is_none_or(|v| v == 0)
        || window.native_epoch.as_ref().is_none_or(|s| s.is_empty())
        || window.window_dpi.is_none_or(|v| v == 0)
    {
        return Err(fault(
            "visual_native_identity_unavailable",
            "Exact process/window epoch and awareness-qualified window DPI must be available.",
        ));
    }
    window
        .dwm_visible_frame
        .or(window.rectangle)
        .ok_or_else(|| {
            fault(
                "window_geometry_unavailable",
                "Visual window geometry is unavailable or exceeds its source budget.",
            )
        })?
        .checked()
}
pub(crate) fn unchanged(before: &WindowRecord, after: &WindowRecord) -> Result<()> {
    rectangle(after)?;
    if before.key() != after.key()
        || before.native_epoch != after.native_epoch
        || before.visible != after.visible
        || before.minimized != after.minimized
        || before.cloaked != after.cloaked
        || before.rectangle != after.rectangle
        || before.dwm_visible_frame != after.dwm_visible_frame
        || before.window_dpi != after.window_dpi
        || before.window_dpi_awareness != after.window_dpi_awareness
    {
        return Err(fault(
            "visual_window_changed",
            "Native window identity, visibility, geometry or DPI changed during capture; discard the visual.",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn window() -> WindowRecord {
        WindowRecord {
            window: "hwnd:1".into(),
            pid: 42,
            process_created: Some(7),
            native_epoch: Some("epoch".into()),
            title: "selected".into(),
            title_truncated: false,
            class: "fixture".into(),
            rectangle: Some(Rect {
                x: -1200,
                y: -300,
                width: 800,
                height: 600,
            }),
            dwm_visible_frame: None,
            visible: true,
            enabled: false,
            minimized: false,
            cloaked: Some(false),
            foreground: false,
            window_dpi: Some(144),
            window_dpi_awareness: 2,
            owner_chain: vec![],
            owner_chain_complete: true,
            parent: None,
            z_index: 0,
        }
    }
    #[test]
    fn nonforeground_disabled_windows_still_have_readonly_visual_geometry() {
        let w = window();
        assert_eq!(
            rectangle(&w).unwrap(),
            Rect {
                x: -1200,
                y: -300,
                width: 800,
                height: 600
            }
        );
        unchanged(&w, &w).unwrap();
    }
    #[test]
    fn window_lifetime_visibility_geometry_and_dpi_changes_are_rejected() {
        let before = window();
        for field in 0..10 {
            let mut after = before.clone();
            match field {
                0 => after.pid += 1,
                1 => after.process_created = Some(8),
                2 => after.native_epoch = Some("new".into()),
                3 => after.visible = false,
                4 => after.minimized = true,
                5 => after.cloaked = None,
                6 => after.rectangle.as_mut().unwrap().x -= 1,
                7 => after.window_dpi = Some(192),
                8 => after.window_dpi_awareness = 1,
                _ => {
                    after.dwm_visible_frame = Some(Rect {
                        x: 0,
                        y: 0,
                        width: 1,
                        height: 1,
                    })
                }
            }
            assert!(unchanged(&before, &after).is_err(), "field {field}");
        }
        let mut oversized = window();
        oversized.rectangle.as_mut().unwrap().width = 100_000;
        assert!(rectangle(&oversized).is_err());
    }
}
