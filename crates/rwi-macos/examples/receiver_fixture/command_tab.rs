//! Opt-in, once-only system-shortcut actor for two explicitly allocated fixtures.
//! Not a backend, an input fallback, or a physical-user attestation.
use super::model::{MouseEvent, MouseKind, Region, State};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Config {
    pub delay_ms: u32,
    pub peer_pid: i32,
    pub peer_window: u32,
    pub peer_bundle: String,
}
impl Config {
    pub fn validate(
        &self,
        regular: bool,
        lifetime: u32,
        sample: Option<u32>,
    ) -> Result<(), String> {
        if !(100..=3000).contains(&self.delay_ms)
            || !regular
            || self.delay_ms >= lifetime
            || sample.is_none_or(|ms| !(25..=200).contains(&ms))
            || self.peer_pid <= 0
            || self.peer_window == 0
            || !self
                .peer_bundle
                .starts_with("com.destroyedteam.rwi.receiver.")
            || self.peer_bundle.len() > 256
            || !self
                .peer_bundle
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
        {
            return Err("Command+Tab actor requires a regular allocated window, bounded delay/lifetime, 25..200ms environment sampling and an exact receiver peer PID/window/bundle".into());
        }
        Ok(())
    }
}

#[derive(Default, Serialize)]
pub struct Oracle {
    pub config: Option<Config>,
    pub first_down_sequence: Option<u64>,
    pub first_down_unix_ms: Option<u64>,
    pub first_down_timestamp_seconds: Option<f64>,
    pub fired: bool,
    pub fired_unix_ms: Option<u64>,
    pub post_access: Option<bool>,
    pub listen_access: Option<bool>,
    pub screen_metadata_access: Option<bool>,
    pub post_calls: u32,
    pub paired_release_attempted: bool,
    pub before_frontmost_pid: Option<i32>,
    pub after_frontmost_pid: Option<i32>,
    pub private_source_state_id: Option<i64>,
    pub peer_process_start: Option<String>,
    pub peer_uid: Option<u32>,
    pub peer_surface_bound_before_post: bool,
    pub guard_hid_ages_seconds: Vec<f64>,
    pub guard_held_key_counts: Vec<[u32; 2]>,
    pub guard_hid_button_masks: Vec<u32>,
    pub guard_modifier_flags: Vec<[u64; 2]>,
    pub error: Option<String>,
    pub physical_human_origin_attested: bool,
}
pub fn arm(state: &mut State, event: &MouseEvent, unix_ms: Option<u64>) -> Option<u32> {
    let actor = &mut state.command_tab;
    let config = actor.config.as_ref()?;
    config
        .validate(
            state.application_identity.activation_policy == Some(0),
            state.lifetime_ms?,
            state.environment_sample_ms,
        )
        .ok()?;
    if actor.first_down_sequence.is_some()
        || actor.fired
        || state.status != "running"
        || state.application_identity.activation_policy != Some(0)
        || state.window_number != Some(event.window_number)
        || event.region != Region::Canvas
        || event.kind != MouseKind::Down
        || event.native_type != 1
        || !event.finite()
        || event.receiving_app_frontmost != Some(false)
        || state.drag.downs != 0
        || config.peer_pid as u32 == state.pid
    {
        return None;
    }
    actor.first_down_sequence = Some(state.sequence.checked_add(1)?);
    actor.first_down_unix_ms = unix_ms;
    actor.first_down_timestamp_seconds = Some(event.timestamp_seconds);
    Some(config.delay_ms)
}

/// Pair ups only with attempted downs, including guard errors and Rust unwinding.
fn balanced<T>(
    pairs: &[(T, T)],
    mut guard: impl FnMut() -> Result<(), String>,
    post: impl FnMut(&T),
    mut pause: impl FnMut(),
) -> Result<(), String> {
    struct Held<'a, T, P: FnMut(&T)> {
        ups: Vec<&'a T>,
        post: P,
    }
    impl<T, P: FnMut(&T)> Drop for Held<'_, T, P> {
        fn drop(&mut self) {
            while let Some(up) = self.ups.pop() {
                (self.post)(up);
            }
        }
    }
    let mut held = Held {
        ups: Vec::with_capacity(pairs.len()),
        post,
    };
    for (down, up) in pairs {
        guard()?;
        held.ups.push(up);
        (held.post)(down);
        pause();
    }
    Ok(())
}

