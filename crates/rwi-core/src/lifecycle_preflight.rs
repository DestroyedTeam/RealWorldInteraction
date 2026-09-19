use super::*;

fn bounded_inventory(mut inventory: Inventory) -> Inventory {
    let mut truncated = inventory.windows.len() > MAX_WINDOWS || inventory.notes.len() > 32;
    inventory.windows.truncate(MAX_WINDOWS);
    inventory.notes.truncate(32);
    while serde_json::to_vec(&inventory)
        .expect("inventory serialization")
        .len()
        > 19_000
    {
        truncated = true;
        if inventory.windows.pop().is_none() && inventory.notes.pop().is_none() {
            break;
        }
    }
    if truncated {
        inventory.complete = false;
        inventory.notes.push("Diagnostic metadata truncated to 20 KiB; native_windows_enumerated is preserved, and partial coverage is never absence.".into());
    }
    inventory
}
fn bounded_events(mut events: EventBatch) -> EventBatch {
    if events.events.len() > MAX_EVENTS {
        events.events.truncate(MAX_EVENTS);
        events.complete = false;
    }
    while serde_json::to_vec(&events)
        .expect("event serialization")
        .len()
        > 19_000
    {
        events.complete = false;
        if events.events.pop().is_none() {
            break;
        }
    }
    events
}
/// Inspect exact launch preconditions without reserving or dispatching any action.
/// Its ready flag is a bounded observation, not a future launch/input authorization.
pub fn preflight(
    g: &LifecycleGrant,
    d: &mut dyn LifecycleDriver,
    authorization: &str,
) -> Result<PreflightReceipt> {
    let p = g
        .launches
        .iter()
        .find(|p| p.authorization_id == authorization)
        .ok_or_else(|| {
            fault(
                "unknown_lifecycle_permission",
                "Select a launch authorization from this connection's lifecycle grant.",
            )
        })?;
    let cleanup = operation::CleanupAudit::begin();
    let mut scope = EventScope { d, finished: false };
    let mut r = PreflightReceipt {
        authorization_id: authorization.into(),
        ready_for_launch: false,
        inventory: None,
        native_events: EventBatch::default(),
        error: None,
        resources_released: false,
        resource_release_uncertain: false,
        reservation_created: false,
        launch_dispatched: false,
        activation_dispatched: false,
        input_dispatched: false,
        goal_verified: false,
    };
    let result = (|| -> Result<()> {
        check(scope.d, g, "lifecycle_readonly_preflight")?;
        let apps = verify_permission_files(scope.d, p, "readonly_preflight")?;
        scope.d.begin_events()?;
        let inventory = scope.d.inventory(&apps)?;
        let validity = inventory_ok(&inventory, &apps, g);
        r.inventory = Some(bounded_inventory(inventory));
        validity?;
        let inventory = r.inventory.as_ref().expect("stored inventory");
        if !inventory.complete {
            return Err(fault(
                "incomplete_window_inventory",
                "Metadata truncation prevents complete window coverage.",
            ));
        }
        if inventory
            .windows
            .iter()
            .any(|w| !p.allowed_existing_windows.contains(&w.key))
        {
            return Err(fault(
                "existing_window_not_authorized",
                "Preflight found existing windows outside explicit reuse scope. Inspect the bounded inventory and request exact owner allocation; no launch or activation occurred.",
            ));
        }
        let events = scope.d.events()?;
        let validity = event_ok(&events);
        r.native_events = bounded_events(events);
        validity?;
        check(scope.d, g, "lifecycle_readonly_preflight_complete")?;
        ensure_cleanup_clean(&cleanup)?;
        Ok(())
    })();
    r.error = result.err();
    match scope.end() {
        Ok(events) => {
            r.resources_released = events.hooks_released;
            if r.error.is_none() {
                r.error = event_ok(&events).err();
            }
            r.native_events = bounded_events(events);
        }
        Err(e) => {
            if r.error.is_none() {
                r.error = Some(e);
            }
        }
    }
    r.resource_release_uncertain = cleanup.uncertain() || !r.resources_released;
    r.resources_released &= !r.resource_release_uncertain;
    if r.resource_release_uncertain {
        operation::resource_release_uncertain();
        if r.error.is_none() {
            r.error = Some(cleanup_fault());
        }
    }
    r.ready_for_launch = r.error.is_none()
        && r.resources_released
        && r.inventory.as_ref().is_some_and(|i| i.complete)
        && r.native_events.complete;
    Ok(r)
}
