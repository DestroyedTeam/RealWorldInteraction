//! Exact-object AX focus. This route never posts HID, activates, clicks or types.
//! Core owns the real lease/durable reservation; a plan is only a write ceiling.
use super::*;
use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};
use rwi_core::semantic_focus::{
    ApplicationFocusCustody, EditableClass, ExactEvidence, FocusBooleanRead, FocusNativeFacts,
    FocusPlatform, FocusProbeUnknownCause, FocusPublicApplicationReadback, FocusReadback,
    FocusReadbackAxisPair, FocusReadbackDiagnostics, FocusReadbackProbe, FocusSetterAttempt,
    FocusWritePlan, ProbeOrigin,
};

struct Sample {
    meta: Metadata,
    application: Cf,
    window: Cf,
    element: Cf,
    owner: u32,
    provider_epoch: String,
    facts: FocusNativeFacts,
    foreground: ForegroundCustodySample,
}

// Private Native witnesses, never caller/deserialized facts or global-key tickets.
struct PublicApplicationIdentity {
    pid: u32,
    process_start: String,
}
pub(super) struct ForegroundCustodySample {
    application: activation::PublicForegroundApplication,
    identity: PublicApplicationIdentity,
    full_window: Option<rwi_core::ForegroundState>,
}
impl ForegroundCustodySample {
    pub(super) fn pid(&self) -> u32 {
        self.identity.pid
    }
    pub(super) fn full_window(&self) -> Option<&rwi_core::ForegroundState> {
        self.full_window.as_ref()
    }
    /// Response only; never a window identity or a global input ticket.
    pub(super) fn public_application_readback(
        &self,
        after: &Self,
    ) -> Option<FocusPublicApplicationReadback> {
        self.full_window
            .is_none()
            .then(|| FocusPublicApplicationReadback {
                scope: "public_frontmost_application_identity_only",
                before_pid: self.identity.pid,
                before_process_start: self.identity.process_start.clone(),
                after_pid: after.identity.pid,
                after_process_start: after.identity.process_start.clone(),
                retained_application_equal: after.application.same_previous(),
                process_lifetime_equal: Some(
                    self.identity.process_start == after.identity.process_start,
                ),
                full_foreground_window_known: false,
            })
    }
}
fn require_public_application_pair(
    before: &PublicApplicationIdentity,
    after: &PublicApplicationIdentity,
    same_application: Option<bool>,
) -> Result<()> {
    if before.pid == 0
        || after.pid != before.pid
        || before.process_start.is_empty()
        || after.process_start != before.process_start
        || same_application != Some(true)
    {
        return Err(fault(
            "focus_public_application_changed",
            "Retained public frontmost application/PID/birth changed or is unknown; no setter or replay.",
        ));
    }
    Ok(())
}
fn read_full_foreground_if_same_application(
    bound_pid: u32,
    public_pid: u32,
    read: impl FnOnce() -> Result<rwi_core::ForegroundState>,
) -> Result<Option<rwi_core::ForegroundState>> {
    if bound_pid == 0 || public_pid == 0 {
        return Err(fault(
            "focus_public_application_unknown",
            "No positive native application identity.",
        ));
    }
    if bound_pid != public_pid {
        // A different frontmost app cannot be a sibling of this selected app.
        // Do not ask that unrelated app for its AX window or invent a tuple.
        return Ok(None);
    }
    let full = read()?;
    full.validate()?;
    if full.pid != public_pid {
        return Err(fault(
            "focus_public_application_changed",
            "Full foreground and public application disagree.",
        ));
    }
    Ok(Some(full))
}
pub(super) fn read_foreground_custody(
    bound_pid: u32,
    previous: Option<&ForegroundCustodySample>,
) -> Result<ForegroundCustodySample> {
    let application = activation::public_foreground_application(
        previous.map(|sample| sample.application.clone()),
    )?;
    let identity = PublicApplicationIdentity {
        pid: application.pid(),
        process_start: process_epoch(application.pid())?,
    };
    if let Some(previous) = previous {
        require_public_application_pair(
            &previous.identity,
            &identity,
            application.same_previous(),
        )?;
    }
    let full_window = read_full_foreground_if_same_application(
        bound_pid,
        identity.pid,
        activation::foreground_state,
    )?;
    if full_window
        .as_ref()
        .is_some_and(|full| full.process_start != identity.process_start)
        || process_epoch(identity.pid)? != identity.process_start
    {
        return Err(fault(
            "focus_public_application_changed",
            "Native application lifetime changed during custody read.",
        ));
    }
    Ok(ForegroundCustodySample {
        application,
        identity,
        full_window,
    })
}