// Like production recovery, distinguish physical HID keys from stale
// combined-session non-modifier keys. Keep physical keys/buttons and unknown
// momentary modifiers blocking before any chord. As in native/hotkey.rs, resource
// preflight runs before our first down: our posted modifier can appear in those
// tables. Every later down still checks HID recency, permission and peer identity.
#[cfg(any(target_os = "macos", test))]
const MOMENTARY_FLAGS: u64 = (1 << 17) | (1 << 18) | (1 << 19) | (1 << 20) | (1 << 23);
#[cfg(any(target_os = "macos", test))]
fn resources_held(hid_keys: u32, hid_flags: u64, session_flags: u64, hid_buttons: u32) -> bool {
    hid_keys != 0
        || hid_buttons != 0
        || hid_flags & MOMENTARY_FLAGS != 0
        || session_flags & MOMENTARY_FLAGS != 0
}

#[cfg(target_os = "macos")]
pub mod native {
    use super::*;
    use core_graphics::{
        event::{CGEvent, CGEventFlags, CGEventTapLocation, CGEventType},
        event_source::{CGEventSource, CGEventSourceStateID},
    };
    use objc2::{
        msg_send,
        rc::Retained,
        runtime::{AnyClass, AnyObject},
    };
    use objc2_foundation::NSString;
    use std::{
        cell::{Cell, RefCell},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightPostEventAccess() -> bool;
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGPreflightListenEventAccess() -> bool;
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event: u32) -> f64;
        fn CGEventSourceKeyState(state: i32, key: u16) -> bool;
        fn CGEventSourceButtonState(state: i32, button: u32) -> bool;
        fn CGEventSourceFlagsState(state: i32) -> u64;
        fn CGWindowListCopyWindowInfo(options: u32, relative: u32) -> *mut AnyObject;
    }
    const TAG: i64 = 0x5257_4943_5441_4231;
    fn class(name: &'static std::ffi::CStr) -> Result<&'static AnyClass, String> {
        AnyClass::get(name).ok_or_else(|| format!("class unavailable: {name:?}"))
    }
    fn value(dict: &AnyObject, key: &str) -> *mut AnyObject {
        let key = NSString::from_str(key);
        unsafe { msg_send![dict, objectForKey:&*key] }
    }
    fn number(dict: &AnyObject, key: &str) -> Result<f64, String> {
        let raw = value(dict, key);
        if raw.is_null() || !unsafe { msg_send![raw, isKindOfClass:class(c"NSNumber")?] } {
            return Err(format!("missing/non-numeric own peer metadata: {key}"));
        }
        let n: f64 = unsafe { msg_send![raw, doubleValue] };
        n.is_finite()
            .then_some(n)
            .ok_or_else(|| format!("non-finite peer metadata: {key}"))
    }
    #[derive(PartialEq)]
    struct Surface {
        bounds: [f64; 4],
        title: Option<String>,
        layer: f64,
    }
    fn surface(config: &Config) -> Result<Surface, String> {
        // Public metadata only. The create-rule CFArray is toll-free NSArray;
        // Retained owns its +1 and releases it, including every error return.
        let rows = unsafe { Retained::from_raw(CGWindowListCopyWindowInfo(1, 0)) }
            .ok_or("CG window inventory unavailable")?;
        let count: usize = unsafe { msg_send![&*rows, count] };
        if count > 2048 {
            return Err("peer inventory exceeds bound".into());
        }
        let mut selected = None;
        let mut peer_rows = Vec::new();
        for i in 0..count {
            let row: *mut AnyObject = unsafe { msg_send![&*rows, objectAtIndex:i] };
            if row.is_null() {
                return Err("null CG metadata row".into());
            }
            let row = unsafe { &*row };
            if number(row, "kCGWindowOwnerPID")? != f64::from(config.peer_pid) {
                continue;
            }
            let id = number(row, "kCGWindowNumber")?;
            let layer = number(row, "kCGWindowLayer")?;
            peer_rows.push((id, layer));
            if id != f64::from(config.peer_window) {
                continue;
            }
            if selected.is_some() {
                return Err("duplicate exact peer CG window identity".into());
            }
            let bounds = value(row, "kCGWindowBounds");
            if bounds.is_null() {
                return Err("peer bounds unavailable".into());
            }
            let bounds = unsafe { &*bounds };
            let bounds = [
                number(bounds, "X")?,
                number(bounds, "Y")?,
                number(bounds, "Width")?,
                number(bounds, "Height")?,
            ];
            if bounds[2] <= 0. || bounds[3] <= 0. {
                return Err("peer bounds unusable".into());
            }
            let title = value(row, "kCGWindowName");
            let title = if title.is_null() {
                None
            } else {
                if !unsafe { msg_send![title, isKindOfClass:class(c"NSString")?] } {
                    return Err("peer title type unknown".into());
                }
                let title = unsafe { &*title.cast::<NSString>() }.to_string();
                if title.len() > 1024 {
                    return Err("peer title exceeds bound".into());
                }
                Some(title)
            };
            selected = Some(Surface {
                bounds,
                title,
                layer,
            });
        }
        selected.ok_or_else(|| format!("allocated peer CG window missing: expected={} rows={peer_rows:?}; no restore/fallback",config.peer_window))
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Birth {
        pid: u32,
        uid: u32,
        ruid: u32,
        seconds: u64,
        micros: u64,
    }
    fn process_birth(pid: i32) -> Result<Birth, String> {
        let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
        let size = std::mem::size_of::<libc::proc_bsdinfo>() as i32;
        let count = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTBSDINFO,
                0,
                info.as_mut_ptr().cast(),
                size,
            )
        };
        if count != size {
            return Err("native peer process birth read unavailable; no PID-only fallback".into());
        }
        let info = unsafe { info.assume_init() };
        if info.pbi_pid != pid as u32
            || info.pbi_uid != unsafe { libc::geteuid() }
            || info.pbi_ruid != unsafe { libc::getuid() }
            || info.pbi_start_tvsec == 0
            || info.pbi_start_tvusec >= 1_000_000
            || info.pbi_status == 5
        {
            return Err("native peer process/user/birth/liveness invalid".into());
        }
        Ok(Birth {
            pid: info.pbi_pid,
            uid: info.pbi_uid,
            ruid: info.pbi_ruid,
            seconds: info.pbi_start_tvsec,
            micros: info.pbi_start_tvusec,
        })
    }
    pub fn existing_permissions(oracle: &mut Oracle) {
        oracle.post_access = Some(unsafe { CGPreflightPostEventAccess() });
        oracle.listen_access = Some(unsafe { CGPreflightListenEventAccess() });
        oracle.screen_metadata_access = Some(unsafe { CGPreflightScreenCaptureAccess() });
    }
    pub struct Actor {
        config: Config,
        peer: Retained<AnyObject>,
        birth: Birth,
        surface: RefCell<Option<Surface>>,
    }
    impl Actor {
        pub fn new(config: &Config) -> Result<Self, String> {
            if config.peer_pid as u32 == std::process::id() {
                return Err("peer must be another allocated fixture".into());
            }
            let peer: Option<Retained<AnyObject>> = unsafe {
                msg_send![class(c"NSRunningApplication")?, runningApplicationWithProcessIdentifier:config.peer_pid]
            };
            let peer = peer.ok_or("allocated peer process absent")?;
            // NSRunningApplication.launchDate is nil for some directly started
            // app bundles. Use the same public kernel birth API as RWI's binding,
            // never a weaker PID-only substitute or permission change.
            let birth = process_birth(config.peer_pid)?;
            let deadline = std::time::Instant::now() + Duration::from_millis(500);
            let observed = loop {
                if process_birth(config.peer_pid)? != birth {
                    return Err("peer birth changed while awaiting initial CG visibility".into());
                }
                match surface(config) {
                    Ok(value) => break Some(value),
                    Err(error)
                        if error.starts_with("allocated peer CG window missing:")
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(25))
                    }
                    Err(error) if error.starts_with("allocated peer CG window missing:") => {
                        break None;
                    }
                    Err(error) => return Err(error),
                }
            };
            let actor = Self {
                config: config.clone(),
                peer,
                birth,
                surface: RefCell::new(observed),
            };
            actor.validate_process()?;
            Ok(actor)
        }
        fn validate_process(&self) -> Result<(), String> {
            let current: Option<Retained<AnyObject>> = unsafe {
                msg_send![class(c"NSRunningApplication")?, runningApplicationWithProcessIdentifier:self.config.peer_pid]
            };
            let current = current.ok_or("peer process disappeared")?;
            let bundle: Option<Retained<NSString>> =
                unsafe { msg_send![&*current, bundleIdentifier] };
            let same: bool = unsafe { msg_send![&*current, isEqual:&*self.peer] };
            let ended: bool = unsafe { msg_send![&*current, isTerminated] };
            let policy: isize = unsafe { msg_send![&*current, activationPolicy] };
            if !same
                || ended
                || policy != 0
                || process_birth(self.config.peer_pid)? != self.birth
                || bundle.as_ref().map(|b| b.to_string()).as_deref()
                    != Some(self.config.peer_bundle.as_str())
            {
                return Err(
                    "allocated peer process/birth/bundle/window fingerprint changed".into(),
                );
            }
            Ok(())
        }
        fn validate_peer(&self) -> Result<(), String> {
            self.validate_process()?;
            let current = surface(&self.config)?;
            let mut pinned = self.surface.borrow_mut();
            if pinned.as_ref().is_some_and(|before| before != &current) {
                return Err("peer window fingerprint changed; no new down".into());
            }
            if pinned.is_none() {
                *pinned = Some(current);
            }
            Ok(())
        }
        pub fn post_once(&self, oracle: &mut Oracle, foreground: impl Fn() -> Option<i32>) {
            if oracle.fired {
                return;
            }
            oracle.fired = true; // A refusal/partial chord is terminal too.
            oracle.fired_unix_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .ok()
                .and_then(|d| u64::try_from(d.as_millis()).ok());
            oracle.before_frontmost_pid = foreground();
            oracle.peer_process_start = Some(format!(
                "{}:{}:{}",
                self.birth.pid, self.birth.seconds, self.birth.micros
            ));
            oracle.peer_uid = Some(self.birth.uid);
            oracle.post_access = Some(unsafe { CGPreflightPostEventAccess() });
            oracle.listen_access = Some(unsafe { CGPreflightListenEventAccess() });
            let calls = Cell::new(0u32);
            let ages = RefCell::new(Vec::with_capacity(2));
            let resources = RefCell::new(Vec::with_capacity(2));
            let result = (|| -> Result<(), String> {
                self.validate_peer()?;
                oracle.peer_surface_bound_before_post = true;
                let pairs = events()?; // Allocate all four before any effect.
                oracle.private_source_state_id = Some(pairs[0].0.get_integer_value_field(45));
                balanced(
                    &pairs,
                    || {
                        if !unsafe {
                            CGPreflightPostEventAccess() && CGPreflightListenEventAccess()
                        } {
                            return Err(
                                "existing post/listen authorization missing; no prompt/fallback"
                                    .into(),
                            );
                        }
                        let age = unsafe { CGEventSourceSecondsSinceLastEventType(1, u32::MAX) };
                        if age.is_finite() {
                            ages.borrow_mut().push(age);
                        }
                        if !age.is_finite() || age < 1.2 {
                            return Err("recent/unknown HID activity; no retry".into());
                        }
                        // Keep diagnostic samples after our down, but do not
                        // mistake our own modifier for an external held resource.
                        // HID-recency/session/peer checks still run per down.
                        let hid_keys = (0..128)
                            .filter(|&key| unsafe { CGEventSourceKeyState(1, key) })
                            .count() as u32;
                        let unknown_combined_keys = (0..128)
                            .filter(|&key| unsafe { CGEventSourceKeyState(0, key) }
                                && !(key == 55 && calls.get() == 1))
                            .count() as u32;
                        let hid_flags = unsafe { CGEventSourceFlagsState(1) };
                        let session_flags = unsafe { CGEventSourceFlagsState(0) };
                        let buttons = (0..5).fold(0u32, |mask, button| {
                            if unsafe { CGEventSourceButtonState(1, button) } {
                                mask | (1 << button)
                            } else {
                                mask
                            }
                        });
                        resources.borrow_mut().push((
                            [hid_keys, unknown_combined_keys],
                            buttons,
                            hid_flags,
                            session_flags,
                        ));
                        if calls.get() == 0
                            && resources_held(hid_keys, hid_flags, session_flags, buttons)
                        {
                            return Err(format!(
                                "human/unknown held resource: HIDkeys={hid_keys}, staleCombinedKeys={unknown_combined_keys}, HIDbuttons={buttons:#x}, HIDflags={hid_flags:#x}, sessionFlags={session_flags:#x}; no further down"
                            ));
                        }
                        if foreground() != Some(self.config.peer_pid) {
                            return Err("foreground peer changed; no further down".into());
                        }
                        self.validate_peer()
                    },
                    |event| {
                        calls.set(calls.get() + 1);
                        event.post(CGEventTapLocation::HID);
                    },
                    || std::thread::sleep(Duration::from_millis(8)),
                )
            })();
            oracle.post_calls = calls.get();
            oracle.guard_hid_ages_seconds = ages.into_inner();
            for (keys, buttons, hid_flags, session_flags) in resources.into_inner() {
                oracle.guard_held_key_counts.push(keys);
                oracle.guard_hid_button_masks.push(buttons);
                oracle.guard_modifier_flags.push([hid_flags, session_flags]);
            }
            oracle.paired_release_attempted = calls.get() > 0; // Drop posted only own pairs' ups.
            oracle.after_frontmost_pid = foreground();
            oracle.error = result.err();
        }
    }
    fn events() -> Result<Vec<(CGEvent, CGEvent)>, String> {
        let source = CGEventSource::new(CGEventSourceStateID::Private)
            .map_err(|_| "private event source unavailable")?;
        let event = |code, down, flags, modifier| -> Result<CGEvent, String> {
            let event = CGEvent::new_keyboard_event(source.clone(), code, down)
                .map_err(|_| "chord allocation failed")?;
            event.set_flags(flags);
            event.set_integer_value_field(42, TAG); // Public kCGEventSourceUserData.
            if modifier {
                event.set_type(CGEventType::FlagsChanged);
            }
            Ok(event)
        };
        let cmd = CGEventFlags::CGEventFlagCommand;
        Ok(vec![
            (
                event(55, true, cmd, true)?,
                event(55, false, CGEventFlags::empty(), true)?,
            ),
            (event(48, true, cmd, false)?, event(48, false, cmd, false)?),
        ])
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn command_tab_kernel_birth_is_exact_same_user_and_missing_pid_is_not_fallback() {
            let pid = std::process::id() as i32;
            let first = process_birth(pid).unwrap();
            assert_eq!(process_birth(pid).unwrap(), first);
            assert_eq!(first.pid, pid as u32);
            assert!(first.seconds > 0);
            assert!(process_birth(i32::MAX).is_err());
        }
        #[test]
        fn command_tab_native_allocation_has_private_tag_and_paired_flags_without_posting() {
            let pairs = events().unwrap();
            assert_eq!(pairs.len(), 2);
            for event in [&pairs[0].0, &pairs[0].1, &pairs[1].0, &pairs[1].1] {
                assert_eq!(event.get_integer_value_field(42), TAG);
            }
            assert_eq!(
                pairs[0].0.get_type() as u32,
                CGEventType::FlagsChanged as u32
            );
            assert_eq!(pairs[0].1.get_flags(), CGEventFlags::empty());
            assert_eq!(pairs[1].0.get_type() as u32, CGEventType::KeyDown as u32);
            assert_eq!(pairs[1].1.get_type() as u32, CGEventType::KeyUp as u32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn base() -> Vec<String> {
        [
            "--open-window",
            "--run-id",
            "own-command-tab",
            "--state-dir",
            "/tmp/rwi-own-command-tab-never-created-by-parser",
            "--display-id",
            "4",
            "--regular-app",
            "--lifetime-ms",
            "90000",
            "--environment-sample-ms",
            "100",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }
    fn extras() -> Vec<String> {
        [
            "--command-tab-on-first-down-after-ms",
            "1000",
            "--command-tab-peer-pid",
            "77",
            "--command-tab-peer-window",
            "123",
            "--command-tab-peer-bundle",
            "com.destroyedteam.rwi.receiver.test.peer",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }
    #[test]
    fn command_tab_resource_classification_preserves_hid_flags_and_buttons_at_preflight() {
        assert!(!resources_held(0, 0, 0, 0));
        assert!(resources_held(1, 0, 0, 0));
        assert!(resources_held(0, 0, 0, 1));
        for flag in [17, 18, 19, 20, 23] {
            assert!(resources_held(0, 1 << flag, 0, 0));
            assert!(resources_held(0, 0, 1 << flag, 0));
        }
        assert!(!resources_held(0, 1 << 16, 1 << 16, 0));
    }

    #[test]
    fn command_tab_is_default_off_and_complete_config_can_be_checked_without_gui() {
        assert!(
            super::super::model::Options::parse(base())
                .unwrap()
                .unwrap()
                .command_tab
                .is_none()
        );
        let o = super::super::model::Options::parse(
            base()
                .into_iter()
                .chain(extras())
                .chain(["--check-only".into()]),
        )
        .unwrap()
        .unwrap();
        assert!(o.check_only);
        let config = o.command_tab.unwrap();
        assert_eq!(config.peer_pid, 77);
        assert_eq!(config.peer_window, 123);
        assert_eq!(config.delay_ms, 1000);
        assert!(super::super::model::Options::parse(["--check-only".into()]).is_err());
    }
    #[test]
    fn command_tab_rejects_missing_duplicate_unbounded_or_foreign_scope_fields() {
        for index in [0, 2, 4, 6] {
            let mut args = extras();
            args.drain(index..index + 2);
            assert!(super::super::model::Options::parse(base().into_iter().chain(args)).is_err());
        }
        for (index, value) in [
            (1, "0"),
            (1, "3001"),
            (3, "0"),
            (3, "-1"),
            (3, "2147483648"),
            (5, "0"),
            (7, "com.apple.Chrome"),
            (7, "com.destroyedteam.rwi.receiver.invalid/path"),
        ] {
            let mut args = extras();
            args[index] = value.into();
            assert!(super::super::model::Options::parse(base().into_iter().chain(args)).is_err());
        }
        assert!(
            super::super::model::Options::parse(base().into_iter().chain(extras()).chain(extras()))
                .is_err()
        );
    }
    #[test]
    fn command_tab_needs_regular_sampling_lifetime_and_no_activation_fallback() {
        for flag in ["--regular-app", "--environment-sample-ms"] {
            let mut args = base();
            let n = args.iter().position(|s| s == flag).unwrap();
            args.remove(n);
            if flag == "--environment-sample-ms" {
                args.remove(n);
            }
            assert!(super::super::model::Options::parse(args.into_iter().chain(extras())).is_err());
        }
        for (flag, value) in [
            ("--lifetime-ms", "1000"),
            ("--environment-sample-ms", "201"),
        ] {
            let mut args = base();
            let n = args.iter().position(|s| s == flag).unwrap();
            args[n + 1] = value.into();
            assert!(super::super::model::Options::parse(args.into_iter().chain(extras())).is_err());
        }
        for flags in [
            ["--self-foreground-after-ms", "1000"],
            ["--self-foreground-on-first-down-after-ms", "1000"],
        ] {
            assert!(
                super::super::model::Options::parse(
                    base()
                        .into_iter()
                        .chain(extras())
                        .chain(flags.into_iter().map(str::to_owned))
                )
                .is_err()
            );
        }
    }
    fn own_event() -> MouseEvent {
        MouseEvent {
            region: Region::Canvas,
            kind: MouseKind::Down,
            local_point: [20., 20.],
            window_point: [44., 44.],
            native_type: 1,
            window_number: 7,
            receiving_app_active: false,
            receiving_window_key: false,
            frontmost_pid: Some(77),
            receiving_app_frontmost: Some(false),
            timestamp_seconds: 12.,
            modifier_flags: 0,
            event_number: Some(1),
            pressure: Some(1.),
            delta: None,
        }
    }
    fn own_state() -> State {
        let mut s = State::new("own-first-down".into());
        s.pid = 88;
        s.status = "running";
        s.application_identity.activation_policy = Some(0);
        s.window_number = Some(7);
        s.lifetime_ms = Some(90000);
        s.environment_sample_ms = Some(100);
        s.command_tab.config =
            super::super::model::Options::parse(base().into_iter().chain(extras()))
                .unwrap()
                .unwrap()
                .command_tab;
        s
    }
    #[test]
    fn command_tab_arms_once_on_actual_background_canvas_down_not_a_goal() {
        let mut s = own_state();
        assert_eq!(arm(&mut s, &own_event(), Some(1234)), Some(1000));
        assert_eq!(s.command_tab.first_down_sequence, Some(1));
        assert_eq!(s.command_tab.first_down_unix_ms, Some(1234));
        assert!(!s.command_tab.fired);
        assert_eq!(s.command_tab.post_calls, 0);
        assert!(!s.command_tab.physical_human_origin_attested);
        assert_eq!(arm(&mut s, &own_event(), Some(1235)), None);
    }
    #[test]
    fn command_tab_arm_rejects_wrong_window_region_event_policy_or_foreground() {
        for index in 0..10 {
            let mut s = own_state();
            let mut e = own_event();
            match index {
                0 => e.window_number = 8,
                1 => e.region = Region::HoverZone,
                2 => e.kind = MouseKind::Up,
                3 => e.local_point[0] = f64::NAN,
                4 => e.receiving_app_frontmost = None,
                5 => e.receiving_app_frontmost = Some(true),
                6 => s.application_identity.activation_policy = None,
                7 => s.status = "closed",
                8 => s.command_tab.config.as_mut().unwrap().peer_pid = 88,
                _ => s.command_tab.config.as_mut().unwrap().delay_ms = 4000,
            }
            assert_eq!(arm(&mut s, &e, None), None);
            assert_eq!(s.command_tab.first_down_sequence, None);
            assert_eq!(s.command_tab.post_calls, 0);
        }
    }
    #[test]
    fn command_tab_balanced_partial_guards_release_only_attempted_downs() {
        let pairs = [("cmd_down", "cmd_up"), ("tab_down", "tab_up")];
        for fail_at in 0..=2 {
            let mut checks = 0;
            let mut log = Vec::new();
            let r = balanced(
                &pairs,
                || {
                    let n = checks;
                    checks += 1;
                    if n == fail_at {
                        Err("changed".into())
                    } else {
                        Ok(())
                    }
                },
                |e| log.push(*e),
                || {},
            );
            assert_eq!(r.is_ok(), fail_at == 2);
            let expected = pairs
                .iter()
                .take(fail_at)
                .map(|p| p.0)
                .chain(pairs.iter().take(fail_at).rev().map(|p| p.1))
                .collect::<Vec<_>>();
            assert_eq!(log, expected);
        }
    }
    #[test]
    fn command_tab_guard_panic_unwinds_only_own_key_up() {
        let mut log = Vec::new();
        let mut checks = 0;
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            balanced(
                &[("cmd_down", "cmd_up"), ("tab_down", "tab_up")],
                || {
                    checks += 1;
                    assert!(checks != 2);
                    Ok(())
                },
                |e| log.push(*e),
                || {},
            )
        }));
        assert!(r.is_err());
        assert_eq!(log, ["cmd_down", "cmd_up"]);
    }
}
