use super::*;
use std::cell::Cell;

#[test]
fn foreground_object_timeout_sets_each_of_four_distinct_objects_once() {
    // All four can represent equal AX objects; each retained object still needs policy.
    let objects = [(1, "app"), (2, "window"), (3, "app"), (4, "window")];
    let mut calls = Vec::new();
    for object in objects {
        let returned = checked_foreground_object_timeout(object, |exact, seconds| {
            calls.push((*exact, seconds));
            0
        })
        .unwrap();
        assert_eq!(returned, object);
    }
    assert_eq!(calls, objects.map(|object| (object, 0.05)));
}

#[test]
fn foreground_object_timeout_native_errors_are_preserved_without_retry() {
    for status in [-25201, -25202, -25204, 17] {
        let calls = Cell::new(0);
        let error = checked_foreground_object_timeout(73, |object, seconds| {
            assert_eq!(*object, 73);
            assert_eq!(seconds, 0.05);
            calls.set(calls.get() + 1);
            status
        })
        .unwrap_err();
        assert_eq!(calls.get(), 1);
        assert_eq!(error.code, "foreground_timeout_configuration_failed");
        let diagnostic = error.diagnostic.unwrap();
        assert_eq!(diagnostic["native_status"], status);
        assert_eq!(diagnostic["scope"], "object_local");
        assert_eq!(diagnostic["replay_allowed"], false);
    }
}

#[test]
fn foreground_object_timeout_failure_stops_before_object_use() {
    let reads = Cell::new(0);
    let result = checked_foreground_object_timeout(37, |_, _| -25202).inspect(|_| {
        reads.set(reads.get() + 1);
    });
    assert!(result.is_err());
    assert_eq!(reads.get(), 0);
}

#[test]
fn foreground_object_timeout_returns_same_owned_object_without_clone() {
    struct Object(String);
    let object = Object("fresh retained object".to_owned());
    let address = object.0.as_ptr();
    let returned = checked_foreground_object_timeout(object, |exact, seconds| {
        assert_eq!(exact.0.as_ptr(), address);
        assert_eq!(seconds, 0.05);
        0
    })
    .ok()
    .unwrap();
    assert_eq!(returned.0.as_ptr(), address);
}
