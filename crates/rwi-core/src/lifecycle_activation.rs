//! Explicit once-only activation; API acceptance is not foreground completion.
use super::*;

// Cross-input-queue activation completes asynchronously; observe, never resend.
// https://devblogs.microsoft.com/oldnewthing/20161118-00/?p=94745
const OBSERVATION_MS: u64 = 500;
const OBSERVATION_SLICE_MS: u64 = 10;
const MAX_OBSERVATION_SLICES: u64 = OBSERVATION_MS / OBSERVATION_SLICE_MS;

struct ObservationBudget {
    deadline: Instant,
    pauses: u64,
}
impl ObservationBudget {
    fn check(&self) -> Result<()> {
        if Instant::now() >= self.deadline {
            return Err(timeout());
        }
        Ok(())
    }
    fn pause(&mut self, d: &mut dyn LifecycleDriver) -> Result<()> {
        self.check()?;
        if self.pauses == MAX_OBSERVATION_SLICES {
            return Err(timeout());
        }
        self.pauses += 1;
        let remaining = self
            .deadline
            .saturating_duration_since(Instant::now())
            .as_millis() as u64;
        d.pause(remaining.min(OBSERVATION_SLICE_MS));
        Ok(())
    }
}

pub struct ActivateRequest<'a> {
    pub authorization_id: &'a str,
    pub request_id: &'a str,
    pub expected_foreground: &'a Option<WindowIdentity>,
}
fn yielded(r: &mut ActivationReceipt) {
    // The request may already have affected focus. This does not undo that effect.
    r.user_intervened = true;
    if r.error.is_none() {
        r.state = "yielded".into();
    }
}
fn failed(r: &mut ActivationReceipt, error: Fault, attempted: bool) {
    r.state = if attempted {
        "uncertain"
    } else {
        "not_dispatched"
    }
    .into();
    if r.error.is_none() {
        r.error = Some(error);
    }
}
fn timeout() -> Fault {
    fault(
        "activation_observation_timeout",
        "Restore/foreground completion exceeded the shared 500 ms / 50 observation-slice bound. An accepted effect may arrive later; reobserve, never resend this request.",
    )
}
fn presentation_fault() -> Fault {
    fault(
        "activation_presentation_unavailable",
        "Exact target is not visibly presented with non-cloaked, non-minimized, usable native bounds. Reobserve; flags and geometry do not verify rendered pixels or a business goal.",
    )
}
fn presented(r: &ActivationReceipt) -> bool {
    r.after_presentation
        .as_ref()
        .is_some_and(WindowPresentation::presented)
}
fn collect_events(r: &mut ActivationReceipt, events: EventBatch) -> Result<()> {
    r.native_events = events;
    event_ok(&r.native_events)?;
    let mut target_seen = false;
    let mut intervention = false;
    for event in &r.native_events.events {
        if event.window == r.window.window && matches!(event.event.as_str(), "create" | "destroy") {
            return Err(fault(
                "application_window_lifetime_changed",
                "The activation target was created/destroyed during observation; the same HWND is not proof of the same window. Reobserve without replay.",
            ));
        }
        if event.event != "foreground" {
            continue;
        }
        if event.window == r.window.window {
            target_seen = true;
        } else if event.window != "hwnd:0"
            && (target_seen
                || r.before_foreground
                    .as_ref()
                    .is_none_or(|w| w.window != event.window))
        {
            intervention = true;
        }
    }
    if intervention {
        // Sticky even when the current foreground has already returned to target.
        yielded(r);
    }
    Ok(())
}
fn sample(
    d: &mut dyn LifecycleDriver,
    g: &LifecycleGrant,
    r: &mut ActivationReceipt,
    input: u64,
    stage: &str,
) -> Result<()> {
    check(d, g, stage)?;
    d.check_window(&r.window)?;
    r.after_presentation = None;
    r.after_presentation = Some(d.window_presentation(&r.window)?);
    r.after_foreground = d.foreground()?;
    // GetLastInputInfo is a wrapping, non-monotonic session token. Equality is
    // only a sampled signal, never proof that all user intervention was absent.
    if d.user_input_tick()? != input
        || r.after_foreground
            .as_ref()
            .is_some_and(|w| w != &r.window && Some(w) != r.before_foreground.as_ref())
    {
        yielded(r);
    }
    check(d, g, stage)
}
fn observe_completion(
    d: &mut dyn LifecycleDriver,
    g: &LifecycleGrant,
    r: &mut ActivationReceipt,
    input: u64,
    budget: &mut ObservationBudget,
) -> Result<()> {
    loop {
        budget.check()?;
        collect_events(r, d.events()?)?;
        sample(d, g, r, input, "activation_observe_completion")?;
        if r.user_intervened {
            return Ok(());
        }
        if !presented(r) {
            return Err(presentation_fault());
        }
        if r.after_foreground.as_ref() == Some(&r.window) {
            r.state = "foreground_observed".into();
            return Ok(());
        }
        if r.api_accepted == Some(false) {
            r.state = "not_foreground".into();
            return Ok(());
        }
        // Only the exact original foreground or transient NULL can wait here.
        budget.pause(d)?;
    }
}