fn raw_attribute(element: &Cf, attribute: &str) -> (i32, Option<Cf>) {
    let name = CFString::new(attribute);
    let mut value = std::ptr::null();
    let status = unsafe {
        ffi::AXUIElementCopyAttributeValue(element.0, name.as_concrete_TypeRef().cast(), &mut value)
    };
    (
        status,
        if status == 0 {
            unsafe { Cf::owned(value).ok() }
        } else {
            None
        },
    )
}

fn boolean(element: &Cf, attribute: &str) -> FocusBooleanRead {
    let (native_status, value) = raw_attribute(element, attribute);
    FocusBooleanRead {
        native_status,
        value: value.and_then(|v| v.boolean()),
    }
}

fn writable(element: &Cf, attribute: &str) -> FocusBooleanRead {
    let (native_status, raw) = element.attribute_settable_status(attribute);
    FocusBooleanRead {
        native_status,
        value: (native_status == 0).then_some(raw != 0),
    }
}

fn ax_type(element: &Cf) -> Result<()> {
    if unsafe { ffi::CFGetTypeID(element.0) != ffi::AXUIElementGetTypeID() } {
        return Err(fault(
            "focus_native_type_unknown",
            "Expected an actual AXUIElement; no identity guess.",
        ));
    }
    Ok(())
}

fn timeout(element: &Cf, seconds: f32) -> Result<()> {
    let status = unsafe { ffi::AXUIElementSetMessagingTimeout(element.0, seconds) };
    if status != 0 {
        let mut error = fault(
            "focus_probe_timeout_configuration_failed",
            "Exact AX timeout configuration failed; no fallback.",
        );
        error.diagnostic = Some(Box::new(
            serde_json::json!({"api":"AXUIElementSetMessagingTimeout","native_status":status,"setter_attempted":false}),
        ));
        return Err(error);
    }
    Ok(())
}

fn cf_focus_match(application: &Cf, attribute: &str, expected: &Cf, pid: u32) -> FocusBooleanRead {
    let (native_status, value) = raw_attribute(application, attribute);
    let matched = value.and_then(|value| {
        ax_type(&value).ok()?;
        let owner = value.pid().ok()?;
        delegation::validate_hosted_descendant(pid, owner).ok()?;
        Some(unsafe { ffi::CFEqual(value.0, expected.0) != 0 })
    });
    FocusBooleanRead {
        native_status,
        value: matched,
    }
}

fn classify_boolean_probe<T>(
    native_status: i32,
    value: Option<T>,
    boolean: impl FnOnce(&T) -> Option<bool>,
) -> FocusReadbackProbe {
    if native_status != 0 {
        return FocusReadbackProbe::unknown(
            native_status,
            FocusProbeUnknownCause::CopyAttributeFailed,
            None,
        );
    }
    let Some(value) = value else {
        return FocusReadbackProbe::unknown(
            native_status,
            FocusProbeUnknownCause::CopyReturnedNull,
            None,
        );
    };
    match boolean(&value) {
        Some(value) => FocusReadbackProbe::known(FocusBooleanRead {
            native_status,
            value: Some(value),
        }),
        None => {
            FocusReadbackProbe::unknown(native_status, FocusProbeUnknownCause::NotCfBoolean, None)
        }
    }
}

fn boolean_readback_probe(element: &Cf, attribute: &str) -> FocusReadbackProbe {
    let (native_status, value) = raw_attribute(element, attribute);
    classify_boolean_probe(native_status, value, Cf::boolean)
}

