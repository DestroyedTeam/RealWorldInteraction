//! Two independent synthetic windows on one physical desktop. No OS access.
use super::*;
#[derive(Clone)]
pub struct Desktop {
    pub windows: [Fixture; 2],
    identity: Identity,
    selected: usize,
    fixed: bool,
    catalog: Option<[String; 2]>,
}
impl Default for Desktop {
    fn default() -> Self {
        let windows = [Fixture::default(), Fixture::default()];
        windows[1].0.lock().unwrap().transform = Some([0.5, 0., 0., 0.5, 200., 0.]);
        Self {
            identity: windows[0].identity(),
            windows,
            selected: 0,
            fixed: false,
            catalog: None,
        }
    }
}
impl Desktop {
    fn name(index: usize) -> String {
        format!("fixture-window-{}", index + 1)
    }
    fn selection(&mut self, s: &WindowSelection) -> Result<usize> {
        let catalog = self
            .catalog
            .as_ref()
            .ok_or_else(|| fault("catalog_expired", "Discover first."))?;
        let index = (0..2)
            .find(|i| s.window == Self::name(*i))
            .ok_or_else(|| fault("selection_unknown", "Exact fixture window required."))?;
        if s.pid != 42
            || !matches!(s.relation, WindowRelation::Initial)
            || s.native_epoch != catalog[index]
            || self.windows[index].observe()?.binding.unwrap().native_epoch != s.native_epoch
        {
            return Err(fault(
                "window_epoch_changed",
                "Fresh exact fixture selection required.",
            ));
        }
        Ok(index)
    }
}
impl Backend for Desktop {
    fn identity(&self) -> Identity {
        self.identity.clone()
    }
    fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({"adapter":"synthetic-window-fixture","native":false,"window_attachment":{"implemented":!self.fixed},"selected":Self::name(self.selected)})
    }
    fn discover(&mut self) -> Result<serde_json::Value> {
        let epochs = [
            self.windows[0].observe()?.binding.unwrap().native_epoch,
            self.windows[1].observe()?.binding.unwrap().native_epoch,
        ];
        let rows = (0..2).map(|i| serde_json::json!({"window":Self::name(i),"pid":42,"native_epoch":epochs[i],"relation":"initial"})).collect::<Vec<_>>();
        self.catalog = Some(epochs);
        Ok(serde_json::json!({"windows":rows,"native":false,"input_sent":false}))
    }
    fn bind_window(&mut self, selection: &WindowSelection) -> Result<serde_json::Value> {
        if self.fixed {
            return Err(fault("fixed_window_target", "Use the root target."));
        }
        self.selected = self.selection(selection)?;
        Ok(serde_json::json!({"bound":selection,"input_sent":false}))
    }
    fn attach_window(&mut self, selection: &WindowSelection) -> Result<WindowAttachment> {
        if self.fixed {
            return Err(fault("attachment_source_required", "Use root."));
        }
        let selected = self.selection(selection)?;
        let mut attached = self.clone();
        attached.selected = selected;
        attached.fixed = true;
        attached.identity = self.windows[selected].identity();
        Ok(WindowAttachment {
            backend: Box::new(attached),
            details: serde_json::json!({"bound":selection}),
        })
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        Some(self.windows.iter().map(|f| f.0.lock().unwrap().calls).sum())
    }
    fn observe(&mut self) -> Result<Scene> {
        let mut scene = self.windows[self.selected].observe()?;
        if let Some(binding) = &mut scene.binding {
            binding.scope.window = Self::name(self.selected);
            binding.focus_window = Self::name(self.selected);
        }
        Ok(scene)
    }
    fn act(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        intent: &Intent,
        value: Option<&str>,
    ) -> Result<Option<bool>> {
        if expected.scope.window != Self::name(self.selected)
            || target.scope.window != Self::name(self.selected)
        {
            return Err(fault("target_mismatch", "Window scope changed."));
        }
        let mut binding = expected.clone();
        binding.scope.window = "fixture-window".into();
        let mut reference = target.clone();
        reference.scope.window = "fixture-window".into();
        self.windows[self.selected].act(&binding, &reference, intent, value)
    }
    fn inspect(&mut self, scope: &Scope, predicate: &Predicate, expected: &str) -> Result<Oracle> {
        let mut underlying = scope.clone();
        underlying.window = "fixture-window".into();
        let mut oracle = self.windows[self.selected].inspect(&underlying, predicate, expected)?;
        oracle.scope = scope.clone();
        Ok(oracle)
    }
}