fn observe_restore(
    d: &mut dyn LifecycleDriver,
    g: &LifecycleGrant,
    r: &mut ActivationReceipt,
    input: u64,
    budget: &mut ObservationBudget,
) -> Result<()> {
    loop {
        budget.check()?;
        collect_events(r, d.events()?)?;
        sample(d, g, r, input, "activation_observe_restore")?;
        if r.user_intervened || presented(r) {
            return Ok(());
        }
        let p = r.after_presentation.as_ref().expect("sampled presentation");
        if !p.visible || p.cloaked != Some(false) {
            return Err(presentation_fault());
        }
        if r.restore_api_accepted == Some(false) {
            return Err(fault(
                "window_restore_not_accepted",
                "Native restore was not accepted and presentation was not independently observed. Do not focus or resend this request.",
            ));
        }
        budget.pause(d)?;
    }
}

fn effect(
    d: &mut dyn LifecycleDriver,
    attempted: &mut bool,
    f: impl FnOnce(&mut dyn LifecycleDriver) -> Result<bool>,
) -> Result<bool> {
    let count = d.native_effect_count();
    let was_attempted = *attempted;
    *attempted = true;
    let result = f(d);
    // A later pre-dispatch rejection cannot erase an earlier restore effect.
    if count.is_some() && count == d.native_effect_count() {
        *attempted = was_attempted;
    }
    result
}