fn classify_cf_focus_probe<T>(
    native_status: i32,
    value: Option<T>,
    validate_type: impl FnOnce(&T) -> Result<()>,
    owner: impl FnOnce(&T) -> Result<u32>,
    validate_provider: impl FnOnce(u32) -> Result<()>,
    matches: impl FnOnce(&T) -> bool,
) -> FocusReadbackProbe {
    if native_status != 0 {
        return FocusReadbackProbe::unknown(
            native_status,
            FocusProbeUnknownCause::CopyAttributeFailed,
            None,
        );
    }
    let Some(value) = value else {
        return FocusReadbackProbe::unknown(
            native_status,
            FocusProbeUnknownCause::CopyReturnedNull,
            None,
        );
    };
    if let Err(error) = validate_type(&value) {
        return FocusReadbackProbe::unknown(
            native_status,
            FocusProbeUnknownCause::NotAxElement,
            Some(&error.code),
        );
    }
    let owner = match owner(&value) {
        Ok(owner) => owner,
        Err(error) => {
            return FocusReadbackProbe::unknown(
                native_status,
                FocusProbeUnknownCause::OwnerLookupFailed,
                Some(&error.code),
            );
        }
    };
    if let Err(error) = validate_provider(owner) {
        return FocusReadbackProbe::unknown(
            native_status,
            FocusProbeUnknownCause::ProviderValidationFailed,
            Some(&error.code),
        );
    }
    FocusReadbackProbe::known(FocusBooleanRead {
        native_status,
        value: Some(matches(&value)),
    })
}

fn cf_focus_readback_probe(
    application: &Cf,
    attribute: &str,
    expected: &Cf,
    pid: u32,
) -> FocusReadbackProbe {
    let (native_status, value) = raw_attribute(application, attribute);
    classify_cf_focus_probe(
        native_status,
        value,
        ax_type,
        Cf::pid,
        |owner| delegation::validate_hosted_descendant(pid, owner).map(|_| ()),
        |value| unsafe { ffi::CFEqual(value.0, expected.0) != 0 },
    )
}

fn known(read: FocusBooleanRead) -> Result<bool> {
    if read.native_status != 0 || read.value.is_none() {
        let mut error = fault(
            "focus_native_probe_unknown",
            "Exact AX read is unknown; inspect it without replay.",
        );
        error.diagnostic = Some(Box::new(
            serde_json::json!({"native_status":read.native_status,"value":read.value,"input_authority":false}),
        ));
        return Err(error);
    }
    Ok(read.value.expect("checked native Boolean"))
}

fn application_custody(
    bound_pid: u32,
    foreground_pid: u32,
    foreground_window_matches: Option<bool>,
    focused_window: FocusBooleanRead,
) -> ApplicationFocusCustody {
    match (focused_window.native_status, focused_window.value) {
        (0, Some(false)) if foreground_pid == bound_pid => {
            ApplicationFocusCustody::ActiveForeignSibling
        }
        (0, Some(true))
            if foreground_pid == bound_pid && foreground_window_matches != Some(true) =>
        {
            ApplicationFocusCustody::Unknown
        }
        (0, Some(_)) => ApplicationFocusCustody::NoActiveForeignSibling,
        _ => ApplicationFocusCustody::Unknown,
    }
}

// A model observation content hash is pixels, while revalidation has a native
// context hash. AX focus is not a pixel mutation; never replace its exact axes
// with whole-Binding equality or make the screenshot hash an authority token.
fn same_native_binding(a: &Binding, b: &Binding) -> bool {
    a.scope == b.scope
        && a.focus_window == b.focus_window
        && a.layout == b.layout
        && a.native_epoch == b.native_epoch
        && a.mutation_context == b.mutation_context
}

/// Native refinement cannot turn a no-write ceiling into a new setter.
fn refine_plan(ceiling: FocusWritePlan, facts: &FocusNativeFacts) -> Result<FocusWritePlan> {
    if facts.platform != FocusPlatform::Macos
        || facts.application_focus_custody != ApplicationFocusCustody::NoActiveForeignSibling
        || facts.probe_origin != ProbeOrigin::FreshExactObject
        || facts.editable_class != EditableClass::PlainEditable
        || [
            facts.input_principal,
            facts.accessibility_permission,
            facts.retained_element,
            facts.element_window_cf,
            facts.process_lifetime,
            facts.cg_window_binding,
            facts.native_context,
        ]
        .iter()
        .any(|v| *v != ExactEvidence::Verified)
    {
        return Err(fault(
            "focus_native_guard_refused",
            "Fresh exact Native facts do not permit semantic focus; no fallback.",
        ));
    }
    if !known(facts.value_writable)? || !known(facts.focused_writable)? {
        return Err(fault(
            "focus_attribute_not_settable",
            "Exact plain element focus/value is not writable; no setter.",
        ));
    }
    if known(facts.focused)? {
        return Ok(FocusWritePlan::NoWriteAlreadyFocused);
    }
    if ceiling == FocusWritePlan::NoWriteAlreadyFocused {
        return Err(fault(
            "focus_zero_write_budget_changed",
            "Fresh focus changed after a no-write plan; never upgrade it or replay.",
        ));
    }
    Ok(FocusWritePlan::SetAXFocusedTrueOnce)
}

