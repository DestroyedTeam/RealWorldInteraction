//! Distinguish rejection before native dispatch from uncertain native effects.
use rwi_core::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
pub struct Observed {
    pub inner: Box<dyn Backend>,
    pub entered: Arc<AtomicBool>,
}
impl Backend for Observed {
    fn drag_surface(
        &mut self,
        endpoint: &drag_between::Endpoint,
        initial: bool,
    ) -> Result<drag_between::Surface> {
        self.inner.drag_surface(endpoint, initial)
    }
    fn drag_between(
        &mut self,
        pair: &drag_between::Pair,
        destination: &mut dyn Backend,
    ) -> Result<Option<bool>> {
        let before = self.inner.input_dispatch_count();
        let previous = self.entered.swap(true, Ordering::AcqRel);
        let result = self.inner.drag_between(pair, destination);
        if !previous
            && before.is_some_and(|n| n < u64::MAX)
            && before == self.inner.input_dispatch_count()
        {
            self.entered.store(false, Ordering::Release);
        }
        result
    }
    fn local_pixel_anchor(&self) -> bool {
        self.inner.local_pixel_anchor()
    }
    fn input_dispatch_count(&self) -> Option<u64> {
        self.inner.input_dispatch_count()
    }
    fn identity(&self) -> Identity {
        self.inner.identity()
    }
    fn capabilities(&self) -> serde_json::Value {
        self.inner.capabilities()
    }
    fn runtime_status(&self) -> serde_json::Value {
        self.inner.runtime_status()
    }
    fn observe(&mut self) -> Result<Scene> {
        self.inner.observe()
    }
    fn capture_visual(&mut self, options: VisualOptions) -> Result<VisualCapture> {
        self.inner.capture_visual(options)
    }
    fn live(
        &mut self,
        action: LiveAction,
        options: Option<LiveOptions>,
    ) -> Result<serde_json::Value> {
        self.inner.live(action, options)
    }
    fn live_view(&mut self, update: LiveViewUpdate) -> Result<serde_json::Value> {
        self.inner.live_view(update)
    }
    fn live_controller(&self) -> Result<Arc<dyn LiveController>> {
        self.inner.live_controller()
    }
    fn observe_region(&mut self, r: [u32; 4]) -> Result<Scene> {
        self.inner.observe_region(r)
    }
    fn revalidate(&mut self, r: &Resolved) -> Result<Scene> {
        self.inner.revalidate(r)
    }
    fn discover(&mut self) -> Result<serde_json::Value> {
        self.inner.discover()
    }
    fn discover_with(&mut self, q: &DiscoveryQuery) -> Result<serde_json::Value> {
        self.inner.discover_with(q)
    }
    fn bind_window(&mut self, s: &WindowSelection) -> Result<serde_json::Value> {
        self.inner.bind_window(s)
    }
    fn attach_window(&mut self, s: &WindowSelection) -> Result<rwi_core::WindowAttachment> {
        let attachment = self.inner.attach_window(s)?;
        Ok(rwi_core::WindowAttachment {
            backend: Box::new(Observed {
                inner: attachment.backend,
                entered: self.entered.clone(),
            }),
            details: attachment.details,
        })
    }
    fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> Result<Oracle> {
        self.inner.inspect(s, p, e)
    }
    fn feedback(&mut self, visible: bool, point: Option<[f64; 2]>) -> Result<serde_json::Value> {
        self.inner.feedback(visible, point)
    }
    fn popups(&mut self, scope: &Scope) -> Result<Vec<Popup>> {
        self.inner.popups(scope)
    }
    fn focus_window(&mut self, r: &Resolved, f: &ForegroundState) -> Result<serde_json::Value> {
        let before = self.inner.input_dispatch_count();
        let previous = self.entered.swap(true, Ordering::AcqRel);
        let result = self.inner.focus_window(r, f);
        let after = self.inner.input_dispatch_count();
        if !previous && before.is_some_and(|n| n < u64::MAX) && before == after {
            self.entered.store(false, Ordering::Release);
        }
        result
    }
    fn hotkey(&mut self, key: &str, f: &ForegroundState) -> Result<serde_json::Value> {
        let before = self.inner.input_dispatch_count();
        let previous = self.entered.swap(true, Ordering::AcqRel);
        let result = self.inner.hotkey(key, f);
        let after = self.inner.input_dispatch_count();
        if !previous && before.is_some_and(|n| n < u64::MAX) && before == after {
            self.entered.store(false, Ordering::Release);
        }
        result
    }
    fn move_window(&mut self, r: &Resolved, d: &DisplayTarget) -> Result<serde_json::Value> {
        let before = self.inner.input_dispatch_count();
        self.entered.store(true, Ordering::Release);
        let result = self.inner.move_window(r, d);
        let after = self.inner.input_dispatch_count();
        if before.is_some_and(|n| n < u64::MAX) && before == after {
            // No native dispatch happened; keep the explicit pre-dispatch proof.
            self.entered.store(false, Ordering::Release);
        }
        result
    }
    fn lifecycle_grant(&self) -> Result<lifecycle::LifecycleGrant> {
        self.inner.lifecycle_grant()
    }
    fn lifecycle_deadline(&self) -> Result<Instant> {
        self.inner.lifecycle_deadline()
    }
    fn validate_activation(&self, w: &lifecycle::WindowIdentity) -> Result<()> {
        self.inner.validate_activation(w)
    }
    fn lifecycle_driver(&mut self) -> Result<Box<dyn lifecycle::LifecycleDriver>> {
        Ok(Box::new(ObservedLifecycle {
            inner: self.inner.lifecycle_driver()?,
            entered: self.entered.clone(),
        }))
    }
    fn act(
        &mut self,
        b: &Binding,
        r: &Resolved,
        i: &Intent,
        v: Option<&str>,
    ) -> Result<Option<bool>> {
        let before = self.inner.input_dispatch_count();
        self.entered.store(true, Ordering::Release);
        let result = self.inner.act(b, r, i, v);
        let after = self.inner.input_dispatch_count();
        if before.is_some_and(|n| n < u64::MAX) && before == after {
            // Native Windows counters prove a pre-dispatch rejection. Preserve
            // its receipt without unnecessarily fencing the entire input domain.
            self.entered.store(false, Ordering::Release);
        }
        result
    }
}

