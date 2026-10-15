//! Offline policy/parser checks only: no window, event post or permission request.
use super::super::model::Options;
use super::*;

fn args(foreground: bool) -> Vec<String> {
    let mut args: Vec<String> = [
        "--open-window",
        "--run-id",
        "f74-policy-only",
        "--state-dir",
        "/tmp/rwi-f74-never-created-by-parser",
        "--display-id",
        "4",
        "--regular-app",
        "--lifetime-ms",
        "90000",
        "--environment-sample-ms",
        "100",
        "--check-only",
        "--command-tab-on-first-down-after-ms",
        "1000",
        "--command-tab-peer-pid",
        "77",
        "--command-tab-peer-window",
        "123",
        "--command-tab-peer-bundle",
        "com.destroyedteam.rwi.receiver.f74.peer",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    if foreground {
        args.push("--command-tab-start-foreground".into());
    }
    args
}
fn state() -> State {
    let mut s = State::new("f74-policy-only".into());
    s.pid = 88;
    s.status = "running";
    s.window_number = Some(7);
    s.application_identity.activation_policy = Some(0);
    s.lifetime_ms = Some(90000);
    s.environment_sample_ms = Some(100);
    s.command_tab.config = Options::parse(args(true)).unwrap().unwrap().command_tab;
    s
}
fn event() -> MouseEvent {
    MouseEvent {
        region: Region::Canvas,
        kind: MouseKind::Down,
        local_point: [20., 20.],
        window_point: [44., 44.],
        native_type: 1,
        window_number: 7,
        receiving_app_active: true,
        receiving_window_key: true,
        frontmost_pid: Some(88),
        receiving_app_frontmost: Some(true),
        timestamp_seconds: 12.,
        modifier_flags: 0,
        event_number: Some(1),
        pressure: Some(1.),
        delta: None,
    }
}
#[test]
fn f74_cmdtab_foreground_mode_is_opt_in_and_background_default_unchanged() {
    let old = Options::parse(args(false)).unwrap().unwrap();
    let new = Options::parse(args(true)).unwrap().unwrap();
    assert!(old.check_only && new.check_only);
    let old = old.command_tab.unwrap();
    let new = new.command_tab.unwrap();
    assert!(!old.start_foreground);
    assert!(new.start_foreground);
    assert_eq!(old.required_foreground_pid(88), Ok(77));
    assert_eq!(new.required_foreground_pid(88), Ok(88));
    assert_eq!(
        serde_json::to_value(&old).unwrap()["start_foreground"],
        false
    );
    assert_eq!(
        serde_json::to_value(&new).unwrap()["start_foreground"],
        true
    );
}
#[test]
fn f74_cmdtab_foreground_mode_needs_full_peer_scope_and_rejects_duplicate() {
    let mut incomplete = args(true);
    let at = incomplete
        .iter()
        .position(|v| v == "--command-tab-peer-window")
        .unwrap();
    incomplete.drain(at..at + 2);
    assert!(Options::parse(incomplete).is_err());
    let mut duplicate = args(true);
    duplicate.push("--command-tab-start-foreground".into());
    assert!(Options::parse(duplicate).is_err());
    assert!(Options::parse(["--command-tab-start-foreground".into()]).is_err());
}
#[test]
fn f74_cmdtab_foreground_mode_arms_once_on_exact_own_foreground_down() {
    let mut s = state();
    assert_eq!(arm(&mut s, &event(), Some(100)), Some(1000));
    assert_eq!(s.command_tab.first_down_sequence, Some(1));
    assert_eq!(arm(&mut s, &event(), Some(101)), None);
    assert!(!s.command_tab.fired);
    assert_eq!(s.command_tab.post_calls, 0);
    assert!(!s.command_tab.physical_human_origin_attested);
}
#[test]
fn f74_cmdtab_foreground_mode_unknown_foreign_or_wrong_context_never_arms() {
    for case in 0..9 {
        let mut s = state();
        let mut e = event();
        match case {
            0 => e.frontmost_pid = None,
            1 => e.frontmost_pid = Some(77),
            2 => e.frontmost_pid = Some(99),
            3 => e.receiving_app_frontmost = None,
            4 => e.receiving_app_frontmost = Some(false),
            5 => e.window_number = 8,
            6 => s.pid = u32::MAX,
            7 => s.command_tab.config.as_mut().unwrap().peer_pid = 88,
            _ => s.drag.downs = 1,
        }
        assert_eq!(arm(&mut s, &e, Some(100)), None);
        assert_eq!(s.command_tab.first_down_sequence, None);
        assert_eq!(s.command_tab.post_calls, 0);
    }
}
#[test]
fn f74_cmdtab_foreground_mode_invalid_own_pid_is_never_unknown_equals_unknown() {
    for mode in [false, true] {
        let c = Options::parse(args(mode))
            .unwrap()
            .unwrap()
            .command_tab
            .unwrap();
        for pid in [0, u32::MAX] {
            assert!(c.required_foreground_pid(pid).is_err());
        }
    }
}
