//! Independent review probes. Pure Rust fake driver; no native OS calls.
use rwi_core::{Identity, Result, lifecycle::*, operation};

fn identity() -> Identity {
    Identity {
        host: "parent-offline".into(),
        guest: None,
        session: "s".into(),
        seat: "seat".into(),
        desktop: "d".into(),
    }
}
fn application() -> ApplicationIdentity {
    ApplicationIdentity {
        executable: r"C:\fixture\app.exe".into(),
        sha256: "a".repeat(64),
    }
}
struct Fake {
    launched: bool,
}
impl LifecycleDriver for Fake {
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
        // Windows process-image inventory closes temporary native handles via
        // Drop, which reports failure through this operation-level function.
        operation::resource_release_uncertain();
        let windows = if self.launched {
            vec![WindowEvidence {
                key: WindowIdentity {
                    window: "hwnd:123".into(),
                    pid: 42,
                    process_created: 100,
                    native_epoch: "epoch".into(),
                },
                application: application(),
                title: "fixture".into(),
                foreground: false,
                candidate_facts: Some(WindowCandidateFacts {
                    visible: true,
                    cloaked: Some(false),
                    owned: false,
                    owner_chain_complete: true,
                }),
            }]
        } else {
            vec![]
        };
        Ok(Inventory {
            identity: identity(),
            windows,
            complete: true,
            native_windows_enumerated: 1,
            captured_unix_ms: now_ms(),
            notes: vec![],
        })
    }
    fn begin_events(&mut self) -> Result<()> {
        Ok(())
    }
    fn events(&mut self) -> Result<EventBatch> {
        Ok(EventBatch {
            events: vec![],
            complete: true,
            hooks_released: false,
        })
    }
    fn end_events(&mut self) -> Result<EventBatch> {
        Ok(EventBatch {
            events: vec![],
            complete: true,
            hooks_released: true,
        })
    }
    fn launch(&mut self, _: &LaunchPermission) -> Result<NativeLaunch> {
        self.launched = true;
        Ok(NativeLaunch {
            api: "offline-fake".into(),
            accepted: true,
            process_id: Some(42),
            process_created: Some(100),
            process_handles_released: true,
            error: None,
        })
    }
    fn foreground(&mut self) -> Result<Option<WindowIdentity>> {
        Ok(None)
    }
    fn user_input_tick(&mut self) -> Result<u64> {
        Ok(0)
    }
    fn check_window(&mut self, _: &WindowIdentity) -> Result<()> {
        Ok(())
    }
    fn activate(&mut self, _: &WindowIdentity) -> Result<bool> {
        Ok(false)
    }
    fn pause(&mut self, _: u64) {}
}

#[test]
fn launch_receipt_must_preserve_inventory_handle_release_uncertainty() {
    let grant = LifecycleGrant {
        run_id: "parent-offline-review".into(),
        route_vm_id: "00000000-0000-0000-0000-000000000001".into(),
        expected_identity: identity(),
        expires_unix_ms: now_ms() + 60_000,
        launches: vec![LaunchPermission {
            authorization_id: "launch".into(),
            application: application(),
            arguments: vec![],
            allowed_files: vec![],
            observed_applications: vec![],
            observed_packages: vec![],
            allowed_existing_windows: vec![],
            application_startup_effects_authorized: true,
        }],
        activations: vec![],
    };
    let dir = tempfile::tempdir().unwrap();
    let registry = operation::Registry::default();
    let op = registry
        .begin(Some("parent-resource-probe".into()))
        .unwrap();
    let mut driver = Fake { launched: false };
    let receipt = operation::run(&op, || {
        rwi_core::lifecycle::launch(
            dir.path(),
            &grant,
            &mut driver,
            LaunchRequest {
                authorization_id: "launch",
                request_id: "request",
                timeout_ms: 1,
            },
        )
    })
    .unwrap();
    op.finish();
    assert!(op.status().resource_release_uncertain);
    assert!(!op.status().resources_released);
    assert!(
        !receipt.resources_released,
        "launch receipt falsely says resources released: receipt={receipt:?}, operation={:?}",
        op.status()
    );
}

#[test]
fn nested_native_call_must_keep_pending_until_outer_guard_ends() {
    let op = operation::Registry::default()
        .begin(Some("parent-nested-probe".into()))
        .unwrap();
    operation::run(&op, || {
        let outer = operation::native_call("outer").unwrap();
        let inner = operation::native_call("inner").unwrap();
        drop(inner);
        assert!(
            op.status().native_call_pending,
            "outer native scope is alive but pending=false"
        );
        drop(outer);
    });
}
