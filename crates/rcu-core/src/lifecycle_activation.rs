//! Explicit once-only activation; native foreground is independently observed.
use super::*;

pub struct ActivateRequest<'a> {
    pub authorization_id: &'a str,
    pub request_id: &'a str,
    pub expected_foreground: &'a Option<WindowIdentity>,
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
    scope.d.check_window(&p.window)?;
    let input = scope.d.user_input_tick()?;
    let before = scope.d.foreground()?;
    if &before != req.expected_foreground {
        return Err(fault(
            "foreground_changed",
            "Yield to user/other app; obtain a fresh observation rather than stealing focus.",
        ));
    }
    let mut r = ActivationReceipt {
        request_id: req.request_id.into(),
        authorization_id: req.authorization_id.into(),
        window: p.window.clone(),
        state: "uncertain".into(),
        api_accepted: None,
        before_foreground: before,
        after_foreground: None,
        user_intervened: false,
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
    let mut attempted = false;
    let result = (|| -> Result<()> {
        check(scope.d, g, "activation_before_dispatch")?;
        scope.d.check_window(&p.window)?;
        if scope.d.user_input_tick()? != input || scope.d.foreground()? != *req.expected_foreground
        {
            r.user_intervened = true;
            r.state = "yielded".into();
            return Ok(());
        }
        if req.expected_foreground.as_ref() == Some(&p.window) {
            r.after_foreground = req.expected_foreground.clone();
            r.state = "already_foreground".into();
            return Ok(());
        }
        ensure_cleanup_clean(&cleanup)?;
        let call = operation::native_call("SetForegroundWindow_once")?;
        attempted = true;
        r.api_accepted = Some(scope.d.activate(&p.window)?);
        drop(call);
        r.after_foreground = scope.d.foreground()?;
        if scope.d.user_input_tick()? != input {
            r.user_intervened = true;
            r.state = "yielded".into();
            return Ok(());
        }
        check(scope.d, g, "activation_verify")?;
        scope.d.check_window(&p.window)?;
        r.state = if r.after_foreground.as_ref() == Some(&p.window) {
            "foreground_observed"
        } else {
            "not_foreground"
        }
        .into();
        Ok(())
    })();
    if let Err(e) = result {
        r.state = if attempted {
            "uncertain"
        } else {
            "not_dispatched"
        }
        .into();
        r.error = Some(e);
    }
    match scope.end() {
        Ok(events) => r.resources_released = events.hooks_released,
        Err(e) => r.error = Some(e),
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
