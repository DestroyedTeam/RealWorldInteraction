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
        let native_action = match (&intent, value.as_deref()) {
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
    fn delayed_semantic_proposal_is_revalidated_and_dispatched_once() {
        let (_d, mut e, f) = setup();
        let frame = e.observe("fixture").unwrap().frame;
        let lease = e.claim("fixture", 60000).unwrap();
        age(&mut e, &frame, 30);
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
        age(&mut e, &frame, INPUT_PROPOSAL_MAX_AGE_SECS + 1);
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

#[cfg(test)]
mod native_action_tests {
    use super::*;
    use crate::fixture::Fixture;
    struct VisiblePointerOnly(Fixture);
    impl Backend for VisiblePointerOnly {
        fn identity(&self) -> Identity {
            self.0.identity()
        }
        fn capabilities(&self) -> serde_json::Value {
            serde_json::json!({"checked_context_policy":"native_element","native_semantic_actions":[]})
        }
        fn observe(&mut self) -> Result<Scene> {
            let mut s = self.0.observe()?;
            for e in &mut s.elements {
                e.visible = Some(true);
                e.occluded = Some(false);
                e.enabled = Some(true);
                e.actions = vec!["AXShowMenu".into()];
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
    fn explicit_native_action_never_uses_a_pointer_reference_on_pointer_only_backend() {
        for value in [
            None,
            Some("AXShowMenu"),
            Some("AXUnknown"),
            Some("unknown_action"),
        ] {
            let d = tempfile::tempdir().unwrap();
            let f = Fixture::default();
            let mut e = Engine::new(d.path().into()).unwrap();
            e.register("t".into(), Box::new(VisiblePointerOnly(f.clone())))
                .unwrap();
            let frame = e.observe("t").unwrap().frame;
            let lease = e.claim("t", 60000).unwrap();
            let result = e.act_checked(CheckedAction {
                target: "t",
                frame: &frame,
                element: "Input",
                lease: &lease,
                request: "one-explicit-action",
                intent: Intent::Click,
                value: value.map(str::to_owned),
            });
            if value.is_none() {
                assert!(result.is_ok());
                assert_eq!(f.0.lock().unwrap().calls, 1);
            } else {
                assert_eq!(result.unwrap_err().code, "native_action_unavailable");
                assert_eq!(f.0.lock().unwrap().calls, 0);
            }
        }
    }
    struct AdvertisedNative(VisiblePointerOnly, bool, bool);
    impl Backend for AdvertisedNative {
        fn identity(&self) -> Identity {
            self.0.identity()
        }
        fn capabilities(&self) -> serde_json::Value {
            serde_json::json!({"checked_context_policy":"native_element","native_semantic_action_policy":"advertised_element_action"})
        }
        fn observe(&mut self) -> Result<Scene> {
            let mut s = self.0.observe()?;
            for e in &mut s.elements {
                e.visible = Some(self.1);
                e.occluded = Some(!self.1);
                e.enabled = Some(self.2);
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
            assert_eq!(r.native_action.as_deref(), Some("AXShowMenu"));
            self.0.act(b, r, i, v)
        }
        fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> Result<Oracle> {
            self.0.inspect(s, p, e)
        }
    }
    #[test]
    fn explicit_native_action_policy_requires_the_exact_advertised_element_action() {
        for visible in [false, true] {
            for enabled in [false, true] {
                for action in ["AXShowMenu", "AXUnknown"] {
                    let pass = enabled && action == "AXShowMenu";
                    let d = tempfile::tempdir().unwrap();
                    let f = Fixture::default();
                    let mut e = Engine::new(d.path().into()).unwrap();
                    e.register(
                        "t".into(),
                        Box::new(AdvertisedNative(
                            VisiblePointerOnly(f.clone()),
                            visible,
                            enabled,
                        )),
                    )
                    .unwrap();
                    let frame = e.observe("t").unwrap().frame;
                    let lease = e.claim("t", 60000).unwrap();
                    let result = e.act_checked(CheckedAction {
                        target: "t",
                        frame: &frame,
                        element: "Input",
                        lease: &lease,
                        request: "one-advertised-action",
                        intent: Intent::Click,
                        value: Some(action.into()),
                    });
                    assert_eq!(result.is_ok(), pass);
                    if !pass {
                        assert_eq!(
                            result.unwrap_err().code,
                            if action == "AXUnknown" {
                                "unsupported_action"
                            } else {
                                "element_disabled"
                            }
                        );
                    }
                    assert_eq!(f.0.lock().unwrap().calls, u64::from(pass));
                    e.release(&lease).unwrap();
                }
            }
        }
    }
    struct SemanticFixture {
        base: Fixture,
        advertised: bool,
        disabled: bool,
        changed: bool,
        reads: usize,
    }
    impl Backend for SemanticFixture {
        fn identity(&self) -> Identity {
            self.base.identity()
        }
        fn capabilities(&self) -> serde_json::Value {
            serde_json::json!({"checked_context_policy":"native_element", "native_semantic_actions":if self.advertised {vec!["AXPress"]} else {vec![]}})
        }
        fn observe(&mut self) -> Result<Scene> {
            self.reads += 1;
            let mut s = self.base.observe()?;
            for e in &mut s.elements {
                e.visible = Some(self.reads.is_multiple_of(2));
                e.occluded = Some(!self.reads.is_multiple_of(2));
                e.actions = vec!["AXPress".into()];
                e.enabled = Some(!self.disabled);
                if self.changed && self.reads > 1 {
                    e.state_revision = Some("changed".into());
                }
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
            assert_eq!(r.native_action.as_deref(), Some("AXPress"));
            self.base.act(b, r, i, v)
        }
        fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> Result<Oracle> {
            self.base.inspect(s, p, e)
        }
    }
    #[test]
    fn native_actions_are_explicit_capability_gated_and_never_pointer_references() {
        for (advertised, disabled, changed, value, pass) in [
            (true, false, false, Some("AXPress"), true),
            (false, false, false, Some("AXPress"), false),
            (true, true, false, Some("AXPress"), false),
            (true, false, true, Some("AXPress"), false),
            (true, false, false, None, false),
            (true, false, false, Some("AXUnknown"), false),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut engine = Engine::new(dir.path().into()).unwrap();
            let f = Fixture::default();
            engine
                .register(
                    "t".into(),
                    Box::new(SemanticFixture {
                        base: f.clone(),
                        advertised,
                        disabled,
                        changed,
                        reads: 0,
                    }),
                )
                .unwrap();
            let frame = engine.observe("t").unwrap().frame;
            assert!(
                engine
                    .resolve("t", &frame, Locator::Element { id: "Input".into() })
                    .is_err()
            );
            let lease = engine.claim("t", 60000).unwrap();
            let call = |e: &mut Engine| {
                e.act_checked(CheckedAction {
                    target: "t",
                    frame: &frame,
                    element: "Input",
                    lease: &lease,
                    request: "one",
                    intent: Intent::Click,
                    value: value.map(str::to_owned),
                })
            };
            let result = call(&mut engine);
            assert_eq!(result.is_ok(), pass);
            assert_eq!(f.0.lock().unwrap().calls, u64::from(pass));
            if pass {
                assert!(call(&mut engine).unwrap().deduplicated);
                assert_eq!(f.0.lock().unwrap().calls, 1);
                let latest = engine.observe("t").unwrap().frame;
                let r = engine
                    .resolve_action(
                        "t",
                        &latest,
                        Locator::Element { id: "Input".into() },
                        Some("AXPress"),
                    )
                    .unwrap();
                assert_eq!(
                    engine
                        .act("t", &r.reference, &lease, "pointer", Intent::Click, None)
                        .unwrap_err()
                        .code,
                    "native_action_mismatch"
                );
            }
        }
    }
}
