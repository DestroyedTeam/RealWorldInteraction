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
        self.leases
            .shared()
            .lock()
            .map_err(|_| fault("lease_error", "Restart observation session."))?
            .check(lease, target, &key)?;
        let element_policy =
            self.backend(target)?.capabilities()["checked_context_policy"] == "native_element";
        let snapshot = self.snapshot(target, frame)?;
        // Only this explicitly checked semantic API gets a bounded proposal
        // horizon. The legacy act/pixel 10-second rule is unchanged.
        if snapshot.at.elapsed() > Duration::from_secs(120) {
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
        let old = self.resolve(target, frame, Locator::Element { id: element.into() })?;
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
            return Err(fault(
                "checked_precondition_changed",
                "Document/focus/layout/element context changed or became unknown; no input, no automatic reobserve loop.",
            ));
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
            },
        ));
        while self.snapshots.len() > SNAPSHOT_LIMIT {
            self.snapshots.pop_front();
        }
        let resolved = self.resolve(target, &fresh, Locator::Element { id: element.into() })?;
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
mod tests {
    use super::*;
    use crate::fixture::Fixture;

    fn setup() -> (tempfile::TempDir, Engine, Fixture) {
        let dir = tempfile::tempdir().unwrap();
        let f = Fixture::default();
        let mut e = Engine::new(dir.path().into()).unwrap();
        e.register("fixture".into(), Box::new(f.clone())).unwrap();
        (dir, e, f)
    }
    #[test]
    fn typed_drag_is_one_durable_intent_and_changed_path_never_replays() {
        use crate::trajectory::{DragPath, Waypoint};
        let (_dir, mut e, f) = setup();
        let frame = e.observe("fixture").unwrap().frame;
        let lease = e.claim("fixture", 60000).unwrap();
        let mut path = DragPath {
            waypoints: vec![
                Waypoint {
                    at_ms: 0,
                    offset: [0., 0.],
                },
                Waypoint {
                    at_ms: 200,
                    offset: [20., 0.],
                },
            ],
        };
        let invoke = |e: &mut Engine, path: DragPath| {
            e.act_checked(CheckedAction {
                target: "fixture",
                frame: &frame,
                element: "Input",
                lease: &lease,
                request: "drag-once",
                intent: Intent::Drag(path),
                value: None,
            })
        };
        assert_eq!(
            invoke(&mut e, path.clone()).unwrap().system_accepted,
            Some(true)
        );
        assert!(invoke(&mut e, path.clone()).unwrap().deduplicated);
        assert_eq!(f.0.lock().unwrap().calls, 1);
        path.waypoints[1].offset = [21., 0.];
        assert_eq!(
            invoke(&mut e, path).unwrap_err().code,
            "idempotency_conflict"
        );
        assert_eq!(f.0.lock().unwrap().calls, 1);
        e.release(&lease).unwrap();
    }
    fn age(e: &mut Engine, frame: &str, seconds: u64) {
        e.snapshots
            .iter_mut()
            .find(|(id, _)| id == frame)
            .unwrap()
            .1
            .at -= Duration::from_secs(seconds);
    }
    fn checked(e: &mut Engine, frame: &str, lease: &str, request: &str) -> Result<Receipt> {
        e.act_checked(CheckedAction {
            target: "fixture",
            frame,
            element: "Input",
            lease,
            request,
            intent: Intent::Text,
            value: Some("checked result".into()),
        })
    }
    #[test]
    fn delayed_semantic_proposal_is_revalidated_and_dispatched_once_without_legacy_ttl_change() {
        let (_d, mut e, f) = setup();
        let frame = e.observe("fixture").unwrap().frame;
        let r = e
            .resolve("fixture", &frame, Locator::Element { id: "Input".into() })
            .unwrap();
        let lease = e.claim("fixture", 60000).unwrap();
        age(&mut e, &frame, 13);
        assert_eq!(
            e.act(
                "fixture",
                &r.reference,
                &lease,
                "old-call",
                Intent::Text,
                Some("old".into())
            )
            .unwrap_err()
            .code,
            "stale_reference"
        );
        assert_eq!(f.0.lock().unwrap().calls, 0);
        let result = checked(&mut e, &frame, &lease, "checked-once").unwrap();
        assert_eq!(result.system_accepted, Some(true));
        assert_ne!(result.frame, frame);
        assert!(!result.goal_verified);
        assert_eq!(f.0.lock().unwrap().value, "checked result");
        assert_eq!(f.0.lock().unwrap().calls, 1);
        assert!(
            checked(&mut e, &frame, &lease, "checked-once")
                .unwrap()
                .deduplicated
        );
        assert_eq!(f.0.lock().unwrap().calls, 1);
        e.release(&lease).unwrap();
    }
    #[test]
    fn unchanged_document_does_not_turn_old_pixel_reference_into_checked_authority() {
        let (_d, mut e, f) = setup();
        let frame = e.observe("fixture").unwrap().frame;
        let pixel = e
            .resolve("fixture", &frame, Locator::Pixel { x: 30., y: 40. })
            .unwrap();
        let lease = e.claim("fixture", 60000).unwrap();
        age(&mut e, &frame, 13);
        assert_eq!(
            e.act(
                "fixture",
                &pixel.reference,
                &lease,
                "pixel",
                Intent::Click,
                None
            )
            .unwrap_err()
            .code,
            "stale_reference"
        );
        assert!(
            e.act_checked(CheckedAction {
                target: "fixture",
                frame: &frame,
                element: &pixel.reference,
                lease: &lease,
                request: "pixel-as-element",
                intent: Intent::Click,
                value: None
            })
            .is_err()
        );
        assert_eq!(f.0.lock().unwrap().calls, 0);
    }
    #[test]
    fn proposal_horizon_cancel_unknown_context_and_expired_lease_remain_fail_closed() {
        let (d, mut e, f) = setup();
        let frame = e.observe("fixture").unwrap().frame;
        let lease = e.claim("fixture", 60000).unwrap();
        age(&mut e, &frame, 121);
        assert_eq!(
            checked(&mut e, &frame, &lease, "expired").unwrap_err().code,
            "checked_proposal_expired"
        );
        let frame = e.observe("fixture").unwrap().frame;
        e.snapshots
            .back_mut()
            .unwrap()
            .1
            .scene
            .binding
            .as_mut()
            .unwrap()
            .mutation_context = None;
        assert_eq!(
            checked(&mut e, &frame, &lease, "unknown").unwrap_err().code,
            "checked_context_unavailable"
        );
        let frame = e.observe("fixture").unwrap().frame;
        let op = operation::Registry::default().begin(None).unwrap();
        op.cancel();
        assert_eq!(
            operation::run(&op, || checked(&mut e, &frame, &lease, "cancelled"))
                .unwrap_err()
                .code,
            "operation_cancelled"
        );
        e.release(&lease).unwrap();
        assert!(checked(&mut e, &frame, &lease, "released-lease").is_err());
        assert_eq!(f.0.lock().unwrap().calls, 0);
        assert_eq!(fs::read_dir(d.path().join("receipts")).unwrap().count(), 0);
    }
    #[test]
    fn checked_request_fingerprint_survives_new_engine_and_rejects_altered_intent() {
        let (d, mut e, f) = setup();
        let frame = e.observe("fixture").unwrap().frame;
        let lease = e.claim("fixture", 60000).unwrap();
        checked(&mut e, &frame, &lease, "persist").unwrap();
        e.release(&lease).unwrap();
        drop(e);
        let mut e = Engine::new(d.path().into()).unwrap();
        e.register("fixture".into(), Box::new(f.clone())).unwrap();
        assert!(
            checked(&mut e, &frame, "not-a-reused-token", "persist")
                .unwrap()
                .deduplicated
        );
        assert_eq!(
            e.act_checked(CheckedAction {
                target: "fixture",
                frame: &frame,
                element: "Input",
                lease: "none",
                request: "persist",
                intent: Intent::Text,
                value: Some("different".into())
            })
            .unwrap_err()
            .code,
            "idempotency_conflict"
        );
        assert_eq!(f.0.lock().unwrap().calls, 1);
    }
    struct NativeLikeEditor(Fixture);
    impl Backend for NativeLikeEditor {
        fn identity(&self) -> Identity {
            self.0.identity()
        }
        fn capabilities(&self) -> serde_json::Value {
            self.0.capabilities()
        }
        fn observe(&mut self) -> Result<Scene> {
            let mut s = self.0.observe()?;
            // Deliberately model the previous native revision weakness, not the
            // stronger original Fixture Input revision that already hashes value.
            for e in &mut s.elements {
                e.state_revision = Some("stable-runtime-name-geometry-focus".into());
            }
            Ok(s)
        }
        fn act(
            &mut self,
            b: &Binding,
            r: &Resolved,
            i: &Intent,
            v: Option<&str>,
        ) -> Result<Option<bool>> {
            self.0.act(b, r, i, v)
        }
        fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> Result<Oracle> {
            self.0.inspect(s, p, e)
        }
    }
    #[test]
    fn value_switch_with_identical_runtime_name_geometry_focus_is_rejected_before_input() {
        let d = tempfile::tempdir().unwrap();
        let f = Fixture::default();
        let mut e = Engine::new(d.path().into()).unwrap();
        e.register("fixture".into(), Box::new(NativeLikeEditor(f.clone())))
            .unwrap();
        let frame = e.observe("fixture").unwrap().frame;
        let lease = e.claim("fixture", 60000).unwrap();
        f.0.lock().unwrap().value = "other document same editor".into();
        assert_eq!(
            checked(&mut e, &frame, &lease, "switched-doc")
                .unwrap_err()
                .code,
            "checked_precondition_changed"
        );
        assert_eq!(f.0.lock().unwrap().calls, 0);
        assert_eq!(fs::read_dir(d.path().join("receipts")).unwrap().count(), 0);
    }
    #[test]
    fn changed_title_context_or_layout_is_not_refreshed_into_permission() {
        let (_d, mut e, f) = setup();
        let lease = e.claim("fixture", 60000).unwrap();
        let frame = e.observe("fixture").unwrap().frame;
        e.snapshots
            .back_mut()
            .unwrap()
            .1
            .scene
            .binding
            .as_mut()
            .unwrap()
            .mutation_context
            .as_mut()
            .unwrap()
            .window_title = "different document".into();
        assert_eq!(
            checked(&mut e, &frame, &lease, "title").unwrap_err().code,
            "checked_precondition_changed"
        );
        let frame = e.observe("fixture").unwrap().frame;
        f.0.lock().unwrap().transform = Some([1., 0., 0., 1., 0., 0.]);
        assert_eq!(
            checked(&mut e, &frame, &lease, "layout").unwrap_err().code,
            "checked_precondition_changed"
        );
        assert_eq!(f.0.lock().unwrap().calls, 0);
    }
}