/// A single closure is the only write site; errors never enter a second attempt.
/// A last fresh Native gate may refine 1 -> 0, never upgrade the ceiling.
fn once<T, W>(
    prepare: impl FnOnce() -> Result<(FocusWritePlan, T)>,
    gate: impl FnOnce(FocusWritePlan, T) -> Result<(FocusWritePlan, T)>,
    write: impl FnOnce(&T) -> Result<W>,
) -> Result<(FocusWritePlan, T, Option<W>)> {
    let (approved, sample) = prepare()?;
    let (plan, sample) = gate(approved, sample)?;
    if approved == FocusWritePlan::NoWriteAlreadyFocused && plan != approved {
        return Err(fault(
            "focus_zero_write_budget_changed",
            "Last Native gate cannot upgrade a no-write ceiling.",
        ));
    }
    let status = if plan == FocusWritePlan::SetAXFocusedTrueOnce {
        Some(write(&sample)?)
    } else {
        None
    };
    Ok((plan, sample, status))
}

impl NativeMac {
    fn semantic_focus_sample(
        &self,
        target: &Resolved,
        expected: Option<&Binding>,
        expected_custody: Option<&ForegroundCustodySample>,
    ) -> Result<Sample> {
        let _call = operation::native_call("macos_semantic_focus_exact_preflight")?;
        if !self.allow_input || unsafe { ffi::AXIsProcessTrusted() } == 0 {
            return Err(fault(
                "focus_input_not_enabled",
                "Existing exact input principal and AX permission required; no permission request.",
            ));
        }
        if self.semantic_only
            || target.placement_only
            || target.native_action.is_some()
            || target.scope.pid != self.pid
            || target.scope.window != self.window_name()
            || target.scope.element.is_none()
            || target.source_region.is_some()
            || target.pixel_anchor.is_some()
            || target.red_marker_anchor.is_some()
            || target.pixel_anchor_used
        {
            return Err(fault(
                "exact_semantic_element_required",
                "Ordinary exact retained visual window/element only; no pixel, AXAction or logical-surface substitution.",
            ));
        }
        // The ordinary native-element policy retains its real observed visibility
        // and revision checks. No forged native_action to bypass settled AX reads.
        let meta = self.metadata_for(Some(target))?;
        if meta.identity != self.identity
            || meta.blocked != Some(false)
            || !meta.mapping_ready
            || expected.is_some_and(|binding| !same_native_binding(binding, &meta.binding))
        {
            return Err(fault(
                "focus_context_changed",
                "Native session/window/context/modal grounding changed; no setter.",
            ));
        }
        let description = meta
            .elements
            .iter()
            .find(|e| Some(&e.element.id) == target.scope.element.as_ref())
            .ok_or_else(|| {
                fault(
                    "semantic_object_expired",
                    "Exact returned Native element is absent; no search substitute.",
                )
            })?;
        if description.element.enabled != Some(true)
            || description.element.visible != Some(true)
            || description.element.occluded != Some(false)
            || description.element.point != target.point
            || target.element_revision.as_ref() != Some(&description.element.revision())
        {
            return Err(fault(
                "focus_element_changed",
                "Exact enabled/visible/revision facts changed or unknown; no setter.",
            ));
        }
        if description.text_class != TextClass::Plain {
            return Err(fault(
                "plain_editable_element_required",
                "Secure/unclassified/nontext targets do not receive focus probes or setters.",
            ));
        }
        let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
        ax_type(&application)?;
        timeout(&application, 0.05)?;
        let row = self.window_record()?;
        let window = self.resolve_window_element(&application, row.rect)?;
        ax_type(&window)?;
        if !row.rect.same(meta.rect)
            || !ax_rect(&window)?.same(row.rect)
            || ffi::window_number_for_ax_element(&window) != Some(self.window)
        {
            return Err(fault(
                "focus_window_identity_changed",
                "Selected AX/CG object and bounds must match exactly; no geometry-only match.",
            ));
        }
        let element = objects::exact(
            &window,
            self.pid,
            target.scope.element.as_deref().expect("checked element"),
            &self.objects,
        )?;
        ax_type(&element)?;
        let owner = element.pid()?;
        let provider_epoch = delegation::validate_hosted_descendant(self.pid, owner)?;
        let foreground = read_foreground_custody(self.pid, expected_custody)?;
        let app_focus = cf_focus_match(&application, "AXFocusedWindow", &window, self.pid);
        let value_writable = writable(&element, "AXValue");
        let focused_writable = writable(&element, "AXFocused");
        let focused = boolean(&element, "AXFocused");
        let facts = FocusNativeFacts {
            platform: FocusPlatform::Macos,
            application_focus_custody: application_custody(
                self.pid,
                foreground.identity.pid,
                foreground
                    .full_window
                    .as_ref()
                    .map(|full| full.window == self.window_name()),
                app_focus,
            ),
            input_principal: ExactEvidence::Verified, // Backend's actual authenticated allow_input, never caller facts.
            accessibility_permission: ExactEvidence::Verified,
            retained_element: ExactEvidence::Verified,
            element_window_cf: ExactEvidence::Verified,
            process_lifetime: ExactEvidence::Verified,
            cg_window_binding: ExactEvidence::Verified,
            native_context: ExactEvidence::Verified,
            editable_class: match (value_writable.native_status, value_writable.value) {
                (0, Some(true)) => EditableClass::PlainEditable,
                (0, Some(false)) => EditableClass::PlainReadOnly,
                _ => EditableClass::Unknown,
            },
            probe_origin: ProbeOrigin::FreshExactObject,
            value_writable,
            focused_writable,
            focused,
        };
        let after = self.capture_guard_metadata()?;
        let after_window = self.resolve_window_element(&application, self.window_record()?.rect)?;
        if after.identity != meta.identity
            || after.binding != meta.binding
            || after.blocked != meta.blocked
            || unsafe { ffi::CFEqual(window.0, after_window.0) } == 0
            || process_epoch(owner)? != provider_epoch
        {
            return Err(fault(
                "focus_context_changed",
                "Exact provider/window/context changed during focus probe; no setter.",
            ));
        }
        let foreground_after = read_foreground_custody(self.pid, Some(&foreground))?;
        let after_app_focus = cf_focus_match(&application, "AXFocusedWindow", &window, self.pid);
        let custody_after = application_custody(
            self.pid,
            foreground_after.identity.pid,
            foreground_after
                .full_window
                .as_ref()
                .map(|full| full.window == self.window_name()),
            after_app_focus,
        );
        if custody_after != facts.application_focus_custody
            || custody_after != ApplicationFocusCustody::NoActiveForeignSibling
        {
            return Err(fault(
                "focus_application_custody_unknown",
                "Active sibling/unknown application-focus resource refuses semantic focus; no automatic foreground switch.",
            ));
        }
        Ok(Sample {
            meta,
            application,
            window,
            element,
            owner,
            provider_epoch,
            facts,
            foreground: foreground_after,
        })
    }

