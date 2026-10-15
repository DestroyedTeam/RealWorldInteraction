//! Stage B1: executable guarded Core semantic-focus route.
//! No platform setter or MCP catalog registration is implemented by this module.
use crate::semantic_focus::{
    FocusCustody, FocusElementRequest, FocusPolicyContext, FocusSetterAttempt, FocusWritePlan,
    JournalCustody, LeaseCustody, assess_focus_element, summarize_focus_readback,
};
use crate::*;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    time::Duration,
};

/// Extends the standard durable record without removing its receipt field.
/// Legacy receipt queries can still deserialize the standard Journal prefix.
#[derive(Serialize, Deserialize)]
struct FocusJournal {
    fingerprint: String,
    receipt: Receipt,
    #[serde(default)]
    focus_readback: Option<serde_json::Value>,
    #[serde(default)]
    grounding_invalidated: serde_json::Value,
}
impl Engine {
    /// Explicit semantic focus. Does not reinterpret click.action, text value,
    /// raise/activate a window, or call keyboard preparation. Real backends must
    /// opt in; the neutral Backend default remains unsupported.
    pub fn focus_element(
        &mut self,
        request: &FocusElementRequest,
        lease: &str,
    ) -> Result<serde_json::Value> {
        request.validate()?;
        let key = self.identity(&request.target)?.key();
        let path = self.journal_path(&key, &request.request_id);
        let fingerprint = digest(&serde_json::to_vec(&("focus_element_v1", request)).unwrap());
        if path.exists() {
            let mut text = String::new();
            File::open(&path)?.take(65536).read_to_string(&mut text)?;
            let mut rows = text
                .split_inclusive('\n')
                .filter(|s| s.ends_with('\n'))
                .filter_map(|s| serde_json::from_str::<FocusJournal>(s).ok());
            let first = rows.next().ok_or_else(|| {
                fault(
                    "uncertain_journal",
                    "Incomplete focus reservation exists; never replay.",
                )
            })?;
            if first.fingerprint != fingerprint {
                return Err(fault(
                    "idempotency_conflict",
                    "Do not reuse a focus request ID for another frame, element or target.",
                ));
            }
            let mut last = rows.next_back().unwrap_or(first);
            last.receipt.deduplicated = true;
            return Ok(
                serde_json::json!({"receipt":last.receipt,"focus_readback":last.focus_readback,
                "grounding_invalidated":last.grounding_invalidated}),
            );
        }
        operation::checkpoint("focus_element_preconditions")?;
        let capability = self.backend(&request.target)?.capabilities();
        if capability["focus_element"]["implemented"] != true
            || capability["focus_element"]["available"] != true
        {
            return Err(fault(
                "capability_unavailable",
                "Explicit semantic element focus is not supported by this selected backend; no fallback.",
            ));
        }
        let book = self.leases.shared();
        let held = book
            .lock()
            .map_err(|_| fault("lease_error", "Inspect cooperative lease state; no setter."))?;
        held.check(lease, &request.target, &key)?;
        let snapshot = self.snapshot(&request.target, &request.frame)?;
        let age = snapshot.at.elapsed();
        if age > Duration::from_secs(INPUT_PROPOSAL_MAX_AGE_SECS) {
            return Err(fault(
                "stale_reference",
                "Focus proposal expired; no setter or automatic reobservation.",
            ));
        }
        let original = snapshot.scene.clone();
        let resolved = self.resolve(
            &request.target,
            &request.frame,
            Locator::Element {
                id: request.element.clone(),
            },
        )?;
        let fresh = self.backend(&request.target)?.revalidate(&resolved)?;
        if fresh.identity != self.identity(&request.target)? || original.identity != fresh.identity
        {
            return Err(fault(
                "session_mismatch",
                "Focus observation belongs to another native context; no setter.",
            ));
        }
        // Read-only native preflight returns trusted typed facts, not model JSON.
        let native = operation::with_dispatch_deadline(
            held.deadline(lease, &request.target, &key)?,
            || {
                self.backend(&request.target)?
                    .focus_element_preflight(&resolved)
            },
        )?;
        held.check(lease, &request.target, &key)?;
        operation::checkpoint("before_focus_durable_reservation")?;
        let mut receipt = Receipt {
            request_id: request.request_id.clone(),
            frame: request.frame.clone(),
            scope: resolved.scope.clone(),
            dispatch: "uncertain".into(),
            dispatch_error: None,
            system_accepted: None,
            text_effect: None,
            goal_verified: false,
            replay: "forbidden".into(),
            deduplicated: false,
        };
        let mut readback = None;
        let mut invalidated = serde_json::json!({"scope":"none","target_count":0});
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let write = |f: &mut File,
                     receipt: &Receipt,
                     focus: &Option<serde_json::Value>,
                     invalidated: &serde_json::Value|
         -> Result<()> {
            let raw = serde_json::to_vec(&FocusJournal {
                fingerprint: fingerprint.clone(),
                receipt: receipt.clone(),
                focus_readback: focus.clone(),
                grounding_invalidated: invalidated.clone(),
            })
            .unwrap();
            f.write_all(&raw)?;
            f.write_all(b"\n")?;
            f.sync_all()?;
            Ok(())
        };
        write(&mut file, &receipt, &readback, &invalidated)?;
        #[cfg(unix)]
        File::open(path.parent().unwrap())?.sync_all()?;
        let plan = assess_focus_element(
            FocusPolicyContext {
                request,
                reference: &resolved,
                original: &original,
                fresh: &fresh,
                proposal_age_ms: u64::try_from(age.as_millis()).unwrap_or(u64::MAX),
            },
            &native,
            FocusCustody {
                lease: LeaseCustody::HeldExactDomainTarget,
                journal: JournalCustody::NewExactReservation,
            },
        );
        match plan {
            Err(error) => {
                // Policy failed in Core before any setter method invocation.
                receipt.dispatch = "rejected_before_dispatch".into();
                receipt.dispatch_error = Some(error);
            }
            Ok(plan) => {
                operation::checkpoint("before_focus_setter")?;
                let before = self.backend(&request.target)?.input_dispatch_count();
                let binding = original.binding.as_ref().ok_or_else(|| {
                    fault(
                        "target_binding_unknown",
                        "Exact original focus binding required.",
                    )
                })?;
                match operation::with_dispatch_deadline(
                    held.deadline(lease, &request.target, &key)?,
                    || {
                        self.backend(&request.target)?
                            .focus_element(binding, &resolved, plan)
                    },
                ) {
                    Ok(raw) => {
                        // A native backend may reduce a one-write plan to a fresh
                        // verified no-op; it may never escalate a zero-write plan.
                        let over_budget = plan == FocusWritePlan::NoWriteAlreadyFocused
                            && (raw.write_plan != FocusWritePlan::NoWriteAlreadyFocused
                                || raw.setter_attempt != FocusSetterAttempt::NotAttempted);
                        let mut summary = summarize_focus_readback(raw);
                        // An independently ready local state never overrides a
                        // violated zero-write budget. Preserve the raw axes while
                        // denying completion in both receipt and compact summary.
                        if over_budget {
                            summary.state = crate::semantic_focus::FocusReadbackState::Unknown;
                            summary.goal_verified = false;
                        }
                        receipt.system_accepted = summary.setter_acknowledged;
                        receipt.goal_verified = summary.goal_verified;
                        let uncertain_setter = summary.setter_attempt
                            == FocusSetterAttempt::Attempted
                            && summary.setter_acknowledged != Some(true);
                        let mut value = serde_json::to_value(&summary).unwrap();
                        value["requested_write_plan"] = serde_json::to_value(plan).unwrap();
                        if over_budget || uncertain_setter {
                            let mut error = fault(
                                "focus_setter_uncertain",
                                "Setter acknowledgement/budget is uncertain; preserve readback, inspect only, never replay or add another focus/typing action.",
                            );
                            error.diagnostic = Some(Box::new(value.clone()));
                            receipt.dispatch_error = Some(error);
                        } else {
                            receipt.dispatch = "returned".into();
                        }
                        readback = Some(value);
                    }
                    Err(error) => {
                        let after = self.backend(&request.target)?.input_dispatch_count();
                        record_dispatch_failure(&mut receipt, error, before, after);
                    }
                }
                // May change this application's firstResponder/window even when
                // OS PID does not change. Retire cached same-app authority; do
                // not enumerate, activate, capture or detach unrelated windows.
                if receipt.dispatch != "rejected_before_dispatch" {
                    let pid = resolved.scope.pid;
                    let aliases = self
                        .targets
                        .iter()
                        .filter(|(name, t)| {
                            t.identity.key() == key
                                && (**name == request.target
                                    || self.snapshots.iter().any(|(_, s)| {
                                        s.target == **name
                                            && s.scene
                                                .binding
                                                .as_ref()
                                                .is_some_and(|b| b.scope.pid == pid)
                                    }))
                        })
                        .map(|(name, _)| name.clone())
                        .collect::<Vec<_>>();
                    self.snapshots.retain(|(_, s)| !aliases.contains(&s.target));
                    self.refs.retain(|r| !aliases.contains(&r.target));
                    invalidated = serde_json::json!({"scope":"same_app_cached_aliases","target_count":aliases.len(),
                        "pid":pid,"context":"current_executor","next":"snapshot_then_resolve; no automatic typing"});
                }
            }
        }
        write(&mut file, &receipt, &readback, &invalidated)?;
        drop(held);
        Ok(
            serde_json::json!({"receipt":receipt,"focus_readback":readback,"grounding_invalidated":invalidated}),
        )
    }
}

#[cfg(test)]
mod tests;
