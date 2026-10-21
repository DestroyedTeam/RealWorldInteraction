//! Bounded provenance from reads already performed. Never an input authority.
use super::{Cf, WindowCanvasProof, local_pixel_canvas, system_window_hit};
use rwi_core::{Result, fault};
#[derive(Debug, serde::Serialize)]
pub(super) struct PointHitEvidence {
    pub selection: &'static str,
    pub application_probe: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ax_error: Option<i64>,
}
impl Default for PointHitEvidence {
    fn default() -> Self {
        Self {
            selection: "not_selected",
            application_probe: "not_attempted",
            ax_error: None,
        }
    }
}
impl PointHitEvidence {
    pub(super) fn record_probe(&mut self, result: Result<Cf>) -> Result<Cf> {
        self.application_probe = "owner_unproven";
        if let Err(error) = &result {
            self.unavailable(error.diagnostic.as_deref());
        }
        result
    }
    pub(super) fn exact_application_hit(&mut self) {
        self.selection = "application_hit";
        self.application_probe = "exact_window_hit";
    }
    pub(super) fn tree_candidate(&mut self, candidate: Option<Cf>, window: &Cf) -> (Cf, bool) {
        self.selection = if candidate.is_some() {
            "exact_window_tree_candidate"
        } else {
            "window_root_geometry_fallback"
        };
        (candidate.unwrap_or_else(|| window.retained()), false)
    }
    pub(super) fn unavailable(&mut self, diagnostic: Option<&serde_json::Value>) {
        self.application_probe = "unavailable";
        self.ax_error = diagnostic
            .and_then(|d| d.get("ax_error"))
            .and_then(serde_json::Value::as_i64);
    }
}

pub(super) fn require_canvas(
    role: Option<&str>,
    proof: WindowCanvasProof,
    point: [f64; 2],
    point_hit_evidence: PointHitEvidence,
    system_hit_evidence: Option<system_window_hit::Evidence>,
) -> Result<()> {
    if local_pixel_canvas(role, proof) {
        return Ok(());
    }
    let mut error = fault(
        "local_pixel_surface_changed",
        "Local pixel grounding needs AXScrollArea, the existing positive childless application hit, or an independently positive exact system-window hit; missing AX children or geometry alone is not proof. No input dispatched.",
    );
    error.diagnostic = Some(Box::new(serde_json::json!({
        "reason":"local_pixel_canvas_unproven", "actual_role":role,
        "window_canvas_proof":proof, "point":point,
        "local_anchor_exception_selected":true,
        "point_hit_evidence":point_hit_evidence,
        "system_hit_evidence":system_hit_evidence,
        "input_authority":false, "native_input_dispatched":false, "replay_allowed":false,
    })));
    Err(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn f406_failed_application_probe_retains_original_error_not_an_occlusion_claim() {
        let mut evidence = PointHitEvidence::default();
        evidence.unavailable(Some(&serde_json::json!({"ax_error":-25204})));
        evidence.selection = "window_root_geometry_fallback";
        let value = serde_json::to_value(evidence).unwrap();
        assert_eq!(value["ax_error"], -25204);
        assert_eq!(value["application_probe"], "unavailable");
        assert_eq!(value["selection"], "window_root_geometry_fallback");
        assert!(value.get("input_authority").is_none());
        assert!(value.get("occluded").is_none());
    }
    #[test]
    fn f406_missing_original_error_stays_missing_instead_of_being_reconstructed() {
        let mut evidence = PointHitEvidence::default();
        evidence.unavailable(None);
        assert!(
            serde_json::to_value(evidence)
                .unwrap()
                .get("ax_error")
                .is_none()
        );
    }
    #[test]
    fn f406_canvas_diagnostics_do_not_relax_the_original_positive_predicate() {
        let proof = WindowCanvasProof {
            application_hit: false,
            exact_window_object: true,
            same_pid: true,
            exact_window_number: true,
            children_count: Some(3),
            system_window_hit: Some(false),
        };
        let error = require_canvas(
            Some("AXWindow"),
            proof,
            [10., 20.],
            PointHitEvidence::default(),
            None,
        )
        .unwrap_err();
        assert_eq!(error.code, "local_pixel_surface_changed");
        let diagnostic = error.diagnostic.unwrap();
        assert_eq!(diagnostic["reason"], "local_pixel_canvas_unproven");
        assert_eq!(diagnostic["point"], serde_json::json!([10., 20.]));
        for key in [
            "input_authority",
            "native_input_dispatched",
            "replay_allowed",
        ] {
            assert_eq!(diagnostic[key], false);
        }
        assert!(
            require_canvas(
                Some("AXWindow"),
                WindowCanvasProof {
                    system_window_hit: Some(true),
                    ..proof
                },
                [10., 20.],
                PointHitEvidence::default(),
                None
            )
            .is_ok()
        );
    }
}
