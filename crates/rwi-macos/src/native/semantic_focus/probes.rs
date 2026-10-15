use super::*;

pub(super) fn raw_attribute(element: &Cf, attribute: &str) -> (i32, Option<Cf>) {
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

pub(super) fn boolean(element: &Cf, attribute: &str) -> FocusBooleanRead {
    let (native_status, value) = raw_attribute(element, attribute);
    FocusBooleanRead {
        native_status,
        value: value.and_then(|v| v.boolean()),
    }
}

pub(super) fn writable(element: &Cf, attribute: &str) -> FocusBooleanRead {
    let (native_status, raw) = element.attribute_settable_status(attribute);
    FocusBooleanRead {
        native_status,
        value: (native_status == 0).then_some(raw != 0),
    }
}

pub(super) fn ax_type(element: &Cf) -> Result<()> {
    if unsafe { ffi::CFGetTypeID(element.0) != ffi::AXUIElementGetTypeID() } {
        return Err(fault(
            "focus_native_type_unknown",
            "Expected an actual AXUIElement; no identity guess.",
        ));
    }
    Ok(())
}

pub(super) fn timeout(element: &Cf, seconds: f32) -> Result<()> {
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

pub(super) fn cf_focus_match(
    application: &Cf,
    attribute: &str,
    expected: &Cf,
    pid: u32,
) -> FocusBooleanRead {
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

pub(super) fn classify_boolean_probe<T>(
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

pub(super) fn boolean_readback_probe(element: &Cf, attribute: &str) -> FocusReadbackProbe {
    let (native_status, value) = raw_attribute(element, attribute);
    classify_boolean_probe(native_status, value, Cf::boolean)
}

pub(super) fn classify_cf_focus_probe<T>(
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

pub(super) fn cf_focus_readback_probe(
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

pub(super) fn known(read: FocusBooleanRead) -> Result<bool> {
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

pub(super) fn application_custody(
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
pub(super) fn same_native_binding(a: &Binding, b: &Binding) -> bool {
    a.scope == b.scope
        && a.focus_window == b.focus_window
        && a.layout == b.layout
        && a.native_epoch == b.native_epoch
        && a.mutation_context == b.mutation_context
}

/// Native refinement cannot turn a no-write ceiling into a new setter.
pub(super) fn refine_plan(
    ceiling: FocusWritePlan,
    facts: &FocusNativeFacts,
) -> Result<FocusWritePlan> {
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
pub(super) fn once<T, W>(
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
