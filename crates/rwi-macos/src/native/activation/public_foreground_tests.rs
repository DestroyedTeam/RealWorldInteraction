use super::*;
use std::{cell::Cell, sync::atomic::AtomicU8};
#[test]
fn foreground_public_nil_and_nonpositive_pid_refuse_before_ax() {
    for raw in [None, Some(-1), Some(0), Some(i32::MIN)] {
        let ax_reads = Cell::new(0);
        let result = positive_public_pid(raw).inspect(|_| ax_reads.set(ax_reads.get() + 1));
        assert_eq!(result.unwrap_err().code, "foreground_identity_unknown");
        assert_eq!(ax_reads.get(), 0);
    }
    assert_eq!(positive_public_pid(Some(17)).unwrap(), 17);
}
fn stable() -> PublicForegroundReadback {
    PublicForegroundReadback {
        original_pid: 17,
        public_pid: 17,
        ax_pid: 17,
        birth_unchanged: true,
        public_application_equal: Some(true),
        ax_application_equal: true,
        ax_window_equal: true,
        geometry_equal: true,
    }
}
#[test]
fn foreground_public_independent_identity_axes_all_fail_closed() {
    require_public_foreground_unchanged(stable()).unwrap();
    for axis in 0..8 {
        let mut read = stable();
        match axis {
            0 => read.public_pid = 18,
            1 => read.ax_pid = 18,
            2 => read.birth_unchanged = false,
            3 => read.public_application_equal = None,
            4 => read.public_application_equal = Some(false),
            5 => read.ax_application_equal = false,
            6 => read.ax_window_equal = false,
            _ => read.geometry_equal = false,
        }
        assert_eq!(
            require_public_foreground_unchanged(read).unwrap_err().code,
            "foreground_changed"
        );
    }
}
#[test]
fn foreground_public_cancelled_expired_or_duplicate_queue_reads_never_run() {
    let reads = Cell::new(0);
    let job = PublicForegroundRead {
        deadline: Instant::now() + Duration::from_secs(1),
        phase: AtomicU8::new(PUBLIC_READ_PENDING),
    };
    job.cancel();
    assert!(
        job.execute(|| {
            reads.set(reads.get() + 1);
            Ok(17)
        })
        .is_err()
    );
    let expired = PublicForegroundRead {
        deadline: Instant::now() - Duration::from_millis(1),
        phase: AtomicU8::new(PUBLIC_READ_PENDING),
    };
    assert!(
        expired
            .execute(|| {
                reads.set(reads.get() + 1);
                Ok(17)
            })
            .is_err()
    );
    assert_eq!(reads.get(), 0);
    let valid = PublicForegroundRead {
        deadline: Instant::now() + Duration::from_secs(1),
        phase: AtomicU8::new(PUBLIC_READ_PENDING),
    };
    assert_eq!(
        valid
            .execute(|| {
                reads.set(reads.get() + 1);
                Ok(17)
            })
            .unwrap(),
        17
    );
    assert!(
        valid
            .execute(|| {
                reads.set(reads.get() + 1);
                Ok(18)
            })
            .is_err()
    );
    assert_eq!(reads.get(), 1);
    // The UI read can complete in time while the waiter resumes too late.
    assert!(expired.accept(Ok(17)).is_err());
    valid.cancel();
    assert!(valid.accept(Ok(17)).is_err());
    let during = PublicForegroundRead {
        deadline: Instant::now() + Duration::from_secs(1),
        phase: AtomicU8::new(PUBLIC_READ_PENDING),
    };
    assert!(
        during
            .execute(|| {
                during.cancel();
                Ok(17)
            })
            .is_err()
    );
}
#[test]
fn foreground_public_queued_native_scope_retains_original_operation() {
    fn send<T: Send>() {}
    send::<PublicForegroundApplication>();
    send::<operation::NativeCall>();
    let slots = std::sync::atomic::AtomicUsize::new(0);
    for _ in 0..PUBLIC_FOREGROUND_QUEUE_LIMIT {
        acquire_public_foreground_slot(&slots).unwrap();
    }
    assert_eq!(
        acquire_public_foreground_slot(&slots).unwrap_err().code,
        "foreground_workspace_queue_busy"
    );
    assert_eq!(slots.load(std::sync::atomic::Ordering::Acquire), 4);
    release_public_foreground_slot(&slots);
    acquire_public_foreground_slot(&slots).unwrap();
    assert_eq!(slots.load(std::sync::atomic::Ordering::Acquire), 4);
    let registry = operation::Registry::default();
    let op = registry.begin(None).unwrap();
    let queued = operation::run(&op, || {
        operation::native_call("offline_queued_read").unwrap()
    });
    op.finish();
    assert!(op.status().native_call_pending);
    assert!(!op.status().resources_released);
    drop(queued);
    assert!(op.status().terminal && op.status().resources_released);
}
#[test]
fn foreground_public_cancel_before_start_cas_never_reads() {
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    };
    let at_start = Arc::new(Barrier::new(2));
    let cancelled = Arc::new(Barrier::new(2));
    let reads = Arc::new(AtomicUsize::new(0));
    let job = Arc::new(PublicForegroundRead {
        deadline: Instant::now() + Duration::from_secs(5),
        phase: AtomicU8::new(PUBLIC_READ_PENDING),
    });
    let worker = job.clone();
    let first = at_start.clone();
    let second = cancelled.clone();
    let count = reads.clone();
    let result = std::thread::spawn(move || {
        worker.execute_after_gate(
            || {
                first.wait();
                second.wait();
            },
            || {
                count.fetch_add(1, Ordering::AcqRel);
                Ok(17)
            },
        )
    });
    at_start.wait();
    job.cancel(); // cancel has completed before the worker's start CAS.
    assert_eq!(job.phase.load(Ordering::Acquire), PUBLIC_READ_CANCELLED);
    cancelled.wait();
    assert_eq!(
        result.join().unwrap().unwrap_err().code,
        "foreground_workspace_read_expired"
    );
    assert_eq!(reads.load(Ordering::Acquire), 0);
    assert_eq!(job.phase.load(Ordering::Acquire), PUBLIC_READ_CANCELLED);
}
