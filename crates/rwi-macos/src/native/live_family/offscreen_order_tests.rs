use super::*;
#[test]
fn one_exact_offscreen_root_has_only_one_layer() {
    assert_eq!(
        capture_layer_order(
            &["cgwindow:8".into()],
            &[],
            Some(SingleRootLayer { pid: 12, window: 8 })
        )
        .unwrap(),
        vec![0]
    );
}
#[test]
fn unverified_single_or_multi_layer_missing_order_still_refuses() {
    assert!(capture_layer_order(&["cgwindow:8".into()], &[], None).is_err());
    assert!(capture_layer_order(&["cgwindow:8".into(), "cgwindow:9".into()], &[], None).is_err());
}
#[test]
fn empty_layers_do_not_invent_an_offscreen_root() {
    assert!(capture_layer_order(&[], &[], None).unwrap().is_empty());
}
fn row(id: u32, pid: u32) -> Window {
    Window {
        id,
        pid,
        rect: Rect {
            x: 0.,
            y: 0.,
            w: 10.,
            h: 10.,
        },
        alpha: 1.,
        title: None,
    }
}
#[test]
fn source22_single_root_verified_scope_does_not_need_inventory_order() {
    let ids = vec!["cgwindow:8".into()];
    let scope = single_root_layer_scope(12, 8, &ids, 0, true, true);
    assert!(scope.is_some());
    assert_eq!(capture_layer_order(&ids, &[], scope).unwrap(), vec![0]);
    assert_eq!(
        capture_layer_order(&ids, &[row(8, 12)], scope).unwrap(),
        vec![0]
    );
    println!(
        "{}",
        json!({"probe":"source22_single_root_order", "verified_single_root_without_catalog_order":true, "Native_capture_or_input_executed":false, "input_authority":false})
    );
}
#[test]
fn source22_single_root_unknown_owner_or_changed_identity_has_no_scope() {
    let ids = vec!["cgwindow:8".into()];
    for (pid, window, additional, owner_complete, native_match) in [
        (12, 8, 0, false, true),
        (12, 8, 0, true, false),
        (12, 8, 1, true, true),
        (0, 8, 0, true, true),
        (12, 0, 0, true, true),
    ] {
        let scope =
            single_root_layer_scope(pid, window, &ids, additional, owner_complete, native_match);
        assert!(scope.is_none());
        assert!(capture_layer_order(&ids, &[], scope).is_err());
    }
    for ids in [
        vec![],
        vec!["cgwindow:9".into()],
        vec!["cgwindow:8".into(), "cgwindow:8".into()],
    ] {
        assert!(single_root_layer_scope(12, 8, &ids, 0, true, true).is_none());
    }
}
#[test]
fn source22_single_root_duplicate_or_foreign_native_rows_are_rejected() {
    let ids = vec!["cgwindow:8".into()];
    let scope = single_root_layer_scope(12, 8, &ids, 0, true, true);
    for catalog in [
        vec![row(8, 99)],
        vec![row(8, 12), row(8, 12)],
        vec![row(8, 12), row(8, 99)],
    ] {
        assert_eq!(
            capture_layer_order(&ids, &catalog, scope).unwrap_err().code,
            "live_layer_order_unknown"
        );
    }
    assert!(capture_layer_order(&["cgwindow:9".into()], &[], scope).is_err());
    assert!(capture_layer_order(&["cgwindow:8".into(), "cgwindow:8".into()], &[], scope).is_err());
}
#[test]
fn source22_single_root_multilayer_and_requested_additions_keep_native_order() {
    let ids = vec!["cgwindow:8".into(), "cgwindow:9".into()];
    let scope = single_root_layer_scope(12, 8, &ids, 0, true, true);
    assert!(scope.is_none());
    assert!(capture_layer_order(&ids, &[], scope).is_err());
    assert_eq!(
        capture_layer_order(&ids, &[row(8, 12), row(9, 13)], scope).unwrap(),
        vec![1, 0]
    );
    let root_only = vec!["cgwindow:8".into()];
    let requested = single_root_layer_scope(12, 8, &root_only, 1, true, true);
    assert!(requested.is_none());
    assert!(capture_layer_order(&root_only, &[], requested).is_err());
}
#[test]
fn source22_single_root_actual_child_classification_unknown_never_grants_scope() {
    let ids = vec!["cgwindow:8".into()];
    for (native_type, role, reason) in [
        (false, Ok("AXSheet"), "owner_child_native_type_unknown"),
        (
            true,
            Err("owner_child_role_read_unavailable"),
            "owner_child_role_read_unavailable",
        ),
        (
            true,
            Err("owner_child_role_value_invalid"),
            "owner_child_role_value_invalid",
        ),
        (true, Ok(""), "owner_child_role_unknown"),
        (true, Ok("AXUnknown"), "owner_child_role_unknown"),
    ] {
        let make_family = || Family {
            sheets: vec![],
            complete: true,
            reasons: vec![],
        };
        let mut before = make_family();
        let mut after = make_family();
        let calls = std::cell::Cell::new(0);
        for family in [&mut before, &mut after] {
            assert!(!family.child_is_sheet(native_type, || {
                calls.set(calls.get() + 1);
                role.map(str::to_owned)
            }));
            assert!(!family.complete);
            assert_eq!(family.reasons, vec![reason]);
        }
        assert_eq!(calls.get(), if native_type { 2 } else { 0 });
        // Both walks can agree on the same unknown result; that is not
        // positive family completeness and cannot mint the single-root scope.
        assert!(before.same(&after));
        let scope =
            single_root_layer_scope(12, 8, &ids, 0, before.complete && after.complete, true);
        assert!(scope.is_none());
        assert!(capture_layer_order(&ids, &[], scope).is_err());
    }
    for role in ["AXButton", "AXGroup", "AXStaticText"] {
        let mut family = Family {
            sheets: vec![],
            complete: true,
            reasons: vec![],
        };
        assert!(!family.child_is_sheet(true, || Ok(role.into())));
        assert!(family.complete && family.reasons.is_empty());
    }
    let mut sheet = Family {
        sheets: vec![],
        complete: true,
        reasons: vec![],
    };
    assert!(sheet.child_is_sheet(true, || Ok("AXSheet".into())));
    assert!(sheet.complete && sheet.reasons.is_empty());
}
