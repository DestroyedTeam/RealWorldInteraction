//! Exact engine mutation methods, separated by responsibility.
use super::*;

impl Engine {
    pub fn move_window(
        &mut self,
        target: &str,
        lease: &str,
        reference: &str,
        request: &str,
        display: &DisplayTarget,
    ) -> Result<serde_json::Value> {
        display.validate()?;
        if request.is_empty() || request.len() > 128 {
            return Err(fault("invalid_arguments", "Bound request_id length."));
        }
        let key = self.identity(target)?.key();
        let path = self.journal_path(&key, request);
        let fingerprint = digest(&serde_json::to_vec(&(target, reference, display)).unwrap());
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
                    "Interrupted reservation exists; do not replay.",
                )
            })?;
            if first.fingerprint != fingerprint {
                return Err(fault(
                    "idempotency_conflict",
                    "Do not reuse request_id for another move.",
                ));
            }
            let mut receipt = records.next_back().unwrap_or(first).receipt;
            receipt.deduplicated = true;
            return Ok(serde_json::json!({"receipt": receipt, "deduplicated": true}));
        }
        operation::checkpoint("before_revalidation")?;
        let resolved = self
            .refs
            .iter()
            .find(|r| r.reference == reference && r.target == target)
            .cloned()
            .ok_or_else(|| fault("unknown_reference", "Observe and resolve this target."))?;
        let snapshot = self.snapshot(target, &resolved.frame)?;
        if snapshot.at.elapsed() > Duration::from_secs(10) {
            return Err(fault("stale_reference", "Reobserve; no move sent."));
        }
        let book = self.leases.shared();
        let held = book
            .lock()
            .map_err(|_| fault("lease_error", "Restart observation session."))?;
        held.check(lease, target, &key)?;
        operation::checkpoint("before_durable_reservation")?;
        let mut receipt = Receipt {
            request_id: request.into(),
            frame: resolved.frame.clone(),
            scope: resolved.scope.clone(),
            dispatch: "uncertain".into(),
            dispatch_error: None,
            system_accepted: None,
            goal_verified: false,
            replay: "forbidden".into(),
            deduplicated: false,
        };
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let write = |f: &mut File, r: &Receipt| -> Result<()> {
            let data = serde_json::to_vec(&Journal {
                fingerprint: fingerprint.clone(),
                receipt: r.clone(),
            })
            .unwrap();
            f.write_all(&data)?;
            f.write_all(b"\n")?;
            f.sync_all()?;
            Ok(())
        };
        write(&mut file, &receipt)?;
        #[cfg(unix)]
        File::open(path.parent().unwrap())?.sync_all()?;
        let mut placed = None;
        let dispatch_before = self.backend(target)?.input_dispatch_count();
        match operation::with_dispatch_deadline(held.deadline(lease, target, &key)?, || {
            self.backend(target)?.move_window(&resolved, display)
        }) {
            Ok(value) => {
                receipt.dispatch = "returned".into();
                placed = Some(value);
            }
            Err(error) => {
                let dispatch_after = self.backend(target)?.input_dispatch_count();
                record_dispatch_failure(&mut receipt, error, dispatch_before, dispatch_after);
            }
        }
        write(&mut file, &receipt)?;
        drop(held);
        // A moved window invalidates frames and references on this desktop.
        let aliases = self
            .targets
            .iter()
            .filter(|(_, t)| t.identity.key() == key)
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>();
        self.snapshots.retain(|(_, s)| !aliases.contains(&s.target));
        self.refs.retain(|r| !aliases.contains(&r.target));
        Ok(match placed {
            Some(value) => serde_json::json!({"receipt": receipt, "placement": value}),
            None => serde_json::json!({"receipt": receipt}),
        })
    }

    pub fn act(
        &mut self,
        target: &str,
        reference: &str,
        lease: &str,
        request: &str,
        intent: Intent,
        value: Option<String>,
    ) -> Result<Receipt> {
        self.act_inner(target, reference, lease, request, intent, value, None)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn act_inner(
        &mut self,
        target: &str,
        reference: &str,
        lease: &str,
        request: &str,
        intent: Intent,
        value: Option<String>,
        checked_fingerprint: Option<String>,
    ) -> Result<Receipt> {
        if request.is_empty()
            || request.len() > 128
            || value.as_ref().is_some_and(|v| v.len() > 16384)
        {
            return Err(fault(
                "invalid_arguments",
                "Bound request_id and value lengths.",
            ));
        }
        intent.validate(value.as_deref())?;
        let key = self.identity(target)?.key();
        let path = self.journal_path(&key, request);
        let fingerprint = checked_fingerprint.unwrap_or_else(|| {
            digest(&serde_json::to_vec(&(target, reference, &intent, &value)).unwrap())
        });
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
                    "Interrupted reservation exists; do not replay.",
                )
            })?;
            if first.fingerprint != fingerprint {
                return Err(fault(
                    "idempotency_conflict",
                    "Do not reuse request_id for another action or reference.",
                ));
            }
            let mut result = records.next_back().unwrap_or(first).receipt;
            result.deduplicated = true;
            return Ok(result);
        }
        operation::checkpoint("before_revalidation")?;
        if matches!(intent, Intent::MovePointer) {
            MovePointerOptions::parse(value.as_deref())?
                .require_capability(&self.backend(target)?.capabilities())?;
        }
        let mut resolved = self
            .refs
            .iter()
            .find(|r| r.reference == reference && r.target == target)
            .cloned()
            .ok_or_else(|| fault("unknown_reference", "Observe and resolve this target."))?;
        if let Some(action) = &resolved.native_action
            && (!matches!(intent, Intent::Click) || value.as_ref() != Some(action))
        {
            return Err(fault(
                "native_action_mismatch",
                "Native-action references cannot dispatch pointer, text, or a different action.",
            ));
        }
        if resolved.placement_only {
            return Err(fault(
                "invalid_reference_class",
                "Window-level references are for placement only; resolve an element or pixel for input.",
            ));
        }
        let snapshot = self.snapshot(target, &resolved.frame)?;
        if snapshot.at.elapsed() > Duration::from_secs(INPUT_PROPOSAL_MAX_AGE_SECS) {
            return Err(fault(
                "stale_reference",
                "Input proposal exceeded 120 seconds; reobserve. No input sent.",
            ));
        }
        let before = snapshot.scene.clone();
        let current = self.backend(target)?.revalidate(&resolved)?;
        if current.identity != self.identity(target)? {
            return Err(fault(
                "session_mismatch",
                "Native revalidation belongs to another session.",
            ));
        }
        if !current.mutation_ready || !before.mutation_ready {
            return Err(fault(
                "capability_unavailable",
                "Native input binding is not ready.",
            ));
        }
        let (grounded, local_pixel, red_marker) = before
            .binding
            .as_ref()
            .map(|binding| evaluate_grounding(binding, &current, &resolved))
            .unwrap_or((false, None, None));
        if !grounded {
            let reason = local_pixel
                .as_ref()
                .and_then(|d| d["reason"].as_str())
                .or_else(|| red_marker.as_ref().and_then(|d| d["reason"].as_str()));
            let message = reason.map_or_else(
                || {
                    let full_frame_changed_without_anchor = resolved.scope.element.is_none()
                        && resolved.pixel_anchor.is_none()
                        && resolved.red_marker_anchor.is_none()
                        && before.binding.as_ref().zip(current.binding.as_ref())
                            .is_some_and(|(a, b)| a.content != b.content);
                    if full_frame_changed_without_anchor {
                        if let Some(reason) = resolved.pixel_anchor_creation_reason {
                            format!("Pixel reference has no local anchor ({reason}); full-frame content changed. Inspect the source snapshot or a stable textured click surface; no input sent, never replay.")
                        } else {
                            "Pixel reference has no local anchor; full-frame content changed. Inspect a fresh stable textured surface; no input sent.".to_owned()
                        }
                    } else {
                        "Reference grounding refused; inspect fresh binding/element state. No input sent.".to_owned()
                    }
                },
                |reason| format!("Local pixel anchor refused: {reason}. Inspect a fresh stable click surface; no input sent, never replay."),
            );
            let mut error = fault("stale_reference", &message);
            // Same-scene metrics are reused, not recomputed from the PNG; this
            // does not renew authority or skip any later native input check.
            let mut diagnostic = grounding_diagnostic(&before, &current, &resolved);
            if let Some(reason) = resolved.pixel_anchor_creation_reason {
                diagnostic["local_anchor_creation_reason"] = serde_json::json!(reason);
            }
            if let Some(local) = local_pixel {
                diagnostic["local_pixel_anchor"] = local;
            }
            if let Some(marker) = red_marker {
                diagnostic["red_marker_cluster_anchor"] = marker;
            }
            error.diagnostic = Some(Box::new(diagnostic));
            return Err(error);
        }
        resolved.pixel_anchor_used = (resolved.pixel_anchor.is_some()
            || resolved.red_marker_anchor.is_some())
            && before.binding.as_ref().map(|binding| &binding.content)
                != current.binding.as_ref().map(|binding| &binding.content);
        if current.blocked != Some(false) {
            return Err(fault("target_blocked", "Inspect modal/occlusion state."));
        }
        let book = self.leases.shared();
        let held = book
            .lock()
            .map_err(|_| fault("lease_error", "Restart observation session."))?;
        held.check(lease, target, &key)?;
        operation::checkpoint("before_durable_reservation")?;
        let mut receipt = Receipt {
            request_id: request.into(),
            frame: resolved.frame.clone(),
            scope: resolved.scope.clone(),
            dispatch: "uncertain".into(),
            dispatch_error: None,
            system_accepted: None,
            goal_verified: false,
            replay: "forbidden".into(),
            deduplicated: false,
        };
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let write = |f: &mut File, r: &Receipt| -> Result<()> {
            let data = serde_json::to_vec(&Journal {
                fingerprint: fingerprint.clone(),
                receipt: r.clone(),
            })
            .unwrap();
            f.write_all(&data)?;
            f.write_all(b"\n")?;
            f.sync_all()?;
            Ok(())
        };
        write(&mut file, &receipt)?;
        // Persist the directory entry before dispatch as well as the reservation bytes.
        #[cfg(unix)]
        File::open(path.parent().unwrap())?.sync_all()?;
        let expected = before
            .binding
            .as_ref()
            .ok_or_else(|| fault("target_binding_unknown", "Native binding required."))?;
        let dispatch_before = self.backend(target)?.input_dispatch_count();
        match operation::with_dispatch_deadline(held.deadline(lease, target, &key)?, || {
            self.backend(target)?
                .act(expected, &resolved, &intent, value.as_deref())
        }) {
            Ok(accepted) => {
                receipt.dispatch = "returned".into();
                receipt.system_accepted = accepted;
            }
            Err(error) => {
                let dispatch_after = self.backend(target)?.input_dispatch_count();
                record_dispatch_failure(&mut receipt, error, dispatch_before, dispatch_after);
            }
        }
        write(&mut file, &receipt)?;
        drop(held);
        let aliases = self
            .targets
            .iter()
            .filter(|(_, t)| t.identity.key() == key)
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        self.refs.retain(|r| !aliases.contains(&r.target));
        self.log_operation(&receipt, target, &intent, &resolved);
        Ok(receipt)
    }
}
