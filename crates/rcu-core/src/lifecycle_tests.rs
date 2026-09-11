#[cfg(test)]
mod tests {
    use super::super::*;
    fn identity() -> Identity {
        Identity {
            host: "offline".into(),
            guest: Some("fixture".into()),
            session: "s".into(),
            seat: "seat".into(),
            desktop: "d".into(),
        }
    }
    fn app() -> ApplicationIdentity {
        ApplicationIdentity {
            executable: r"C:\fixture\app.exe".into(),
            sha256: "a".repeat(64),
        }
    }
    fn window(n: u32) -> WindowEvidence {
        WindowEvidence {
            key: WindowIdentity {
                window: format!("hwnd:{n:x}"),
                pid: 10,
                process_created: 100,
                native_epoch: format!("epoch-{n}"),
            },
            application: app(),
            title: "existing".into(),
            foreground: false,
        }
    }
    fn grant() -> LifecycleGrant {
        LifecycleGrant {
            run_id: "offline-run".into(),
            route_vm_id: "00000000-0000-0000-0000-000000000001".into(),
            expected_identity: identity(),
            expires_unix_ms: now_ms() + 60_000,
            launches: vec![LaunchPermission {
                authorization_id: "launch".into(),
                application: app(),
                arguments: vec!["exact argument".into()],
                allowed_files: vec![],
                observed_applications: vec![],
                observed_packages: vec![],
                allowed_existing_windows: vec![],
                application_startup_effects_authorized: true,
            }],
            activations: vec![ActivationPermission {
                authorization_id: "activate".into(),
                window: window(1).key,
            }],
        }
    }
    struct Fake {
        before: Vec<WindowEvidence>,
        after: Vec<WindowEvidence>,
        events: Vec<NativeEvent>,
        launches: usize,
        activations: usize,
        ended: usize,
        incomplete: bool,
        inventory_cleanup: bool,
        activation_cleanup: bool,
        verify_failure: bool,
        pin_failure: Option<(String, Fault)>,
        verified_paths: Vec<String>,
        package_enabled: bool,
        package_calls: Vec<ObservedPackage>,
        release: bool,
        late_events: Vec<NativeEvent>,
        session_changes: bool,
        panics: bool,
        cancel: Option<operation::Operation>,
        foreground: Option<WindowIdentity>,
        ticks: std::collections::VecDeque<u64>,
        activate_accept: bool,
    }
    impl Default for Fake {
        fn default() -> Self {
            Self {
                before: vec![],
                after: vec![window(1)],
                events: vec![],
                launches: 0,
                activations: 0,
                ended: 0,
                incomplete: false,
                inventory_cleanup: false,
                activation_cleanup: false,
                verify_failure: false,
                pin_failure: None,
                verified_paths: vec![],
                package_enabled: false,
                package_calls: vec![],
                release: true,
                late_events: vec![],
                session_changes: false,
                panics: false,
                cancel: None,
                foreground: None,
                ticks: Default::default(),
                activate_accept: true,
            }
        }
    }
    impl Fake {
        fn verify_path(&mut self, path: &str) -> Result<()> {
            self.verified_paths.push(path.into());
            if let Some((denied, error)) = &self.pin_failure
                && denied == path
            {
                return Err(error.clone());
            }
            Ok(())
        }
    }
    impl LifecycleDriver for Fake {
        fn identity(&mut self) -> Result<Identity> {
            let mut i = identity();
            if self.session_changes && self.launches > 0 {
                i.session = "new".into();
            }
            Ok(i)
        }
        fn verify_application(&mut self, app: &ApplicationIdentity) -> Result<()> {
            if self.verify_failure {
                Err(fault(
                    "fixture_hash_mismatch",
                    "Synthetic preflight failure.",
                ))
            } else {
                self.verify_path(&app.executable)
            }
        }
        fn verify_file(&mut self, file: &AllowedFile) -> Result<()> {
            self.verify_path(&file.path)
        }
        fn verify_observed_package(
            &mut self,
            package: &ObservedPackage,
            windows: &[WindowIdentity],
        ) -> Result<()> {
            self.package_calls.push(package.clone());
            assert!(!windows.is_empty());
            if self.package_enabled {
                Ok(())
            } else {
                Err(fault(
                    "fixture_package_proof_unavailable",
                    "No package proof and no strong-pin fallback.",
                ))
            }
        }
        fn inventory(&mut self, _: &[ApplicationIdentity]) -> Result<Inventory> {
            if self.inventory_cleanup {
                operation::resource_release_uncertain();
            }
            Ok(Inventory {
                identity: self.identity()?,
                windows: if self.launches > 0 {
                    self.after.clone()
                } else {
                    self.before.clone()
                },
                complete: !self.incomplete,
                native_windows_enumerated: if self.incomplete { 143 } else { 2 },
                captured_unix_ms: now_ms(),
                notes: if self.incomplete {
                    vec!["coverage_failures_by_code: fixture_birth_missing=3".into()]
                } else {
                    vec![]
                },
            })
        }
        fn begin_events(&mut self) -> Result<()> {
            Ok(())
        }
        fn events(&mut self) -> Result<EventBatch> {
            Ok(EventBatch {
                events: if self.launches > 0 {
                    self.events.clone()
                } else {
                    vec![]
                },
                complete: true,
                hooks_released: false,
            })
        }
        fn end_events(&mut self) -> Result<EventBatch> {
            self.ended += 1;
            let mut e = self.events()?;
            e.hooks_released = self.release;
            e.events.extend(self.late_events.clone());
            Ok(e)
        }
        fn launch(&mut self, _: &LaunchPermission) -> Result<NativeLaunch> {
            self.launches += 1;
            if self.panics {
                panic!("interrupted after reservation");
            }
            if let Some(op) = &self.cancel {
                op.cancel();
            }
            Ok(NativeLaunch {
                api: "synthetic-fixture".into(),
                accepted: true,
                process_id: Some(99),
                process_created: Some(999),
                process_handles_released: true,
                error: None,
            })
        }
        fn foreground(&mut self) -> Result<Option<WindowIdentity>> {
            Ok(self.foreground.clone())
        }
        fn user_input_tick(&mut self) -> Result<u64> {
            Ok(self.ticks.pop_front().unwrap_or(1))
        }
        fn check_window(&mut self, _: &WindowIdentity) -> Result<()> {
            Ok(())
        }
        fn activate(&mut self, w: &WindowIdentity) -> Result<bool> {
            self.activations += 1;
            if self.activation_cleanup {
                operation::resource_release_uncertain();
            }
            if self.activate_accept {
                self.foreground = Some(w.clone());
            }
            Ok(self.activate_accept)
        }
        fn pause(&mut self, ms: u64) {
            std::thread::sleep(Duration::from_millis(ms));
        }
    }
    fn run(root: &Path, g: &LifecycleGrant, d: &mut Fake, request: &str) -> Result<LaunchReceipt> {
        launch(
            root,
            g,
            d,
            LaunchRequest {
                authorization_id: "launch",
                request_id: request,
                timeout_ms: 1,
            },
        )
    }
    fn event(name: &str, n: u32) -> NativeEvent {
        NativeEvent {
            event: name.into(),
            window: window(n).key.window,
            event_time_ms: 1,
            thread_id: 3,
        }
    }
    fn packaged_grant() -> LifecycleGrant {
        let mut g = grant();
        g.launches[0].allowed_existing_windows = vec![window(1).key];
        g.launches[0].allowed_files = vec![AllowedFile {
            path: r"C:\run\baseline.txt".into(),
            sha256: "b".repeat(64),
        }];
        g.launches[0].observed_packages = vec![ObservedPackage {
            application: ApplicationIdentity { executable: r"C:\Program Files\WindowsApps\Name_1.0.0.0_arm64__publisher\Notepad\Notepad.exe".into(), sha256: "c".repeat(64) },
            package_full_name: "Name_1.0.0.0_arm64__publisher".into(),
            install_path: r"C:\Program Files\WindowsApps\Name_1.0.0.0_arm64__publisher".into(),
        }];
        g
    }
    #[test]
    fn explicit_package_role_does_not_call_strong_pin_for_observed_image() {
        let g = packaged_grant();
        let mut driver = Fake {
            package_enabled: true,
            ..Default::default()
        };
        let receipt = preflight(&g, &mut driver, "launch").unwrap();
        assert!(receipt.ready_for_launch && receipt.resources_released);
        assert_eq!(
            driver.verified_paths,
            [
                g.launches[0].application.executable.clone(),
                g.launches[0].allowed_files[0].path.clone()
            ]
        );
        assert_eq!(driver.package_calls, g.launches[0].observed_packages);
        assert_eq!(
            (driver.launches, driver.activations, driver.ended),
            (0, 0, 1)
        );
        assert!(!receipt.reservation_created && !receipt.goal_verified);
    }
    #[test]
    fn unavailable_package_proof_is_not_ignored_or_fallback_and_never_reserves() {
        let g = packaged_grant();
        let mut driver = Fake::default();
        let r = preflight(&g, &mut driver, "launch").unwrap();
        assert!(!r.ready_for_launch && r.inventory.is_none() && r.resources_released);
        let e = r.error.unwrap();
        assert_eq!(e.code, "fixture_package_proof_unavailable");
        assert_eq!(
            e.diagnostic.unwrap()["verification_context"]["role"],
            "observed_package"
        );
        assert_eq!(driver.verified_paths.len(), 2);
        let mut driver = Fake::default();
        let dir = tempfile::tempdir().unwrap();
        assert!(run(dir.path(), &g, &mut driver, "denied").is_err());
        assert_eq!((driver.launches, driver.ended), (0, 1));
        assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
    }
    #[test]
    fn package_role_does_not_weaken_launcher_or_allowed_file_denial() {
        let g = packaged_grant();
        for path in [
            &g.launches[0].application.executable,
            &g.launches[0].allowed_files[0].path,
        ] {
            let mut driver = Fake {
                package_enabled: true,
                pin_failure: Some((
                    path.clone(),
                    fault("native_pin_denied", "No role fallback."),
                )),
                ..Default::default()
            };
            let r = preflight(&g, &mut driver, "launch").unwrap();
            assert_eq!(r.error.unwrap().code, "native_pin_denied");
            assert!(driver.package_calls.is_empty());
            assert!(!r.ready_for_launch && !r.reservation_created && r.resources_released);
        }
    }
    #[test]
    fn package_grant_rejects_path_aliases_cross_role_collision_and_incomplete_scope() {
        let base = packaged_grant();
        base.validate(&identity(), now_ms()).unwrap();
        for variant in 0..9 {
            let mut g = base.clone();
            let p = &mut g.launches[0];
            match variant {
                0 => p.observed_packages[0].package_full_name.clear(),
                1 => p.observed_packages[0].package_full_name = "Name*".into(),
                2 => p.observed_packages[0].install_path = r"C:\OtherRoot".into(),
                3 => p.allowed_existing_windows.clear(),
                4 => p.application = p.observed_packages[0].application.clone(),
                5 => {
                    p.allowed_files[0].path =
                        format!(r"\\?\{}", p.observed_packages[0].application.executable)
                }
                6 => p
                    .observed_applications
                    .push(p.observed_packages[0].application.clone()),
                7 => p.observed_packages.push(p.observed_packages[0].clone()),
                _ => {
                    p.observed_packages[0].application.executable =
                        p.observed_packages[0].install_path.clone()
                }
            }
            assert!(
                g.validate(&identity(), now_ms()).is_err(),
                "variant={variant}"
            );
        }
    }
    #[test]
    fn old_launch_json_defaults_package_role_to_empty_without_wire_expansion() {
        let mut value = serde_json::to_value(&grant().launches[0]).unwrap();
        value.as_object_mut().unwrap().remove("observed_packages");
        let permission: LaunchPermission = serde_json::from_value(value.clone()).unwrap();
        assert!(permission.observed_packages.is_empty());
        assert_eq!(serde_json::to_value(permission).unwrap(), value);
    }
    #[test]
    fn native_pin_fault_keeps_exact_context_for_launcher_file_and_packaged_image() {
        let mut g = grant();
        g.launches[0].application.executable = r"C:\WINDOWS\System32\notepad.exe".into();
        g.launches[0].allowed_files = vec![AllowedFile {
            path: r"D:\Other Parent\case\baseline.txt".into(),
            sha256: "b".repeat(64),
        }];
        g.launches[0].observed_applications = vec![ApplicationIdentity {
            executable: r"C:\Program Files\WindowsApps\Microsoft.WindowsNotepad_11.2607.14.0_arm64__8wekyb3d8bbwe\Notepad\Notepad.exe".into(),
            sha256: "c".repeat(64),
        }];
        let roles = ["selected_launcher", "allowed_file", "observed_application"];
        let paths = [
            g.launches[0].application.executable.clone(),
            g.launches[0].allowed_files[0].path.clone(),
            g.launches[0].observed_applications[0].executable.clone(),
        ];
        for (index, path) in paths.iter().enumerate() {
            // Deliberately inject a native-format error; this does not execute a
            // Windows ACL check and cannot identify the old run's failing parent.
            let parent = path.rsplit_once('\\').unwrap().0;
            let original = serde_json::json!({"schema":"rcu.windows.pin.v1", "api":"CreateFileW", "stage":"open",
                "request":{"path":parent,"authorized_file":path,"desired_access":32,"share_mode":1,"flags":0x0220_0000},
                "native_error":{"hresult":"0x80070005","win32_code":5},
                "effective_token":{"status":"unknown","reason":"offline fixture"}});
            let mut error = fault(
                "application_directory_pin_failed",
                "Original native failure; no weaker retry.",
            );
            error.diagnostic = Some(Box::new(original.clone()));
            for phase in ["readonly_preflight", "launch_preflight"] {
                let mut d = Fake {
                    pin_failure: Some((path.clone(), error.clone())),
                    ..Default::default()
                };
                let returned = if phase == "readonly_preflight" {
                    let r = preflight(&g, &mut d, "launch").unwrap();
                    assert!(!r.ready_for_launch && !r.reservation_created && !r.launch_dispatched);
                    assert!(!r.activation_dispatched && !r.input_dispatched && !r.goal_verified);
                    assert!(
                        r.inventory.is_none()
                            && r.resources_released
                            && !r.resource_release_uncertain
                    );
                    r.error.unwrap()
                } else {
                    let dir = tempfile::tempdir().unwrap();
                    let error = run(dir.path(), &g, &mut d, "denied").unwrap_err();
                    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
                    error
                };
                assert_eq!(returned.code, "application_directory_pin_failed");
                assert_eq!(returned.next_step, error.next_step);
                let wire = serde_json::to_value(&returned).unwrap();
                let decoded: Fault = serde_json::from_value(wire).unwrap();
                let diagnostic = decoded.diagnostic.unwrap();
                for key in [
                    "schema",
                    "api",
                    "stage",
                    "request",
                    "native_error",
                    "effective_token",
                ] {
                    assert_eq!(diagnostic[key], original[key]);
                }
                assert_eq!(
                    diagnostic["verification_context"],
                    serde_json::json!({"phase":phase,"role":roles[index],"authorized_path":path})
                );
                assert_eq!(d.verified_paths, paths[..=index]);
                assert_eq!((d.launches, d.activations, d.ended), (0, 0, 1));
            }
        }
    }
    #[test]
    fn historical_fault_json_without_diagnostic_remains_readable() {
        let e: Fault =
            serde_json::from_value(serde_json::json!({"code":"old","next_step":"held"})).unwrap();
        assert!(e.diagnostic.is_none());
        assert_eq!(
            serde_json::to_value(e).unwrap(),
            serde_json::json!({"code":"old","next_step":"held"})
        );
    }
    #[test]
    fn launch_observation_is_not_goal_or_input_and_once_is_per_authorization() {
        let dir = tempfile::tempdir().unwrap();
        let g = grant();
        let mut d = Fake::default();
        let r = run(dir.path(), &g, &mut d, "one").unwrap();
        assert_eq!(r.state, LaunchState::NewWindowObserved);
        assert!(!r.goal_verified && !r.input_authority_granted);
        assert_eq!(r.native_launch.unwrap().process_id, Some(99));
        assert_eq!(r.candidates[0].pid, 10); // broker handoff != new PID
        assert!(r.resources_released);
        assert_eq!(d.ended, 1);
        let replay = run(dir.path(), &g, &mut d, "different-request").unwrap();
        assert_eq!(replay.replay, "forbidden_deduplicated");
        assert_eq!(d.launches, 1);
        let mut changed = g.clone();
        changed.launches[0]
            .arguments
            .push("not-authorized-originally".into());
        assert_eq!(
            run(dir.path(), &changed, &mut d, "third").unwrap_err().code,
            "idempotency_conflict"
        );
    }
    #[test]
    fn same_pid_new_hwnd_is_new_not_process_based() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = grant();
        g.launches[0].allowed_existing_windows = vec![window(1).key];
        let mut d = Fake {
            before: vec![window(1)],
            after: vec![window(1), window(2)],
            ..Default::default()
        };
        let r = run(dir.path(), &g, &mut d, "one").unwrap();
        assert_eq!(r.state, LaunchState::NewWindowObserved);
        assert_eq!(r.candidates, vec![window(2).key]);
    }
    #[test]
    fn same_hwnd_new_tab_remains_reused_and_authorized() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = grant();
        g.launches[0].allowed_existing_windows = vec![window(1).key];
        let mut d = Fake {
            before: vec![window(1)],
            events: vec![event("namechange", 1)],
            ..Default::default()
        };
        d.after[0].title = "new tab title".into();
        let r = run(dir.path(), &g, &mut d, "one").unwrap();
        assert_eq!(r.state, LaunchState::ReusedExistingWindow);
    }
    #[test]
    fn unauthorized_existing_window_blocks_without_launch() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Fake {
            before: vec![window(1)],
            ..Default::default()
        };
        assert_eq!(
            run(dir.path(), &grant(), &mut d, "one").unwrap_err().code,
            "existing_window_not_authorized"
        );
        assert_eq!(d.launches, 0);
        assert_eq!(d.ended, 1);
    }
    #[test]
    fn no_window_and_multiple_candidates_are_distinct() {
        for (after, expected) in [
            (vec![], LaunchState::LaunchedNoWindow),
            (vec![window(1), window(2)], LaunchState::Ambiguous),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut d = Fake {
                after,
                ..Default::default()
            };
            assert_eq!(
                run(dir.path(), &grant(), &mut d, "one").unwrap().state,
                expected
            );
        }
    }
    #[test]
    fn incomplete_inventory_is_not_absence() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Fake {
            incomplete: true,
            ..Default::default()
        };
        assert_eq!(
            run(dir.path(), &grant(), &mut d, "one").unwrap_err().code,
            "incomplete_window_inventory"
        );
        assert_eq!(d.launches, 0);
        assert_eq!(d.ended, 1);
    }
    #[test]
    fn session_change_and_cancellation_after_launch_are_uncertain() {
        for cancelled in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let op = operation::Registry::default().begin(None).unwrap();
            let mut d = Fake {
                session_changes: !cancelled,
                cancel: cancelled.then_some(op.clone()),
                ..Default::default()
            };
            let r = operation::run(&op, || run(dir.path(), &grant(), &mut d, "one")).unwrap();
            assert_eq!(r.state, LaunchState::Uncertain);
            assert_eq!(d.launches, 1);
            assert!(r.resources_released);
            assert_eq!(d.ended, 1);
        }
    }
    #[test]
    fn interrupted_dispatch_retains_durable_no_replay_and_unhooks() {
        let dir = tempfile::tempdir().unwrap();
        let g = grant();
        let mut d = Fake {
            panics: true,
            ..Default::default()
        };
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run(dir.path(), &g, &mut d, "one")
        }));
        assert!(panic.is_err());
        assert_eq!(d.ended, 1);
        d.panics = false;
        let r = run(dir.path(), &g, &mut d, "another").unwrap();
        assert_eq!(r.state, LaunchState::Uncertain);
        assert_eq!(d.launches, 1);
        assert_eq!(
            receipt(dir.path(), &g, "launch", "launch", "one").unwrap()["state"],
            "uncertain"
        );
    }
    #[test]
    fn failed_cleanup_never_becomes_released_at_worker_finish() {
        let dir = tempfile::tempdir().unwrap();
        let op = operation::Registry::default().begin(None).unwrap();
        let mut d = Fake {
            release: false,
            ..Default::default()
        };
        let r = operation::run(&op, || run(dir.path(), &grant(), &mut d, "one")).unwrap();
        op.finish();
        assert_eq!(r.state, LaunchState::Uncertain);
        assert!(!r.resources_released);
        assert!(op.status().terminal);
        assert!(!op.status().resources_released);
    }
    #[test]
    fn grant_and_paths_reject_ambiguous_scope() {
        let now = now_ms();
        let mut g = grant();
        assert!(g.validate(&identity(), now).is_ok());
        for p in [
            r"relative.exe",
            r"C:app.exe",
            r"\\server\share\a.exe",
            r"C:\a\..\b.exe",
            r"C:\a.exe:ads",
            r"C:\a.\b.exe",
        ] {
            assert!(!valid_windows_path(p), "{p}");
        }
        g.launches[0].application_startup_effects_authorized = false;
        assert!(g.validate(&identity(), now).is_err());
        let mut g = grant();
        g.expires_unix_ms = now + 3_600_001;
        assert!(g.validate(&identity(), now).is_err());
        let mut g = grant();
        g.expected_identity.desktop = "other".into();
        assert_eq!(
            g.validate(&identity(), now).unwrap_err().code,
            "session_mismatch"
        );
        let mut v = serde_json::to_value(grant()).unwrap();
        v["automatic_input_grant"] = true.into();
        assert!(serde_json::from_value::<LifecycleGrant>(v).is_err());
    }
    #[test]
    fn activation_is_explicit_once_and_never_grants_input() {
        let dir = tempfile::tempdir().unwrap();
        let g = grant();
        let mut d = Fake::default();
        let request = || ActivateRequest {
            authorization_id: "activate",
            request_id: "one",
            expected_foreground: &None,
        };
        let r = activate(dir.path(), &g, &mut d, request()).unwrap();
        assert_eq!(r.state, "foreground_observed");
        assert!(!r.goal_verified && !r.input_authority_granted);
        assert_eq!(d.activations, 1);
        activate(dir.path(), &g, &mut d, request()).unwrap();
        assert_eq!(d.activations, 1);
    }
    #[test]
    fn foreground_change_or_user_input_yields_without_retry() {
        let dir = tempfile::tempdir().unwrap();
        let g = grant();
        let mut d = Fake {
            foreground: Some(window(2).key),
            ..Default::default()
        };
        let request = || ActivateRequest {
            authorization_id: "activate",
            request_id: "one",
            expected_foreground: &None,
        };
        assert_eq!(
            activate(dir.path(), &g, &mut d, request())
                .unwrap_err()
                .code,
            "foreground_changed"
        );
        assert_eq!(d.activations, 0);
        d.foreground = None;
        d.ticks = [1, 2].into();
        let r = activate(dir.path(), &g, &mut d, request()).unwrap();
        assert_eq!(r.state, "yielded");
        assert_eq!(d.activations, 0);
    }
    #[test]
    fn activation_refusal_or_post_input_is_not_goal() {
        for accept in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let mut d = Fake {
                activate_accept: accept,
                ticks: if accept {
                    [1, 1, 2].into()
                } else {
                    Default::default()
                },
                ..Default::default()
            };
            let r = activate(
                dir.path(),
                &grant(),
                &mut d,
                ActivateRequest {
                    authorization_id: "activate",
                    request_id: "one",
                    expected_foreground: &None,
                },
            )
            .unwrap();
            assert_eq!(r.state, if accept { "yielded" } else { "not_foreground" });
            assert_eq!(d.activations, 1);
            assert!(!r.goal_verified);
        }
    }
    #[test]
    fn expired_dispatch_deadline_does_not_launch() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Fake::default();
        let result =
            operation::with_dispatch_deadline(Instant::now() - Duration::from_millis(1), || {
                run(dir.path(), &grant(), &mut d, "one")
            });
        assert!(result.is_err());
        assert_eq!(d.launches, 0);
    }
    #[test]
    fn same_handle_destroy_create_cannot_be_called_existing_window_reuse() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = grant();
        g.launches[0].allowed_existing_windows = vec![window(1).key];
        let mut d = Fake {
            before: vec![window(1)],
            events: vec![event("destroy", 1), event("create", 1)],
            ..Default::default()
        };
        assert_eq!(
            run(dir.path(), &g, &mut d, "one").unwrap().state,
            LaunchState::Ambiguous
        );
    }
    #[test]
    fn final_unhook_events_participate_in_reuse_classification() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = grant();
        g.launches[0].allowed_existing_windows = vec![window(1).key];
        let mut d = Fake {
            before: vec![window(1)],
            events: vec![event("namechange", 1)],
            late_events: vec![event("destroy", 1), event("create", 1)],
            ..Default::default()
        };
        assert_eq!(
            run(dir.path(), &g, &mut d, "one").unwrap().state,
            LaunchState::Ambiguous
        );
    }
    #[test]
    fn equivalent_vm_uuid_spellings_cannot_replay_launch() {
        let dir = tempfile::tempdir().unwrap();
        let mut g = grant();
        g.route_vm_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into();
        let mut d = Fake::default();
        run(dir.path(), &g, &mut d, "one").unwrap();
        for route in [
            g.route_vm_id.to_uppercase(),
            format!("{{{}}}", g.route_vm_id),
            format!("urn:uuid:{}", g.route_vm_id),
        ] {
            let mut same = g.clone();
            same.route_vm_id = route;
            assert!(same.validate(&identity(), now_ms()).is_err());
            let r = run(dir.path(), &same, &mut d, "changed-format").unwrap();
            assert_eq!(r.replay, "forbidden_deduplicated");
            assert_eq!(d.launches, 1);
        }
    }
    #[test]
    fn receipt_reader_rejects_mismatched_final_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let g = grant();
        let mut d = Fake::default();
        run(dir.path(), &g, &mut d, "one").unwrap();
        let path =
            journal_path(dir.path(), &g, "launch", "launch", "one").with_extension("result.json");
        let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        v["fingerprint"] = "different-reservation".into();
        fs::write(path, serde_json::to_vec(&v).unwrap()).unwrap();
        assert_eq!(
            receipt(dir.path(), &g, "launch", "launch", "one")
                .unwrap_err()
                .code,
            "uncertain_journal"
        );
        assert_eq!(d.launches, 1);
    }
    #[test]
    fn direct_launch_without_registry_still_captures_cleanup_failure() {
        let dir = tempfile::tempdir().unwrap();
        let g = grant();
        let mut d = Fake {
            inventory_cleanup: true,
            ..Default::default()
        };
        let r = run(dir.path(), &g, &mut d, "one").unwrap();
        assert!(!r.resources_released && r.resource_release_uncertain);
        assert_eq!(r.state, LaunchState::Uncertain);
        assert_eq!(d.launches, 0);
        let saved = receipt(dir.path(), &g, "launch", "launch", "one").unwrap();
        assert_eq!(saved["resources_released"], false);
        assert_eq!(saved["resource_release_uncertain"], true);
    }
    #[test]
    fn activation_primary_and_durable_receipt_match_cleanup_status() {
        let dir = tempfile::tempdir().unwrap();
        let g = grant();
        let op = operation::Registry::default().begin(None).unwrap();
        let mut d = Fake {
            activation_cleanup: true,
            ..Default::default()
        };
        let r = operation::run(&op, || {
            activate(
                dir.path(),
                &g,
                &mut d,
                ActivateRequest {
                    authorization_id: "activate",
                    request_id: "one",
                    expected_foreground: &None,
                },
            )
        })
        .unwrap();
        op.finish();
        assert_eq!(d.activations, 1);
        assert_eq!(r.state, "uncertain");
        assert!(!r.resources_released && r.resource_release_uncertain);
        assert!(!op.status().resources_released);
        let saved = receipt(dir.path(), &g, "activation", "activate", "one").unwrap();
        assert_eq!(saved["resources_released"], false);
        assert_eq!(saved["resource_release_uncertain"], true);
    }
    #[test]
    fn readonly_preflight_preserves_partial_coverage_and_never_dispatches() {
        let mut d = Fake {
            incomplete: true,
            ..Default::default()
        };
        let r = preflight(&grant(), &mut d, "launch").unwrap();
        assert!(!r.ready_for_launch);
        assert_eq!(
            r.error.as_ref().unwrap().code,
            "incomplete_window_inventory"
        );
        let i = r.inventory.unwrap();
        assert_eq!(i.native_windows_enumerated, 143);
        assert!(!i.complete);
        assert!(i.notes[0].contains("fixture_birth_missing=3"));
        assert_eq!((d.launches, d.activations, d.ended), (0, 0, 1));
        assert!(r.resources_released);
        assert!(
            !r.reservation_created
                && !r.launch_dispatched
                && !r.activation_dispatched
                && !r.input_dispatched
                && !r.goal_verified
        );
    }
    #[test]
    fn preflight_cleanup_uncertainty_is_explicit_before_any_reservation() {
        for hash_error in [false, true] {
            let op = operation::Registry::default().begin(None).unwrap();
            let mut d = Fake {
                verify_failure: hash_error,
                inventory_cleanup: !hash_error,
                release: !hash_error,
                ..Default::default()
            };
            let r = operation::run(&op, || preflight(&grant(), &mut d, "launch")).unwrap();
            op.finish();
            assert!(!r.ready_for_launch && !r.resources_released && r.resource_release_uncertain);
            assert!(!op.status().resources_released);
            assert_eq!((d.launches, d.activations), (0, 0));
            assert!(serde_json::to_vec(&r).unwrap().len() < MAX_METADATA_BYTES);
        }
    }
    #[test]
    fn preflight_success_is_only_readiness_not_a_launch_or_goal() {
        let mut d = Fake::default();
        let r = preflight(&grant(), &mut d, "launch").unwrap();
        assert!(r.ready_for_launch && r.resources_released);
        assert_eq!((d.launches, d.activations), (0, 0));
        assert!(!r.reservation_created && !r.goal_verified);
    }

    #[test]
    fn launch_precommit_error_keeps_cleanup_failure_without_a_reservation() {
        let dir = tempfile::tempdir().unwrap();
        let op = operation::Registry::default().begin(None).unwrap();
        let mut d = Fake {
            verify_failure: true,
            release: false,
            ..Default::default()
        };
        let error = operation::run(&op, || run(dir.path(), &grant(), &mut d, "one")).unwrap_err();
        op.finish();
        assert_eq!(error.code, "fixture_hash_mismatch");
        assert_eq!(d.launches, 0);
        assert_eq!(d.ended, 1);
        assert!(!op.status().resources_released && op.status().resource_release_uncertain);
        assert!(!dir.path().join("lifecycle").exists());
    }
    #[test]
    fn activation_precommit_error_keeps_cleanup_failure_without_a_reservation() {
        let dir = tempfile::tempdir().unwrap();
        let op = operation::Registry::default().begin(None).unwrap();
        let mut d = Fake {
            foreground: Some(window(2).key),
            release: false,
            ..Default::default()
        };
        let error = operation::run(&op, || {
            activate(
                dir.path(),
                &grant(),
                &mut d,
                ActivateRequest {
                    authorization_id: "activate",
                    request_id: "one",
                    expected_foreground: &None,
                },
            )
        })
        .unwrap_err();
        op.finish();
        assert_eq!(error.code, "foreground_changed");
        assert_eq!(d.activations, 0);
        assert_eq!(d.ended, 1);
        assert!(!op.status().resources_released && op.status().resource_release_uncertain);
        assert!(!dir.path().join("lifecycle").exists());
    }
    #[test]
    fn oversized_preflight_keeps_counts_and_explicit_partial_diagnostics() {
        let mut d = Fake {
            before: (1..=40)
                .map(|n| {
                    let mut w = window(n);
                    w.title = "x".repeat(4096);
                    w
                })
                .collect(),
            incomplete: true,
            ..Default::default()
        };
        let r = preflight(&grant(), &mut d, "launch").unwrap();
        assert!(!r.ready_for_launch);
        assert!(serde_json::to_vec(&r).unwrap().len() < MAX_METADATA_BYTES);
        let i = r.inventory.unwrap();
        assert!(!i.complete);
        assert_eq!(i.native_windows_enumerated, 143);
        assert!(i.notes.iter().any(|n| n.contains("truncated")));
        assert_eq!((d.launches, d.activations), (0, 0));
    }
}
