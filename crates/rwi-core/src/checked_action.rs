//! One service-side checked semantic action. No coordinate TTL extension, app
//! focus, retry, regrant or OS-atomicity claim. The original frame supplies exact
//! optimistic preconditions; native reads must prove they still hold.
use super::*;

pub struct CheckedAction<'a> {
    pub target: &'a str,
    pub frame: &'a str,
    pub element: &'a str,
    pub lease: &'a str,
    pub request: &'a str,
    pub intent: Intent,
    pub value: Option<String>,
}

impl Engine {
    pub fn act_checked(&mut self, action: CheckedAction<'_>) -> Result<Receipt> {
        let CheckedAction {
            target,
            frame,
            element,
            lease,
            request,
            intent,
            value,
        } = action;
        if request.is_empty()
            || request.len() > 128
            || element.is_empty()
            || element.len() > 512
            || value.as_ref().is_some_and(|v| v.len() > 16384)
        {
            return Err(fault(
                "invalid_arguments",
                "Bound the exact element, request ID and value.",
            ));
        }
        intent.validate(value.as_deref())?;
        let key = self.identity(target)?.key();
        let fingerprint = digest(
            &serde_json::to_vec(&("act_checked_v1", target, frame, element, &intent, &value))
                .unwrap(),
        );
        let path = self.journal_path(&key, request);
        // Check the logical request before doing new observation/resolution. Its
        // fingerprint never depends on a newly generated internal reference ID.
        if path.exists() {
            let mut text = String::new();
            File::open(&path)?.take(65536).read_to_string(&mut text)?;
            let mut records = text
                .split_inclusive('\n')
                .filter(|s| s.ends_with('\n'))
                .filter_map(|s| serde_json::from_str::<Journal>(s).ok());
            let first = records.next().ok_or_else(|| {
                fault(
                    "uncertain_journal",
                    "Interrupted reservation exists; never replay.",
                )
            })?;
            if first.fingerprint != fingerprint {
                return Err(fault(
                    "idempotency_conflict",
                    "Do not reuse request_id with different checked preconditions.",
                ));
            }
            let mut result = records.next_back().unwrap_or(first).receipt;
            result.deduplicated = true;
            return Ok(result);
        }
        operation::checkpoint("checked_action_preconditions")?;
        if matches!(intent, Intent::Click)
            && let Some(options) = ClickDeliveryOptions::from_click_value(value.as_deref())?
        {
            options.require_capability(&self.backend(target)?.capabilities())?;
        }
        self.leases
            .shared()
            .lock()
            .map_err(|_| fault("lease_error", "Restart observation session."))?
            .check(lease, target, &key)?;
        let element_policy =
            self.backend(target)?.capabilities()["checked_context_policy"] == "native_element";
        let snapshot = self.snapshot(target, frame)?;
        // Semantic and pixel proposals share a model-friendly bounded horizon.
        // Their distinct native revalidation policies remain unchanged.
        if snapshot.at.elapsed() > Duration::from_secs(INPUT_PROPOSAL_MAX_AGE_SECS) {
            return Err(fault(
                "checked_proposal_expired",
                "Observe anew; do not retry an uncertain action.",
            ));
        }
        let before = snapshot.scene.clone();
        let expected = before
            .binding
            .as_ref()
            .ok_or_else(|| fault("target_binding_unknown", "Exact native binding required."))?;
        if (!element_policy
            && expected
                .mutation_context
                .as_ref()
                .is_none_or(|c| c.document_state.is_none()))
            || !before.mutation_ready
            || before.blocked != Some(false)
        {
            return Err(fault(
                "checked_context_unavailable",
                if element_policy {
                    "The observed native window must be input-ready and unblocked; inspect native foreground/element evidence. No legacy/pixel fallback."
                } else {
                    "A complete bounded document context and ready native binding are required; no legacy/pixel fallback."
                },
            ));
        }
        let native_action = match (
            &intent,
            ClickDeliveryOptions::native_action_value(value.as_deref()),
        ) {
            (Intent::Click, Some(action)) => {
                let caps = self.backend(target)?.capabilities();
                if action.is_empty()
                    || !(caps["native_semantic_action_policy"] == "advertised_element_action"
                        || caps["native_semantic_actions"]
                            .as_array()
                            .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(action))))
                {
                    return Err(fault(
                        "native_action_unavailable",
                        "The selected backend does not support the explicitly requested native action. No pointer fallback or input was dispatched.",
                    ));
                }
                // resolve_action checks the exact retained element's advertised
                // actions; native revalidation preserves provider/window identity.
                Some(action)
            }
            _ => None,
        };
        let old = self.resolve_action(
            target,
            frame,
            Locator::Element { id: element.into() },
            native_action,
        )?;
        let mut current = self.backend(target)?.revalidate(&old)?;
        if current.identity != self.identity(target)? || current.identity != before.identity {
            return Err(fault(
                "session_mismatch",
                "Checked revalidation belongs to another native session.",
            ));
        }
        if !current.mutation_ready
            || current.blocked != Some(false)
            || current.binding.as_ref().is_none_or(|b| {
                !element_policy
                    && b.mutation_context
                        .as_ref()
                        .is_none_or(|c| c.document_state.is_none())
            })
            || !grounding_matches(expected, &current, &old)
        {
            let mut error = fault(
                "checked_precondition_changed",
                "Document/focus/layout/element context changed or became unknown; no input, no automatic reobserve loop.",
            );
            error.diagnostic = Some(Box::new(grounding_diagnostic(&before, &current, &old)));
            return Err(error);
        }
        operation::checkpoint("checked_action_fresh_resolution")?;
        if serde_json::to_vec(&current)
            .map_err(|_| fault("invalid_scene", "Inspect adapter metadata."))?
            .len()
            > 256 * 1024
        {
            return Err(fault(
                "observation_budget",
                "Checked native scene exceeds the bounded metadata budget.",
            ));
        }
        current.png.clear();
        let fresh = id();
        self.snapshots.push_back((
            fresh.clone(),
            Snapshot {
                target: target.into(),
                at: Instant::now(),
                scene: current,
                artifact_id: None,
                source_region: old.source_region,
            },
        ));
        while self.snapshots.len() > SNAPSHOT_LIMIT {
            self.snapshots.pop_front();
        }
        let resolved = self.resolve_action(
            target,
            &fresh,
            Locator::Element { id: element.into() },
            native_action,
        )?;
        // The unchanged dispatch path still revalidates, holds the lease, writes
        // its durable no-replay reservation and invokes exactly one native act.
        self.act_inner(
            target,
            &resolved.reference,
            lease,
            request,
            intent,
            value,
            Some(fingerprint),
        )
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod native_action_tests;
