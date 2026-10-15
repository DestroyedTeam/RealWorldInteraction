//! Pure native pointer-surface policy. No OS reads, input, focus or fallback.
use rwi_core::{Result, fault};

// Positive window-kind evidence only; non-main is not a dialog classification.
// Raw AXModal is distinct from modal_state's "blocked"/focused-window policy.
pub(super) fn native_pointer_requires_axpress(
    role: Option<&str>,
    subrole: Option<&str>,
    modal: Option<bool>,
) -> Result<bool> {
    if role == Some("AXSheet")
        || (role == Some("AXWindow") && matches!(subrole, Some("AXDialog" | "AXSystemDialog")))
        || modal == Some(true)
    {
        return Ok(true);
    }
    if role == Some("AXWindow") && subrole == Some("AXStandardWindow") && modal == Some(false) {
        return Ok(false);
    }
    let mut error = fault(
        "native_click_surface_unknown",
        "The exact window's role/subrole/raw modality does not attest an ordinary pointer surface or a protected dialog. Inspect fresh native window facts; no AXPress, focus, fallback or replay was issued.",
    );
    error.diagnostic = Some(Box::new(serde_json::json!({
        "stage":"macos_pointer_surface_classification", "role":role, "subrole":subrole,
        "raw_modal":modal, "native_input_dispatched":false, "replay_allowed":false,
    })));
    Err(error)
}
