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
    action: &'static str,
    disabled: bool,
    changed: bool,
    reads: usize,
}

#[test]
fn explicit_menu_actions_require_backend_and_element_advertisement_and_never_replay() {
    for action in ["AXShowMenu", "AXCancel", "AXPick"] {
        for advertised in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let mut engine = Engine::new(dir.path().into()).unwrap();
            let f = Fixture::default();
            engine
                .register(
                    "menu".into(),
                    Box::new(SemanticFixture {
                        base: f.clone(),
                        advertised,
                        action,
                        disabled: false,
                        changed: false,
                        reads: 0,
                    }),
                )
                .unwrap();
            let frame = engine.observe("menu").unwrap().frame;
            let lease = engine.claim("menu", 60000).unwrap();
            let call = |engine: &mut Engine, value: &str| {
                engine.act_checked(CheckedAction {
                    target: "menu",
                    frame: &frame,
                    element: "Input",
                    lease: &lease,
                    request: "one-explicit-menu-action",
                    intent: Intent::Click,
                    value: Some(value.into()),
                })
            };
            let receipt = call(&mut engine, action);
            assert_eq!(receipt.is_ok(), advertised);
            assert_eq!(f.0.lock().unwrap().calls, u64::from(advertised));
            if advertised {
                assert!(!receipt.unwrap().goal_verified);
                assert!(call(&mut engine, action).unwrap().deduplicated);
                assert!(call(&mut engine, "AXPress").is_err());
                assert_eq!(f.0.lock().unwrap().calls, 1);
            }
            engine.release(&lease).unwrap();
        }
    }
}
impl Backend for SemanticFixture {
    fn identity(&self) -> Identity {
        self.base.identity()
    }
    fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({"checked_context_policy":"native_element", "native_semantic_actions":if self.advertised {vec![self.action]} else {vec![]}})
    }
    fn observe(&mut self) -> Result<Scene> {
        self.reads += 1;
        let mut s = self.base.observe()?;
        for e in &mut s.elements {
            e.visible = Some(self.reads.is_multiple_of(2));
            e.occluded = Some(!self.reads.is_multiple_of(2));
            e.actions = vec![self.action.into()];
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
        assert_eq!(r.native_action.as_deref(), Some(self.action));
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
                    action: "AXPress",
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
