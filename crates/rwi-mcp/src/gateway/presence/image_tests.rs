use super::*;
fn setup() -> (Arc<Views>, String, String) {
    let v = Arc::new(Views::default());
    let (key, page, _) = v.test_app_page("owned-target");
    v.app_event(&key, page.clone(), 1, true).unwrap();
    (v, key, page)
}
#[test]
fn f32_live_image_page_read_does_not_renew_presence_or_duration() {
    let (v, k, p) = setup();
    let (life, ttl) = {
        let e = v.entries.lock().unwrap();
        let x = &e[&k];
        (x.expires, x.image.visible_until)
    };
    let t = v.image_read(&k, &p, 1).unwrap();
    assert!(t.current());
    assert_eq!(t.target, "owned-target");
    assert!(v.entries.lock().unwrap()[&k].image.busy.is_some());
    assert_eq!(v.entries.lock().unwrap()[&k].expires, life);
    assert_eq!(v.entries.lock().unwrap()[&k].image.visible_until, ttl);
    drop(t);
    assert!(v.entries.lock().unwrap()[&k].image.busy.is_none());
}
#[test]
fn f32_live_image_hidden_unmounted_foreign_page_or_expired_presence_refuse() {
    for mode in 0..5 {
        let (v, k, p) = setup();
        match mode {
            0 => {
                v.app_event(&k, p.clone(), 2, false).unwrap();
            }
            1 => {
                v.app_page("owned-target").unwrap();
            }
            2 => {
                v.entries.lock().unwrap().get_mut(&k).unwrap().expires = Instant::now();
            }
            3 => {
                v.entries
                    .lock()
                    .unwrap()
                    .get_mut(&k)
                    .unwrap()
                    .image
                    .visible_until = Some(Instant::now());
            }
            4 => {
                v.remove_target("owned-target");
            }
            _ => unreachable!(),
        }
        assert!(v.image_read(&k, &p, 1).is_err());
    }
}
#[test]
fn f32_live_image_hidden_then_visible_drops_old_inflight_frame() {
    let (v, k, p) = setup();
    let t = v.image_read(&k, &p, 1).unwrap();
    v.app_event(&k, p.clone(), 2, false).unwrap();
    assert!(!t.current());
    v.app_event(&k, p, 3, true).unwrap();
    assert!(!t.current());
}
#[test]
fn f32_live_image_regular_presence_heartbeat_keeps_current_read_but_cannot_revive_expired_one() {
    let (v, k, p) = setup();
    let t = v.image_read(&k, &p, 1).unwrap();
    v.app_event(&k, p.clone(), 2, true).unwrap();
    assert!(t.current());
    v.entries
        .lock()
        .unwrap()
        .get_mut(&k)
        .unwrap()
        .image
        .visible_until = Some(Instant::now());
    v.app_event(&k, p, 3, true).unwrap();
    assert!(!t.current());
}
#[test]
fn f32_live_image_replacement_page_survives_old_reader_drop() {
    let (v, k, p) = setup();
    let old = v.image_read(&k, &p, 1).unwrap();
    let (_, new, _) = v.app_page("owned-target").unwrap();
    v.app_event(&k, new.clone(), 1, true).unwrap();
    let current = v.image_read(&k, &new, 1).unwrap();
    assert!(!old.current());
    drop(old);
    assert!(current.current());
}
#[test]
fn f32_live_image_rate_and_monotonic_request_sequence_are_independent_of_presence() {
    let (v, k, p) = setup();
    let t = v.image_read(&k, &p, 1).unwrap();
    assert!(v.image_read(&k, &p, 2).is_err());
    drop(t);
    assert_eq!(
        v.image_read(&k, &p, 2).err(),
        Some("live_image_rate_limited")
    );
    v.entries
        .lock()
        .unwrap()
        .get_mut(&k)
        .unwrap()
        .image
        .next_read = None;
    for seq in [0, 1, 9_007_199_254_740_992] {
        assert_eq!(v.image_read(&k, &p, seq).err(), Some("stale_image_request"));
    }
    assert!(v.image_read(&k, &p, 2).is_ok());
    assert_eq!(v.entries.lock().unwrap()[&k].ui_sequence, 1);
}
#[test]
fn f32_live_image_global_two_reader_budget_is_raii_bounded_and_revoked_on_close() {
    let (v, k, p) = setup();
    let (a, b, _) = v.test_app_page("second");
    v.app_event(&a, b.clone(), 1, true).unwrap();
    let (c, d, _) = v.test_app_page("third");
    v.app_event(&c, d.clone(), 1, true).unwrap();
    let one = v.image_read(&k, &p, 1).unwrap();
    let two = v.image_read(&a, &b, 1).unwrap();
    assert_eq!(v.image_read(&c, &d, 1).err(), Some("live_image_busy"));
    drop(two);
    let three = v.image_read(&c, &d, 1).unwrap();
    v.remove_target("owned-target");
    assert!(!one.current());
    assert!(three.current());
    drop(one);
    drop(three);
    assert_eq!(v.image_readers.0.available_permits(), 2);
}
