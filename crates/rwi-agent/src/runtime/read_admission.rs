//! Bounded serialization for exact-window observation and its preview, never input.
use super::*;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub(super) const LIMIT: usize = 8;
pub(super) const WAIT: Duration = Duration::from_secs(5);
const CHECK_INTERVAL: Duration = Duration::from_millis(20);

pub(super) fn eligible(tool: &str, writes: bool) -> bool {
    !writes
        && matches!(
            tool,
            "computer.observe" | "computer.capture_visual" | "computer.image"
        )
}

fn check(operation: &Operation, closed: &AtomicBool) -> Result<(), DeviceResult> {
    if closed.load(Ordering::Acquire) {
        return Err(DeviceResult::error(
            "context_closed",
            "No dispatch after context closure.",
        ));
    }
    rwi_core::operation::run(operation, || {
        rwi_core::operation::checkpoint("read_admission")
    })
    .map_err(|error| DeviceResult::error(&error.code, &error.next_step))
}

pub(super) async fn acquire(
    gate: Arc<Semaphore>,
    waiters: Arc<Semaphore>,
    can_wait: bool,
    operation: &Operation,
    closed: &AtomicBool,
    budget: Duration,
) -> Result<OwnedSemaphorePermit, DeviceResult> {
    check(operation, closed)?;
    if let Ok(permit) = gate.clone().try_acquire_owned() {
        return Ok(permit);
    }
    if !can_wait {
        return Err(DeviceResult::error(
            "context_busy",
            "Only this context is busy; this execution was not dispatched.",
        ));
    }
    let _queued = waiters.try_acquire_owned().map_err(|_| {
        DeviceResult::error(
            "context_busy",
            "Bounded observation queue is full; this execution was not dispatched.",
        )
    })?;
    let pending = gate.acquire_owned();
    let deadline = tokio::time::sleep(budget);
    tokio::pin!(pending, deadline);
    loop {
        check(operation, closed)?;
        tokio::select! {
            result = &mut pending => {
                let permit = result.map_err(|_| DeviceResult::error("context_closed", "Observation gate was closed; no dispatch."))?;
                check(operation, closed)?;
                return Ok(permit);
            }
            _ = &mut deadline => return Err(DeviceResult::error("context_busy", "Bounded observation wait expired before dispatch; no native work was started.")),
            _ = tokio::time::sleep(CHECK_INTERVAL) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The standard fixture has no observation PNG. This synthetic wrapper adds
    // its existing fixed visual PNG so the composite preview path is exercised.
    #[cfg(feature = "fixture")]
    struct RasterFixture(rwi_core::fixture::Fixture);
    #[cfg(feature = "fixture")]
    impl rwi_core::Backend for RasterFixture {
        fn identity(&self) -> rwi_core::Identity {
            self.0.identity()
        }
        fn capabilities(&self) -> serde_json::Value {
            self.0.capabilities()
        }
        fn observe(&mut self) -> rwi_core::Result<rwi_core::Scene> {
            let mut scene = self.0.observe()?;
            let visual = self.0.capture_visual(rwi_core::VisualOptions::default())?;
            scene.image_size = visual.image_size;
            scene.png = visual.png;
            Ok(scene)
        }
        fn act(
            &mut self,
            binding: &rwi_core::Binding,
            resolved: &rwi_core::Resolved,
            intent: &rwi_core::Intent,
            value: Option<&str>,
        ) -> rwi_core::Result<Option<bool>> {
            self.0.act(binding, resolved, intent, value)
        }
        fn inspect(
            &mut self,
            scope: &rwi_core::Scope,
            predicate: &rwi_core::Predicate,
            expected: &str,
        ) -> rwi_core::Result<rwi_core::Oracle> {
            self.0.inspect(scope, predicate, expected)
        }
    }
    fn operation() -> Operation {
        Registry::default().begin(None).unwrap()
    }
    #[cfg(feature = "fixture")]
    #[tokio::test]
    async fn read_admission_runtime_burst_completes_without_input_or_leaked_resources() {
        use super::super::dispatch_tests::{invoke, tracked_backend_context};
        let temp = tempfile::tempdir().unwrap();
        let principal = Principal {
            journal_identity: None,
            certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
            observe: true,
            input: true,
        };
        let runtime = Runtime::new(Config {
            version: 1,
            device_id: "device".into(),
            bind: "127.0.0.1:0".parse().unwrap(),
            identity: rwi_link_trust::IdentityPaths::in_profile(temp.path()),
            principals: vec![principal.clone()],
            state_root: temp.path().join("state"),
            platform: crate::config::PlatformPolicy::Fixture,
        })
        .unwrap();
        let fixture = rwi_core::fixture::Fixture::default();
        let (token, target) = tracked_backend_context(
            &runtime,
            &principal,
            Box::new(RasterFixture(fixture.clone())),
        )
        .await;
        let context = runtime.context(principal.identity(), Some(&token)).unwrap();
        let native = context.gate.clone().acquire_owned().await.unwrap();
        let input = context.domain.gate.clone().acquire_owned().await.unwrap();
        let mut pending = Vec::new();
        for _ in 0..LIMIT {
            let (runtime, principal, token, target) = (
                runtime.clone(),
                principal.clone(),
                token.clone(),
                target.clone(),
            );
            pending.push(tokio::spawn(async move {
                let observation = invoke(
                    &runtime,
                    &principal,
                    &token,
                    "computer.observe",
                    json!({"target":target,"inline_image":false}),
                )
                .await;
                assert!(!observation.is_error, "{:?}", observation.data);
                // Snapshot is composite: its preview must also wait rather than
                // failing between two successful, serialized observations.
                invoke(
                    &runtime,
                    &principal,
                    &token,
                    "computer.image",
                    json!({"target":target,"frame":observation.data["frame"],
                        "artifact_id":observation.data["artifact_id"],"max_edge":512}),
                )
                .await
            }));
        }
        tokio::time::timeout(Duration::from_secs(2), async {
            while context.read_waiters.available_permits() != 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("all bounded readers reached admission");
        drop(native);
        for task in pending {
            let result = task.await.unwrap();
            assert!(!result.is_error, "{:?}", result.data);
            assert_eq!(result.data["operation"]["terminal"], true);
            assert_eq!(result.data["operation"]["resources_released"], true);
            assert_eq!(result.data["operation"]["native_call_pending"], false);
        }
        assert_eq!(fixture.0.lock().unwrap().calls, 0, "no input sent");
        assert_eq!(
            context.domain.gate.available_permits(),
            0,
            "input permit untouched"
        );
        drop(input);
        assert_eq!(context.gate.available_permits(), 1);
        assert_eq!(context.read_waiters.available_permits(), LIMIT);
    }
    #[test]
    fn read_admission_is_an_explicit_observation_allowlist_not_a_write_flag_shortcut() {
        for name in [
            "computer.observe",
            "computer.capture_visual",
            "computer.image",
        ] {
            assert!(eligible(name, false));
            assert!(!eligible(name, true));
        }
        for name in [
            "computer.act",
            "computer.bind_window",
            "computer.claim",
            "computer.recover_input",
            "computer.live",
            "unknown",
        ] {
            assert!(!eligible(name, false));
        }
    }
    #[tokio::test]
    async fn read_admission_waits_once_for_its_context_without_acquiring_input() {
        let gate = Arc::new(Semaphore::new(1));
        let held = gate.clone().acquire_owned().await.unwrap();
        let slots = Arc::new(Semaphore::new(LIMIT));
        let op = operation();
        let closed = AtomicBool::new(false);
        let acquire = acquire(gate.clone(), slots.clone(), true, &op, &closed, WAIT);
        let release = async {
            tokio::time::sleep(Duration::from_millis(25)).await;
            drop(held);
        };
        let (result, ()) = tokio::join!(acquire, release);
        let permit = result.unwrap();
        assert_eq!(gate.available_permits(), 0);
        assert_eq!(slots.available_permits(), LIMIT);
        assert!(!op.status().native_call_pending);
        assert!(
            !op.status().terminal,
            "admission does not finish its caller's operation"
        );
        drop(permit);
        op.finish();
        assert!(op.status().terminal && op.status().resources_released);
        assert_eq!(gate.available_permits(), 1);
    }
    #[tokio::test]
    async fn read_admission_writes_recovery_and_binding_remain_immediate_busy() {
        let gate = Arc::new(Semaphore::new(0));
        let slots = Arc::new(Semaphore::new(LIMIT));
        let result = acquire(
            gate,
            slots.clone(),
            false,
            &operation(),
            &AtomicBool::new(false),
            WAIT,
        )
        .await;
        assert_eq!(result.unwrap_err().data["code"], "context_busy");
        assert_eq!(slots.available_permits(), LIMIT);
    }
    #[tokio::test]
    async fn read_admission_queue_capacity_and_wait_are_bounded() {
        let gate = Arc::new(Semaphore::new(0));
        let slots = Arc::new(Semaphore::new(0));
        let result = acquire(
            gate.clone(),
            slots,
            true,
            &operation(),
            &AtomicBool::new(false),
            WAIT,
        )
        .await;
        assert_eq!(result.unwrap_err().data["code"], "context_busy");
        let slots = Arc::new(Semaphore::new(LIMIT));
        let result = acquire(
            gate,
            slots.clone(),
            true,
            &operation(),
            &AtomicBool::new(false),
            Duration::from_millis(1),
        )
        .await;
        assert_eq!(result.unwrap_err().data["code"], "context_busy");
        assert_eq!(slots.available_permits(), LIMIT);
    }
    #[tokio::test]
    async fn read_admission_cancel_and_close_release_waiters_without_native_calls() {
        for cancel in [true, false] {
            let gate = Arc::new(Semaphore::new(0));
            let slots = Arc::new(Semaphore::new(LIMIT));
            let op = operation();
            let closed = AtomicBool::new(false);
            let acquire = acquire(gate, slots.clone(), true, &op, &closed, WAIT);
            let interrupt = async {
                tokio::time::sleep(Duration::from_millis(1)).await;
                if cancel {
                    op.cancel();
                } else {
                    closed.store(true, Ordering::Release);
                }
            };
            let (result, ()) = tokio::join!(acquire, interrupt);
            assert!(result.is_err());
            assert_eq!(slots.available_permits(), LIMIT);
            assert!(!op.status().native_call_pending);
        }
    }
    #[tokio::test]
    async fn read_admission_abandoned_future_drops_its_queue_slot() {
        let gate = Arc::new(Semaphore::new(0));
        let slots = Arc::new(Semaphore::new(LIMIT));
        let op = operation();
        let closed = AtomicBool::new(false);
        {
            let wait = acquire(gate, slots.clone(), true, &op, &closed, WAIT);
            tokio::pin!(wait);
            tokio::select! {_=&mut wait=>panic!("must wait"),_=tokio::time::sleep(Duration::from_millis(1))=>{}}
            assert_eq!(slots.available_permits(), LIMIT - 1);
        }
        assert_eq!(slots.available_permits(), LIMIT);
    }
}