struct ObservedLifecycle {
    inner: Box<dyn lifecycle::LifecycleDriver>,
    entered: Arc<AtomicBool>,
}
impl ObservedLifecycle {
    fn effect<T>(
        &mut self,
        f: impl FnOnce(&mut dyn lifecycle::LifecycleDriver) -> Result<T>,
    ) -> Result<T> {
        let before = self.inner.native_effect_count();
        let already_entered = self.entered.swap(true, Ordering::AcqRel);
        let result = f(self.inner.as_mut());
        // Unknown drivers and panics retain the conservative effect signal.
        if !already_entered && before.is_some() && before == self.inner.native_effect_count() {
            self.entered.store(false, Ordering::Release);
        }
        result
    }
}
impl lifecycle::LifecycleDriver for ObservedLifecycle {
    fn identity(&mut self) -> Result<Identity> {
        self.inner.identity()
    }
    fn verify_application(&mut self, a: &lifecycle::ApplicationIdentity) -> Result<()> {
        self.inner.verify_application(a)
    }
    fn verify_file(&mut self, f: &lifecycle::AllowedFile) -> Result<()> {
        self.inner.verify_file(f)
    }
    fn verify_observed_package(
        &mut self,
        p: &lifecycle::ObservedPackage,
        w: &[lifecycle::WindowIdentity],
    ) -> Result<()> {
        self.inner.verify_observed_package(p, w)
    }
    fn inventory(&mut self, a: &[lifecycle::ApplicationIdentity]) -> Result<lifecycle::Inventory> {
        self.inner.inventory(a)
    }
    fn begin_events(&mut self) -> Result<()> {
        self.inner.begin_events()
    }
    fn begin_activation_events(&mut self, w: &lifecycle::WindowIdentity) -> Result<()> {
        self.inner.begin_activation_events(w)
    }
    fn events(&mut self) -> Result<lifecycle::EventBatch> {
        self.inner.events()
    }
    fn end_events(&mut self) -> Result<lifecycle::EventBatch> {
        self.inner.end_events()
    }
    fn launch(&mut self, p: &lifecycle::LaunchPermission) -> Result<lifecycle::NativeLaunch> {
        self.effect(|d| d.launch(p))
    }
    fn foreground(&mut self) -> Result<Option<lifecycle::WindowIdentity>> {
        self.inner.foreground()
    }
    fn user_input_tick(&mut self) -> Result<u64> {
        self.inner.user_input_tick()
    }
    fn check_window(&mut self, w: &lifecycle::WindowIdentity) -> Result<()> {
        self.inner.check_window(w)
    }
    fn window_presentation(
        &mut self,
        w: &lifecycle::WindowIdentity,
    ) -> Result<lifecycle::WindowPresentation> {
        self.inner.window_presentation(w)
    }
    fn restore(&mut self, w: &lifecycle::WindowIdentity) -> Result<bool> {
        self.effect(|d| d.restore(w))
    }
    fn activate(&mut self, w: &lifecycle::WindowIdentity) -> Result<bool> {
        self.effect(|d| d.activate(w))
    }
    fn native_effect_count(&self) -> Option<u64> {
        self.inner.native_effect_count()
    }
    fn pause(&mut self, ms: u64) {
        self.inner.pause(ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lifecycle::*;
    #[derive(Default)]
    struct Driver {
        count: u64,
        known_counter: bool,
        reject_before: bool,
        fail_after: bool,
        panic_after: bool,
    }
    fn identity() -> Identity {
        Identity {
            host: "offline".into(),
            guest: None,
            session: "s".into(),
            seat: "seat".into(),
            desktop: "d".into(),
        }
    }
    fn window() -> WindowIdentity {
        WindowIdentity {
            window: "hwnd:1".into(),
            pid: 1,
            process_created: 1,
            native_epoch: "epoch".into(),
        }
    }
    impl Driver {
        fn mutate(&mut self) -> Result<bool> {
            if self.reject_before {
                return Err(fault("fixture_predispatch", "No native effect."));
            }
            self.count += 1;
            assert!(!self.panic_after, "fixture panic after native effect");
            if self.fail_after {
                return Err(fault("fixture_after_dispatch", "Effect is uncertain."));
            }
            Ok(true)
        }
    }
    impl LifecycleDriver for Driver {
        fn identity(&mut self) -> Result<Identity> {
            Ok(identity())
        }
        fn verify_application(&mut self, _: &ApplicationIdentity) -> Result<()> {
            Ok(())
        }
        fn verify_file(&mut self, _: &AllowedFile) -> Result<()> {
            Ok(())
        }
        fn inventory(&mut self, _: &[ApplicationIdentity]) -> Result<Inventory> {
            unreachable!()
        }
        fn begin_events(&mut self) -> Result<()> {
            Ok(())
        }
        fn events(&mut self) -> Result<EventBatch> {
            Ok(EventBatch {
                complete: true,
                ..Default::default()
            })
        }
        fn end_events(&mut self) -> Result<EventBatch> {
            Ok(EventBatch {
                complete: true,
                hooks_released: true,
                ..Default::default()
            })
        }
        fn launch(&mut self, _: &LaunchPermission) -> Result<NativeLaunch> {
            self.mutate()?;
            Ok(NativeLaunch {
                api: "fixture".into(),
                accepted: true,
                process_id: None,
                process_created: None,
                process_handles_released: true,
                error: None,
            })
        }
        fn foreground(&mut self) -> Result<Option<WindowIdentity>> {
            Ok(None)
        }
        fn user_input_tick(&mut self) -> Result<u64> {
            Ok(1)
        }
        fn check_window(&mut self, _: &WindowIdentity) -> Result<()> {
            Ok(())
        }
        fn window_presentation(&mut self, _: &WindowIdentity) -> Result<WindowPresentation> {
            Ok(WindowPresentation {
                visible: true,
                minimized: false,
                cloaked: Some(false),
                usable_geometry: true,
            })
        }
        fn restore(&mut self, _: &WindowIdentity) -> Result<bool> {
            self.mutate()
        }
        fn activate(&mut self, _: &WindowIdentity) -> Result<bool> {
            self.mutate()
        }
        fn native_effect_count(&self) -> Option<u64> {
            self.known_counter.then_some(self.count)
        }
        fn pause(&mut self, _: u64) {}
    }
    struct DriverBackend;
    impl Backend for DriverBackend {
        fn identity(&self) -> Identity {
            identity()
        }
        fn capabilities(&self) -> serde_json::Value {
            serde_json::Value::Null
        }
        fn observe(&mut self) -> Result<Scene> {
            unreachable!()
        }
        fn act(
            &mut self,
            _: &Binding,
            _: &Resolved,
            _: &Intent,
            _: Option<&str>,
        ) -> Result<Option<bool>> {
            unreachable!()
        }
        fn inspect(&mut self, _: &Scope, _: &Predicate, _: &str) -> Result<Oracle> {
            unreachable!()
        }
        fn lifecycle_driver(&mut self) -> Result<Box<dyn LifecycleDriver>> {
            Ok(Box::new(Driver {
                known_counter: true,
                ..Default::default()
            }))
        }
    }
    fn wrapper(inner: Driver) -> ObservedLifecycle {
        ObservedLifecycle {
            inner: Box::new(inner),
            entered: Arc::new(AtomicBool::new(false)),
        }
    }
    #[test]
    fn lifecycle_driver_creation_and_readonly_preflight_do_not_mark_native_dispatch() {
        let entered = Arc::new(AtomicBool::new(false));
        let mut backend = Observed {
            inner: Box::new(DriverBackend),
            entered: entered.clone(),
        };
        let mut driver = backend.lifecycle_driver().unwrap();
        driver.identity().unwrap();
        driver.begin_activation_events(&window()).unwrap();
        driver.check_window(&window()).unwrap();
        assert!(driver.window_presentation(&window()).unwrap().presented());
        driver.foreground().unwrap();
        driver.user_input_tick().unwrap();
        driver.events().unwrap();
        driver.end_events().unwrap();
        assert!(!entered.load(Ordering::Acquire));
        assert_eq!(driver.native_effect_count(), Some(0));
        driver.activate(&window()).unwrap();
        assert!(entered.load(Ordering::Acquire));
        assert_eq!(driver.native_effect_count(), Some(1));
    }
    #[test]
    fn known_predispatch_rejection_is_precise_and_unknown_driver_remains_conservative() {
        for known_counter in [false, true] {
            let mut driver = wrapper(Driver {
                known_counter,
                reject_before: true,
                ..Default::default()
            });
            assert!(driver.activate(&window()).is_err());
            assert_eq!(driver.entered.load(Ordering::Acquire), !known_counter);
        }
    }
    #[test]
    fn native_errors_panics_and_prior_effects_cannot_clear_dispatch_evidence() {
        let mut driver = wrapper(Driver {
            known_counter: true,
            fail_after: true,
            ..Default::default()
        });
        assert!(driver.activate(&window()).is_err());
        assert!(driver.entered.load(Ordering::Acquire));
        let mut driver = wrapper(Driver {
            known_counter: true,
            panic_after: true,
            ..Default::default()
        });
        let panic =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| driver.activate(&window())));
        assert!(panic.is_err() && driver.entered.load(Ordering::Acquire));
        let mut driver = wrapper(Driver {
            known_counter: true,
            reject_before: true,
            ..Default::default()
        });
        driver.entered.store(true, Ordering::Release);
        assert!(driver.activate(&window()).is_err());
        assert!(driver.entered.load(Ordering::Acquire));
    }
    #[test]
    fn launch_and_activation_share_only_real_effect_accounting() {
        let permission = LaunchPermission {
            authorization_id: "launch".into(),
            application: ApplicationIdentity {
                executable: "fixture".into(),
                sha256: "a".repeat(64),
            },
            arguments: vec![],
            allowed_files: vec![],
            observed_applications: vec![],
            observed_packages: vec![],
            allowed_existing_windows: vec![],
            application_startup_effects_authorized: true,
        };
        for reject_before in [false, true] {
            let mut driver = wrapper(Driver {
                known_counter: true,
                reject_before,
                ..Default::default()
            });
            assert_eq!(driver.launch(&permission).is_err(), reject_before);
            assert_eq!(driver.entered.load(Ordering::Acquire), !reject_before);
        }
    }
    #[test]
    fn restore_is_an_observed_effect_including_rejection_failure_and_panic() {
        for known_counter in [false, true] {
            let mut driver = wrapper(Driver {
                known_counter,
                reject_before: true,
                ..Default::default()
            });
            assert!(driver.restore(&window()).is_err());
            assert_eq!(driver.entered.load(Ordering::Acquire), !known_counter);
        }
        for panic_after in [false, true] {
            let mut driver = wrapper(Driver {
                known_counter: true,
                fail_after: !panic_after,
                panic_after,
                ..Default::default()
            });
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                driver.restore(&window())
            }));
            assert!(result.is_err() || result.unwrap().is_err());
            assert!(driver.entered.load(Ordering::Acquire));
            assert_eq!(driver.native_effect_count(), Some(1));
        }
        let mut driver = wrapper(Driver {
            known_counter: true,
            ..Default::default()
        });
        assert!(driver.restore(&window()).unwrap());
        assert!(driver.activate(&window()).unwrap());
        assert!(driver.entered.load(Ordering::Acquire));
        assert_eq!(driver.native_effect_count(), Some(2));
    }
}
