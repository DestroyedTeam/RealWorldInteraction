//! Explicit whole-value replacement; durable once reservation, never keyboard fallback.
use crate::semantic_focus::{FocusCustody, FocusPolicyContext, JournalCustody, LeaseCustody};
use crate::set_text::{SetTextRequest, assess_set_text, summarize_set_text_readback};
use crate::*;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    time::Duration,
};

/// Extends the standard durable record without removing its receipt field.
/// Legacy receipt queries can still deserialize the standard Journal prefix.
#[derive(Serialize, Deserialize)]
struct SetTextJournal {
    fingerprint: String,
    receipt: Receipt,
    #[serde(default)]
    value_readback: Option<serde_json::Value>,
    #[serde(default)]
    grounding_invalidated: serde_json::Value,
}
impl Engine {
    /// Explicit whole plain-text AXValue replacement. Never raises/activates a
    /// window, invokes focus, or calls keyboard preparation. Real backends must
    /// opt in; the neutral Backend default remains unsupported.
    pub fn set_text(&mut self, request: &SetTextRequest, lease: &str) -> Result<serde_json::Value> {
        request.validate()?;
        let key = self.identity(&request.target)?.key();
        let path = self.journal_path(&key, &request.request_id);
        let fingerprint = digest(&serde_json::to_vec(&("set_text_v1", request)).unwrap());
        if path.exists() {
            let mut text = String::new();
            File::open(&path)?.take(65536).read_to_string(&mut text)?;
            let mut rows = text
                .split_inclusive('\n')
                .filter(|s| s.ends_with('\n'))
                .filter_map(|s| serde_json::from_str::<SetTextJournal>(s).ok());
            let first = rows.next().ok_or_else(|| {
                fault(
                    "uncertain_journal",
                    "Incomplete set-text reservation exists; never replay.",
                )
            })?;
            if first.fingerprint != fingerprint {
                return Err(fault(
                    "idempotency_conflict",
                    "Do not reuse a set-text request ID for another frame, element or target.",
                ));
            }
            let mut last = rows.next_back().unwrap_or(first);
            last.receipt.deduplicated = true;
            return Ok(
                serde_json::json!({"receipt":last.receipt,"value_readback":last.value_readback,
                "grounding_invalidated":last.grounding_invalidated}),
            );
        }
        operation::checkpoint("set_text_preconditions")?;
        let capability = self.backend(&request.target)?.capabilities();
        if capability["set_text"]["implemented"] != true
            || capability["set_text"]["available"] != true
        {
            return Err(fault(
                "capability_unavailable",
                "Explicit whole-text replacement is not supported by this selected backend; no fallback.",
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
                "Set-text proposal expired; no setter or automatic reobservation.",
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
                "Set-text observation belongs to another native context; no setter.",
            ));
        }
        // Read-only native preflight returns trusted typed facts, not model JSON.
        let native = operation::with_dispatch_deadline(
            held.deadline(lease, &request.target, &key)?,
            || self.backend(&request.target)?.set_text_preflight(&resolved),
        )?;
        held.check(lease, &request.target, &key)?;
        operation::checkpoint("before_set_text_durable_reservation")?;
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
                     value: &Option<serde_json::Value>,
                     invalidated: &serde_json::Value|
         -> Result<()> {
            let raw = serde_json::to_vec(&SetTextJournal {
                fingerprint: fingerprint.clone(),
                receipt: receipt.clone(),
                value_readback: value.clone(),
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
        let focus_identity = request.focus_identity();
        let plan = assess_set_text(
            FocusPolicyContext {
                request: &focus_identity,
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
            Ok(()) => {
                operation::checkpoint("before_set_text_setter")?;
                let before = self.backend(&request.target)?.input_dispatch_count();
                let binding = original.binding.as_ref().ok_or_else(|| {
                    fault(
                        "target_binding_unknown",
                        "Exact original set-text binding required.",
                    )
                })?;
                match operation::with_dispatch_deadline(
                    held.deadline(lease, &request.target, &key)?,
                    || {
                        self.backend(&request.target)?
                            .set_text(binding, &resolved, &request.text)
                    },
                ) {
                    Ok(raw) => {
                        let value = summarize_set_text_readback(raw);
                        receipt.system_accepted =
                            value["setter_status"].as_i64().map(|status| status == 0);
                        // This is exact AX readback, never independent DOM/business verification.
                        receipt.goal_verified = false;
                        if value["setter_attempts"] != 1 || receipt.system_accepted != Some(true) {
                            let mut error = fault(
                                "set_text_setter_uncertain",
                                "Preserve original operation and readback; no replay or keyboard fallback.",
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
                // Text replacement may alter app content/revisions even when
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
            serde_json::json!({"receipt":receipt,"value_readback":readback,"grounding_invalidated":invalidated}),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::fixture::Fixture;
    use crate::semantic_focus::*;
    use crate::set_text::*;
    use crate::*;
    use serde_json::{Value, json};
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    struct Mock {
        fixture: Fixture,
        count: Arc<AtomicU64>,
        ordinary: bool,
        error_after: bool,
    }
    impl Backend for Mock {
        fn identity(&self) -> Identity {
            self.fixture.identity()
        }
        fn capabilities(&self) -> Value {
            json!({"set_text":{"implemented":true,"available":true}})
        }
        fn input_dispatch_count(&self) -> Option<u64> {
            Some(self.count.load(Ordering::SeqCst))
        }
        fn observe(&mut self) -> Result<Scene> {
            let mut scene = self.fixture.observe()?;
            let e = scene.elements.iter_mut().find(|e| e.id == "Input").unwrap();
            e.role = Some("AXTextField".into());
            e.enabled = Some(true);
            Ok(scene)
        }
        fn set_text_preflight(&mut self, _: &Resolved) -> Result<SetTextNativeFacts> {
            let yes = FocusBooleanRead {
                native_status: 0,
                value: Some(true),
            };
            Ok(SetTextNativeFacts {
                ordinary_string_value: self.ordinary,
                exact: FocusNativeFacts {
                    platform: FocusPlatform::Macos,
                    application_focus_custody: ApplicationFocusCustody::NoActiveForeignSibling,
                    input_principal: ExactEvidence::Verified,
                    accessibility_permission: ExactEvidence::Verified,
                    retained_element: ExactEvidence::Verified,
                    element_window_cf: ExactEvidence::Verified,
                    process_lifetime: ExactEvidence::Verified,
                    cg_window_binding: ExactEvidence::Verified,
                    native_context: ExactEvidence::Verified,
                    editable_class: EditableClass::PlainEditable,
                    probe_origin: ProbeOrigin::FreshExactObject,
                    value_writable: yes,
                    focused_writable: yes,
                    focused: yes,
                },
            })
        }
        fn set_text(&mut self, _: &Binding, _: &Resolved, text: &str) -> Result<SetTextReadback> {
            self.count.fetch_add(1, Ordering::SeqCst);
            if self.error_after {
                return Err(fault(
                    "mock_set_text_after_call",
                    "Actual counted mock setter then failure; never replay.",
                ));
            }
            let read = || TextValueRead {
                native_status: 0,
                cf_string: Some(true),
                matches_requested: Some(true),
                utf8_bytes: Some(text.len()),
                sha256: Some(digest(text.as_bytes())),
            };
            Ok(SetTextReadback {
                setter_attempts: 1,
                setter_status: 0,
                first: read(),
                second: read(),
                context_verified: true,
                focus_readback: None,
                settlement: None,
            })
        }
        fn act(
            &mut self,
            _: &Binding,
            _: &Resolved,
            _: &Intent,
            _: Option<&str>,
        ) -> Result<Option<bool>> {
            panic!("no keyboard or generic act fallback")
        }
        fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> Result<Oracle> {
            self.fixture.inspect(s, p, e)
        }
    }
    fn case(
        ordinary: bool,
        error_after: bool,
    ) -> (
        tempfile::TempDir,
        Engine,
        SetTextRequest,
        String,
        Arc<AtomicU64>,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let count = Arc::new(AtomicU64::new(0));
        let mut engine = Engine::new(dir.path().join("state")).unwrap();
        engine
            .register(
                "t".into(),
                Box::new(Mock {
                    fixture: Fixture::default(),
                    count: count.clone(),
                    ordinary,
                    error_after,
                }),
            )
            .unwrap();
        let frame = engine.observe("t").unwrap().frame;
        let request = SetTextRequest {
            target: "t".into(),
            frame,
            element: "Input".into(),
            request_id: "once".into(),
            text: "🙂\n 中文 ".into(),
        };
        let lease = engine.claim("t", 60000).unwrap();
        (dir, engine, request, lease, count)
    }
    #[test]
    fn explicit_set_text_core_exact_once_dedup_without_lease_and_text_conflict() {
        let (_dir, mut e, r, lease, count) = case(true, false);
        let result = e.set_text(&r, &lease).unwrap();
        assert_eq!(result["receipt"]["dispatch"], "returned");
        assert_eq!(result["receipt"]["goal_verified"], false);
        assert_eq!(result["value_readback"]["observed_ax_replacement"], true);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        e.release(&lease).unwrap();
        let duplicate = e.set_text(&r, "released-lease").unwrap();
        assert_eq!(duplicate["receipt"]["deduplicated"], true);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        let mut changed = r.clone();
        changed.text.push('x');
        assert_eq!(
            e.set_text(&changed, "released").unwrap_err().code,
            "idempotency_conflict"
        );
    }
    #[test]
    fn explicit_set_text_core_bad_class_zero_dispatch_and_uncertain_never_replays() {
        let (_dir, mut e, r, lease, count) = case(false, false);
        let result = e.set_text(&r, &lease).unwrap();
        assert_eq!(result["receipt"]["dispatch"], "rejected_before_dispatch");
        assert_eq!(count.load(Ordering::SeqCst), 0);
        e.set_text(&r, &lease).unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 0);
        let (_dir, mut e, r, lease, count) = case(true, true);
        let result = e.set_text(&r, &lease).unwrap();
        assert_eq!(result["receipt"]["dispatch"], "uncertain");
        assert_eq!(count.load(Ordering::SeqCst), 1);
        e.set_text(&r, &lease).unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn explicit_set_text_core_invalid_lease_or_stale_frame_has_zero_setters() {
        let (_dir, mut e, r, lease, count) = case(true, false);
        assert!(e.set_text(&r, "not-the-lease").is_err());
        assert_eq!(count.load(Ordering::SeqCst), 0);
        e.snapshots
            .iter_mut()
            .find(|(f, _)| f == &r.frame)
            .unwrap()
            .1
            .at -= std::time::Duration::from_secs(121);
        assert_eq!(e.set_text(&r, &lease).unwrap_err().code, "stale_reference");
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
}
