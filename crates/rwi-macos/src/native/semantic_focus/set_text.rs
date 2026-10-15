//! Explicit public AXValue replacement. Parent exact-object custody is readonly.
use super::*;
use rwi_core::set_text::{
    SET_TEXT_SETTLEMENT_BUDGET_MS, SET_TEXT_SETTLEMENT_MAX_READS, SetTextNativeFacts,
    SetTextReadback, SetTextSettlement, TextSettlementRead, TextValueRead, set_text_pair_matches,
    set_text_settlement_needed, set_text_value_read_known, summarize_set_text_readback,
    validate_text,
};

fn ordinary_role(role: &str, subrole: Option<&str>) -> bool {
    matches!(role, "AXTextField" | "AXTextArea") && subrole == Some("")
}
fn checked_plain_value<T>(
    role: &str,
    subrole: Option<&str>,
    read: impl FnOnce() -> Result<T>,
) -> Result<T> {
    if !ordinary_role(role, subrole) || TextClass::classify(role, subrole) != TextClass::Plain {
        return Err(fault(
            "plain_string_value_required",
            "Ordinary known nonspecialized text role required; no secure or unknown value read.",
        ));
    }
    read()
}
fn ordinary_class(element: &Cf) -> Result<()> {
    let role = element.attr("AXRole")?.string()?;
    let subrole = match element.attr("AXSubrole") {
        Ok(value) => Some(value.string()?),
        Err(error) if ffi::attribute_is_absent(&error, "AXSubrole") => Some(String::new()),
        Err(error) => return Err(error),
    };
    checked_plain_value(&role, subrole.as_deref(), || Ok(()))
}
fn ordinary_value(element: &Cf) -> Result<String> {
    ordinary_class(element)?;
    element.attr("AXValue")?.string()
}
fn value_read(element: &Cf, requested: &str) -> Result<TextValueRead> {
    // Reclassify before every post-call value read. No newly secure value leak.
    ordinary_class(element)?;
    Ok(read_string_value(element, requested))
}
fn read_string_value(element: &Cf, requested: &str) -> TextValueRead {
    let (native_status, value) = raw_attribute(element, "AXValue");
    let cf_string = value
        .as_ref()
        .map(|v| unsafe { ffi::CFGetTypeID(v.0) == ffi::CFStringGetTypeID() });
    let full = value.as_ref().and_then(|v| v.string().ok());
    TextValueRead {
        native_status,
        cf_string,
        matches_requested: full.as_ref().map(|s| s == requested),
        utf8_bytes: full.as_ref().map(String::len),
        sha256: full.map(|s| digest(s.as_bytes())),
    }
}
fn require_eligibility(sample: &Sample) -> Result<()> {
    // Exact principal, provider, CF identity, context, app-sibling and writable
    // facts are freshly refined. Discard the returned focus plan: no focus write.
    let _ = refine_plan(FocusWritePlan::SetAXFocusedTrueOnce, &sample.facts)?;
    ordinary_value(&sample.element)?;
    Ok(())
}
/// The callback is read-only. Fake clock/tests use the same bounded collector.
fn collect_settlement(
    initial: &SetTextReadback,
    mut elapsed: impl FnMut() -> Duration,
    mut read: impl FnMut(Duration) -> Result<TextSettlementRead>,
) -> Result<Option<SetTextSettlement>> {
    // The usual successful pair must not call clock/probe or add a default wait.
    if !set_text_settlement_needed(initial) {
        return Ok(None);
    }
    let budget = Duration::from_millis(SET_TEXT_SETTLEMENT_BUDGET_MS);
    let mut reads = Vec::new();
    let fail = |mut error: rwi_core::Fault, reads: &[TextSettlementRead], spent: Duration| {
        let probe_error = error.clone();
        error.diagnostic = Some(Box::new(serde_json::json!({
            "initial_readback":summarize_set_text_readback(initial.clone()),
            "settlement":{"state":"unknown", "reads":reads,
                "elapsed_ms":spent.as_secs_f64()*1000.0,
                "budget_ms":SET_TEXT_SETTLEMENT_BUDGET_MS,
                "context_verified":false, "probe_error":probe_error},
            "replay_allowed":false, "setter_attempts":initial.setter_attempts
        })));
        error
    };
    let mut previous = initial.second.clone();
    for _ in 0..SET_TEXT_SETTLEMENT_MAX_READS {
        let spent = elapsed();
        let remaining = budget.checked_sub(spent).filter(|d| !d.is_zero()).ok_or_else(|| {
            fail(fault("set_text_settlement_budget_expired", "Readonly settlement budget exhausted; preserve initial evidence, no replay."), &reads, spent)
        })?;
        let sample = read(remaining).map_err(|e| fail(e, &reads, elapsed()))?;
        let matched = set_text_pair_matches(&previous, &sample.value);
        previous = sample.value.clone();
        reads.push(sample);
        if !set_text_value_read_known(&previous) {
            return Err(fail(
                fault(
                    "set_text_settlement_read_unknown",
                    "Extra AXValue type/status/string is unknown; stop observation, no replay.",
                ),
                &reads,
                elapsed(),
            ));
        }
        let spent = elapsed();
        if spent >= budget {
            return Err(fail(
                fault(
                    "set_text_settlement_budget_expired",
                    "Native read exceeded the readonly budget; no strong preemption or replay claimed.",
                ),
                &reads,
                spent,
            ));
        }
        if matched {
            return Ok(Some(SetTextSettlement {
                reads,
                elapsed_ms: spent.as_secs_f64() * 1000.0,
                budget_ms: SET_TEXT_SETTLEMENT_BUDGET_MS,
                context_verified: true,
                stop_reason: "matched_pair".into(),
            }));
        }
    }
    Ok(Some(SetTextSettlement {
        reads,
        elapsed_ms: elapsed().as_secs_f64() * 1000.0,
        budget_ms: SET_TEXT_SETTLEMENT_BUDGET_MS,
        context_verified: true,
        stop_reason: "read_limit".into(),
    }))
}

