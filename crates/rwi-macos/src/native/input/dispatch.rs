//! A batch is cancellable before down, never between an allocated down/up pair.
use rwi_core::{Result, operation};
use std::sync::atomic::{AtomicU64, Ordering};
pub(super) fn count(counter: &AtomicU64) {
    let _ = counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
        Some(n.saturating_add(1))
    });
}
pub(super) fn pair<T>(
    counter: &AtomicU64,
    down: T,
    up: T,
    check: impl FnOnce() -> Result<()>,
    mut post: impl FnMut(&T),
) -> Result<()> {
    check()?;
    let _call = operation::native_call("macos_input_pair")?;
    count(counter);
    post(&down);
    // The release is cleanup of this exact allocated down event, not a new intent.
    // No cancellation/lease checkpoint can strand the key or mouse button here.
    count(counter);
    post(&up);
    Ok(())
}
pub(super) fn single(counter: &AtomicU64, post: impl FnOnce()) -> Result<()> {
    let _call = operation::native_call("macos_scroll_event")?;
    count(counter);
    post();
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancel_before_down_has_zero_posts_but_cancel_after_down_keeps_release() {
        for before in [true, false] {
            let op = operation::Registry::default().begin(None).unwrap();
            if before {
                op.cancel();
            }
            let counter = AtomicU64::new(0);
            let mut posts = vec![];
            let result = operation::run(&op, || {
                pair(
                    &counter,
                    "down",
                    "up",
                    || Ok(()),
                    |event| {
                        posts.push(*event);
                        op.cancel();
                    },
                )
            });
            assert_eq!(result.is_err(), before);
            assert_eq!(counter.load(Ordering::Acquire), if before { 0 } else { 2 });
            assert_eq!(posts, if before { vec![] } else { vec!["down", "up"] });
            op.finish();
            assert!(op.status().resources_released);
        }
    }
    #[test]
    fn later_chunks_double_clicks_and_expired_lease_do_not_replay_the_first_pair() {
        let op = operation::Registry::default().begin(None).unwrap();
        let counter = AtomicU64::new(0);
        operation::run(&op, || {
            pair(&counter, 1, 2, || Ok(()), |_| ()).unwrap();
            op.cancel();
            assert!(pair(&counter, 3, 4, || Ok(()), |_| panic!("no further post")).is_err());
        });
        assert_eq!(counter.load(Ordering::Acquire), 2);
        operation::with_dispatch_deadline(
            std::time::Instant::now() - std::time::Duration::from_millis(1),
            || {
                assert!(single(&counter, || panic!("expired lease")).is_err());
            },
        );
        assert_eq!(counter.load(Ordering::Acquire), 2);
    }
    #[test]
    fn native_focus_permission_or_layout_rejection_is_proven_before_any_effect() {
        let counter = AtomicU64::new(0);
        let result = pair(
            &counter,
            1,
            2,
            || Err(rwi_core::fault("changed", "synthetic native guard failure")),
            |_| panic!("not posted"),
        );
        assert_eq!(result.unwrap_err().code, "changed");
        assert_eq!(counter.load(Ordering::Acquire), 0);
    }
}
