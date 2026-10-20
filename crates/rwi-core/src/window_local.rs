//! Explicit window-local main/key selection, not element focus or input readiness.
//! Native facts are trusted typed backend data, never model/cached-sidecar JSON.
use crate::semantic_focus::{
    ApplicationFocusCustody, ExactEvidence, FocusBooleanRead, FocusCustody, FocusPlatform,
    FocusPublicApplicationReadback, FocusSetterAttempt, JournalCustody, LeaseCustody, ProbeOrigin,
};
use crate::{ForegroundState, Resolved, Result, Scene, fault};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WindowLocalKind {
    Main,
    Key,
}
impl WindowLocalKind {
    pub fn attribute(self) -> &'static str {
        match self {
            Self::Main => "AXMain",
            Self::Key => "AXFocused",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WindowLocalRequest {
    pub target: String,
    pub frame: String,
    pub kind: WindowLocalKind,
    pub request_id: String,
}
impl WindowLocalRequest {
    pub fn validate(&self) -> Result<()> {
        for (value, max) in [
            (&self.target, 512),
            (&self.frame, 128),
            (&self.request_id, 128),
        ] {
            if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
                return Err(fault(
                    "invalid_window_local_request",
                    "Use bounded target/frame/request_id and explicit main|key; no implicit focus, options or value.",
                ));
            }
        }
        Ok(())
    }
}

/// No Deserialize; all identity proofs originate from fresh exact native APIs.
#[derive(Debug, Clone)]
pub struct WindowLocalNativeFacts {
    pub kind: WindowLocalKind,
    pub platform: FocusPlatform,
    pub application_focus_custody: ApplicationFocusCustody,
    pub input_principal: ExactEvidence,
    pub accessibility_permission: ExactEvidence,
    pub retained_window: ExactEvidence,
    pub provider_lifetime: ExactEvidence,
    pub process_lifetime: ExactEvidence,
    pub cg_window_binding: ExactEvidence,
    pub native_context: ExactEvidence,
    pub probe_origin: ProbeOrigin,
    pub main_writable: FocusBooleanRead,
    pub focused_writable: FocusBooleanRead,
    pub window_main: FocusBooleanRead,
    pub application_main_window_cf_match: FocusBooleanRead,
    pub window_focused: FocusBooleanRead,
    pub application_focused_window_cf_match: FocusBooleanRead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowLocalWritePlan {
    NoWriteAlreadySelected,
    SetAttributeTrueOnce,
}

pub fn known_boolean(v: FocusBooleanRead, attribute: &str) -> Result<bool> {
    if v.native_status != 0 || v.value.is_none() {
        let mut error = fault(
            "window_local_probe_unknown",
            "Requery the exact window; missing/raw-error data cannot authorize a setter.",
        );
        error.diagnostic = Some(Box::new(
            serde_json::json!({"attribute":attribute,"native_status":v.native_status,"value":v.value,"input_authority":false}),
        ));
        return Err(error);
    }
    Ok(v.value.expect("checked native boolean"))
}

pub struct WindowLocalPolicyContext<'a> {
    pub request: &'a WindowLocalRequest,
    pub reference: &'a Resolved,
    pub original: &'a Scene,
    pub fresh: &'a Scene,
    pub proposal_age_ms: u64,
}

/// Pure policy only, not an authority token. Backend must recheck before mutation.
pub fn assess_window_local(
    c: WindowLocalPolicyContext<'_>,
    native: &WindowLocalNativeFacts,
    custody: FocusCustody,
) -> Result<WindowLocalWritePlan> {
    c.request.validate()?;
    if native.platform != FocusPlatform::Macos || native.kind != c.request.kind {
        return Err(fault(
            "capability_unavailable",
            "This explicit kind has no exact supported macOS provider route; never use Main as Key or another platform fallback.",
        ));
    }
    if native.application_focus_custody != ApplicationFocusCustody::NoActiveForeignSibling {
        return Err(fault(
            "window_local_custody_unknown",
            "Active user sibling or unknown app-focus custody forbids window selection.",
        ));
    }
    let r = c.reference;
    if !r.placement_only
        || r.scope.element.is_some()
        || r.native_action.is_some()
        || r.target != c.request.target
        || r.frame != c.request.frame
        || r.pixel_anchor_used
        || r.pixel_anchor.is_some()
        || r.red_marker_anchor.is_some()
    {
        return Err(fault(
            "exact_window_reference_required",
            "Use the internally resolved exact selected window, never an element/pixel/action surrogate.",
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
            "window_local_context_changed",
            "Session, age or modal readiness changed; no selection.",
        ));
    }
    let old = c.original.binding.as_ref().ok_or_else(|| {
        fault(
            "target_binding_unknown",
            "Exact original selected window binding required.",
        )
    })?;
    let now = c.fresh.binding.as_ref().ok_or_else(|| {
        fault(
            "target_binding_unknown",
            "Exact fresh selected window binding required.",
        )
    })?;
    // content is screenshot hash; native identity/layout/context are the custody axes.
    // Binding.focus_window is selected grounding, NOT app AXFocusedWindow readback.
    if old.scope != now.scope
        || old.focus_window != now.focus_window
        || old.layout != now.layout
        || old.native_epoch != now.native_epoch
        || old.mutation_context != now.mutation_context
        || r.scope != old.scope
    {
        return Err(fault(
            "window_local_context_changed",
            "Exact native window/provider/layout grounding changed; no selection.",
        ));
    }
    if [
        native.input_principal,
        native.accessibility_permission,
        native.retained_window,
        native.provider_lifetime,
        native.process_lifetime,
        native.cg_window_binding,
        native.native_context,
    ]
    .iter()
    .any(|p| *p != ExactEvidence::Verified)
        || native.probe_origin != ProbeOrigin::FreshExactObject
    {
        return Err(fault(
            "window_local_native_identity_unknown",
            "Fresh retained CF, owner/provider lifetime, CG window and native context are mandatory; a sidecar is not a ticket.",
        ));
    }
    if custody.lease != LeaseCustody::HeldExactDomainTarget {
        return Err(fault(
            "window_local_lease_unavailable",
            "Actual exact resource lease required.",
        ));
    }
    if custody.journal != JournalCustody::NewExactReservation {
        return Err(fault(
            "window_local_request_not_new",
            "Pending/conflicting/attempted durable request must never be replayed.",
        ));
    }
    let (selected, app_cf, writable) = match c.request.kind {
        WindowLocalKind::Main => (
            native.window_main,
            native.application_main_window_cf_match,
            native.main_writable,
        ),
        WindowLocalKind::Key => (
            native.window_focused,
            native.application_focused_window_cf_match,
            native.focused_writable,
        ),
    };
    let selected = known_boolean(selected, c.request.kind.attribute())?;
    let app_cf = known_boolean(app_cf, "exact requested application window CF")?;
    if selected && app_cf {
        return Ok(WindowLocalWritePlan::NoWriteAlreadySelected);
    }
    if !known_boolean(writable, c.request.kind.attribute())? {
        return Err(fault(
            "window_local_kind_unsupported",
            "The exact provider does not allow this attribute; refuse before write. Main is not a Key fallback.",
        ));
    }
    Ok(WindowLocalWritePlan::SetAttributeTrueOnce)
}

#[derive(Debug, Clone)]
pub struct WindowLocalReadback {
    pub kind: WindowLocalKind,
    pub write_plan: WindowLocalWritePlan,
    pub setter_attempt: FocusSetterAttempt,
    pub setter_status: Option<i32>,
    pub window_main: FocusBooleanRead,
    pub application_main_window_cf_match: FocusBooleanRead,
    pub window_focused: FocusBooleanRead,
    pub application_focused_window_cf_match: FocusBooleanRead,
    pub os_foreground_before: Option<ForegroundState>,
    pub os_foreground_after: Option<ForegroundState>,
    /// Response-only coverage; deliberately not deserializable authority.
    pub public_application_custody: Option<FocusPublicApplicationReadback>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowLocalReadbackState {
    Selected,
    Partial,
    Unknown,
}
#[derive(Debug, Clone, Serialize)]
pub struct WindowLocalReadbackSummary {
    pub kind: WindowLocalKind,
    pub write_plan: WindowLocalWritePlan,
    pub setter_attempt: FocusSetterAttempt,
    pub setter_status: Option<i32>,
    pub setter_acknowledged: Option<bool>,
    pub state: WindowLocalReadbackState,
    pub window_main: FocusBooleanRead,
    pub application_main_window_cf_match: FocusBooleanRead,
    pub window_focused: FocusBooleanRead,
    pub application_focused_window_cf_match: FocusBooleanRead,
    pub os_foreground_before: Option<ForegroundState>,
    pub os_foreground_after: Option<ForegroundState>,
    pub os_foreground_unchanged: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_application_custody: Option<FocusPublicApplicationReadback>,
    pub selected_goal_verified: bool,
    /// This operation never authorizes element typing, even when Main is selected.
    pub typing_authorized: bool,
    /// Element/UIElement axes were not queried by this window-only operation.
    pub semantic_element_ready: Option<bool>,
}
pub fn summarize_window_local(r: WindowLocalReadback) -> WindowLocalReadbackSummary {
    let (selected, app) = match r.kind {
        WindowLocalKind::Main => (r.window_main, r.application_main_window_cf_match),
        WindowLocalKind::Key => (r.window_focused, r.application_focused_window_cf_match),
    };
    let selected = known_boolean(selected, r.kind.attribute()).ok();
    let app = known_boolean(app, "requested application window CF").ok();
    let acknowledged = if r.setter_attempt == FocusSetterAttempt::Attempted {
        r.setter_status.map(|s| s == 0)
    } else {
        None
    };
    let phase_known = matches!(
        (r.write_plan, r.setter_attempt, r.setter_status),
        (
            WindowLocalWritePlan::NoWriteAlreadySelected,
            FocusSetterAttempt::NotAttempted,
            None
        ) | (
            WindowLocalWritePlan::SetAttributeTrueOnce,
            FocusSetterAttempt::Attempted,
            Some(0)
        )
    );
    let state = if !phase_known || selected.is_none() || app.is_none() {
        WindowLocalReadbackState::Unknown
    } else if selected == Some(true) && app == Some(true) {
        WindowLocalReadbackState::Selected
    } else {
        WindowLocalReadbackState::Partial
    };
    let unchanged = r
        .os_foreground_before
        .as_ref()
        .zip(r.os_foreground_after.as_ref())
        .and_then(|(a, b)| (a.validate().is_ok() && b.validate().is_ok()).then_some(a == b));
    WindowLocalReadbackSummary {
        kind: r.kind,
        write_plan: r.write_plan,
        setter_attempt: r.setter_attempt,
        setter_status: r.setter_status,
        setter_acknowledged: acknowledged,
        state,
        window_main: r.window_main,
        application_main_window_cf_match: r.application_main_window_cf_match,
        window_focused: r.window_focused,
        application_focused_window_cf_match: r.application_focused_window_cf_match,
        os_foreground_before: r.os_foreground_before,
        os_foreground_after: r.os_foreground_after,
        os_foreground_unchanged: unchanged,
        public_application_custody: r.public_application_custody,
        selected_goal_verified: state == WindowLocalReadbackState::Selected,
        typing_authorized: false,
        semantic_element_ready: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn b(value: bool) -> FocusBooleanRead {
        FocusBooleanRead {
            native_status: 0,
            value: Some(value),
        }
    }
    fn r(kind: WindowLocalKind) -> WindowLocalReadback {
        WindowLocalReadback {
            kind,
            write_plan: WindowLocalWritePlan::SetAttributeTrueOnce,
            setter_attempt: FocusSetterAttempt::Attempted,
            setter_status: Some(0),
            window_main: b(true),
            application_main_window_cf_match: b(true),
            window_focused: b(false),
            application_focused_window_cf_match: b(false),
            os_foreground_before: None,
            os_foreground_after: None,
            public_application_custody: None,
        }
    }
    #[test]
    fn strict_four_fields_and_kind() {
        let v = serde_json::json!({"target":"T","frame":"F","kind":"main","request_id":"R"});
        assert!(serde_json::from_value::<WindowLocalRequest>(v.clone()).is_ok());
        for (key, value) in [
            ("options", serde_json::json!({})),
            ("element", serde_json::json!("E")),
        ] {
            let mut x = v.clone();
            x[key] = value;
            assert!(serde_json::from_value::<WindowLocalRequest>(x).is_err());
        }
        for kind in [
            serde_json::json!("focused"),
            serde_json::Value::Null,
            serde_json::json!(true),
        ] {
            let mut x = v.clone();
            x["kind"] = kind;
            assert!(serde_json::from_value::<WindowLocalRequest>(x).is_err());
        }
    }
    #[test]
    fn main_is_not_key_or_semantic_ready() {
        let m = summarize_window_local(r(WindowLocalKind::Main));
        assert!(m.selected_goal_verified);
        assert!(!m.typing_authorized);
        assert_eq!(m.semantic_element_ready, None);
        let k = summarize_window_local(r(WindowLocalKind::Key));
        assert!(!k.selected_goal_verified);
        assert_eq!(k.state, WindowLocalReadbackState::Partial);
    }
    #[test]
    fn raw_error_with_true_never_authorizes() {
        assert!(
            known_boolean(
                FocusBooleanRead {
                    native_status: -25204,
                    value: Some(true)
                },
                "AXMain"
            )
            .is_err()
        );
        let mut x = r(WindowLocalKind::Main);
        x.application_main_window_cf_match.native_status = -25204;
        assert_eq!(
            summarize_window_local(x).state,
            WindowLocalReadbackState::Unknown
        );
    }
    #[test]
    fn missing_ack_and_no_write_are_distinct() {
        let mut x = r(WindowLocalKind::Main);
        x.setter_status = None;
        assert_eq!(
            summarize_window_local(x.clone()).state,
            WindowLocalReadbackState::Unknown
        );
        x.write_plan = WindowLocalWritePlan::NoWriteAlreadySelected;
        x.setter_attempt = FocusSetterAttempt::NotAttempted;
        let s = summarize_window_local(x);
        assert!(s.selected_goal_verified);
        assert_eq!(s.setter_acknowledged, None);
    }
    #[test]
    fn foreground_is_a_separate_axis() {
        let fg = |pid| ForegroundState {
            pid,
            window: "cgwindow:7".into(),
            process_start: "7:start".into(),
            native_epoch: "a".repeat(64),
        };
        let mut x = r(WindowLocalKind::Main);
        x.os_foreground_before = Some(fg(7));
        x.os_foreground_after = Some(fg(8));
        let s = summarize_window_local(x);
        assert!(s.selected_goal_verified);
        assert_eq!(s.os_foreground_unchanged, Some(false));
    }
}
