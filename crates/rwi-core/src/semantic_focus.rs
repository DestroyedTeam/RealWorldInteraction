//! Stage A: explicit semantic element-focus policy, NOT an executable backend.
//! Native facts are trusted backend inputs (not Deserialize/MCP arguments).
//! This module posts nothing, sets no AX attribute, and advertises no tool.
use crate::{ForegroundState, Resolved, Result, Scene, fault};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FocusElementRequest {
    pub target: String,
    pub frame: String,
    pub element: String,
    pub request_id: String,
}
impl FocusElementRequest {
    pub fn validate(&self) -> Result<()> {
        for (value, max) in [
            (&self.target, 512),
            (&self.frame, 128),
            (&self.element, 512),
            (&self.request_id, 128),
        ] {
            if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
                return Err(fault(
                    "invalid_focus_element_request",
                    "Use bounded exact service identities; no value, action, options or fallback.",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPlatform {
    Macos,
    Windows,
    Linux,
    Other,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExactEvidence {
    Verified,
    Different,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditableClass {
    PlainEditable,
    PlainReadOnly,
    Secure,
    NotText,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationFocusCustody {
    NoActiveForeignSibling,
    ActiveForeignSibling,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeOrigin {
    FreshExactObject,
    ObservationSidecar,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseCustody {
    HeldExactDomainTarget,
    Expired,
    Different,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalCustody {
    NewExactReservation,
    PendingOrUncertain,
    Conflict,
    AlreadyAttempted,
    Unknown,
}

/// Raw native read status and an independently type-checked Boolean.
/// status=0 + value=None remains unknown; an error cannot be overridden by true.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct FocusBooleanRead {
    pub native_status: i32,
    pub value: Option<bool>,
}
impl FocusBooleanRead {
    fn known(&self, attribute: &str) -> Result<bool> {
        if self.native_status != 0 || self.value.is_none() {
            let mut error = fault(
                "focus_native_probe_unknown",
                "Read this exact native object anew; no cached-sidecar or input fallback.",
            );
            error.diagnostic = Some(Box::new(serde_json::json!({
                "attribute": attribute, "native_status": self.native_status,
                "value": self.value, "input_authority": false
            })));
            return Err(error);
        }
        Ok(self.value.expect("checked native Boolean"))
    }
}
/// No Deserialize: none of these facts can be supplied by an MCP/model payload.
/// Future macOS code must obtain them from exact retained CF/AX objects and fresh
/// APIs under its own native custody guards, not roles/names/geometry matching.
#[derive(Debug, Clone)]
pub struct FocusNativeFacts {
    pub platform: FocusPlatform,
    /// Fresh app AXFocusedWindow + OS foreground exact-window custody; not PID equality.
    pub application_focus_custody: ApplicationFocusCustody,
    pub input_principal: ExactEvidence,
    pub accessibility_permission: ExactEvidence,
    pub retained_element: ExactEvidence,
    pub element_window_cf: ExactEvidence,
    pub process_lifetime: ExactEvidence,
    pub cg_window_binding: ExactEvidence,
    pub native_context: ExactEvidence,
    pub editable_class: EditableClass,
    pub probe_origin: ProbeOrigin,
    pub value_writable: FocusBooleanRead,
    pub focused_writable: FocusBooleanRead,
    pub focused: FocusBooleanRead,
}
/// Future Engine must mint custody from its actual checked shared lease and
/// durable create_new/sync_all reservation. Test values here are only mocks.
#[derive(Debug, Clone, Copy)]
pub struct FocusCustody {
    pub lease: LeaseCustody,
    pub journal: JournalCustody,
}
pub struct FocusPolicyContext<'a> {
    pub request: &'a FocusElementRequest,
    pub reference: &'a Resolved,
    pub original: &'a Scene,
    pub fresh: &'a Scene,
    pub proposal_age_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusWritePlan {
    NoWriteAlreadyFocused,
    #[serde(rename = "set_ax_focused_true_once")]
    SetAXFocusedTrueOnce,
}
/// Pure policy decision only. It is NOT a native execution or authorization token.
pub fn assess_focus_element(
    c: FocusPolicyContext<'_>,
    native: &FocusNativeFacts,
    custody: FocusCustody,
) -> Result<FocusWritePlan> {
    c.request.validate()?;
    if native.platform != FocusPlatform::Macos {
        return Err(fault(
            "capability_unavailable",
            "Semantic AX element focus is macOS-specific; Windows/Linux/other backends have no fallback.",
        ));
    }
    if native.application_focus_custody != ApplicationFocusCustody::NoActiveForeignSibling {
        return Err(fault(
            "focus_application_custody_unknown",
            "An active user sibling or unknown app-focus custody forbids a setter plan. Future Native must recheck AXFocusedWindow + OS foreground and app-focus resource custody; PID equality is not isolation.",
        ));
    }
    if c.reference.placement_only
        || c.reference.native_action.is_some()
        || c.reference.scope.element.as_deref() != Some(&c.request.element)
        || c.reference.target != c.request.target
        || c.reference.frame != c.request.frame
        || c.reference.pixel_anchor_used
        || c.reference.pixel_anchor.is_some()
        || c.reference.red_marker_anchor.is_some()
    {
        return Err(fault(
            "exact_semantic_element_required",
            "Use an exact observed element, not a pixel/window/action/name surrogate.",
        ));
    }
    if c.proposal_age_ms > crate::INPUT_PROPOSAL_MAX_AGE_SECS * 1000
        || c.original.identity != c.fresh.identity
        || !c.original.mutation_ready
        || !c.fresh.mutation_ready
        || c.original.blocked != Some(false)
        || c.fresh.blocked != Some(false)
    {
        return Err(fault(
            "focus_context_changed",
            "Session, proposal age, or modal readiness changed; no setter.",
        ));
    }
    let old = c.original.binding.as_ref().ok_or_else(|| {
        fault(
            "target_binding_unknown",
            "Exact original native binding required.",
        )
    })?;
    let now = c.fresh.binding.as_ref().ok_or_else(|| {
        fault(
            "target_binding_unknown",
            "Exact fresh native binding required.",
        )
    })?;
    // Binding.focus_window is retained selected-window grounding, NOT a readback
    // of application's AXFocusedWindow or of global OS foreground.
    if old.scope != now.scope
        || old.focus_window != now.focus_window
        || old.layout != now.layout
        || old.native_epoch != now.native_epoch
        || old.mutation_context != now.mutation_context
        || c.reference.scope.window != old.scope.window
        || c.reference.scope.pid != old.scope.pid
    {
        return Err(fault(
            "focus_context_changed",
            "Exact native window/provider/layout grounding changed; no setter.",
        ));
    }
    let original = c
        .original
        .elements
        .iter()
        .find(|e| e.id == c.request.element)
        .ok_or_else(|| fault("element_changed", "Original exact element is absent."))?;
    let fresh = c
        .fresh
        .elements
        .iter()
        .find(|e| e.id == c.request.element)
        .ok_or_else(|| {
            fault(
                "element_changed",
                "Fresh exact element is absent; no same-name search.",
            )
        })?;
    if fresh.enabled != Some(true)
        || original.enabled != Some(true)
        || fresh.visible != Some(true)
        || original.visible != Some(true)
        || fresh.occluded != Some(false)
        || original.occluded != Some(false)
        || original.point != c.reference.point
        || fresh.point != c.reference.point
        || c.reference.element_revision.as_ref() != Some(&original.revision())
        || fresh.revision() != original.revision()
    {
        return Err(fault(
            "element_changed",
            "Exact element metadata/revision/enabled state changed or unknown.",
        ));
    }
    if [
        native.input_principal,
        native.accessibility_permission,
        native.retained_element,
        native.element_window_cf,
        native.process_lifetime,
        native.cg_window_binding,
        native.native_context,
    ]
    .iter()
    .any(|proof| *proof != ExactEvidence::Verified)
    {
        return Err(fault(
            "focus_native_identity_unknown",
            "Native principal, CF identity, window, lifetime or context is not independently exact; no setter.",
        ));
    }
    if native.editable_class != EditableClass::PlainEditable {
        return Err(fault(
            "plain_editable_element_required",
            "Plain editable only; secure, NotText, read-only and unknown are unavailable.",
        ));
    }
    if native.probe_origin != ProbeOrigin::FreshExactObject {
        return Err(fault(
            "fresh_focus_probe_required",
            "Requery the one exact element; observation sidecar/budget omission cannot authorize input.",
        ));
    }
    if !native.value_writable.known("AXValue writable")?
        || !native.focused_writable.known("AXFocused writable")?
    {
        return Err(fault(
            "focus_attribute_not_settable",
            "Fresh exact editable/focus attribute is not writable; no setter.",
        ));
    }
    let focused = native.focused.known("AXFocused")?;
    if custody.lease != LeaseCustody::HeldExactDomainTarget {
        return Err(fault(
            "focus_lease_unavailable",
            "Actual exact cooperative lease required; no setter.",
        ));
    }
    if custody.journal != JournalCustody::NewExactReservation {
        return Err(fault(
            "focus_request_not_new",
            "Pending, conflicting, attempted or unknown durable request cannot be replayed.",
        ));
    }
    Ok(if focused {
        FocusWritePlan::NoWriteAlreadyFocused
    } else {
        FocusWritePlan::SetAXFocusedTrueOnce
    })
}

/// Each axis must come from an independent post-operation native query.
/// No Deserialize/MCP setter parameters; global foreground is NOT inferred
/// from an element's AXFocused or the application's AXFocusedWindow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusProbeUnknownCause {
    CopyAttributeFailed,
    CopyReturnedNull,
    NotCfBoolean,
    NotAxElement,
    OwnerLookupFailed,
    ProviderValidationFailed,
}
/// Response-only evidence from an existing read, not a new probe or input ticket.
#[derive(Debug, Clone, Serialize)]
pub struct FocusReadbackProbe {
    #[serde(flatten)]
    pub raw: FocusBooleanRead,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unknown_cause: Option<FocusProbeUnknownCause>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_error_code: Option<String>,
}
impl FocusReadbackProbe {
    pub fn known(raw: FocusBooleanRead) -> Self {
        Self {
            raw,
            unknown_cause: None,
            validation_error_code: None,
        }
    }
    pub fn unknown(
        native_status: i32,
        cause: FocusProbeUnknownCause,
        validation_error_code: Option<&str>,
    ) -> Self {
        Self {
            raw: FocusBooleanRead {
                native_status,
                value: None,
            },
            unknown_cause: Some(cause),
            // Retain only a bounded UTF-8 prefix of the actual Fault.code, never
            // an invented numeric status from a later owner/provider validator.
            validation_error_code: validation_error_code.map(|code| {
                let mut end = code.len().min(64);
                while !code.is_char_boundary(end) {
                    end -= 1;
                }
                code[..end].to_owned()
            }),
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct FocusReadbackAxisPair {
    pub first: FocusReadbackProbe,
    pub second: FocusReadbackProbe,
    pub native_status_changed: bool,
    pub value_changed: bool,
}
impl FocusReadbackAxisPair {
    pub fn new(first: FocusReadbackProbe, second: FocusReadbackProbe) -> Self {
        Self {
            native_status_changed: first.raw.native_status != second.raw.native_status,
            value_changed: first.raw.value != second.raw.value,
            first,
            second,
        }
    }
}
/// Response-only public application coverage, not a global foreground/window ticket.
/// Native owns the retained object comparison; this type is deliberately not Deserialize.
#[derive(Debug, Clone, Serialize)]
pub struct FocusPublicApplicationReadback {
    pub scope: &'static str,
    pub before_pid: u32,
    pub before_process_start: String,
    pub after_pid: u32,
    pub after_process_start: String,
    pub retained_application_equal: Option<bool>,
    pub process_lifetime_equal: Option<bool>,
    pub full_foreground_window_known: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct FocusReadbackDiagnostics {
    pub version: u8,
    pub input_authority: bool,
    pub non_atomic_samples: bool,
    pub element_focused: FocusReadbackAxisPair,
    pub focused_ui_element_cf_match: FocusReadbackAxisPair,
    pub application_focused_window_cf_match: FocusReadbackAxisPair,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_application_custody: Option<FocusPublicApplicationReadback>,
}
impl FocusReadbackDiagnostics {
    pub fn new(
        element_focused: FocusReadbackAxisPair,
        focused_ui_element_cf_match: FocusReadbackAxisPair,
        application_focused_window_cf_match: FocusReadbackAxisPair,
    ) -> Self {
        Self {
            version: 1,
            input_authority: false,
            non_atomic_samples: true,
            element_focused,
            focused_ui_element_cf_match,
            application_focused_window_cf_match,
            public_application_custody: None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusSetterAttempt {
    NotAttempted,
    Attempted,
}
#[derive(Debug, Clone)]
pub struct FocusReadback {
    pub write_plan: FocusWritePlan,
    pub setter_attempt: FocusSetterAttempt,
    pub setter_status: Option<i32>,
    pub element_focused: FocusBooleanRead,
    pub focused_ui_element_cf_match: FocusBooleanRead,
    pub application_focused_window_cf_match: FocusBooleanRead,
    pub os_foreground_before: Option<ForegroundState>,
    pub os_foreground_after: Option<ForegroundState>,
    pub readback_diagnostics: Option<FocusReadbackDiagnostics>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusReadbackState {
    Ready,
    Partial,
    Unknown,
}
#[derive(Debug, Clone, Serialize)]
pub struct FocusReadbackSummary {
    pub write_plan: FocusWritePlan,
    pub setter_attempt: FocusSetterAttempt,
    pub setter_acknowledged: Option<bool>,
    pub state: FocusReadbackState,
    pub setter_status: Option<i32>,
    pub element_focused: FocusBooleanRead,
    pub focused_ui_element_cf_match: FocusBooleanRead,
    pub application_focused_window_cf_match: FocusBooleanRead,
    pub os_foreground_before: Option<ForegroundState>,
    pub os_foreground_after: Option<ForegroundState>,
    pub os_foreground_unchanged: Option<bool>,
    pub goal_verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub readback_diagnostics: Option<FocusReadbackDiagnostics>,
}
pub fn summarize_focus_readback(r: FocusReadback) -> FocusReadbackSummary {
    let e = r.element_focused.known("AXFocused").ok();
    let u = r
        .focused_ui_element_cf_match
        .known("AXFocusedUIElement exact CF")
        .ok();
    let w = r
        .application_focused_window_cf_match
        .known("AXFocusedWindow exact CF")
        .ok();
    let setter_acknowledged = if r.setter_attempt == FocusSetterAttempt::Attempted {
        r.setter_status.map(|status| status == 0)
    } else {
        None
    };
    let phase_known = matches!(
        (r.write_plan, r.setter_attempt, r.setter_status),
        (
            FocusWritePlan::NoWriteAlreadyFocused,
            FocusSetterAttempt::NotAttempted,
            None
        ) | (
            FocusWritePlan::SetAXFocusedTrueOnce,
            FocusSetterAttempt::Attempted,
            Some(0)
        )
    );
    let setter_uncertain = !phase_known;
    let state = if setter_uncertain || e.is_none() || u.is_none() || w.is_none() {
        FocusReadbackState::Unknown
    } else if e == Some(true) && u == Some(true) && w == Some(true) {
        FocusReadbackState::Ready
    } else {
        FocusReadbackState::Partial
    };
    let unchanged = r
        .os_foreground_before
        .as_ref()
        .zip(r.os_foreground_after.as_ref())
        .and_then(|(before, after)| {
            (before.validate().is_ok() && after.validate().is_ok()).then_some(before == after)
        });
    let keep_diagnostics = state == FocusReadbackState::Unknown
        || r.readback_diagnostics
            .as_ref()
            .is_some_and(|diagnostics| diagnostics.public_application_custody.is_some());
    FocusReadbackSummary {
        write_plan: r.write_plan,
        setter_attempt: r.setter_attempt,
        setter_acknowledged,
        state,
        setter_status: r.setter_status,
        element_focused: r.element_focused,
        focused_ui_element_cf_match: r.focused_ui_element_cf_match,
        application_focused_window_cf_match: r.application_focused_window_cf_match,
        os_foreground_before: r.os_foreground_before,
        os_foreground_after: r.os_foreground_after,
        os_foreground_unchanged: unchanged,
        goal_verified: state == FocusReadbackState::Ready,
        readback_diagnostics: if keep_diagnostics {
            r.readback_diagnostics
        } else {
            None
        },
    }
}

#[cfg(test)]
mod tests;
