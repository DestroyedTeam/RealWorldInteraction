//! Exact-window Main/Key selection. No activation, input, element or fallback.
//! A Core plan is a ceiling; the Native boundary re-reads retained CF identities.
use super::semantic_focus::{ForegroundCustodySample, read_foreground_custody};
use super::*;
use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};
use rwi_core::semantic_focus::{
    ApplicationFocusCustody, ExactEvidence, FocusBooleanRead, FocusPlatform, FocusSetterAttempt,
    ProbeOrigin,
};
use rwi_core::window_local::{
    WindowLocalKind, WindowLocalNativeFacts, WindowLocalReadback, WindowLocalWritePlan,
};

struct Sample {
    meta: Metadata,
    application: Cf,
    window: Cf,
    owner: u32,
    provider_epoch: String,
    facts: WindowLocalNativeFacts,
    foreground: ForegroundCustodySample,
}
fn raw_attribute(object: &Cf, attribute: &str) -> (i32, Option<Cf>) {
    let name = CFString::new(attribute);
    let mut value = std::ptr::null();
    let status = unsafe {
        ffi::AXUIElementCopyAttributeValue(object.0, name.as_concrete_TypeRef().cast(), &mut value)
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
fn boolean(object: &Cf, attribute: &str) -> FocusBooleanRead {
    let (native_status, value) = raw_attribute(object, attribute);
    FocusBooleanRead {
        native_status,
        value: value.and_then(|v| v.boolean()),
    }
}
fn writable(object: &Cf, attribute: &str) -> FocusBooleanRead {
    let (native_status, value) = object.attribute_settable_status(attribute);
    FocusBooleanRead {
        native_status,
        value: (native_status == 0).then_some(value != 0),
    }
}
fn ax_type(object: &Cf) -> Result<()> {
    if unsafe { ffi::CFGetTypeID(object.0) != ffi::AXUIElementGetTypeID() } {
        return Err(fault(
            "window_local_native_type_unknown",
            "An actual retained AXUIElement is required; no object guess.",
        ));
    }
    Ok(())
}
fn timeout(object: &Cf) -> Result<()> {
    let status = unsafe { ffi::AXUIElementSetMessagingTimeout(object.0, 0.05) };
    if status != 0 {
        let mut e = fault(
            "window_local_timeout_configuration_failed",
            "Exact AX timeout setup failed before setter.",
        );
        e.diagnostic = Some(Box::new(
            serde_json::json!({"api":"AXUIElementSetMessagingTimeout","native_status":status,"native_setter_attempted":false}),
        ));
        return Err(e);
    }
    Ok(())
}
fn cf_window_match(application: &Cf, attribute: &str, window: &Cf, pid: u32) -> FocusBooleanRead {
    let (native_status, value) = raw_attribute(application, attribute);
    let value = value.and_then(|v| {
        ax_type(&v).ok()?;
        delegation::validate_hosted_descendant(pid, v.pid().ok()?).ok()?;
        Some(unsafe { ffi::CFEqual(v.0, window.0) != 0 })
    });
    FocusBooleanRead {
        native_status,
        value,
    }
}
fn custody(
    bound: u32,
    public_pid: u32,
    foreground_window_matches: Option<bool>,
    focus: FocusBooleanRead,
) -> ApplicationFocusCustody {
    if bound == 0 || public_pid == 0 {
        return ApplicationFocusCustody::Unknown;
    }
    match (focus.native_status, focus.value) {
        (0, Some(false)) if public_pid == bound => ApplicationFocusCustody::ActiveForeignSibling,
        (0, Some(true)) if public_pid == bound && foreground_window_matches != Some(true) => {
            ApplicationFocusCustody::Unknown
        }
        (0, Some(_)) => ApplicationFocusCustody::NoActiveForeignSibling,
        _ => ApplicationFocusCustody::Unknown,
    }
}
fn same_full_foreground(
    before: Option<&rwi_core::ForegroundState>,
    after: Option<&rwi_core::ForegroundState>,
) -> bool {
    before == after
}
fn same_context(a: &Binding, b: &Binding) -> bool {
    a.scope == b.scope
        && a.focus_window == b.focus_window
        && a.layout == b.layout
        && a.native_epoch == b.native_epoch
        && a.mutation_context == b.mutation_context
    // content is screenshot pixels; it is not native window identity.
}
fn reference_valid(r: &Resolved, pid: u32, window: &str) -> bool {
    r.placement_only
        && r.scope.pid == pid
        && r.scope.window == window
        && r.scope.element.is_none()
        && r.native_action.is_none()
        && r.source_region.is_none()
        && r.pixel_anchor.is_none()
        && r.red_marker_anchor.is_none()
        && !r.pixel_anchor_used
}
fn known(r: FocusBooleanRead) -> Result<bool> {
    rwi_core::window_local::known_boolean(r, "requested exact window attribute")
}
fn refine(
    ceiling: WindowLocalWritePlan,
    f: &WindowLocalNativeFacts,
) -> Result<WindowLocalWritePlan> {
    if f.platform != FocusPlatform::Macos
        || f.probe_origin != ProbeOrigin::FreshExactObject
        || f.application_focus_custody != ApplicationFocusCustody::NoActiveForeignSibling
        || [
            f.input_principal,
            f.accessibility_permission,
            f.retained_window,
            f.provider_lifetime,
            f.process_lifetime,
            f.cg_window_binding,
            f.native_context,
        ]
        .iter()
        .any(|p| *p != ExactEvidence::Verified)
    {
        return Err(fault(
            "window_local_native_guard_refused",
            "Fresh exact window/provider/context/principal facts required; no setter.",
        ));
    }
    let (selected, app, write) = match f.kind {
        WindowLocalKind::Main => (
            f.window_main,
            f.application_main_window_cf_match,
            f.main_writable,
        ),
        WindowLocalKind::Key => (
            f.window_focused,
            f.application_focused_window_cf_match,
            f.focused_writable,
        ),
    };
    let selected = known(selected)?;
    let app = known(app)?;
    if selected && app {
        return Ok(WindowLocalWritePlan::NoWriteAlreadySelected);
    }
    if ceiling == WindowLocalWritePlan::NoWriteAlreadySelected {
        return Err(fault(
            "window_local_zero_write_budget_changed",
            "The exact selection changed after a no-write plan; never upgrade it.",
        ));
    }
    if !known(write)? {
        return Err(fault(
            "window_local_kind_unsupported",
            "The exact provider does not allow this kind. Main is never a Key fallback.",
        ));
    }
    Ok(WindowLocalWritePlan::SetAttributeTrueOnce)
}
fn once<T, W>(
    prepare: impl FnOnce() -> Result<(WindowLocalWritePlan, T)>,
    gate: impl FnOnce(WindowLocalWritePlan, T) -> Result<(WindowLocalWritePlan, T)>,
    write: impl FnOnce(&T) -> Result<W>,
) -> Result<(WindowLocalWritePlan, T, Option<W>)> {
    let (approved, sample) = prepare()?;
    let (plan, sample) = gate(approved, sample)?;
    if approved == WindowLocalWritePlan::NoWriteAlreadySelected && plan != approved {
        return Err(fault(
            "window_local_zero_write_budget_changed",
            "The final gate cannot upgrade a no-write ceiling.",
        ));
    }
    let status = if plan == WindowLocalWritePlan::SetAttributeTrueOnce {
        Some(write(&sample)?)
    } else {
        None
    };
    Ok((plan, sample, status))
}
fn consistent(a: FocusBooleanRead, b: FocusBooleanRead) -> FocusBooleanRead {
    if a.native_status == b.native_status && a.value == b.value {
        a
    } else {
        FocusBooleanRead {
            native_status: b.native_status,
            value: None,
        }
    }
}

impl NativeMac {
    /// Only this new route skips pixels/tree. All legacy revalidate callers retain their behavior.
    pub(crate) fn window_local_revalidate_scene(&self, target: &Resolved) -> Result<Scene> {
        if self.semantic_only || !reference_valid(target, self.pid, &self.window_name()) {
            return Err(fault(
                "exact_window_reference_required",
                "Ordinary exact visual window only; no element/action/region/pixel surrogate.",
            ));
        }
        let meta = self.capture_guard_metadata()?;
        if meta.binding.scope != target.scope {
            return Err(fault(
                "target_mismatch",
                "Exact window metadata belongs to another native scope.",
            ));
        }
        Ok(self.metadata_scene(meta, [0, 0], Vec::new(), None))
    }
    fn window_local_sample(
        &self,
        target: &Resolved,
        kind: WindowLocalKind,
        expected: Option<&Binding>,
        expected_custody: Option<&ForegroundCustodySample>,
    ) -> Result<Sample> {
        let _call = operation::native_call("macos_window_local_readonly_exact")?;
        if !self.allow_input || unsafe { ffi::AXIsProcessTrusted() } == 0 {
            return Err(fault(
                "window_local_input_not_enabled",
                "Existing authenticated input principal and AX trust required; no prompt.",
            ));
        }
        if self.semantic_only || !reference_valid(target, self.pid, &self.window_name()) {
            return Err(fault(
                "exact_window_reference_required",
                "Ordinary exact visual window only; no element/action/region/pixel substitute.",
            ));
        }
        let meta = self.capture_guard_metadata()?;
        if meta.identity != self.identity
            || meta.blocked != Some(false)
            || meta.modal != Some(false)
            || !meta.mapping_ready
            || expected.is_some_and(|b| !same_context(b, &meta.binding))
        {
            return Err(fault(
                "window_local_context_changed",
                "Session/modal/layout/context changed or unknown; no setter.",
            ));
        }
        let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(self.pid as i32))? };
        ax_type(&application)?;
        timeout(&application)?;
        if application.pid()? != self.pid {
            return Err(fault(
                "window_local_application_owner_changed",
                "Application CF owner must match pinned PID.",
            ));
        }
        let row = self.window_record()?;
        let window = self.resolve_window_element(&application, row.rect)?;
        ax_type(&window)?;
        timeout(&window)?;
        if window.attr("AXRole")?.string()? != "AXWindow"
            || !row.rect.same(meta.rect)
            || !ax_rect(&window)?.same(row.rect)
            || ffi::window_number_for_ax_element(&window) != Some(self.window)
        {
            return Err(fault(
                "window_local_window_identity_changed",
                "Only an exact ordinary AXWindow with pinned CG ID and bounds; no dialog/geometry substitute.",
            ));
        }
        let owner = window.pid()?;
        let provider_epoch = delegation::validate_hosted_descendant(self.pid, owner)?;
        let foreground = read_foreground_custody(self.pid, expected_custody)?;
        let app_focus = cf_window_match(&application, "AXFocusedWindow", &window, self.pid);
        let facts = WindowLocalNativeFacts {
            kind,
            platform: FocusPlatform::Macos,
            application_focus_custody: custody(
                self.pid,
                foreground.pid(),
                foreground
                    .full_window()
                    .map(|full| full.window == self.window_name()),
                app_focus,
            ),
            input_principal: ExactEvidence::Verified,
            accessibility_permission: ExactEvidence::Verified,
            retained_window: ExactEvidence::Verified,
            provider_lifetime: ExactEvidence::Verified,
            process_lifetime: ExactEvidence::Verified,
            cg_window_binding: ExactEvidence::Verified,
            native_context: ExactEvidence::Verified,
            probe_origin: ProbeOrigin::FreshExactObject,
            main_writable: writable(&window, "AXMain"),
            focused_writable: writable(&window, "AXFocused"),
            window_main: boolean(&window, "AXMain"),
            application_main_window_cf_match: cf_window_match(
                &application,
                "AXMainWindow",
                &window,
                self.pid,
            ),
            window_focused: boolean(&window, "AXFocused"),
            application_focused_window_cf_match: app_focus,
        };
        self.window_local_check_retained(&meta, &application, &window, owner, &provider_epoch)?;
        let after = read_foreground_custody(self.pid, Some(&foreground))?;
        let after_focus = cf_window_match(&application, "AXFocusedWindow", &window, self.pid);
        let after_custody = custody(
            self.pid,
            after.pid(),
            after
                .full_window()
                .map(|full| full.window == self.window_name()),
            after_focus,
        );
        if !same_full_foreground(foreground.full_window(), after.full_window())
            || after_custody != facts.application_focus_custody
            || after_custody != ApplicationFocusCustody::NoActiveForeignSibling
        {
            return Err(fault(
                "window_local_custody_changed",
                "Exact foreground/app-focus custody changed, is an active sibling or unknown; no setter.",
            ));
        }
        Ok(Sample {
            meta,
            application,
            window,
            owner,
            provider_epoch,
            facts,
            foreground: after,
        })
    }
    fn window_local_check_retained(
        &self,
        meta: &Metadata,
        application: &Cf,
        window: &Cf,
        owner: u32,
        provider_epoch: &str,
    ) -> Result<()> {
        let after = self.capture_guard_metadata()?;
        let row = self.window_record()?;
        let fresh = self.resolve_window_element(application, row.rect)?;
        if after.identity != meta.identity
            || !same_context(&after.binding, &meta.binding)
            || after.blocked != Some(false)
            || after.modal != Some(false)
            || !after.mapping_ready
            || !row.rect.same(meta.rect)
            || unsafe { ffi::CFEqual(window.0, fresh.0) } == 0
            || fresh.attr("AXRole")?.string()? != "AXWindow"
            || fresh.pid()? != owner
            || process_epoch(owner)? != provider_epoch
        {
            return Err(fault(
                "window_local_retained_context_changed",
                "Exact retained provider/window/context changed; no conclusion or replay.",
            ));
        }
        Ok(())
    }
    pub(crate) fn window_local_preflight(
        &self,
        target: &Resolved,
        kind: WindowLocalKind,
    ) -> Result<WindowLocalNativeFacts> {
        Ok(self.window_local_sample(target, kind, None, None)?.facts)
    }
    fn window_local_readback(
        &self,
        s: &Sample,
        plan: WindowLocalWritePlan,
        status: Option<i32>,
    ) -> Result<WindowLocalReadback> {
        let read = || {
            (
                boolean(&s.window, "AXMain"),
                cf_window_match(&s.application, "AXMainWindow", &s.window, self.pid),
                boolean(&s.window, "AXFocused"),
                cf_window_match(&s.application, "AXFocusedWindow", &s.window, self.pid),
            )
        };
        let a = read();
        self.window_local_check_retained(
            &s.meta,
            &s.application,
            &s.window,
            s.owner,
            &s.provider_epoch,
        )?;
        let b = read();
        self.window_local_check_retained(
            &s.meta,
            &s.application,
            &s.window,
            s.owner,
            &s.provider_epoch,
        )?;
        let foreground_after = read_foreground_custody(self.pid, Some(&s.foreground))?;
        Ok(WindowLocalReadback {
            kind: s.facts.kind,
            write_plan: plan,
            setter_attempt: if status.is_some() {
                FocusSetterAttempt::Attempted
            } else {
                FocusSetterAttempt::NotAttempted
            },
            setter_status: status,
            window_main: consistent(a.0, b.0),
            application_main_window_cf_match: consistent(a.1, b.1),
            window_focused: consistent(a.2, b.2),
            application_focused_window_cf_match: consistent(a.3, b.3),
            os_foreground_before: s.foreground.full_window().cloned(),
            os_foreground_after: foreground_after.full_window().cloned(),
            public_application_custody: s.foreground.public_application_readback(&foreground_after),
        })
    }
    pub(crate) fn dispatch_window_local(
        &self,
        expected: &Binding,
        target: &Resolved,
        kind: WindowLocalKind,
        ceiling: WindowLocalWritePlan,
    ) -> Result<WindowLocalReadback> {
        let (plan, sample, attempted) = once(
            || {
                let fresh = self.window_local_sample(target, kind, Some(expected), None)?;
                Ok((refine(ceiling, &fresh.facts)?, fresh))
            },
            |approved, sample| {
                let mut fresh = self.window_local_sample(
                    target,
                    kind,
                    Some(expected),
                    Some(&sample.foreground),
                )?;
                if unsafe { ffi::CFEqual(sample.window.0, fresh.window.0) } == 0
                    || sample.owner != fresh.owner
                    || sample.provider_epoch != fresh.provider_epoch
                    || !same_full_foreground(
                        sample.foreground.full_window(),
                        fresh.foreground.full_window(),
                    )
                {
                    return Err(fault(
                        "window_local_boundary_changed",
                        "Exact CF/provider/foreground changed at write boundary; no setter.",
                    ));
                }
                fresh.facts.window_main = boolean(&fresh.window, "AXMain");
                fresh.facts.application_main_window_cf_match =
                    cf_window_match(&fresh.application, "AXMainWindow", &fresh.window, self.pid);
                fresh.facts.window_focused = boolean(&fresh.window, "AXFocused");
                fresh.facts.application_focused_window_cf_match = cf_window_match(
                    &fresh.application,
                    "AXFocusedWindow",
                    &fresh.window,
                    self.pid,
                );
                fresh.facts.main_writable = writable(&fresh.window, "AXMain");
                fresh.facts.focused_writable = writable(&fresh.window, "AXFocused");
                let plan = refine(approved, &fresh.facts)?;
                let fg = read_foreground_custody(self.pid, Some(&fresh.foreground))?;
                if !same_full_foreground(fg.full_window(), fresh.foreground.full_window())
                    || custody(
                        self.pid,
                        fg.pid(),
                        fg.full_window()
                            .map(|full| full.window == self.window_name()),
                        fresh.facts.application_focused_window_cf_match,
                    ) != ApplicationFocusCustody::NoActiveForeignSibling
                {
                    return Err(fault(
                        "window_local_custody_changed",
                        "Final exact foreground/app-focus resource changed; no setter.",
                    ));
                }
                if plan == WindowLocalWritePlan::SetAttributeTrueOnce
                    && fg.pid() == self.pid
                    && crate::activity::seconds_since_user_input().is_none_or(|t| t < 1.2)
                {
                    return Err(fault(
                        "user_active",
                        "Recent/unknown input may affect this foreground app focus resource; no setter.",
                    ));
                }
                fresh.foreground = fg;
                Ok((plan, fresh))
            },
            |s| {
                let _call = operation::native_call("macos_window_local_setter_once")?;
                let name = CFString::new(kind.attribute());
                let yes = CFBoolean::true_value();
                operation::checkpoint("before_exact_window_local_setter")?;
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
                        s.window.0,
                        name.as_concrete_TypeRef().cast(),
                        yes.as_concrete_TypeRef().cast(),
                    )
                };
                let after = self.dispatches.load(std::sync::atomic::Ordering::Acquire);
                Ok((native_status, before, after))
            },
        )?;
        let status = attempted.map(|(raw, _, _)| raw);
        let readback = self.window_local_readback(&sample, plan, status);
        if let Some(native_status) = status
            && (native_status != 0 || readback.is_err())
        {
            let mut e = fault(
                "window_local_selection_unconfirmed",
                "Window attribute returned or readback failed; effect unknown. Query the original operation; never replay or activate/type/raise.",
            );
            e.diagnostic = Some(Box::new(
                serde_json::json!({"version":1,"operation_kind":"window_local_selection",
                "api":"AXUIElementSetAttributeValue","attribute":kind.attribute(),"kind":kind,"native_window_role":"AXWindow",
                "window":self.window_name(),"pid":self.pid,"native_setter_attempted":true,"native_setter_attempts":1,
                "native_status":native_status,"native_dispatch_counter_known":true,
                "native_dispatch_count_before":attempted.map(|(_,b,_)|b),"native_dispatch_count_after":attempted.map(|(_,_,a)|a),
                "native_call_pending":false,"native_resources_released":true,"held_input_resources":false,"replay_allowed":false,
                "scope":target.scope,"readback_error":readback.as_ref().err(),
                "window_local_readback":readback.as_ref().ok().map(|r|rwi_core::window_local::summarize_window_local(r.clone()))}),
            ));
            return Err(e);
        }
        readback
    }
}
#[cfg(test)]
mod tests;