    pub(crate) fn semantic_focus_preflight(&self, target: &Resolved) -> Result<FocusNativeFacts> {
        Ok(self.semantic_focus_sample(target, None, None)?.facts)
    }

    fn semantic_focus_readback(
        &self,
        sample: &Sample,
        plan: FocusWritePlan,
        status: Option<i32>,
    ) -> Result<FocusReadback> {
        timeout(&sample.element, 0.05)?;
        let element_focused = boolean_readback_probe(&sample.element, "AXFocused");
        let focused_ui_element_cf_match = cf_focus_readback_probe(
            &sample.application,
            "AXFocusedUIElement",
            &sample.element,
            self.pid,
        );
        let application_focused_window_cf_match = cf_focus_readback_probe(
            &sample.application,
            "AXFocusedWindow",
            &sample.window,
            self.pid,
        );
        let after = self.capture_guard_metadata()?;
        let after_window =
            self.resolve_window_element(&sample.application, self.window_record()?.rect)?;
        if after.identity != sample.meta.identity
            || after.binding != sample.meta.binding
            || after.blocked != sample.meta.blocked
            || unsafe { ffi::CFEqual(sample.window.0, after_window.0) } == 0
            || process_epoch(sample.owner)? != sample.provider_epoch
        {
            return Err(fault(
                "focus_context_changed_after_call",
                "Focus readback crossed an exact provider/window/context change; no goal conclusion.",
            ));
        }
        // A second independent set of focus reads rejects non-atomic disagreement,
        // but never resends a setter or pretends it continuously observed focus.
        let e2 = boolean_readback_probe(&sample.element, "AXFocused");
        let u2 = cf_focus_readback_probe(
            &sample.application,
            "AXFocusedUIElement",
            &sample.element,
            self.pid,
        );
        let w2 = cf_focus_readback_probe(
            &sample.application,
            "AXFocusedWindow",
            &sample.window,
            self.pid,
        );
        let consistent = |a: FocusBooleanRead, b: FocusBooleanRead| {
            if a.native_status == b.native_status && a.value == b.value {
                a
            } else {
                FocusBooleanRead {
                    native_status: b.native_status,
                    value: None,
                }
            }
        };
        let final_guard = self.capture_guard_metadata()?;
        let final_window =
            self.resolve_window_element(&sample.application, self.window_record()?.rect)?;
        if final_guard.identity != sample.meta.identity
            || final_guard.binding != sample.meta.binding
            || final_guard.blocked != sample.meta.blocked
            || unsafe { ffi::CFEqual(sample.window.0, final_window.0) } == 0
            || process_epoch(sample.owner)? != sample.provider_epoch
        {
            return Err(fault(
                "focus_context_changed_after_call",
                "Final independent readback context changed; no goal conclusion or replay.",
            ));
        }
        let foreground_after = read_foreground_custody(self.pid, Some(&sample.foreground))?;
        // Capture Copy-valued axis summaries before diagnostics owns the probes.
        let element_focused_raw = consistent(element_focused.raw, e2.raw);
        let focused_ui_element_cf_match_raw = consistent(focused_ui_element_cf_match.raw, u2.raw);
        let application_focused_window_cf_match_raw =
            consistent(application_focused_window_cf_match.raw, w2.raw);
        let mut diagnostics = FocusReadbackDiagnostics::new(
            FocusReadbackAxisPair::new(element_focused, e2),
            FocusReadbackAxisPair::new(focused_ui_element_cf_match, u2),
            FocusReadbackAxisPair::new(application_focused_window_cf_match, w2),
        );
        if sample.foreground.full_window.is_none() {
            diagnostics.public_application_custody = Some(FocusPublicApplicationReadback {
                scope: "public_frontmost_application_identity_only",
                before_pid: sample.foreground.identity.pid,
                before_process_start: sample.foreground.identity.process_start.clone(),
                after_pid: foreground_after.identity.pid,
                after_process_start: foreground_after.identity.process_start.clone(),
                retained_application_equal: foreground_after.application.same_previous(),
                process_lifetime_equal: Some(
                    sample.foreground.identity.process_start
                        == foreground_after.identity.process_start,
                ),
                full_foreground_window_known: false,
            });
        }
        Ok(FocusReadback {
            write_plan: plan,
            setter_attempt: if status.is_some() {
                FocusSetterAttempt::Attempted
            } else {
                FocusSetterAttempt::NotAttempted
            },
            setter_status: status,
            element_focused: element_focused_raw,
            focused_ui_element_cf_match: focused_ui_element_cf_match_raw,
            application_focused_window_cf_match: application_focused_window_cf_match_raw,
            os_foreground_before: sample.foreground.full_window.clone(),
            os_foreground_after: foreground_after.full_window,
            readback_diagnostics: Some(diagnostics),
        })
    }

