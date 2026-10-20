//! Durable global keyboard dispatch, separate from frame/element input.
use crate::*;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};

impl Engine {
    pub fn hotkey(
        &mut self,
        target: &str,
        lease: &str,
        request: &str,
        key_name: &str,
        expected: &ForegroundState,
    ) -> Result<serde_json::Value> {
        expected.validate()?;
        if !device::valid_id(request)
            || key_name.is_empty()
            || key_name.len() > 64
            || !key_name.is_ascii()
            || key_name.bytes().any(|b| b.is_ascii_control())
        {
            return Err(fault(
                "invalid_arguments",
                "Use a stable request_id and one bounded physical key/chord.",
            ));
        }
        let identity = self.identity(target)?;
        if self.targets[target].parent.is_some() {
            return Err(fault(
                "desktop_target_required",
                "Global input requires the original authorized desktop, not an attached window.",
            ));
        }
        let domain = identity.key();
        let path = self.journal_path(&domain, request);
        let fingerprint =
            digest(&serde_json::to_vec(&("hotkey", target, key_name, expected)).unwrap());
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
                    "Do not reuse request_id for a different global key/receiver.",
                ));
            }
            let mut receipt = records.next_back().unwrap_or(first).receipt;
            receipt.deduplicated = true;
            return Ok(serde_json::json!({"receipt":receipt,"deduplicated":true}));
        }
        if self.backend(target)?.capabilities()["foreground_keyboard"]["implemented"] != true {
            return Err(fault(
                "capability_unavailable",
                "This desktop has no authorized foreground keyboard route; no fallback.",
            ));
        }
        let book = self.leases.shared();
        let held = book
            .lock()
            .map_err(|_| fault("lease_error", "Restart observation session."))?;
        held.check(lease, target, &domain)?;
        operation::checkpoint("before_durable_reservation")?;
        let mut receipt = Receipt {
            request_id: request.into(),
            frame: format!("foreground:{}", expected.native_epoch),
            scope: Scope {
                pid: expected.pid,
                window: expected.window.clone(),
                element: None,
            },
            dispatch: "uncertain".into(),
            dispatch_error: None,
            system_accepted: None,
            text_effect: None,
            pointer_dispatch: None,
            goal_verified: false,
            replay: "forbidden".into(),
            deduplicated: false,
        };
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let write = |f: &mut File, r: &Receipt| -> Result<()> {
            f.write_all(
                &serde_json::to_vec(&Journal {
                    fingerprint: fingerprint.clone(),
                    receipt: r.clone(),
                })
                .unwrap(),
            )?;
            f.write_all(b"\n")?;
            f.sync_all()?;
            Ok(())
        };
        write(&mut file, &receipt)?;
        #[cfg(unix)]
        File::open(path.parent().unwrap())?.sync_all()?;
        let before = self.backend(target)?.input_dispatch_count();
        let mut keyboard = None;
        match operation::with_dispatch_deadline(held.deadline(lease, target, &domain)?, || {
            self.backend(target)?.hotkey(key_name, expected)
        }) {
            Ok(value) => {
                receipt.dispatch = "returned".into();
                keyboard = Some(value);
            }
            Err(error) => {
                let after = self.backend(target)?.input_dispatch_count();
                record_dispatch_failure(&mut receipt, error, before, after);
            }
        }
        write(&mut file, &receipt)?;
        drop(held);
        // A system shortcut may change app/window/focus; no alias keeps old input proposals.
        let aliases = self
            .targets
            .iter()
            .filter(|(_, t)| t.identity.key() == domain)
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        self.snapshots.retain(|(_, s)| !aliases.contains(&s.target));
        self.refs.retain(|r| !aliases.contains(&r.target));
        let grounding_invalidated = serde_json::json!({
            "scope": "same_native_desktop", "target_count": aliases.len(),
            "context": "current_executor", "next": "snapshot_then_resolve"
        });
        Ok(match keyboard {
            Some(value) => serde_json::json!({"receipt":receipt,"keyboard":value,
                "grounding_invalidated":grounding_invalidated}),
            None => serde_json::json!({"receipt":receipt,
                "grounding_invalidated":grounding_invalidated}),
        })
    }
}