pub fn activate(
    root: &Path,
    g: &LifecycleGrant,
    d: &mut dyn LifecycleDriver,
    req: ActivateRequest<'_>,
) -> Result<ActivationReceipt> {
    if !identifier(req.request_id)
        || req
            .expected_foreground
            .as_ref()
            .is_some_and(|w| !valid_window(w))
    {
        return Err(fault(
            "invalid_lifecycle_request",
            "Use a bounded request ID and fresh exact foreground tuple (null only when no foreground exists).",
        ));
    }
    let p = g
        .activations
        .iter()
        .find(|p| p.authorization_id == req.authorization_id)
        .ok_or_else(|| {
            fault(
                "unknown_lifecycle_permission",
                "Choose a preauthorized exact window activation.",
            )
        })?;
    let path = journal_path(root, g, "activation", req.authorization_id, req.request_id);
    let fingerprint = digest(
        &serde_json::to_vec(&(p, req.expected_foreground)).expect("activation serialization"),
    );
    if let Some(value) = existing(&path, &fingerprint)? {
        return serde_json::from_value(value).map_err(|_| {
            fault(
                "uncertain_journal",
                "Never replay a corrupt activation receipt.",
            )
        });
    }
    let cleanup = operation::CleanupAudit::begin();
    let mut scope = EventScope { d, finished: false };
    check(scope.d, g, "activation_preflight")?;
    scope.d.begin_activation_events(&p.window)?;
    scope.d.check_window(&p.window)?;
    let input = scope.d.user_input_tick()?;
    let before = scope.d.foreground()?;
    if &before != req.expected_foreground {
        return Err(fault(
            "foreground_changed",
            "Yield to user/other app; obtain a fresh observation rather than stealing focus.",
        ));
    }
    let before_presentation = scope.d.window_presentation(&p.window)?;
    let mut r = ActivationReceipt {
        request_id: req.request_id.into(),
        authorization_id: req.authorization_id.into(),
        window: p.window.clone(),
        state: "uncertain".into(),
        api_accepted: None,
        restore_api_accepted: None,
        before_presentation: Some(before_presentation),
        after_presentation: None,
        before_foreground: before,
        after_foreground: None,
        user_intervened: false,
        native_events: EventBatch::default(),
        error: None,
        goal_verified: false,
        input_authority_granted: false,
        replay: "forbidden".into(),
        resources_released: false,
        resource_release_uncertain: false,
    };
    let reservation = Reservation::create(
        path,
        fingerprint,
        serde_json::to_value(&r).expect("activation receipt serialization"),
    )?;
    let mut budget = ObservationBudget {
        deadline: Instant::now() + Duration::from_millis(OBSERVATION_MS),
        pauses: 0,
    };
    let mut attempted = false;
    let result = (|| -> Result<()> {
        collect_events(&mut r, scope.d.events()?)?;
        sample(scope.d, g, &mut r, input, "activation_before_dispatch")?;
        if r.after_foreground != *req.expected_foreground {
            yielded(&mut r);
        }
        if r.user_intervened {
            return Ok(());
        }
        if r.after_presentation
            .as_ref()
            .is_some_and(WindowPresentation::restore_eligible)
        {
            ensure_cleanup_clean(&cleanup)?;
            budget.check()?;
            let call = operation::native_call("ShowWindowAsync_restore_once")?;
            r.restore_api_accepted =
                Some(effect(scope.d, &mut attempted, |d| d.restore(&p.window))?);
            drop(call);
            observe_restore(scope.d, g, &mut r, input, &mut budget)?;
            if r.user_intervened {
                return Ok(());
            }
            // Restoration may activate by itself. Never reuse evidence from
            // before restoration to authorize an additional foreground request.
            collect_events(&mut r, scope.d.events()?)?;
            sample(scope.d, g, &mut r, input, "activation_after_restore")?;
            if r.user_intervened {
                return Ok(());
            }
        }
        ensure_cleanup_clean(&cleanup)?;
        budget.check()?;
        if !presented(&r) {
            return Err(presentation_fault());
        }
        if r.after_foreground.as_ref() == Some(&p.window) {
            r.state = if r.restore_api_accepted.is_none() {
                "already_foreground"
            } else {
                "foreground_observed"
            }
            .into();
            return Ok(());
        }
        let call = operation::native_call("SetForegroundWindow_once")?;
        r.api_accepted = Some(effect(scope.d, &mut attempted, |d| d.activate(&p.window))?);
        drop(call);
        observe_completion(scope.d, g, &mut r, input, &mut budget)
    })();
    if let Err(e) = result {
        failed(&mut r, e, attempted);
    }
    let observed = matches!(
        r.state.as_str(),
        "foreground_observed" | "already_foreground"
    );
    // End drains events while hooks still exist, releases native resources, and
    // returns the final cumulative batch. Late interference cannot be discarded.
    match scope.end() {
        Ok(events) => {
            r.resources_released = events.hooks_released;
            if let Err(e) = collect_events(&mut r, events) {
                failed(&mut r, e, attempted);
            }
        }
        Err(e) => failed(&mut r, e, attempted),
    }
    let final_check = sample(scope.d, g, &mut r, input, "activation_after_cleanup");
    if let Err(e) = final_check {
        failed(&mut r, e, attempted);
    } else if !r.user_intervened && r.error.is_none() {
        if !presented(&r) {
            failed(&mut r, presentation_fault(), attempted);
        } else if observed && r.after_foreground.as_ref() != Some(&r.window) {
            // Even returning to the old foreground after success is interference.
            yielded(&mut r);
        } else if r.after_foreground.as_ref() == Some(&r.window) {
            if budget.check().is_err() {
                failed(&mut r, timeout(), attempted);
            } else if r.api_accepted == Some(false) {
                // API refusal and independently observed foreground are separate
                // facts; completion may arrive during the final bounded drain.
                r.state = "foreground_observed".into();
            }
        }
    }
    r.resource_release_uncertain = cleanup.uncertain() || !r.resources_released;
    r.resources_released &= !r.resource_release_uncertain;
    if r.resource_release_uncertain {
        operation::resource_release_uncertain();
        r.state = "uncertain".into();
        if r.error.is_none() {
            r.error = Some(cleanup_fault());
        }
    }
    reservation.finish(&r)?;
    Ok(r)
}

#[cfg(test)]
#[path = "lifecycle_activation_tests.rs"]
mod tests;
