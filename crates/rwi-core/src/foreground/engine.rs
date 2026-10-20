//! Durable explicit foreground operation, separate from ordinary input.
use crate::*;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    time::Duration,
};

impl Engine {
    pub fn focus_window(
        &mut self,
        target: &str,
        lease: &str,
        reference: &str,
        request: &str,
        expected_foreground: &ForegroundState,
    ) -> Result<serde_json::Value> {
        expected_foreground.validate()?;
        if request.is_empty() || request.len() > 128 {
            return Err(fault("invalid_arguments", "Bound request_id length."));
        }
        let key = self.identity(target)?.key();
        let path = self.journal_path(&key, request);
        let fingerprint = digest(
            &serde_json::to_vec(&("focus_window", target, reference, expected_foreground)).unwrap(),
        );
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
                    "Do not reuse request_id for another foreground request.",
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
        if !resolved.placement_only
            || resolved.scope.element.is_some()
            || resolved.native_action.is_some()
        {
            return Err(fault(
                "window_reference_required",
                "Foreground changes require an explicit window-level reference, not a pixel/control.",
            ));
        }
        let snapshot = self.snapshot(target, &resolved.frame)?;
        if snapshot.at.elapsed() > Duration::from_secs(10) {
            return Err(fault(
                "stale_reference",
                "Reobserve; no foreground request sent.",
            ));
        }
        let before = snapshot.scene.binding.clone().ok_or_else(|| {
            fault(
                "target_binding_unknown",
                "Exact native window binding required.",
            )
        })?;
        let current = self.backend(target)?.revalidate(&resolved)?;
        let now = current
            .binding
            .as_ref()
            .ok_or_else(|| fault("target_binding_unknown", "Fresh native binding missing."))?;
        if current.identity != self.identity(target)?
            || before.scope != now.scope
            || before.layout != now.layout
            || before.native_epoch != now.native_epoch
            || before.focus_window != now.focus_window
            || before.mutation_context != now.mutation_context
            || current.blocked != Some(false)
            || !current.mutation_ready
        {
            return Err(fault(
                "stale_reference",
                "Window/session/context changed before explicit foreground; no request sent.",
            ));
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
            text_effect: None,
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
            self.backend(target)?
                .focus_window(&resolved, expected_foreground)
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
        // Explicit foreground changes invalidate all aliases on the same desktop.
        let aliases = self
            .targets
            .iter()
            .filter(|(_, t)| t.identity.key() == key)
            .map(|(n, _)| n.clone())
            .collect::<Vec<_>>();
        self.snapshots.retain(|(_, s)| !aliases.contains(&s.target));
        self.refs.retain(|r| !aliases.contains(&r.target));
        let grounding_invalidated = serde_json::json!({
            "scope": "same_native_desktop", "target_count": aliases.len(),
            "context": "current_executor", "next": "snapshot_then_resolve"
        });
        Ok(match placed {
            Some(value) => serde_json::json!({"receipt": receipt, "activation": value,
                "grounding_invalidated": grounding_invalidated}),
            None => serde_json::json!({"receipt": receipt,
                "grounding_invalidated": grounding_invalidated}),
        })
    }
}
