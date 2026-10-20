//! Bounded multi-snapshot discovery and display fingerprint regression tests.
use super::tests::{expire, identity, row, selection};
use super::*;

#[test]
fn independent_pid_and_filtered_queries_preserve_exact_selections_and_cursors() {
    let mut catalog = Catalog::default();
    let first = row(1);
    let same_pid_filtered = row(2);
    let mut other_pid = row(3);
    other_pid.pid = 200;
    other_pid.process_start = "200:1:1".into();
    let original = catalog
        .replace((1..=12).map(row).collect(), json!({"pid":100}))
        .unwrap();
    let cursor = original["next_cursor"].as_str().unwrap().to_owned();
    catalog
        .replace(
            vec![same_pid_filtered.clone()],
            json!({"pid":100,"title_contains":"Window 2"}),
        )
        .unwrap();
    let latest = catalog
        .replace(vec![other_pid.clone()], json!({"pid":200}))
        .unwrap();
    for expected in [&first, &same_pid_filtered, &other_pid] {
        assert_eq!(catalog.select(&selection(expected)).unwrap(), *expected);
    }
    let second_page = catalog.page(Some(&cursor)).unwrap();
    assert_eq!(second_page["catalog_id"], original["catalog_id"]);
    assert_eq!(second_page["pid"], 100);
    assert_eq!(second_page["windows"][0]["window"], "cgwindow:9");
    assert_eq!(
        catalog.page(None).unwrap()["catalog_id"],
        latest["catalog_id"]
    );
    assert_eq!(
        latest["catalog_cache"],
        json!({
            "mode":"multi_snapshot",
            "max_snapshots":8,
            "max_total_rows":2048,
            "per_snapshot_ttl_ms":10000,
        })
    );
}

#[test]
fn snapshot_ttl_is_independent_and_expired_rows_are_pruned_on_replace() {
    let mut catalog = Catalog::default();
    let a = catalog.replace(vec![row(1)], json!({})).unwrap();
    let a_id = a["catalog_id"].as_str().unwrap();
    let a_at = catalog.snapshots[0].at;
    let b = catalog.replace(vec![row(2)], json!({})).unwrap();
    assert_eq!(catalog.snapshots[0].at, a_at);
    assert_eq!(
        catalog.page(Some(&format!("{a_id}/0"))).unwrap()["windows"],
        a["windows"]
    );
    assert_eq!(catalog.snapshots[0].at, a_at);
    expire(&mut catalog, a_id);
    assert_eq!(
        catalog.select(&selection(&row(1))).unwrap_err().code,
        "window_not_discovered"
    );
    assert!(catalog.select(&selection(&row(2))).is_ok());
    assert_eq!(
        catalog.page(Some(&format!("{a_id}/0"))).unwrap_err().code,
        "cursor_expired"
    );
    let b_id = b["catalog_id"].as_str().unwrap();
    let b_at = catalog.snapshots[1].at;
    catalog.replace(vec![row(3)], json!({})).unwrap();
    assert_eq!(catalog.snapshots.len(), 2);
    assert_eq!(catalog.snapshots[0].id, b_id);
    assert_eq!(catalog.snapshots[0].at, b_at);
    expire(&mut catalog, b_id);
    let last_id = catalog.snapshots.back().unwrap().id.clone();
    expire(&mut catalog, &last_id);
    assert_eq!(catalog.page(None).unwrap_err().code, "catalog_expired");
    assert_eq!(
        catalog.select(&selection(&row(3))).unwrap_err().code,
        "catalog_expired"
    );
}

#[test]
fn snapshot_count_and_total_row_capacity_evict_oldest_deterministically() {
    let mut catalog = Catalog::default();
    let first = catalog.replace(vec![row(1)], json!({})).unwrap();
    for id in 2..=8 {
        catalog.replace(vec![row(id)], json!({})).unwrap();
    }
    assert!(catalog.select(&selection(&row(1))).is_ok());
    catalog.replace(vec![row(9)], json!({})).unwrap();
    assert_eq!(catalog.snapshots.len(), MAX_SNAPSHOTS);
    assert_eq!(catalog.snapshots.front().unwrap().rows[0], row(2));
    assert_eq!(
        catalog.select(&selection(&row(1))).unwrap_err().code,
        "window_not_discovered"
    );
    assert_eq!(
        catalog
            .page(Some(&format!(
                "{}/0",
                first["catalog_id"].as_str().unwrap()
            )))
            .unwrap_err()
            .code,
        "cursor_expired"
    );

    let mut catalog = Catalog::default();
    let a = catalog
        .replace((1..=1024).map(row).collect(), json!({}))
        .unwrap();
    catalog
        .replace((1025..=2048).map(row).collect(), json!({}))
        .unwrap();
    assert_eq!(
        catalog
            .snapshots
            .iter()
            .map(|s| s.rows.len())
            .sum::<usize>(),
        MAX_TOTAL_ROWS
    );
    catalog.replace(vec![row(2049)], json!({})).unwrap();
    assert_eq!(catalog.snapshots.len(), 2);
    assert_eq!(catalog.snapshots.front().unwrap().rows[0], row(1025));
    assert_eq!(
        catalog.select(&selection(&row(1))).unwrap_err().code,
        "window_not_discovered"
    );
    assert_eq!(
        catalog
            .page(Some(a["next_cursor"].as_str().unwrap()))
            .unwrap_err()
            .code,
        "cursor_expired"
    );
    assert!(catalog.select(&selection(&row(1025))).is_ok());
}

