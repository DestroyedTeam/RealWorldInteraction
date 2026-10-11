use super::*;
fn identity() -> Identity {
    Identity {
        host: "mac".into(),
        guest: None,
        session: "uid:501:security:1:boot:1".into(),
        seat: "console:1".into(),
        desktop: "quartz-console".into(),
    }
}
fn row(id: u32) -> WindowRecord {
    WindowRecord::from_native(
        &identity(),
        &Window {
            id,
            pid: 100,
            rect: Rect {
                x: 0.,
                y: 0.,
                w: 10.,
                h: 10.,
            },
            alpha: 1.,
            title: Some(format!("Window {id}")),
        },
        "100:1:1".into(),
        None,
    )
    .unwrap()
}
fn selection(row: &WindowRecord) -> WindowSelection {
    WindowSelection {
        window: row.window.clone(),
        pid: row.pid,
        native_epoch: row.native_epoch.clone(),
        relation: WindowRelation::Initial,
    }
}
#[test]
fn empty_catalog_and_pagination_never_imply_hidden_window_coverage() {
    let mut catalog = Catalog::default();
    let header = json!({"enumeration_complete":true,"coverage":"on_screen_only"});
    let empty = catalog.replace(vec![], header.clone()).unwrap();
    assert_eq!(empty["windows"], json!([]));
    assert_eq!(empty["coverage"], "on_screen_only");
    let mut page = catalog
        .replace((1..=25).map(row).collect(), header)
        .unwrap();
    let mut seen = 0;
    loop {
        assert!(page["windows"].as_array().unwrap().len() <= 8);
        assert!(serde_json::to_vec(&page).unwrap().len() <= PAGE_BYTES);
        seen += page["windows"].as_array().unwrap().len();
        assert_eq!(page["complete"], false);
        let Some(cursor) = page["next_cursor"].as_str() else {
            break;
        };
        page = catalog.page(Some(cursor)).unwrap();
    }
    assert_eq!(seen, 25);
}
#[test]
fn maximum_display_inventory_stays_inside_window_page_budget() {
    let displays = (0..32)
        .map(|id| DisplayRecord {
            id,
            uuid: Some(format!("00000000-0000-0000-0000-{id:012X}")),
            built_in: id == 0,
            bounds: [f64::from(id) * 1920., 0., 1920., 1080.],
            pixels: [3840, 2160],
            rotation: 0.,
        })
        .collect::<Vec<_>>();
    let mut catalog = Catalog::default();
    let mut page = catalog
        .replace(
            (1..=25).map(row).collect(),
            json!({"enumeration_complete":true,"displays":displays}),
        )
        .unwrap();
    let mut seen = 0;
    loop {
        assert_eq!(page["displays"].as_array().unwrap().len(), 32);
        assert!(serde_json::to_vec(&page).unwrap().len() <= PAGE_BYTES);
        seen += page["windows"].as_array().unwrap().len();
        let Some(cursor) = page["next_cursor"].as_str() else {
            break;
        };
        page = catalog.page(Some(cursor)).unwrap();
    }
    assert_eq!(seen, 25);
}
#[test]
fn exact_pid_epoch_freshness_context_and_owner_transition_are_required() {
    let mut c = Catalog::default();
    c.replace(vec![row(1)], json!({"enumeration_complete":true}))
        .unwrap();
    let initial = selection(&row(1));
    assert!(c.select(&initial).is_ok());
    for change in 0..3 {
        let mut s = initial.clone();
        match change {
            0 => s.pid += 1,
            1 => s.native_epoch = "other".into(),
            _ => s.relation = WindowRelation::OwnedPopup,
        }
        assert!(c.select(&s).is_err());
    }
    assert!(c.page(Some("different-context/0")).is_err());
    c.at = Some(Instant::now() - Duration::from_secs(11));
    assert_eq!(c.select(&initial).unwrap_err().code, "catalog_expired");
}
#[test]
fn changed_process_window_geometry_title_and_duplicates_cannot_bind() {
    for change in 0..4 {
        let first = row(1);
        let mut now = first.clone();
        match change {
            0 => now.process_start = "100:2:1".into(),
            1 => now.bounds[0] += 1.,
            2 => now.title = Some("different".into()),
            _ => now.pid += 1,
        }
        assert!(first.require_same(&now).is_err());
    }
    assert!(
        Catalog::default()
            .replace(vec![row(1), row(1)], json!({}))
            .is_err()
    );
}
#[test]
fn utf8_and_unknown_titles_are_bounded_and_filters_are_explicit() {
    let w = Window {
        id: 1,
        pid: 100,
        rect: row(1).rect(),
        alpha: 1.,
        title: Some("中文🙂".repeat(2000)),
    };
    let r = WindowRecord::from_native(&identity(), &w, "100:1:1".into(), None).unwrap();
    assert!(r.title_truncated && r.title.as_ref().unwrap().len() <= 1024);
    assert!(matches_query(
        &w,
        &DiscoveryQuery {
            title_contains: Some("中文".into()),
            ..Default::default()
        }
    ));
    let missing = Window { title: None, ..w };
    assert!(!matches_query(
        &missing,
        &DiscoveryQuery {
            title_contains: Some("text".into()),
            ..Default::default()
        }
    ));
}
#[test]
fn authorized_display_requires_an_unambiguous_matching_display() {
    let built_in = DisplayRecord {
        id: 1,
        uuid: None,
        built_in: true,
        bounds: [282.0, 1152.0, 1800.0, 1169.0],
        pixels: [3600, 2338],
        rotation: 0.0,
    };
    let external = DisplayRecord {
        id: 3,
        uuid: None,
        built_in: false,
        bounds: [0.0, 0.0, 2048.0, 1152.0],
        pixels: [4096, 2304],
        rotation: 0.0,
    };
    let policy = AuthorizedDisplay {
        place_on_bind: false,
        built_in: true,
        id: None,
    };
    assert!(policy.allows(Some(&built_in)));
    assert!(!policy.allows(Some(&external)));
    assert!(!policy.allows(None));
    assert!(
        AuthorizedDisplay {
            place_on_bind: false,
            built_in: true,
            id: Some(1)
        }
        .allows(Some(&built_in))
    );
    assert!(
        !AuthorizedDisplay {
            place_on_bind: false,
            built_in: true,
            id: Some(2)
        }
        .allows(Some(&built_in))
    );
}