impl NativeMac {
    fn settlement_context(&self, sample: &Sample, deadline: Instant) -> Result<()> {
        operation::checkpoint("set_text_settlement_context")?;
        if Instant::now() >= deadline {
            return Err(fault(
                "set_text_settlement_budget_expired",
                "Extra readonly budget expired; no replay.",
            ));
        }
        let guard = self.capture_guard_metadata()?;
        let window =
            self.resolve_window_element(&sample.application, self.window_record()?.rect)?;
        let foreground = read_foreground_custody(self.pid, Some(&sample.foreground))?;
        let element_window = sample.element.attr("AXWindow")?;
        ax_type(&element_window)?;
        if guard.identity != sample.meta.identity
            || guard.binding != sample.meta.binding
            || guard.blocked != Some(false)
            || !guard.mapping_ready
            || unsafe { ffi::CFEqual(window.0, sample.window.0) } == 0
            || unsafe { ffi::CFEqual(element_window.0, sample.window.0) } == 0
            || sample.element.pid()? != sample.owner
            || process_epoch(sample.owner)? != sample.provider_epoch
            || foreground.full_window != sample.foreground.full_window
            || unsafe { ffi::AXIsProcessTrusted() } == 0
            || !self.allow_input
        {
            return Err(fault(
                "set_text_settlement_context_changed",
                "Retained element/window/provider/native context changed; effect unknown, never replay.",
            ));
        }
        operation::checkpoint("after_set_text_settlement_context")?;
        if Instant::now() >= deadline {
            return Err(fault(
                "set_text_settlement_budget_expired",
                "Extra context read exceeded readonly budget; no replay.",
            ));
        }
        Ok(())
    }
    fn set_text_settlement(
        &self,
        sample: &Sample,
        text: &str,
        initial: &SetTextReadback,
        setter_returned: Instant,
    ) -> Result<Option<SetTextSettlement>> {
        if !set_text_settlement_needed(initial) {
            return Ok(None);
        }
        // Capture clock before remaining budget: delayed sampling cannot extend
        // the original operation or lease. Only the extra phase gets 100ms.
        let started = Instant::now();
        let remaining = operation::dispatch_budget()?;
        let deadline =
            started + remaining.min(Duration::from_millis(SET_TEXT_SETTLEMENT_BUDGET_MS));
        collect_settlement(
            initial,
            || started.elapsed(),
            |extra_remaining| {
                operation::checkpoint("before_set_text_settlement_read")?;
                let remaining = deadline
                    .checked_duration_since(Instant::now())
                    .filter(|d| !d.is_zero())
                    .ok_or_else(|| {
                        fault(
                            "set_text_settlement_budget_expired",
                            "No extra readonly time remains; no replay.",
                        )
                    })?
                    .min(extra_remaining)
                    .min(operation::dispatch_budget()?);
                let _call = operation::native_call("macos_set_text_readonly_settlement")?;
                timeout(
                    &sample.element,
                    remaining.min(Duration::from_millis(50)).as_secs_f32(),
                )?;
                self.settlement_context(sample, deadline)?;
                let read_started = Instant::now();
                ordinary_class(&sample.element)?;
                // Classification can consume time. Recheck before the AXValue
                // read itself rather than issue it with an old timeout budget.
                operation::checkpoint("before_extra_full_axvalue_read")?;
                let value_remaining = deadline
                    .checked_duration_since(Instant::now())
                    .filter(|d| !d.is_zero())
                    .ok_or_else(|| {
                        fault(
                            "set_text_settlement_budget_expired",
                            "Classification exhausted extra budget; no AXValue read or replay.",
                        )
                    })?
                    .min(operation::dispatch_budget()?);
                timeout(
                    &sample.element,
                    value_remaining.min(Duration::from_millis(50)).as_secs_f32(),
                )?;
                let value = read_string_value(&sample.element, text);
                let read_completed = Instant::now();
                let observed = TextSettlementRead {
                    value,
                    offset_since_setter_return_ms: read_completed
                        .duration_since(setter_returned)
                        .as_secs_f64()
                        * 1000.0,
                    read_elapsed_ms: read_completed.duration_since(read_started).as_secs_f64()
                        * 1000.0,
                };
                // Preserve a returned value even when its subsequent context bracket
                // fails; such a sample is evidence, never confirmed replacement.
                self.settlement_context(sample, deadline)
                    .map_err(|mut error| {
                        let context_error = error.clone();
                        error.diagnostic = Some(Box::new(serde_json::json!({
                            "unconfirmed_value_read":observed, "context_error":context_error,
                            "context_verified":false, "replay_allowed":false
                        })));
                        error
                    })?;
                Ok(observed)
            },
        )
    }
    pub(crate) fn set_text_preflight_native(
        &self,
        target: &Resolved,
    ) -> Result<SetTextNativeFacts> {
        let sample = self.semantic_focus_sample(target, None, None)?;
        require_eligibility(&sample)?;
        Ok(SetTextNativeFacts {
            exact: sample.facts,
            ordinary_string_value: true,
        })
    }
    pub(crate) fn dispatch_set_text(
        &self,
        expected: &Binding,
        target: &Resolved,
        text: &str,
    ) -> Result<SetTextReadback> {
        validate_text(text)?;
        let first = self.semantic_focus_sample(target, Some(expected), None)?;
        require_eligibility(&first)?;
        let before_value = ordinary_value(&first.element)?;
        // Allocate the exact unencoded string before the final identity guard.
        let name = CFString::new("AXValue");
        let replacement = CFString::new(text);
        let sample = self.semantic_focus_sample(target, Some(expected), Some(&first.foreground))?;
        require_eligibility(&sample)?;
        if unsafe { ffi::CFEqual(first.window.0, sample.window.0) } == 0
            || unsafe { ffi::CFEqual(first.element.0, sample.element.0) } == 0
            || first.owner != sample.owner
            || first.provider_epoch != sample.provider_epoch
            || first.foreground.full_window != sample.foreground.full_window
            || ordinary_value(&sample.element)? != before_value
        {
            return Err(fault(
                "set_text_native_object_changed",
                "Retained element/window/provider, value or custody changed; zero setters, no replay.",
            ));
        }
        // Foreground same-app mutation keeps the existing HID/sibling policy.
        if sample.foreground.identity.pid == self.pid
            && crate::activity::seconds_since_user_input().is_none_or(|s| s < 1.2)
        {
            return Err(fault(
                "user_active",
                "Recent/unknown user activity forbids foreground value replacement.",
            ));
        }
        let foreground = read_foreground_custody(self.pid, Some(&sample.foreground))?;
        if foreground.full_window != sample.foreground.full_window {
            return Err(fault(
                "set_text_context_changed",
                "Final foreground window custody changed; no setter.",
            ));
        }
        let guard = self.capture_guard_metadata()?;
        let window =
            self.resolve_window_element(&sample.application, self.window_record()?.rect)?;
        if guard.identity != sample.meta.identity
            || guard.binding != sample.meta.binding
            || guard.blocked != Some(false)
            || !guard.mapping_ready
            || unsafe { ffi::CFEqual(window.0, sample.window.0) } == 0
            || process_epoch(sample.owner)? != sample.provider_epoch
            || unsafe { ffi::AXIsProcessTrusted() } == 0
            || !self.allow_input
        {
            return Err(fault(
                "set_text_context_changed",
                "Final native/TCC/provider/window/context check refused; no setter.",
            ));
        }
        // Last exact-object class/value/writability reads after the native
        // context bracket. Never make a focus/key call to repair this gate.
        if ordinary_value(&sample.element)? != before_value
            || !known(writable(&sample.element, "AXValue"))?
        {
            return Err(fault(
                "set_text_value_changed",
                "Final full value or writability changed; zero setters.",
            ));
        }
        let _call = operation::native_call("macos_set_text_setter_once")?;
        operation::checkpoint("before_exact_set_text_setter")?;
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
                replacement.as_concrete_TypeRef().cast(),
            )
        };
        let setter_returned = Instant::now();
        let after = self.dispatches.load(std::sync::atomic::Ordering::Acquire);
        drop(_call);
        let readback = (|| {
            timeout(&sample.element, 0.05)?;
            let first = value_read(&sample.element, text)?;
            // Existing exact-window/provider/context bracket, no setter/keyboard.
            let focus =
                self.semantic_focus_readback(&sample, FocusWritePlan::NoWriteAlreadyFocused, None)?;
            let second = value_read(&sample.element, text)?;
            let guard = self.capture_guard_metadata()?;
            let window =
                self.resolve_window_element(&sample.application, self.window_record()?.rect)?;
            let final_foreground = read_foreground_custody(self.pid, Some(&sample.foreground))?;
            if guard.identity != sample.meta.identity
                || guard.binding != sample.meta.binding
                || guard.blocked != Some(false)
                || unsafe { ffi::CFEqual(window.0, sample.window.0) } == 0
                || process_epoch(sample.owner)? != sample.provider_epoch
                || final_foreground.full_window != sample.foreground.full_window
            {
                return Err(fault(
                    "set_text_context_changed_after_call",
                    "Readback context changed after one setter; effect unknown, never replay.",
                ));
            }
            Ok(SetTextReadback {
                setter_attempts: 1,
                setter_status: native_status,
                first,
                second,
                context_verified: true,
                focus_readback: Some(
                    serde_json::to_value(rwi_core::semantic_focus::summarize_focus_readback(focus))
                        .unwrap(),
                ),
                settlement: None,
            })
        })();
        let readback = readback.and_then(|mut raw| {
            raw.settlement = self
                .set_text_settlement(&sample, text, &raw, setter_returned)
                .map_err(|mut error| {
                    let settlement_error = error.clone();
                    error.diagnostic = Some(Box::new(serde_json::json!({
                        "initial_readback":summarize_set_text_readback(raw.clone()),
                        "settlement_error":settlement_error, "replay_allowed":false,
                        "native_setter_attempts":1
                    })));
                    error
                })?;
            Ok(raw)
        });
        if native_status != 0 || readback.is_err() {
            let mut error = fault(
                "set_text_unconfirmed",
                "One exact AXValue setter returned an error or readback failed; preserve receipt and inspect only. No keyboard/focus/clipboard fallback.",
            );
            error.diagnostic = Some(Box::new(
                serde_json::json!({"version":1,"api":"AXUIElementSetAttributeValue","attribute":"AXValue",
                "operation_kind":"set_text","native_setter_attempted":true,"native_setter_attempts":1,"native_status":native_status,
                "native_dispatch_counter_known":true,"native_dispatch_count_before":before,"native_dispatch_count_after":after,
                "resource_release_evidence":"actual Agent operation status, not inferred from readback","held_input_resources":false,"replay_allowed":false,
                "scope":target.scope,"requested_utf8_bytes":text.len(),"requested_sha256":digest(text.as_bytes()),
                "readback_error":readback.as_ref().err(),"value_readback":readback.as_ref().ok()}),
            ));
            return Err(error);
        }
        readback
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_set_text_native_plain_roles_reject_secure_unknown_specialized_before_value() {
        for role in ["AXTextField", "AXTextArea"] {
            assert!(ordinary_role(role, Some("")));
        }
        for role in [
            "AXComboBox",
            "AXDateField",
            "AXTimeField",
            "AXSlider",
            "AXStaticText",
            "AXCheckBox",
        ] {
            assert!(!ordinary_role(role, Some("")));
        }
        for subrole in [
            None,
            Some("AXUnknown"),
            Some("AXSecureTextField"),
            Some("AXSearchField"),
            Some("AXNumericTextField"),
        ] {
            assert!(!ordinary_role("AXTextField", subrole));
        }
    }
    #[test]
    fn explicit_set_text_native_bad_class_never_reads_or_writes_value() {
        for (role, subrole) in [
            ("AXTextField", Some("AXSecureTextField")),
            ("AXTextField", None),
            ("AXComboBox", Some("")),
            ("AXDateField", Some("")),
        ] {
            assert!(
                checked_plain_value::<String>(role, subrole, || panic!(
                    "no secure/unknown AXValue access"
                ))
                .is_err()
            );
        }
    }
    #[test]
    fn explicit_set_text_native_cfstring_roundtrip_is_exact_not_scalar_keyboard() {
        for text in ["", " 中文\t🙂\n𝄞 e\u{301} 👩\u{200d}💻 "] {
            let value = CFString::new(text);
            assert_eq!(
                unsafe { ffi::string(value.as_concrete_TypeRef().cast()) }.unwrap(),
                text
            );
        }
    }
    fn settlement_initial(first: bool, second: bool) -> SetTextReadback {
        let read = |matches| TextValueRead {
            native_status: 0,
            cf_string: Some(true),
            matches_requested: Some(matches),
            utf8_bytes: Some(if matches { 33 } else { 0 }),
            sha256: Some(if matches { "requested" } else { "empty" }.into()),
        };
        SetTextReadback {
            setter_attempts: 1,
            setter_status: 0,
            first: read(first),
            second: read(second),
            context_verified: true,
            focus_readback: None,
            settlement: None,
        }
    }
    #[test]
    fn bounded_set_text_settlement_fast_path_and_two_read_progression_no_sleep() {
        let good = settlement_initial(true, true);
        assert!(
            collect_settlement(
                &good,
                || panic!("no extra timer"),
                |_| panic!("no extra read")
            )
            .unwrap()
            .is_none()
        );
        for second in [false, true] {
            let initial = settlement_initial(false, second);
            let clock = std::cell::Cell::new(Duration::ZERO);
            let count = std::cell::Cell::new(0);
            let result = collect_settlement(
                &initial,
                || clock.get(),
                |remaining| {
                    assert!(remaining <= Duration::from_millis(100));
                    clock.set(clock.get() + Duration::from_millis(1));
                    count.set(count.get() + 1);
                    Ok(TextSettlementRead {
                        value: settlement_initial(true, true).second,
                        offset_since_setter_return_ms: 220.0 + count.get() as f64,
                        read_elapsed_ms: 1.0,
                    })
                },
            )
            .unwrap()
            .unwrap();
            assert_eq!(count.get(), if second { 1 } else { 2 });
            assert_eq!(result.stop_reason, "matched_pair");
            assert_eq!(initial.first.matches_requested, Some(false));
            assert_eq!(initial.setter_attempts, 1);
        }
    }
    #[test]
    fn bounded_set_text_settlement_errors_and_fake_deadline_stop_without_replay() {
        let initial = settlement_initial(false, false);
        for code in [
            "cancelled",
            "lease_expired_during_dispatch",
            "native_context_changed",
            "plain_string_value_required",
        ] {
            let count = std::cell::Cell::new(0);
            let error = collect_settlement(
                &initial,
                || Duration::ZERO,
                |_| {
                    count.set(count.get() + 1);
                    Err(fault(code, "read refused"))
                },
            )
            .unwrap_err();
            assert_eq!(error.code, code);
            assert_eq!(count.get(), 1);
            let detail = error.diagnostic.unwrap();
            assert_eq!(detail["setter_attempts"], 1);
            assert_eq!(
                detail["initial_readback"]["first"]["matches_requested"],
                false
            );
            assert_eq!(detail["replay_allowed"], false);
        }
        let count = std::cell::Cell::new(0);
        let error = collect_settlement(
            &initial,
            || Duration::from_millis(100),
            |_| {
                count.set(count.get() + 1);
                panic!("expired budget must not read")
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "set_text_settlement_budget_expired");
        assert_eq!(count.get(), 0);
        let count = std::cell::Cell::new(0);
        let error = collect_settlement(
            &initial,
            || Duration::ZERO,
            |_| {
                count.set(count.get() + 1);
                let mut value = settlement_initial(true, true).second;
                value.cf_string = None;
                Ok(TextSettlementRead {
                    value,
                    offset_since_setter_return_ms: 221.0,
                    read_elapsed_ms: 1.0,
                })
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "set_text_settlement_read_unknown");
        assert_eq!(count.get(), 1);
        let clock = std::cell::Cell::new(Duration::ZERO);
        let error = collect_settlement(
            &initial,
            || clock.get(),
            |_| {
                clock.set(Duration::from_millis(101));
                Ok(TextSettlementRead {
                    value: settlement_initial(true, true).second,
                    offset_since_setter_return_ms: 321.0,
                    read_elapsed_ms: 101.0,
                })
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "set_text_settlement_budget_expired");
        assert_eq!(
            error.diagnostic.unwrap()["settlement"]["reads"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}