#[test]
fn refused_candidates_preserve_retained_rows_cursors_and_sequence_atomically() {
    let mut catalog = Catalog::default();
    let original = catalog
        .replace((1..=12).map(row).collect(), json!({}))
        .unwrap();
    let cursor = original["next_cursor"].as_str().unwrap();
    for id in 20..27 {
        catalog.replace(vec![row(id)], json!({})).unwrap();
    }
    let ids: Vec<_> = catalog
        .snapshots
        .iter()
        .map(|s| (s.id.clone(), s.at))
        .collect();
    let sequence = catalog.sequence;
    for (rows, header, expected) in [
        (
            vec![row(100), row(100)],
            json!({}),
            "catalog_identity_ambiguous",
        ),
        (
            (1..=2049).map(row).collect(),
            json!({}),
            "catalog_identity_ambiguous",
        ),
        (vec![row(100)], Value::Null, "catalog_encoding"),
        (
            vec![row(100)],
            json!({"large":"x".repeat(PAGE_BYTES)}),
            "catalog_page_budget",
        ),
    ] {
        assert_eq!(catalog.replace(rows, header).unwrap_err().code, expected);
        assert_eq!(catalog.sequence, sequence);
        assert_eq!(
            catalog
                .snapshots
                .iter()
                .map(|s| (s.id.clone(), s.at))
                .collect::<Vec<_>>(),
            ids
        );
        assert!(catalog.select(&selection(&row(1))).is_ok());
        assert_eq!(
            catalog.page(Some(cursor)).unwrap()["catalog_id"],
            original["catalog_id"]
        );
    }
}

#[test]
fn snapshot_cursors_and_rows_are_owned_by_their_context_instance() {
    let mut a = Catalog::default();
    let mut b = Catalog::default();
    let a_page = a.replace((1..=12).map(row).collect(), json!({})).unwrap();
    let b_page = b.replace(vec![row(200)], json!({})).unwrap();
    assert_ne!(a_page["catalog_id"], b_page["catalog_id"]);
    assert_eq!(
        b.page(Some(a_page["next_cursor"].as_str().unwrap()))
            .unwrap_err()
            .code,
        "cursor_expired"
    );
    assert_eq!(
        b.select(&selection(&row(1))).unwrap_err().code,
        "window_not_discovered"
    );
    assert!(a.select(&selection(&row(1))).is_ok());
    assert!(b.select(&selection(&row(200))).is_ok());
}

#[test]
fn identical_retained_triples_are_accepted_but_conflicting_full_records_are_refused() {
    let mut catalog = Catalog::default();
    let first = row(1);
    let first_page = catalog.replace(vec![first.clone()], json!({})).unwrap();
    catalog.replace(vec![first.clone()], json!({})).unwrap();
    assert_eq!(catalog.select(&selection(&first)).unwrap(), first);
    let mut changed = first.clone();
    changed.process_start = "100:2:1".into();
    catalog.replace(vec![changed], json!({})).unwrap();
    assert_eq!(
        catalog.select(&selection(&first)).unwrap_err().code,
        "catalog_identity_ambiguous"
    );
    assert_eq!(
        catalog
            .page(Some(&format!(
                "{}/0",
                first_page["catalog_id"].as_str().unwrap()
            )))
            .unwrap()["windows"][0]["process_start"],
        "100:1:1"
    );
    for snapshot in &mut catalog.snapshots {
        if snapshot.rows[0].process_start == "100:1:1" {
            snapshot.at = Instant::now() - TTL - Duration::from_secs(1);
        }
    }
    assert_eq!(
        catalog.select(&selection(&first)).unwrap().process_start,
        "100:2:1"
    );
}

