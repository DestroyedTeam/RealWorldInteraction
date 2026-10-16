//! Exact-target-only convergence for asynchronous accessibility hit providers.
//! Full snapshot walks retain single-sample visibility and their existing budget.
use super::*;
use crate::native::hit_settle::{self, Sample, Verdict};

pub(super) fn observed(
    native: &NativeMac,
    node: &Cf,
    window: &Cf,
    application: &Cf,
    bounds: Rect,
    blocked: Option<bool>,
    diagnostics: &mut Vec<serde_json::Value>,
) -> Result<AxElement> {
    let mut element = objects::describe(node, window, &native.objects)?;
    let point = element.element.point;
    // Keep ordinary bounds/modal refusal behavior, including unknown modality.
    if !bounds.contains(point) || blocked != Some(false) {
        return observed_element(
            node,
            window,
            application,
            bounds,
            blocked,
            diagnostics,
            &native.objects,
        );
    }
    let expected_owner = node.pid()?;
    let expected_owner_epoch = process_epoch(expected_owner)?;
    let started = Instant::now();
    let mut records = Vec::with_capacity(3);
    let mut outcome = hit_settle::settle(
        point,
        |same_point, budget| {
            operation::checkpoint("macos_exact_hit_sample_guard")?;
            if console_identity()? != native.identity
                || process_epoch(native.pid)? != native.process
                || process_epoch(expected_owner)? != expected_owner_epoch
            {
                return Err(fault(
                    "session_or_process_changed",
                    "Exact hit sampling lost its pinned context/process; no input.",
                ));
            }
            let current = native.window_record()?;
            if !current.rect.same(bounds) || !ax_rect(window)?.same(bounds) {
                return Err(fault(
                    "window_binding_changed",
                    "Exact hit sampling lost its original window geometry; no input.",
                ));
            }
            let remaining = hit_settle::BUDGET.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(fault(
                    "ax_hit_settle_deadline",
                    "No budget remains for a native hit read; no input.",
                ));
            }
            let budget = budget.min(remaining);
            let code =
                unsafe { ffi::AXUIElementSetMessagingTimeout(application.0, budget.as_secs_f32()) };
            if code != 0 {
                return Err(fault(
                    "ax_hit_probe_timeout_configuration",
                    "Cannot bound this exact hit read; no input.",
                ));
            }
            let _call = operation::native_call("macos_exact_hit_sample")?;
            let hit = hit_test(application, same_point)?;
            let hit_owner = hit.pid().ok();
            let exact = unsafe { ffi::CFEqual(hit.0, node.0) != 0 };
            let equivalent = label_hit::equivalent(
                node,
                &hit,
                window,
                same_point,
                element.element.role.as_deref(),
                hit_settle::BUDGET.saturating_sub(started.elapsed()),
            )?;
            let same_window = if unsafe { ffi::CFEqual(hit.0, window.0) != 0 } {
                Some(true)
            } else {
                hit.attr("AXWindow")
                    .ok()
                    .map(|owner| unsafe { ffi::CFEqual(owner.0, window.0) != 0 })
            };
            let rect = ax_rect(&hit).ok();
            let geometry = rect.is_some_and(|rect| rect.contains(same_point));
            let record = serde_json::json!({
                "sample":records.len()+1,"elapsed_ms":started.elapsed().as_millis(),
                "point":same_point,"node_pid":expected_owner,
                "node_window":ffi::window_number_for_ax_element(window),
                "hit_pid":hit_owner,"hit_window":hit.attr("AXWindow").ok().as_ref().and_then(ffi::window_number_for_ax_element),
                "hit_role":hit.attr("AXRole").ok().and_then(|role|role.string().ok()).map(|role|role.chars().take(64).collect::<String>()),
                "hit_rect":rect.map(|rect|[rect.x,rect.y,rect.w,rect.h]),
                "equal":exact,"equivalent":equivalent,"label_equivalent":equivalent && !exact,
                "same_native_window":same_window,"point_inside_hit":geometry,
                "native_input_dispatched":false
            });
            records.push(record.clone());
            if hit_owner != Some(expected_owner) {
                return Err(fault(
                    "ax_hit_provider_changed",
                    "Hit provider differs from the retained native target; no input or cross-PID replacement.",
                ));
            }
            let verdict = match (equivalent, same_window, geometry) {
                (true, Some(true), true) => Verdict::Exact,
                (_, Some(false), _) => Verdict::ForeignWindow,
                (false, Some(true), true) => Verdict::SameWindowMismatch,
                _ => Verdict::Unknown,
            };
            Ok(Sample {
                verdict,
                detail: record,
            })
        },
        || started.elapsed(),
        std::thread::sleep,
        || operation::checkpoint("macos_exact_hit_settle"),
    );
    // metadata_read constructed this scope-local application with 50 ms reads.
    // Restore that caller budget before its remaining metadata work, including
    // on cancellation/refusal; no global AX object or getter is involved.
    let restore_code = unsafe { ffi::AXUIElementSetMessagingTimeout(application.0, 0.05) };
    if restore_code != 0 && outcome.is_ok() {
        outcome = Err(fault(
            "ax_hit_probe_timeout_restore",
            "Cannot restore the caller's scope-local AX read budget; no input.",
        ));
    }
    match outcome {
        Ok(outcome) if outcome.accepted => {
            debug_assert!(outcome.samples.len() <= 3);
            debug_assert!(outcome.elapsed < hit_settle::BUDGET);
            debug_assert!(
                outcome
                    .samples
                    .last()
                    .is_some_and(|sample| sample.detail["equivalent"] == true)
            );
            element.element.visible = Some(true);
            element.element.occluded = Some(false);
            Ok(element)
        }
        result => {
            diagnostics.extend(
                records
                    .iter()
                    .take(8usize.saturating_sub(diagnostics.len()))
                    .cloned(),
            );
            if let Ok(mut saved) = native.hit_diagnostics.lock() {
                *saved = diagnostics.clone();
            }
            let mut error = result.err().unwrap_or_else(|| fault("ax_hit_not_stable", "Same-point native hits did not converge twice on the original target; no input or focus fallback."));
            error.diagnostic = Some(Box::new(serde_json::json!({
                "stage":"macos_exact_target_hit_settle","budget_ms":120,"max_samples":3,
                "samples":records,"timeout_restore_code":restore_code,
                "native_input_dispatched":false,"replay_allowed":false
            })));
            Err(error)
        }
    }
}
