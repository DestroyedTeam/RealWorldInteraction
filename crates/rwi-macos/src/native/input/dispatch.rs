//! A batch is cancellable before down, never between an allocated down/up pair.
use rwi_core::{Result, operation};
use std::sync::atomic::{AtomicU64, Ordering};
pub(super) fn count(counter: &AtomicU64) {
    let _ = counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
        Some(n.saturating_add(1))
    });
}
/// A returned failure after an addressed AppKit preparation event is still an
/// uncertain application effect. An unchanged, unsaturated counter after that
/// event proves no requested mouse/key/scroll/drag/AX effect entered this call.
/// This does not undo preparation, authorize replay, or reconcile an old fence.
pub(super) fn preparation_only_failure(
    mut error: rwi_core::Fault,
    receiver_prepared: bool,
    requested_input_before: u64,
    requested_input_after: u64,
) -> rwi_core::Fault {
    if receiver_prepared
        && requested_input_before < u64::MAX
        && requested_input_before == requested_input_after
    {
        let prior = error.diagnostic.take().map(|value| *value);
        let mut diagnostic = match prior {
            Some(value) if value.is_object() => value,
            Some(value) => serde_json::json!({"cause": value}),
            None => serde_json::json!({}),
        };
        diagnostic["dispatch_evidence"] = serde_json::json!({
            "version": 1,
            "kind": "macos_background_preparation_only",
            "api": "CGEventPostToPid",
            "preparation_event": "NSApplicationActivatedEventType",
            "preparation_events": 1,
            "requested_input_events": 0,
            "held_input_resources": false,
            "replay_allowed": false
        });
        error.diagnostic = Some(Box::new(diagnostic));
        error.next_step = "Requested input was not sent; one addressed AppKit preparation event was posted. Inspect fresh state, never replay.".into();
    }
    error
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
    fn preparation_effect_is_not_confused_with_a_requested_input_or_zero_effect() {
        for (prepared, before, after, proven) in [
            (true, 1, 1, true),
            (true, 8, 8, true),
            (false, 0, 0, false),
            (true, 1, 2, false),
            (true, 2, 1, false),
            (true, u64::MAX, u64::MAX, false),
        ] {
            let mut cause = rwi_core::fault("stale_reference", "Inspect original request.");
            cause.diagnostic = Some(Box::new(serde_json::json!({"content_changed": true})));
            let error = preparation_only_failure(cause, prepared, before, after);
            assert_eq!(error.code, "stale_reference");
            let diagnostic = error.diagnostic.unwrap();
            assert_eq!(diagnostic["content_changed"], true);
            assert_eq!(diagnostic.get("dispatch_evidence").is_some(), proven);
            if proven {
                assert_eq!(diagnostic["dispatch_evidence"]["requested_input_events"], 0);
                assert_eq!(
                    diagnostic["dispatch_evidence"]["held_input_resources"],
                    false
                );
                assert_eq!(diagnostic["dispatch_evidence"]["replay_allowed"], false);
                assert!(error.next_step.contains("preparation event was posted"));
            }
        }
    }
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