    pub(crate) fn dispatch_semantic_focus(
        &self,
        expected: &Binding,
        target: &Resolved,
        ceiling: FocusWritePlan,
    ) -> Result<FocusReadback> {
        let (plan, sample, attempted) = once(
            || {
                let fresh = self.semantic_focus_sample(target, Some(expected), None)?;
                Ok((refine_plan(ceiling, &fresh.facts)?, fresh))
            },
            |approved, sample| {
                // Re-read the actual exact object at the Native write boundary; the
                // earlier Core preflight/sidecar cannot stand in for this guard.
                let fresh =
                    self.semantic_focus_sample(target, Some(expected), Some(&sample.foreground))?;
                if unsafe { ffi::CFEqual(sample.window.0, fresh.window.0) } == 0
                    || unsafe { ffi::CFEqual(sample.element.0, fresh.element.0) } == 0
                    || sample.owner != fresh.owner
                    || sample.provider_epoch != fresh.provider_epoch
                {
                    return Err(fault(
                        "focus_native_object_changed",
                        "Exact retained window/element/provider changed at boundary; no setter.",
                    ));
                }
                let mut fresh = fresh;
                fresh.facts.focused = boolean(&fresh.element, "AXFocused");
                fresh.facts.focused_writable = writable(&fresh.element, "AXFocused");
                fresh.facts.value_writable = writable(&fresh.element, "AXValue");
                let plan = refine_plan(approved, &fresh.facts)?;
                let foreground = read_foreground_custody(self.pid, Some(&fresh.foreground))?;
                let custody = application_custody(
                    self.pid,
                    foreground.identity.pid,
                    foreground
                        .full_window
                        .as_ref()
                        .map(|full| full.window == self.window_name()),
                    cf_focus_match(
                        &fresh.application,
                        "AXFocusedWindow",
                        &fresh.window,
                        self.pid,
                    ),
                );
                if custody != ApplicationFocusCustody::NoActiveForeignSibling {
                    return Err(fault(
                        "focus_application_custody_unknown",
                        "Final exact global/app-focus samples conflict or show an active sibling; no setter.",
                    ));
                }
                if plan == FocusWritePlan::SetAXFocusedTrueOnce
                    && foreground.identity.pid == self.pid
                    && crate::activity::seconds_since_user_input()
                        .is_none_or(|seconds| seconds < 1.2)
                {
                    return Err(fault(
                        "user_active",
                        "Recent/unknown activity may affect this foreground app-focus resource; no setter.",
                    ));
                }
                fresh.foreground = foreground;
                Ok((plan, fresh))
            },
            |sample| {
                let _call = operation::native_call("macos_semantic_focus_setter_once")?;
                let name = CFString::new("AXFocused");
                let yes = CFBoolean::true_value();
                operation::checkpoint("before_exact_semantic_focus_setter")?;
                // This actual pair is retained even when saturated/concurrent. The
                // Agent exemption accepts only known non-saturated before + 1.
                let before = self
                    .dispatches
                    .fetch_update(
                        std::sync::atomic::Ordering::AcqRel,
                        std::sync::atomic::Ordering::Acquire,
                        |n| Some(n.saturating_add(1)),
                    )
                    .expect("saturating update always returns Some");
                let native_status = unsafe {
                    ffi::AXUIElementSetAttributeValue(
                        sample.element.0,
                        name.as_concrete_TypeRef().cast(),
                        yes.as_concrete_TypeRef().cast(),
                    )
                };
                let after = self.dispatches.load(std::sync::atomic::Ordering::Acquire);
                Ok((native_status, before, after))
            },
        )?;
        let status = attempted.map(|(native_status, _, _)| native_status);
        let readback = self.semantic_focus_readback(&sample, plan, status);
        if let Some(native_status) = status
            && (native_status != 0 || readback.is_err())
        {
            let mut error = fault(
                "semantic_focus_unconfirmed",
                "Exact AXFocused setter returned or readback failed; effect unknown. Read existing operation, never replay or fall back.",
            );
            error.diagnostic = Some(Box::new(serde_json::json!({
                "version":1,"api":"AXUIElementSetAttributeValue","attribute":"AXFocused",
                "operation_kind":"semantic_focus","native_setter_attempted":true,"native_setter_attempts":1,
                "native_status":native_status,"native_dispatch_counter_known":true,
                "native_dispatch_count_before":attempted.map(|(_, before, _)| before),
                "native_dispatch_count_after":attempted.map(|(_, _, after)| after),
                "native_call_pending":false,"native_resources_released":true,
                "held_input_resources":false,"replay_allowed":false,
                "scope":target.scope,"readback_error":readback.as_ref().err(),
                "focus_readback":readback.as_ref().ok().map(|r| rwi_core::semantic_focus::summarize_focus_readback(r.clone()))
            })));
            return Err(error);
        }
        readback
    }
}

mod set_text;

#[cfg(test)]
mod tests;