#[test]
fn same_epoch_display_and_semantic_authority_conflicts_are_not_arbitrarily_selected() {
    for semantic in [false, true] {
        let mut catalog = Catalog::default();
        let original = row(1);
        catalog.replace(vec![original.clone()], json!({})).unwrap();
        let mut changed = original.clone();
        if semantic {
            changed.semantic_only = true;
        } else {
            changed.display = Some(DisplayRecord {
                id: 4,
                uuid: Some("different-display".into()),
                built_in: false,
                bounds: [1800., 0., 1920., 1080.],
                pixels: [1920, 1080],
                rotation: 0.,
            });
        }
        assert_eq!(
            selection(&changed).native_epoch,
            selection(&original).native_epoch
        );
        catalog.replace(vec![changed], json!({})).unwrap();
        assert_eq!(
            catalog.select(&selection(&original)).unwrap_err().code,
            "catalog_identity_ambiguous"
        );
    }
}

#[test]
fn malformed_and_out_of_bounds_cursors_never_replace_or_refresh_the_snapshot() {
    let mut catalog = Catalog::default();
    let page = catalog
        .replace((1..=12).map(row).collect(), json!({}))
        .unwrap();
    let id = page["catalog_id"].as_str().unwrap();
    let at = catalog.snapshots[0].at;
    for cursor in [
        "missing-separator".to_owned(),
        format!("{id}/-1"),
        format!("{id}/not-an-offset"),
        format!("{id}/{}0", usize::MAX),
        format!("{id}/12"),
        format!("{id}/13"),
    ] {
        assert_eq!(
            catalog.page(Some(&cursor)).unwrap_err().code,
            "invalid_catalog_cursor"
        );
        assert_eq!(catalog.snapshots[0].at, at);
        assert!(catalog.select(&selection(&row(1))).is_ok());
    }
    assert_eq!(
        catalog
            .page(Some(page["next_cursor"].as_str().unwrap()))
            .unwrap()["windows"][0]["window"],
        "cgwindow:9"
    );
}

#[test]
fn overwritten_header_keys_cannot_hide_unbounded_retained_metadata() {
    let mut catalog = Catalog::default();
    let page = catalog.replace(vec![row(1)], json!({})).unwrap();
    for key in ["windows", "catalog_id", "catalog_cache", "next_cursor"] {
        let mut header = json!({});
        header[key] = json!("x".repeat(PAGE_BYTES));
        assert_eq!(
            catalog.replace(vec![row(2)], header).unwrap_err().code,
            "catalog_page_budget"
        );
        assert_eq!(
            catalog.page(None).unwrap()["catalog_id"],
            page["catalog_id"]
        );
        assert_eq!(catalog.snapshots.len(), 1);
    }
}

#[test]
fn changed_display_fingerprint_allows_exact_rediscovery_without_reusing_old_native_authority() {
    fn make(window: &Window, display: Option<&crate::native::Display>) -> WindowRecord {
        WindowRecord::from_native(&identity(), window, "100:1:1".into(), display).unwrap()
    }
    let window = Window {
        id: 1,
        pid: 100,
        rect: Rect {
            x: 1810.,
            y: 10.,
            w: 100.,
            h: 100.,
        },
        alpha: 1.,
        title: Some("same-window".into()),
    };
    let display = crate::native::Display {
        id: 4,
        uuid: Some("original-display".into()),
        built_in: false,
        rect: Rect {
            x: 1800.,
            y: 0.,
            w: 1920.,
            h: 1080.,
        },
        pixels: [1920, 1080],
        rotation: 0.,
    };
    let original = make(&window, Some(&display));
    assert_eq!(original, make(&window, Some(&display)));
    for change in 0..7 {
        let mut changed_display = display.clone();
        match change {
            0 => changed_display.id += 1,
            1 => changed_display.uuid = Some("rediscovered-display".into()),
            2 => changed_display.built_in = true,
            3 => changed_display.rect.x += 1.,
            4 => changed_display.rect.w += 1.,
            5 => changed_display.pixels[0] *= 2,
            _ => changed_display.rotation = 90.,
        }
        let changed = make(&window, Some(&changed_display));
        assert_ne!(original.native_epoch, changed.native_epoch);
        assert_eq!(changed.native_epoch.len(), 64);
        assert!(
            changed
                .native_epoch
                .chars()
                .all(|ch| ch.is_ascii_hexdigit())
        );
        let mut catalog = Catalog::default();
        catalog.replace(vec![original.clone()], json!({})).unwrap();
        catalog.replace(vec![changed.clone()], json!({})).unwrap();
        assert_eq!(catalog.select(&selection(&changed)).unwrap(), changed);
        let old_observation = catalog.select(&selection(&original)).unwrap();
        assert_eq!(old_observation, original);
        assert_eq!(
            old_observation.require_same(&changed).unwrap_err().code,
            "window_epoch_changed"
        );
        assert_ne!(changed.clone().logical().native_epoch, changed.native_epoch);
    }
    assert_ne!(make(&window, None).native_epoch, original.native_epoch);
}
