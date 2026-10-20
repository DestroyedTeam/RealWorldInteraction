//! Explicit extra Live visual selections; never an inferred owner or input target.
use super::*;

impl NativeMac {
    /// A caller-selected extra visual source, never an inferred owner edge or
    /// an input backend. Recreate the exact current native fingerprint once.
    pub(crate) fn additional_visual_source(
        &self,
        selection: &rwi_core::WindowSelection,
    ) -> Result<Self> {
        if selection.pid == std::process::id()
            || selection.window == format!("cgwindow:{}", self.window)
        {
            return Err(fault(
                "invalid_live_sources",
                "Do not select the root twice or capture an RWI presenter.",
            ));
        }
        let records = windows()?;
        let matches = records
            .iter()
            .filter(|window| {
                window.pid == selection.pid && format!("cgwindow:{}", window.id) == selection.window
            })
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(fault(
                "window_not_discovered",
                "The explicit additional visual source is absent or ambiguous; no substitute is selected.",
            ));
        }
        let window = matches[0];
        let process = process_epoch(window.pid)?;
        let record = record_for_visual(&self.identity, window, process.clone(), &displays()?)?;
        if record.native_epoch != selection.native_epoch
            || !matches!(selection.relation, rwi_core::WindowRelation::Initial)
        {
            return Err(fault(
                "window_epoch_changed",
                "Discover the exact additional visual window again; no ownership or title alias is inferred.",
            ));
        }
        let copy = Self::with_shared_resources(
            window.pid,
            window.id,
            false,
            Capturer::default(),
            self.dispatches.clone(),
        )?;
        if copy.identity != self.identity || copy.process != process {
            return Err(fault(
                "session_or_process_changed",
                "The additional visual source changed native context or process birth.",
            ));
        }
        Ok(copy)
    }
}
