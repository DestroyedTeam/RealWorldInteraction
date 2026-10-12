//! Pure receiving-state oracle and bounded atomic JSON output; no AppKit calls.
use super::command_tab;
use serde::Serialize;
use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::PathBuf,
};

pub const USAGE: &str = "RWI receiver fixture (no window without --open-window)\n\
Usage: receiver_fixture --open-window --run-id RUN_ID --state-dir ABS_NEW_DIRECTORY --display-id NONZERO_ID [--lifetime-ms 1..600000]\n\
Creates only its own RWI acceptance window. Root must allocate that exact window\n\
before native testing; default behavior uses only its own window on that display.\n\
An explicit peer-pinned Command+Tab test actor and --native-dnd-controls are default-off; --check-only validates allocated configuration without GUI/input.\n\
Default own-window lifetime is 120000 ms; no timer is started by help/no arguments.";
const HISTORY_LIMIT: usize = 128;
const PATH_LIMIT: usize = 128;

#[derive(Debug)]
pub struct Options {
    pub run_id: String,
    pub state_dir: PathBuf,
    pub display_id: u32,
    pub lifetime_ms: u32,
    pub cover_controls: bool,
    /// Explicit non-overlapping second own app on the allocated display.
    pub right_side: bool,
    /// Explicit read-only geometry fixture: last quarter outside the selected screen.
    pub boundary_right: bool,
    /// Place the ordinary drag canvas across the allocated screen's right edge.
    /// Geometry only: does not change event acceptance, responders or AX roles.
    pub boundary_canvas_right: bool,
    pub modal_controls: bool,
    pub system_panel_controls: bool,
    /// Default-off native pasteboard source/destination, not a canvas drag oracle.
    pub native_dnd_controls: bool,
    pub environment_sample_ms: Option<u32>,
    pub regular_app: bool,
    /// Own-client calibration only; never an RWI input preparation or fallback.
    pub hover_first_responder: bool,
    /// Explicit own-app lifecycle test, never enabled by ordinary fixtures.
    pub self_foreground_after_ms: Option<u32>,
    /// Explicit receiving-fixture actor; armed once by its first actual canvas down.
    pub self_foreground_on_first_down_after_ms: Option<u32>,
    pub legacy_self_foreground: bool,
    /// Explicit own-provider latency test, no production agent behavior.
    pub ax_windows_delay_ms: Option<u32>,
    pub command_tab: Option<command_tab::Config>,
    /// Validation only: no state directory, framework call, window or timer.
    pub check_only: bool,
}
impl Options {
    #[cfg(test)]
    fn test_timed_foreground_options() -> Self {
        Self::parse(
            [
                "--open-window",
                "--run-id",
                "own-timed-foreground",
                "--state-dir",
                "/tmp/rwi-owned-timed-fixture",
                "--display-id",
                "4",
                "--regular-app",
                "--lifetime-ms",
                "20000",
                "--self-foreground-after-ms",
                "3500",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("allocated opt-in own foreground transition must parse")
        .unwrap()
    }
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Option<Self>, String> {
        let mut args = args.into_iter();
        let mut open = false;
        let mut run_id = None;
        let mut state_dir = None;
        let mut display_id = None;
        let mut lifetime_ms = None;
        let mut cover_controls = false;
        let mut right_side = false;
        let mut boundary_right = false;
        let mut boundary_canvas_right = false;
        let mut modal_controls = false;
        let mut system_panel_controls = false;
        let mut native_dnd_controls = false;
        let mut environment_sample_ms = None;
        let mut regular_app = false;
        let mut hover_first_responder = false;
        let mut self_foreground_after_ms = None;
        let mut self_foreground_on_first_down_after_ms = None;
        let mut legacy_self_foreground = false;
        let mut ax_windows_delay_ms = None;
        let mut command_tab_delay = None;
        let mut command_tab_peer_pid = None;
        let mut command_tab_peer_window = None;
        let mut command_tab_peer_bundle = None;
        let mut check_only = false;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => return Ok(None),
                "--open-window" if !open => open = true,
                "--check-only" if !check_only => check_only = true,
                "--command-tab-on-first-down-after-ms" if command_tab_delay.is_none() => {
                    command_tab_delay = Some(
                        args.next()
                            .ok_or("Command+Tab delay missing")?
                            .parse::<u32>()
                            .map_err(|_| "Command+Tab delay must be u32")?,
                    );
                }
                "--command-tab-peer-pid" if command_tab_peer_pid.is_none() => {
                    command_tab_peer_pid = Some(
                        args.next()
                            .ok_or("Command+Tab peer PID missing")?
                            .parse::<i32>()
                            .map_err(|_| "Command+Tab peer PID must be i32")?,
                    );
                }
                "--command-tab-peer-window" if command_tab_peer_window.is_none() => {
                    command_tab_peer_window = Some(
                        args.next()
                            .ok_or("Command+Tab peer window missing")?
                            .parse::<u32>()
                            .map_err(|_| "Command+Tab peer window must be u32")?,
                    );
                }
                "--command-tab-peer-bundle" if command_tab_peer_bundle.is_none() => {
                    command_tab_peer_bundle =
                        Some(args.next().ok_or("Command+Tab peer bundle missing")?);
                }
                "--cover-controls" if !cover_controls => cover_controls = true,
                "--right-side" if !right_side => right_side = true,
                "--boundary-right" if !boundary_right => boundary_right = true,
                "--boundary-canvas-right" if !boundary_canvas_right => boundary_canvas_right = true,
                "--modal-controls" if !modal_controls => modal_controls = true,
                "--system-panel-controls" if !system_panel_controls => system_panel_controls = true,
                "--native-dnd-controls" if !native_dnd_controls => native_dnd_controls = true,
                "--regular-app" if !regular_app => regular_app = true,
                "--hover-first-responder" if !hover_first_responder => hover_first_responder = true,
                "--legacy-self-foreground" if !legacy_self_foreground => {
                    legacy_self_foreground = true
                }
                "--self-foreground-after-ms" if self_foreground_after_ms.is_none() => {
                    self_foreground_after_ms = Some(
                        args.next()
                            .ok_or("--self-foreground-after-ms value missing")?
                            .parse::<u32>()
                            .ok()
                            .filter(|ms| (100..=60000).contains(ms))
                            .ok_or("own foreground delay must be 100..60000 ms")?,
                    );
                }
                "--self-foreground-on-first-down-after-ms"
                    if self_foreground_on_first_down_after_ms.is_none() =>
                {
                    self_foreground_on_first_down_after_ms = Some(
                        args.next()
                            .ok_or("--self-foreground-on-first-down-after-ms value missing")?
                            .parse::<u32>()
                            .ok()
                            .filter(|ms| (100..=5000).contains(ms))
                            .ok_or("first-down own foreground delay must be 100..5000 ms")?,
                    );
                }
                "--ax-windows-delay-ms" if ax_windows_delay_ms.is_none() => {
                    ax_windows_delay_ms = Some(
                        args.next()
                            .ok_or("AX windows delay value missing")?
                            .parse::<u32>()
                            .ok()
                            .filter(|ms| (75..=150).contains(ms))
                            .ok_or("own AX windows delay must be 75..150 ms")?,
                    );
                }
                "--environment-sample-ms" if environment_sample_ms.is_none() => {
                    let value = args.next().ok_or("--environment-sample-ms value missing")?;
                    environment_sample_ms = Some(
                        value
                            .parse::<u32>()
                            .ok()
                            .filter(|ms| (25..=2000).contains(ms))
                            .ok_or("environment sample interval must be 25..2000 ms")?,
                    );
                }
                "--run-id" if run_id.is_none() => run_id = args.next(),
                "--state-dir" if state_dir.is_none() => state_dir = args.next().map(PathBuf::from),
                "--display-id" if display_id.is_none() => {
                    display_id = Some(
                        args.next()
                            .ok_or("--display-id value missing")?
                            .parse::<u32>()
                            .map_err(|_| "display ID must be a nonzero u32")?,
                    );
                }
                "--lifetime-ms" if lifetime_ms.is_none() => {
                    let value = args.next().ok_or("--lifetime-ms value missing")?;
                    let milliseconds = value
                        .parse::<u32>()
                        .ok()
                        .filter(|value| (1..=600_000).contains(value))
                        .ok_or("--lifetime-ms must be 1..600000 ms")?;
                    lifetime_ms = Some(milliseconds);
                }
                _ => return Err(format!("unknown, duplicate or missing argument: {arg}")),
            }
        }
        if !open
            && run_id.is_none()
            && state_dir.is_none()
            && display_id.is_none()
            && lifetime_ms.is_none()
            && !cover_controls
            && !right_side
            && !boundary_right
            && !boundary_canvas_right
            && !modal_controls
            && !system_panel_controls
            && !native_dnd_controls
            && environment_sample_ms.is_none()
            && !regular_app
            && !hover_first_responder
            && self_foreground_after_ms.is_none()
            && self_foreground_on_first_down_after_ms.is_none()
            && !legacy_self_foreground
            && ax_windows_delay_ms.is_none()
            && command_tab_delay.is_none()
            && command_tab_peer_pid.is_none()
            && command_tab_peer_window.is_none()
            && command_tab_peer_bundle.is_none()
            && !check_only
        {
            return Ok(None);
        }
        if !open {
            return Err("--open-window is required to create a GUI fixture".into());
        }
        let run_id = run_id.ok_or("--run-id is required")?;
        if run_id.is_empty()
            || run_id.len() > 64
            || !run_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return Err("RUN_ID must be 1..64 ASCII letters, digits, - or _".into());
        }
        let state_dir = state_dir.ok_or("--state-dir is required")?;
        if !state_dir.is_absolute() || state_dir.file_name().is_none() {
            return Err("state directory must be an absolute new child directory".into());
        }
        let display_id = display_id
            .filter(|id| *id != 0)
            .ok_or("--display-id must be a nonzero u32")?;
        let lifetime_ms = lifetime_ms.unwrap_or(120_000);
        if native_dnd_controls
            && (cover_controls
                || modal_controls
                || system_panel_controls
                || boundary_right
                || boundary_canvas_right
                || hover_first_responder
                || self_foreground_after_ms.is_some()
                || command_tab_delay.is_some())
        {
            return Err(
                "native DnD fixture cannot combine with a panel, geometry, launch-time activation or global input actor".into(),
            );
        }
        if boundary_right && boundary_canvas_right {
            return Err("choose exactly one boundary geometry mode".into());
        }
        if (boundary_right || boundary_canvas_right)
            && (right_side
                || cover_controls
                || modal_controls
                || system_panel_controls
                || command_tab_delay.is_some())
        {
            return Err(
                "boundary geometry fixture cannot combine with another window/actor mode".into(),
            );
        }
        if self_foreground_after_ms.is_some() && self_foreground_on_first_down_after_ms.is_some() {
            return Err(
                "own foreground actor needs exactly one launch or first-down trigger".into(),
            );
        }
        if legacy_self_foreground
            && self_foreground_after_ms.is_none()
            && self_foreground_on_first_down_after_ms.is_none()
        {
            return Err(
                "legacy own foreground method requires the explicit allocated foreground timer"
                    .into(),
            );
        }
        if self_foreground_after_ms
            .or(self_foreground_on_first_down_after_ms)
            .is_some_and(|ms| !regular_app || ms >= lifetime_ms)
        {
            return Err("own foreground transition requires --regular-app and a delay shorter than the own lifetime".into());
        }
        if ax_windows_delay_ms.is_some() && !regular_app {
            return Err("own AX windows provider test requires --regular-app".into());
        }
        let command_tab = if command_tab_delay.is_some()
            || command_tab_peer_pid.is_some()
            || command_tab_peer_window.is_some()
            || command_tab_peer_bundle.is_some()
        {
            let config = command_tab::Config {
                delay_ms: command_tab_delay.ok_or("Command+Tab delay required")?,
                peer_pid: command_tab_peer_pid.ok_or("Command+Tab peer PID required")?,
                peer_window: command_tab_peer_window.ok_or("Command+Tab peer window required")?,
                peer_bundle: command_tab_peer_bundle.ok_or("Command+Tab peer bundle required")?,
            };
            config.validate(regular_app, lifetime_ms, environment_sample_ms)?;
            if self_foreground_after_ms.is_some()
                || self_foreground_on_first_down_after_ms.is_some()
                || legacy_self_foreground
            {
                return Err(
                    "Command+Tab actor cannot combine with an activation actor/fallback".into(),
                );
            }
            Some(config)
        } else {
            None
        };
        Ok(Some(Self {
            run_id,
            state_dir,
            display_id,
            lifetime_ms,
            cover_controls,
            right_side,
            boundary_right,
            boundary_canvas_right,
            modal_controls,
            system_panel_controls,
            native_dnd_controls,
            environment_sample_ms,
            regular_app,
            hover_first_responder,
            self_foreground_after_ms,
            self_foreground_on_first_down_after_ms,
            legacy_self_foreground,
            ax_windows_delay_ms,
            command_tab,
            check_only,
        }))
    }
}

#[cfg(test)]
mod timed_foreground_contract {
    use super::*;
    #[test]
    fn legacy_foreground_method_requires_its_explicit_allocated_timer() {
        let base = [
            "--open-window",
            "--run-id",
            "own-legacy",
            "--state-dir",
            "/tmp/own-legacy",
            "--display-id",
            "4",
            "--regular-app",
        ];
        let parse = |extra: &[&str]| {
            Options::parse(
                base.into_iter()
                    .chain(extra.iter().copied())
                    .map(str::to_owned),
            )
        };
        assert!(!parse(&[]).unwrap().unwrap().legacy_self_foreground);
        assert!(parse(&["--legacy-self-foreground"]).is_err());
        let valid = [
            "--self-foreground-after-ms",
            "3500",
            "--legacy-self-foreground",
        ];
        assert!(parse(&valid).unwrap().unwrap().legacy_self_foreground);
        assert!(
            parse(&[
                "--self-foreground-after-ms",
                "3500",
                "--legacy-self-foreground",
                "--legacy-self-foreground",
            ])
            .is_err()
        );
        assert!(Options::parse(valid.into_iter().map(str::to_owned)).is_err());
    }
    #[test]
    fn allocated_timed_foreground_transition_requires_explicit_regular_window() {
        let options = Options::test_timed_foreground_options();
        assert!(options.regular_app);
        assert_eq!(options.display_id, 4);
        assert_eq!(options.lifetime_ms, 20000);
        assert_eq!(options.self_foreground_after_ms, Some(3500));
        assert!(!options.legacy_self_foreground);
    }
    #[test]
    fn timed_foreground_is_opt_in_bounded_and_cannot_bypass_window_allocation() {
        assert!(
            Options::parse(
                ["--self-foreground-after-ms", "3500"]
                    .into_iter()
                    .map(str::to_owned)
            )
            .is_err()
        );
        let base = [
            "--open-window",
            "--run-id",
            "own-only",
            "--state-dir",
            "/tmp/owned-only",
            "--display-id",
            "4",
        ];
        let parse = |extra: &[&str]| {
            Options::parse(
                base.into_iter()
                    .chain(extra.iter().copied())
                    .map(str::to_owned),
            )
        };
        assert_eq!(parse(&[]).unwrap().unwrap().self_foreground_after_ms, None);
        for extra in [
            vec!["--self-foreground-after-ms", "3500"],
            vec!["--regular-app", "--self-foreground-after-ms", "0"],
            vec!["--regular-app", "--self-foreground-after-ms", "60001"],
            vec![
                "--regular-app",
                "--lifetime-ms",
                "3000",
                "--self-foreground-after-ms",
                "3500",
            ],
            vec![
                "--regular-app",
                "--self-foreground-after-ms",
                "3500",
                "--self-foreground-after-ms",
                "3500",
            ],
        ] {
            assert!(parse(&extra).is_err());
        }
    }
}

#[cfg(test)]
mod responder_calibration_contract {
    use super::*;
    fn parse(extra: &[&str]) -> Result<Option<Options>, String> {
        Options::parse(
            [
                "--open-window",
                "--run-id",
                "calibration",
                "--state-dir",
                "/tmp/rwi-pure-calibration",
                "--display-id",
                "4",
            ]
            .into_iter()
            .chain(extra.iter().copied())
            .map(str::to_owned),
        )
    }
    #[test]
    fn first_responder_calibration_is_explicit_and_duplicate_flags_refuse() {
        assert!(!parse(&[]).unwrap().unwrap().hover_first_responder);
        assert!(
            parse(&["--hover-first-responder"])
                .unwrap()
                .unwrap()
                .hover_first_responder
        );
        assert!(parse(&["--hover-first-responder", "--hover-first-responder"]).is_err());
        assert!(
            Options::parse(["--hover-first-responder"].into_iter().map(str::to_owned)).is_err()
        );
    }
    #[test]
    fn responder_metadata_is_unknown_and_never_manufactures_mouse_tracking() {
        let state = State::new("readback".into());
        let value = serde_json::to_value(&state).unwrap();
        assert_eq!(value["responder"]["calibration_requested"], false);
        assert!(value["responder"]["calibration_accepted"].is_null());
        assert!(value["responder"]["first_responder_class"].is_null());
        assert!(value["responder"]["first_responder_region"].is_null());
        assert!(value["responder"]["accepts_mouse_moved"].is_null());
        assert_eq!(state.hover.moves, 0);
        assert_eq!(state.hover.enters, 0);
        assert_eq!(state.hover.tooltip_requests, 0);
        assert_eq!(state.dispatch_probe.application_events, 0);
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn contains(self, other: Self) -> bool {
        [
            self.x,
            self.y,
            self.width,
            self.height,
            other.x,
            other.y,
            other.width,
            other.height,
        ]
        .into_iter()
        .all(f64::is_finite)
            && self.width > 0.
            && self.height > 0.
            && other.width > 0.
            && other.height > 0.
            && other.x >= self.x
            && other.y >= self.y
            && other.x + other.width <= self.x + self.width
            && other.y + other.height <= self.y + self.height
    }
    pub fn place(self, width: f64, height: f64) -> Result<Self, &'static str> {
        let placed = Self {
            x: self.x + 24.,
            y: self.y + 24.,
            width,
            height,
        };
        self.contains(placed)
            .then_some(placed)
            .ok_or("fixture window cannot fit explicit display")
    }
    pub fn place_right(self, width: f64, height: f64) -> Result<Self, &'static str> {
        let mut placed = self.place(width, height)?;
        placed.x = self.x + self.width - width - 24.;
        self.contains(placed)
            .then_some(placed)
            .ok_or("fixture right-side window cannot fit explicit display")
    }
    pub fn place_boundary_right(self, width: f64, height: f64) -> Result<Self, &'static str> {
        let mut placed = self.place(width, height)?;
        placed.x = self.x + self.width - width * 0.75;
        Ok(placed)
    }

    pub fn place_boundary_canvas_right(
        self,
        width: f64,
        height: f64,
    ) -> Result<Self, &'static str> {
        let mut placed = self.place(width, height)?;
        // The unchanged canvas spans local x24..384. Its rightmost 120 points
        // now lie outside this allocated display; there is no event calibration.
        placed.x = self.x + self.width - 264.;
        Ok(placed)
    }
    pub fn same(self, other: Self) -> bool {
        [
            self.x - other.x,
            self.y - other.y,
            self.width - other.width,
            self.height - other.height,
        ]
        .into_iter()
        .all(|d| d.is_finite() && d.abs() < 0.01)
    }
}

#[cfg(test)]
mod boundary_geometry_contract {
    use super::*;
    fn options(extra: &[&str]) -> Result<Option<Options>, String> {
        Options::parse(
            [
                "--open-window",
                "--run-id",
                "own-boundary",
                "--state-dir",
                "/tmp/rwi-own-boundary",
                "--display-id",
                "4",
            ]
            .into_iter()
            .chain(extra.iter().copied())
            .map(str::to_owned),
        )
    }
    #[test]
    fn boundary_is_explicit_and_rejects_duplicate_or_mixed_allocations() {
        assert!(!options(&[]).unwrap().unwrap().boundary_right);
        assert!(
            options(&["--boundary-right"])
                .unwrap()
                .unwrap()
                .boundary_right
        );
        assert!(Options::parse(["--boundary-right"].into_iter().map(str::to_owned)).is_err());
        for extra in [
            "--boundary-right",
            "--right-side",
            "--cover-controls",
            "--modal-controls",
            "--system-panel-controls",
        ] {
            assert!(options(&["--boundary-right", extra]).is_err());
        }
    }
    #[test]
    fn boundary_geometry_keeps_three_quarters_on_allocated_screen_and_checks_readback() {
        let screen = Rect {
            x: 1800.,
            y: 89.,
            width: 1920.,
            height: 1080.,
        };
        let own = screen.place_boundary_right(660., 552.).unwrap();
        assert_eq!(own.x, 3225.);
        assert_eq!(own.y, 113.);
        assert_eq!(screen.x + screen.width - own.x, own.width * 0.75);
        assert!(!screen.contains(own));
        assert!(own.same(own));
        assert!(!own.same(Rect {
            x: own.x - 1.,
            ..own
        }));
        assert!(!own.same(Rect { x: f64::NAN, ..own }));
        assert!(
            Rect {
                width: 600.,
                ..screen
            }
            .place_boundary_right(660., 552.)
            .is_err()
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    HoverZone,
    Canvas,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseKind {
    Entered,
    Exited,
    Moved,
    Down,
    Dragged,
    Up,
}
#[derive(Clone, Debug, Serialize)]
pub struct MouseEvent {
    pub region: Region,
    pub kind: MouseKind,
    pub local_point: [f64; 2],
    pub window_point: [f64; 2],
    pub native_type: usize,
    pub window_number: isize,
    pub receiving_app_active: bool,
    pub receiving_window_key: bool,
    pub frontmost_pid: Option<i32>,
    pub receiving_app_frontmost: Option<bool>,
    pub timestamp_seconds: f64,
    pub modifier_flags: usize,
    // NSEvent.h does not guarantee eventNumber for mouseMoved.
    pub event_number: Option<isize>,
    pub pressure: Option<f32>,
    /// Raw AppKit delta for moved/dragged only; not converted to view coordinates.
    pub delta: Option<[f64; 2]>,
}
impl MouseEvent {
    pub(super) fn finite(&self) -> bool {
        self.local_point
            .into_iter()
            .chain(self.window_point)
            .chain([self.timestamp_seconds])
            .all(f64::is_finite)
            && self.pressure.is_none_or(f32::is_finite)
            && self
                .delta
                .is_none_or(|delta| delta.into_iter().all(f64::is_finite))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchStage {
    Application,
    Window,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchRegion {
    SafeButton,
    HoverZone,
    Canvas,
    OtherOwnContent,
}
#[derive(Clone, Debug, Serialize)]
pub struct DispatchEvent {
    pub sequence: u64,
    pub stage: DispatchStage,
    pub native_type: usize,
    pub window_number: isize,
    /// Unmodified AppKit window points, not screenshot or desktop pixels.
    pub window_point: [f64; 2],
    /// Tracking enter/exit events do not support the ordinary mouse accessor.
    pub subtype: Option<i16>,
    pub timestamp_seconds: f64,
    pub own_window_match: bool,
    /// Public own-content hitTest snapshot, not a received view callback.
    pub hit_region: Option<DispatchRegion>,
    /// Read-only public framework state, not a view receipt or HID attestation.
    pub framework_pointer: Option<FrameworkPointerSnapshot>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct FrameworkPointerSnapshot {
    /// AppKit screen points, lower-left origin; never screenshot coordinates.
    pub screen_point: Option<[f64; 2]>,
    pub screen_converted_to_own_window: Option<[f64; 2]>,
    pub own_window_outside_event_stream: Option<[f64; 2]>,
    pub screen_converted_to_own_hover: Option<[f64; 2]>,
    pub screen_point_in_own_hover: Option<bool>,
}
pub fn is_dispatch_mouse_type(native_type: usize) -> bool {
    // NSEvent.h mouse down/up/moved/dragged/entered/exited/other button types.
    // Keyboard, flags, scroll, tablet and own ApplicationDefined wake excluded.
    matches!(native_type, 1..=9 | 25..=27)
}
pub fn dispatch_subtype_supported(native_type: usize) -> bool {
    // Enter/exit are tracking events, not ordinary mouse events. On the tested
    // AppKit, calling their subtype getter raises an Objective-C exception.
    matches!(native_type, 1..=7 | 25..=27)
}
#[derive(Default, Serialize)]
pub struct DispatchProbe {
    pub application_events: u64,
    pub window_events: u64,
    pub own_window_events: u64,
    pub invalid_events: u64,
    pub borrow_skips: u64,
    pub dropped_records: u64,
    pub records: VecDeque<DispatchEvent>,
}
impl DispatchProbe {
    pub fn receive(&mut self, mut event: DispatchEvent) {
        if !is_dispatch_mouse_type(event.native_type)
            || !event
                .window_point
                .into_iter()
                .chain([event.timestamp_seconds])
                .all(f64::is_finite)
            || (!event.own_window_match && event.hit_region.is_some())
        {
            self.invalid_events += 1;
            return;
        }
        match event.stage {
            DispatchStage::Application => self.application_events += 1,
            DispatchStage::Window => self.window_events += 1,
        }
        self.own_window_events += u64::from(event.own_window_match);
        event.sequence = self.application_events + self.window_events;
        if self.records.len() == HISTORY_LIMIT {
            self.records.pop_front();
            self.dropped_records += 1;
        }
        self.records.push_back(event);
    }
}

#[derive(Default, Serialize)]
pub struct ModalOracle {
    pub open_actions: u64,
    pub safe_actions: u64,
    pub close_actions: u64,
    pub window_number: Option<isize>,
    pub visible: Option<bool>,
    pub is_sheet: Option<bool>,
    pub attached_to_own_parent: Option<bool>,
    pub window_key: Option<bool>,
    pub frame: Option<Rect>,
    pub display_id: Option<u32>,
}

#[derive(Default, Serialize)]
pub struct Hover {
    pub inside: bool,
    pub enters: u64,
    pub exits: u64,
    pub moves: u64,
    pub last_point: Option<[f64; 2]>,
    /// Registration is not proof that AppKit actually displayed a tooltip.
    pub tooltip_registered: bool,
    /// A native request for tooltip text is not proof of a visible tooltip.
    pub tooltip_requests: u64,
    /// Raw NSView-local AppKit point provided to the tooltip owner callback.
    pub last_tooltip_point: Option<[f64; 2]>,
}
#[derive(Clone, Serialize)]
pub struct DragPath {
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub moves: u64,
    pub points: VecDeque<[f64; 2]>,
    pub dropped_points: u64,
}
impl DragPath {
    fn new(point: [f64; 2]) -> Self {
        Self {
            start: point,
            end: point,
            moves: 0,
            points: VecDeque::from([point]),
            dropped_points: 0,
        }
    }
    fn append(&mut self, point: [f64; 2]) {
        self.end = point;
        if self.points.len() == PATH_LIMIT {
            self.points.pop_front();
            self.dropped_points += 1;
        }
        self.points.push_back(point);
    }
}
#[derive(Default, Serialize)]
pub struct Drag {
    pub downs: u64,
    pub moves: u64,
    pub ups: u64,
    pub completed: u64,
    pub click_without_drag: u64,
    pub orphan_moves: u64,
    pub orphan_ups: u64,
    pub cancelled: u64,
    pub active: Option<DragPath>,
    pub last_completed: Option<DragPath>,
}
#[derive(Serialize)]
pub struct Record {
    pub sequence: u64,
    pub action: &'static str,
    pub receiving_app_active: Option<bool>,
    pub frontmost_pid: Option<i32>,
    pub receiving_app_frontmost: Option<bool>,
    pub mouse: Option<MouseEvent>,
}
/// Bounded consumed state of this fixture's own nonsecure text field.
#[derive(Default, Serialize)]
pub struct TextOracle {
    pub changes: u64,
    pub value: String,
    pub truncated: bool,
}
/// Public NSClipView bounds read after normal NSWindow scroll processing.
#[derive(Default, Serialize)]
pub struct ScrollOracle {
    pub events: u64,
    pub clip_origin: Option<[f64; 2]>,
    pub last_delta: Option<[f64; 2]>,
}

/// Observed retained AppKit shared panel. Requests and native states are distinct.
#[derive(Default, Serialize)]
pub struct SystemPanelOracle {
    pub open_actions: u64,
    pub window_number: Option<isize>,
    pub visible: Option<bool>,
    pub window_key: Option<bool>,
    pub frame: Option<Rect>,
    pub display_id: Option<u32>,
    pub is_sheet: Option<bool>,
    pub attached_to_own_parent: Option<bool>,
    /// AppKit visibility alone is not a WindowServer onscreen assertion.
    pub hides_on_deactivate: Option<bool>,
    pub on_active_space: Option<bool>,
    pub occlusion_state: Option<usize>,
    /// Exact retained panel ID only; no desktop capture or input authority.
    pub quartz_description: Option<serde_json::Value>,
    /// Only a real native file-panel completion callback supplies this value.
    pub completion_response: Option<isize>,
    pub completion_callbacks_dropped: u64,
}

#[cfg(test)]
#[test]
fn shared_panel_visibility_diagnostics_remain_unknown_until_native_readback() {
    let data = serde_json::to_value(SystemPanelOracle::default()).unwrap();
    for key in [
        "hides_on_deactivate",
        "on_active_space",
        "occlusion_state",
        "quartz_description",
    ] {
        assert!(data.get(key).is_some(), "missing diagnostic {key}");
        assert!(data[key].is_null(), "unobserved {key} must not mean false");
    }
}
#[derive(Default, Serialize)]
pub struct SystemPanels {
    pub enabled: bool,
    pub color: SystemPanelOracle,
    pub font: SystemPanelOracle,
    pub file: SystemPanelOracle,
}
/// Public own-window responder readback, separate from consumed input counters.
#[derive(Default, Serialize)]
pub struct ResponderProbe {
    /// Public getter on the allocated own window; unknown before native sampling.
    pub window_miniaturized: Option<bool>,
    pub calibration_requested: bool,
    pub calibration_accepted: Option<bool>,
    pub first_responder_class: Option<String>,
    pub first_responder_region: Option<Region>,
    pub accepts_mouse_moved: Option<bool>,
    pub hover_tracking_areas: Option<usize>,
}

/// Own application metadata, not proof of a Dock entry or a user's selection.
#[derive(Default, Serialize)]
pub struct ApplicationIdentity {
    pub activation_policy_before: Option<isize>,
    pub activation_policy_request_accepted: Option<bool>,
    pub activation_policy: Option<isize>,
    pub bundle_identifier: Option<String>,
    pub localized_name: Option<String>,
}

/// A false no-op acknowledgement is usable only with matching before/after
/// native readback. Never accept an unacknowledged change or another policy.
pub fn activation_policy_ready(
    before: isize,
    policy: isize,
    accepted: bool,
    observed: isize,
) -> bool {
    matches!(policy, 0 | 1) && observed == policy && (accepted || before == policy)
}

#[cfg(test)]
#[test]
fn activation_policy_noop_requires_exact_existing_and_final_native_policy() {
    assert!(activation_policy_ready(0, 0, false, 0));
    assert!(activation_policy_ready(1, 1, false, 1));
    assert!(activation_policy_ready(1, 0, true, 0));
    assert!(!activation_policy_ready(1, 0, false, 0));
    assert!(!activation_policy_ready(0, 0, true, 1));
    assert!(!activation_policy_ready(0, 0, false, -1));
    assert!(!activation_policy_ready(2, 2, true, 2));
}

/// One sampled state. Missing native values remain unknown rather than false.
#[derive(Serialize)]
pub struct EnvironmentSample {
    pub unix_ms: u64,
    pub probe_tick: u64,
    pub input_sequence: u64,
    pub app_active: Option<bool>,
    pub window_key: Option<bool>,
    pub frontmost_pid: Option<i32>,
    pub app_frontmost: Option<bool>,
}
impl EnvironmentSample {
    fn same_state(&self, other: &Self) -> bool {
        self.app_active == other.app_active
            && self.window_key == other.window_key
            && self.frontmost_pid == other.frontmost_pid
            && self.app_frontmost == other.app_frontmost
    }
}

/// Initial sample and changes only; bounded, independent of input receipts.
#[derive(Default, Serialize)]
pub struct EnvironmentHistory {
    pub observed_changes: u64,
    pub dropped_records: u64,
    pub physical_human_origin_attested: bool,
    pub records: VecDeque<EnvironmentSample>,
}
impl EnvironmentHistory {
    fn observe(&mut self, sample: EnvironmentSample) {
        if self
            .records
            .back()
            .is_some_and(|last| last.same_state(&sample))
        {
            return;
        }
        self.observed_changes = self.observed_changes.saturating_add(1);
        if self.records.len() == HISTORY_LIMIT {
            self.records.pop_front();
            self.dropped_records = self.dropped_records.saturating_add(1);
        }
        self.records.push_back(sample);
    }
}

#[cfg(test)]
#[test]
fn environment_history_keeps_unknowns_and_does_not_record_input_sequence_as_a_transition() {
    let mut state = State::new("history-unknown".into());
    state.environment_sample_ms = Some(100);
    state.note_environment_probe(1);
    assert!(state.environment_history.records.is_empty());
    state.status = "running";
    state.note_environment_probe(2);
    assert_eq!(state.environment_history.observed_changes, 1);
    let first = state.environment_history.records.front().unwrap();
    assert_eq!(first.app_active, None);
    assert_eq!(first.window_key, None);
    assert_eq!(first.frontmost_pid, None);
    assert_eq!(first.app_frontmost, None);
    assert!(!state.environment_history.physical_human_origin_attested);
    assert!(state.records.is_empty());
    state.sequence = 9;
    state.note_environment_probe(3);
    assert_eq!(state.environment_probe_ticks, 2);
    assert_eq!(state.environment_history.records.len(), 1);
    assert_eq!(state.sequence, 9);
    assert_eq!(state.drag.completed, 0);
    assert_eq!(state.application_identity.activation_policy, None);
    assert_eq!(state.application_identity.bundle_identifier, None);
}

#[cfg(test)]
#[test]
fn environment_history_retains_initial_and_real_state_changes_without_input_receipts() {
    let mut state = State::new("history-change".into());
    state.status = "running";
    state.environment_sample_ms = Some(100);
    state.app_frontmost = Some(false);
    state.note_environment_probe(100);
    state.app_frontmost = Some(true);
    state.window_key = Some(true);
    state.note_environment_probe(200);
    state.note_environment_probe(300);
    assert_eq!(state.environment_history.observed_changes, 2);
    assert_eq!(
        state.environment_history.records.back().unwrap().unix_ms,
        200
    );
    assert_eq!(
        state.environment_history.records.back().unwrap().probe_tick,
        2
    );
    assert_eq!(state.sequence, 0);
    assert!(state.dispatch_probe.records.is_empty());
    state.close();
    let count = state.environment_probe_ticks;
    state.note_environment_probe(400);
    assert_eq!(state.environment_probe_ticks, count);
    assert_eq!(state.environment_history.observed_changes, 2);
}

#[cfg(test)]
#[test]
fn environment_history_is_bounded_and_accounts_for_eviction() {
    let mut state = State::new("history-bound".into());
    state.status = "running";
    state.environment_sample_ms = Some(25);
    for tick in 0..HISTORY_LIMIT + 5 {
        state.window_key = Some(tick % 2 == 0);
        state.note_environment_probe(tick as u64);
    }
    assert_eq!(state.environment_history.records.len(), HISTORY_LIMIT);
    assert_eq!(state.environment_history.dropped_records, 5);
    assert_eq!(
        state.environment_history.observed_changes,
        (HISTORY_LIMIT + 5) as u64
    );
    assert_eq!(
        state.environment_history.records.front().unwrap().unix_ms,
        5
    );
    assert!(state.records.is_empty());
    assert_eq!(state.sequence, 0);
    assert_eq!(state.safe_button_actions, 0);
}

#[derive(Serialize)]
pub struct State {
    pub schema_version: u32,
    pub run_id: String,
    pub pid: u32,
    pub window_title: String,
    pub window_number: Option<isize>,
    pub cover_window_number: Option<isize>,
    pub cover_frame: Option<Rect>,
    pub cover_visible: Option<bool>,
    pub cover_above_primary: Option<bool>,
    pub cover_display_id: Option<u32>,
    pub app_active: Option<bool>,
    pub window_key: Option<bool>,
    pub frontmost_pid: Option<i32>,
    pub app_frontmost: Option<bool>,
    pub requested_display_id: Option<u32>,
    pub actual_display_id: Option<u32>,
    pub lifetime_ms: Option<u32>,
    pub lifetime_close_requests: u64,
    pub environment_sample_ms: Option<u32>,
    pub environment_sampled_unix_ms: Option<u64>,
    pub environment_probe_ticks: u64,
    pub environment_history: EnvironmentHistory,
    pub application_identity: ApplicationIdentity,
    pub self_foreground_after_ms: Option<u32>,
    pub self_foreground_on_first_down_after_ms: Option<u32>,
    pub self_foreground_first_down_sequence: Option<u64>,
    pub self_foreground_first_down_unix_ms: Option<u64>,
    pub self_foreground_first_down_event_timestamp_seconds: Option<f64>,
    pub self_foreground_requests: u64,
    pub self_foreground_method: Option<&'static str>,
    pub requested_window_frame: Option<Rect>,
    pub actual_window_frame: Option<Rect>,
    pub local_coordinates: &'static str,
    pub status: &'static str,
    pub sequence: u64,
    pub safe_button_actions: u64,
    pub modal: ModalOracle,
    pub system_panels: SystemPanels,
    pub hover: Hover,
    pub drag: Drag,
    pub native_dnd: super::native_dnd::Oracle,
    pub text: TextOracle,
    pub scroll: ScrollOracle,
    pub dispatch_probe: DispatchProbe,
    pub responder: ResponderProbe,
    pub invalid_events: u64,
    pub dropped_records: u64,
    pub records: VecDeque<Record>,
    pub output_error: Option<String>,
    pub ax_provider_probe: serde_json::Value,
    pub command_tab: command_tab::Oracle,
}
impl State {
    pub fn new(run_id: String) -> Self {
        let pid = std::process::id();
        Self {
            schema_version: 1,
            command_tab: command_tab::Oracle::default(),
            window_title: format!("RWI 验收 · Receiver · {run_id} · PID {pid}"),
            run_id,
            pid,
            window_number: None,
            cover_window_number: None,
            cover_frame: None,
            cover_visible: None,
            cover_above_primary: None,
            cover_display_id: None,
            app_active: None,
            window_key: None,
            frontmost_pid: None,
            app_frontmost: None,
            requested_display_id: None,
            actual_display_id: None,
            lifetime_ms: None,
            lifetime_close_requests: 0,
            environment_sample_ms: None,
            environment_sampled_unix_ms: None,
            environment_probe_ticks: 0,
            environment_history: EnvironmentHistory::default(),
            application_identity: ApplicationIdentity::default(),
            self_foreground_after_ms: None,
            self_foreground_on_first_down_after_ms: None,
            self_foreground_first_down_sequence: None,
            self_foreground_first_down_unix_ms: None,
            self_foreground_first_down_event_timestamp_seconds: None,
            self_foreground_requests: 0,
            self_foreground_method: None,
            requested_window_frame: None,
            actual_window_frame: None,
            local_coordinates: "view-local AppKit points; flipped upper-left origin",
            status: "prepared",
            sequence: 0,
            safe_button_actions: 0,
            modal: ModalOracle::default(),
            system_panels: SystemPanels::default(),
            hover: Hover::default(),
            drag: Drag::default(),
            native_dnd: super::native_dnd::Oracle::default(),
            text: TextOracle::default(),
            scroll: ScrollOracle::default(),
            dispatch_probe: DispatchProbe::default(),
            responder: ResponderProbe::default(),
            invalid_events: 0,
            dropped_records: 0,
            records: VecDeque::new(),
            output_error: None,
            ax_provider_probe: serde_json::json!({"requested_delay_ms":0,"getter_calls":0,"delayed_getter_calls":0,"AX_error_attested":false,"input_sent":false}),
        }
    }
    pub fn note_environment_probe(&mut self, unix_ms: u64) {
        if self.status == "running" && self.environment_sample_ms.is_some() {
            self.environment_sampled_unix_ms = Some(unix_ms);
            self.environment_probe_ticks = self.environment_probe_ticks.saturating_add(1);
            self.environment_history.observe(EnvironmentSample {
                unix_ms,
                probe_tick: self.environment_probe_ticks,
                input_sequence: self.sequence,
                app_active: self.app_active,
                window_key: self.window_key,
                frontmost_pid: self.frontmost_pid,
                app_frontmost: self.app_frontmost,
            });
        }
    }
    fn record(&mut self, action: &'static str, mouse: Option<MouseEvent>) {
        self.sequence += 1;
        if self.records.len() == HISTORY_LIMIT {
            self.records.pop_front();
            self.dropped_records += 1;
        }
        self.records.push_back(Record {
            sequence: self.sequence,
            action,
            receiving_app_active: self.app_active,
            frontmost_pid: self.frontmost_pid,
            receiving_app_frontmost: self.app_frontmost,
            mouse,
        });
    }
    /// Only an explicitly enabled running own fixture may request its panels.
    /// This counter does not imply visible/key/modal/ownership/goal success.
    pub fn system_panel_open(&mut self, kind: &'static str) -> bool {
        if !self.system_panels.enabled || self.status != "running" {
            return false;
        }
        let action = match kind {
            "color" => {
                self.system_panels.color.open_actions += 1;
                "fixture_system_color_open_action"
            }
            "font" => {
                self.system_panels.font.open_actions += 1;
                "fixture_system_font_open_action"
            }
            "file" => {
                self.system_panels.file.open_actions += 1;
                "fixture_system_file_open_action"
            }
            _ => return false,
        };
        self.record(action, None);
        true
    }
    /// A native completion response is not a window-visibility or owner proof.
    pub fn native_file_panel_completed(&mut self, response: isize) {
        self.system_panels.file.completion_response = Some(response);
        self.record("fixture_native_file_panel_completion", None);
    }
    pub fn modal_action(&mut self, action: &'static str) {
        match action {
            "fixture_open_sheet_action" => self.modal.open_actions += 1,
            "fixture_sheet_safe_action" => self.modal.safe_actions += 1,
            "fixture_close_sheet_action" => self.modal.close_actions += 1,
            _ => return,
        }
        self.record(action, None);
    }
    pub fn click(&mut self) {
        self.safe_button_actions += 1;
        self.record("nsbutton_target_action", None);
    }
    pub fn request_lifetime_close(&mut self) {
        self.lifetime_close_requests += 1;
        // A timer request is not a real window close notification or proof.
        self.record("lifetime_timer_close_requested", None);
    }
    /// Arm only the explicit own regular-window actor. This records a cause,
    /// not an activation request, a delivered input receipt or an observed goal.
    pub fn arm_foreground_on_first_canvas_down(
        &mut self,
        event: &MouseEvent,
        unix_ms: Option<u64>,
    ) -> Option<u32> {
        let delay = self.self_foreground_on_first_down_after_ms?;
        if !(100..=5000).contains(&delay)
            || self.status != "running"
            || self.application_identity.activation_policy != Some(0)
            || self.window_number != Some(event.window_number)
            || event.region != Region::Canvas
            || event.kind != MouseKind::Down
            || event.native_type != 1
            || !event.finite()
            || self.drag.downs != 0
            || self.self_foreground_first_down_sequence.is_some()
        {
            return None;
        }
        self.self_foreground_first_down_sequence = Some(self.sequence.checked_add(1)?);
        self.self_foreground_first_down_unix_ms = unix_ms;
        self.self_foreground_first_down_event_timestamp_seconds = Some(event.timestamp_seconds);
        Some(delay)
    }
    pub fn request_self_foreground(&mut self) {
        self.self_foreground_requests = self.self_foreground_requests.saturating_add(1);
        // An app's own request is not proof that the OS completed a transition.
        self.record("fixture_own_foreground_requested", None);
    }
    pub fn tooltip_requested(&mut self, point: [f64; 2]) {
        if !point.into_iter().all(f64::is_finite) {
            self.invalid_events += 1;
            self.record("rejected_nonfinite_tooltip_request", None);
            return;
        }
        self.hover.tooltip_requests += 1;
        self.hover.last_tooltip_point = Some(point);
        // AppKit asks for text separately from tracking/mouse callbacks. Do not
        // fabricate mouseEntered, inside, a mouse receipt, or actual visibility.
        self.record("nstooltip_owner_text_request", None);
    }
    pub fn receive(&mut self, event: MouseEvent) {
        if !event.finite() {
            self.invalid_events += 1;
            self.record("rejected_nonfinite_mouse_event", None);
            return;
        }
        if event.region == Region::HoverZone {
            self.hover.last_point = Some(event.local_point);
            match event.kind {
                MouseKind::Entered => {
                    self.hover.inside = true;
                    self.hover.enters += 1;
                }
                MouseKind::Exited => {
                    self.hover.inside = false;
                    self.hover.exits += 1;
                }
                MouseKind::Moved => self.hover.moves += 1,
                _ => {}
            }
        } else {
            match event.kind {
                MouseKind::Down => {
                    self.drag.downs += 1;
                    if self.drag.active.is_some() {
                        self.drag.cancelled += 1;
                    }
                    self.drag.active = Some(DragPath::new(event.local_point));
                }
                MouseKind::Dragged => {
                    self.drag.moves += 1;
                    if let Some(path) = &mut self.drag.active {
                        path.moves += 1;
                        path.append(event.local_point);
                    } else {
                        self.drag.orphan_moves += 1;
                    }
                }
                MouseKind::Up => {
                    self.drag.ups += 1;
                    if let Some(mut path) = self.drag.active.take() {
                        path.append(event.local_point);
                        if path.moves == 0 {
                            self.drag.click_without_drag += 1;
                        } else {
                            self.drag.completed += 1;
                            self.drag.last_completed = Some(path);
                        }
                    } else {
                        self.drag.orphan_ups += 1;
                    }
                }
                _ => {}
            }
        }
        self.record("nsresponder_mouse_callback", Some(event));
    }
    pub fn close(&mut self) {
        if self.drag.active.take().is_some() {
            self.drag.cancelled += 1;
        }
        self.status = "closed";
        self.record("window_closed", None);
    }
    pub fn text_changed(&mut self, value: &str) {
        self.text.changes += 1;
        self.text.value = value.chars().take(1024).collect();
        self.text.truncated = value.chars().count() > 1024;
        self.record("native_text_field_changed", None);
    }
    pub fn scroll_received(&mut self, delta: [f64; 2], clip: [f64; 2]) {
        if !delta.iter().chain(clip.iter()).all(|n| n.is_finite()) {
            self.invalid_events += 1;
            return;
        }
        self.scroll.events += 1;
        self.scroll.last_delta = Some(delta);
        self.scroll.clip_origin = Some(clip);
        self.record("native_scroll_clip_readback", None);
    }
    pub fn summary(&self) -> String {
        if self.native_dnd.enabled {
            let d = &self.native_dnd;
            return format!(
                "Native sessions requested/began/ended: {}/{}/{} · active: {:?}\n\
                 Native drops consumed: {} · external source: {:?} · last operation: {:?}\n\
                 Last payload: {} · status: {}{}",
                d.source_requests,
                d.source_began,
                d.source_ended,
                d.active_sequence,
                d.consumed,
                d.last_source_external,
                d.last_source_operation,
                d.last_payload
                    .as_ref()
                    .map(|p| p.value.as_str())
                    .unwrap_or("unobserved"),
                self.status,
                if self.output_error.is_some() {
                    " · JSON OUTPUT ERROR"
                } else {
                    ""
                }
            );
        }
        format!(
            "Safe actions: {} · Hover enter/move/exit: {}/{}/{} · Tooltip requests: {}\n\
             Drag completed: {} · moves: {} · orphan move/up: {}/{} · active: {}\n\
             Sequence: {} · status: {}{}",
            self.safe_button_actions,
            self.hover.enters,
            self.hover.moves,
            self.hover.exits,
            self.hover.tooltip_requests,
            self.drag.completed,
            self.drag.moves,
            self.drag.orphan_moves,
            self.drag.orphan_ups,
            self.drag.active.is_some(),
            self.sequence,
            self.status,
            if self.output_error.is_some() {
                " · JSON OUTPUT ERROR"
            } else {
                ""
            }
        )
    }
}

pub struct Store {
    pub path: PathBuf,
}
impl Store {
    /// The caller must supply a new directory; never overwrite another run.
    pub fn create(directory: &std::path::Path) -> io::Result<Self> {
        let parent = directory
            .parent()
            .ok_or_else(|| io::Error::other("state directory parent missing"))?
            .canonicalize()?;
        let child = directory
            .file_name()
            .ok_or_else(|| io::Error::other("state directory child missing"))?;
        let directory = parent.join(child);
        let builder = fs::DirBuilder::new();
        #[cfg(unix)]
        let mut builder = builder;
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&directory)?;
        Ok(Self {
            path: directory.join("state.json"),
        })
    }
    /// Atomic same-directory rename: concurrent readers see complete JSON only.
    pub fn persist(&self, state: &State) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(state)?;
        let temporary = self
            .path
            .with_file_name(format!(".state-{}-{}.tmp", state.pid, state.sequence));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        // If create_new fails, the temporary file is not ours to remove.
        let mut file = options.open(&temporary)?;
        let result = (|| {
            file.write_all(&bytes)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            fs::rename(&temporary, &self.path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dispatch_probe_schema_is_explicit_and_independent_from_view_receipts() {
        let state = State::new("schema-probe".into());
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(
            json["dispatch_probe"]["application_events"],
            serde_json::json!(0)
        );
        assert_eq!(
            json["dispatch_probe"]["window_events"],
            serde_json::json!(0)
        );
        assert_eq!(
            json["dispatch_probe"]["dropped_records"],
            serde_json::json!(0)
        );
        assert_eq!(json["dispatch_probe"]["records"], serde_json::json!([]));
        assert_eq!(state.hover.moves, 0);
        assert_eq!(state.hover.enters, 0);
        assert_eq!(state.hover.tooltip_requests, 0);
        assert_eq!(state.safe_button_actions, 0);
        assert_eq!(state.drag.completed, 0);
    }
    fn dispatch(stage: DispatchStage) -> DispatchEvent {
        DispatchEvent {
            sequence: 0,
            stage,
            native_type: 5,
            window_number: 42,
            window_point: [12.25, 34.5],
            subtype: Some(-3),
            timestamp_seconds: 1.5,
            own_window_match: true,
            hit_region: Some(DispatchRegion::HoverZone),
            framework_pointer: None,
        }
    }
    #[test]
    fn dispatch_stage_receipts_preserve_raw_values_without_view_or_tooltip_receipts() {
        let mut state = State::new("pure-dispatch".into());
        for stage in [DispatchStage::Application, DispatchStage::Window] {
            state.dispatch_probe.receive(dispatch(stage));
        }
        assert_eq!(state.dispatch_probe.application_events, 1);
        assert_eq!(state.dispatch_probe.window_events, 1);
        assert_eq!(state.dispatch_probe.own_window_events, 2);
        assert_eq!(state.dispatch_probe.records[0].sequence, 1);
        assert_eq!(state.dispatch_probe.records[1].sequence, 2);
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["dispatch_probe"]["records"][0]["subtype"], -3);
        assert_eq!(
            json["dispatch_probe"]["records"][0]["window_point"],
            serde_json::json!([12.25, 34.5])
        );
        assert_eq!(json["dispatch_probe"]["records"][1]["stage"], "window");
        assert_eq!(state.hover.moves, 0);
        assert_eq!(state.hover.enters, 0);
        assert!(!state.hover.inside);
        assert_eq!(state.hover.tooltip_requests, 0);
        assert_eq!(state.drag.completed, 0);
        assert!(state.records.is_empty());
        assert_eq!(state.sequence, 0);
    }
    #[test]
    fn dispatch_mouse_filter_excludes_keyboard_scroll_tablet_and_own_wake() {
        for native_type in (1..=9).chain(25..=27) {
            assert!(is_dispatch_mouse_type(native_type));
        }
        let mut probe = DispatchProbe::default();
        for native_type in [0, 10, 11, 12, 13, 14, 15, 16, 17, 22, 23, 24, 28, 29, 64] {
            assert!(!is_dispatch_mouse_type(native_type));
            let mut event = dispatch(DispatchStage::Application);
            event.native_type = native_type;
            probe.receive(event);
        }
        assert_eq!(probe.invalid_events, 15);
        assert_eq!(probe.application_events, 0);
        assert!(probe.records.is_empty());
    }
    #[test]
    fn dispatch_subtype_filter_keeps_tracking_events_without_reading_invalid_field() {
        for native_type in (1..=9).chain(25..=27) {
            assert!(is_dispatch_mouse_type(native_type));
            assert_eq!(
                dispatch_subtype_supported(native_type),
                !matches!(native_type, 8 | 9)
            );
        }
        for native_type in [0, 10, 12, 13, 14, 15, 17, 22, 23, 24, 28, usize::MAX] {
            assert!(!dispatch_subtype_supported(native_type));
        }
        let mut probe = DispatchProbe::default();
        for native_type in [8, 9] {
            let mut event = dispatch(DispatchStage::Application);
            event.native_type = native_type;
            event.subtype = None;
            probe.receive(event);
        }
        assert_eq!(probe.application_events, 2);
        assert_eq!(probe.invalid_events, 0);
        assert!(serde_json::to_value(&probe).unwrap()["records"][0]["subtype"].is_null());
    }
    #[test]
    fn dispatch_history_is_bounded_without_truncating_stage_counters() {
        let mut probe = DispatchProbe::default();
        for index in 0..400 {
            let stage = if index % 2 == 0 {
                DispatchStage::Application
            } else {
                DispatchStage::Window
            };
            probe.receive(dispatch(stage));
        }
        assert_eq!(probe.application_events, 200);
        assert_eq!(probe.window_events, 200);
        assert_eq!(probe.records.len(), HISTORY_LIMIT);
        assert_eq!(probe.dropped_records, 400 - HISTORY_LIMIT as u64);
        assert_eq!(probe.records.front().unwrap().sequence, 273);
        assert_eq!(probe.records.back().unwrap().sequence, 400);
    }
    #[test]
    fn dispatch_rejects_nonfinite_or_false_owned_hit_without_fabricating_receipts() {
        let mut probe = DispatchProbe::default();
        for point in [[f64::NAN, 0.], [0., f64::INFINITY]] {
            let mut event = dispatch(DispatchStage::Application);
            event.window_point = point;
            probe.receive(event);
        }
        let mut event = dispatch(DispatchStage::Window);
        event.own_window_match = false;
        probe.receive(event);
        assert_eq!(probe.invalid_events, 3);
        assert!(probe.records.is_empty());
        let mut unmatched = dispatch(DispatchStage::Application);
        unmatched.own_window_match = false;
        unmatched.hit_region = None;
        probe.receive(unmatched);
        assert_eq!(probe.application_events, 1);
        assert_eq!(probe.own_window_events, 0);
        assert_eq!(probe.records[0].hit_region, None);
    }
    fn mouse(kind: MouseKind, region: Region, point: [f64; 2]) -> MouseEvent {
        MouseEvent {
            region,
            kind,
            local_point: point,
            window_point: point,
            native_type: 1,
            window_number: 42,
            receiving_app_active: false,
            receiving_window_key: false,
            frontmost_pid: Some(99),
            receiving_app_frontmost: Some(false),
            timestamp_seconds: 1.0,
            modifier_flags: 0,
            event_number: (kind != MouseKind::Moved).then_some(1),
            pressure: Some(0.5),
            delta: matches!(kind, MouseKind::Moved | MouseKind::Dragged).then_some([0., 0.]),
        }
    }

    #[test]
    fn right_side_setup_is_explicit_allocated_and_non_overlapping_when_space_allows() {
        let base = [
            "--open-window",
            "--run-id",
            "right",
            "--state-dir",
            "/tmp/rwi-right",
            "--display-id",
            "4",
        ];
        let parse = |extra: &[&str]| {
            Options::parse(
                base.into_iter()
                    .chain(extra.iter().copied())
                    .map(str::to_owned),
            )
        };
        assert!(!parse(&[]).unwrap().unwrap().right_side);
        assert!(parse(&["--right-side"]).unwrap().unwrap().right_side);
        assert!(parse(&["--right-side", "--right-side"]).is_err());
        assert!(Options::parse(["--right-side"].into_iter().map(str::to_owned)).is_err());
        let screen = Rect {
            x: 1800.,
            y: 0.,
            width: 1920.,
            height: 1080.,
        };
        let left = screen.place(660., 552.).unwrap();
        let right = screen.place_right(660., 552.).unwrap();
        assert_eq!(right.x, 3036.);
        assert!(right.x >= left.x + left.width);
        assert!(screen.contains(right));
        assert!(screen.place_right(2000., 552.).is_err());
        assert!(screen.place_right(f64::NAN, 552.).is_err());
    }
    #[test]
    fn first_down_foreground_actor_is_allocated_bounded_and_opt_in() {
        let base = [
            "--open-window",
            "--run-id",
            "first-down",
            "--state-dir",
            "/tmp/rwi-first-down",
            "--display-id",
            "4",
        ];
        let parse = |extra: &[&str]| {
            Options::parse(
                base.into_iter()
                    .chain(extra.iter().copied())
                    .map(str::to_owned),
            )
        };
        assert_eq!(
            parse(&[])
                .unwrap()
                .unwrap()
                .self_foreground_on_first_down_after_ms,
            None
        );
        let valid = [
            "--regular-app",
            "--self-foreground-on-first-down-after-ms",
            "1000",
        ];
        let options = parse(&valid).unwrap().unwrap();
        assert_eq!(options.self_foreground_on_first_down_after_ms, Some(1000));
        assert_eq!(options.self_foreground_after_ms, None);
        let legacy = [
            "--regular-app",
            "--self-foreground-on-first-down-after-ms",
            "1000",
            "--legacy-self-foreground",
        ];
        assert!(parse(&legacy).unwrap().unwrap().legacy_self_foreground);
        for extra in [
            vec!["--self-foreground-on-first-down-after-ms", "1000"],
            vec![
                "--regular-app",
                "--self-foreground-on-first-down-after-ms",
                "0",
            ],
            vec![
                "--regular-app",
                "--self-foreground-on-first-down-after-ms",
                "5001",
            ],
            vec!["--regular-app", "--self-foreground-on-first-down-after-ms"],
            vec![
                "--regular-app",
                "--lifetime-ms",
                "1000",
                "--self-foreground-on-first-down-after-ms",
                "1000",
            ],
            vec![
                "--regular-app",
                "--self-foreground-on-first-down-after-ms",
                "1000",
                "--self-foreground-on-first-down-after-ms",
                "1000",
            ],
            vec![
                "--regular-app",
                "--self-foreground-after-ms",
                "1000",
                "--self-foreground-on-first-down-after-ms",
                "1000",
            ],
        ] {
            assert!(parse(&extra).is_err(), "{extra:?}");
        }
        assert!(Options::parse(valid.into_iter().map(str::to_owned)).is_err());
    }
    fn first_down_actor_state() -> State {
        let mut state = State::new("first-down-actor".into());
        state.status = "running";
        state.window_number = Some(42);
        state.application_identity.activation_policy = Some(0);
        state.self_foreground_on_first_down_after_ms = Some(1000);
        state
    }
    #[test]
    fn first_down_actor_records_cause_once_without_fabricating_foreground_or_input() {
        let mut state = first_down_actor_state();
        let event = mouse(MouseKind::Down, Region::Canvas, [10., 20.]);
        assert_eq!(
            state.arm_foreground_on_first_canvas_down(&event, Some(1234)),
            Some(1000)
        );
        assert_eq!(state.self_foreground_first_down_sequence, Some(1));
        assert_eq!(state.self_foreground_first_down_unix_ms, Some(1234));
        assert_eq!(
            state.self_foreground_first_down_event_timestamp_seconds,
            Some(1.)
        );
        assert_eq!(state.sequence, 0);
        assert_eq!(state.drag.downs, 0);
        assert_eq!(state.self_foreground_requests, 0);
        assert_eq!(state.app_frontmost, None);
        assert!(state.records.is_empty());
        state.receive(event.clone());
        assert_eq!(state.records[0].sequence, 1);
        assert_eq!(
            state.arm_foreground_on_first_canvas_down(&event, Some(2222)),
            None
        );
        state.request_self_foreground();
        assert_eq!(state.self_foreground_requests, 1);
        assert_eq!(state.app_frontmost, None);
        assert_eq!(state.drag.downs, 1);
    }
    #[test]
    fn first_down_actor_rejects_wrong_lifetime_policy_window_kind_region_and_nonfinite() {
        let down = mouse(MouseKind::Down, Region::Canvas, [10., 20.]);
        for variant in 0..8 {
            let mut state = first_down_actor_state();
            let mut event = down.clone();
            match variant {
                0 => state.status = "closed",
                1 => state.application_identity.activation_policy = Some(1),
                2 => event.window_number = 43,
                3 => event.region = Region::HoverZone,
                4 => event.kind = MouseKind::Dragged,
                5 => event.native_type = 2,
                6 => event.local_point[0] = f64::NAN,
                _ => state.self_foreground_on_first_down_after_ms = None,
            }
            assert_eq!(
                state.arm_foreground_on_first_canvas_down(&event, None),
                None
            );
            assert_eq!(state.self_foreground_first_down_sequence, None);
            assert_eq!(state.self_foreground_requests, 0);
            assert!(state.records.is_empty());
        }
    }
    #[test]
    fn system_panel_controls_require_explicit_own_allocation_and_never_default_on() {
        let base = [
            "--open-window",
            "--run-id",
            "system",
            "--state-dir",
            "/tmp/rwi-system",
            "--display-id",
            "4",
        ];
        let parse = |extra: &[&str]| {
            Options::parse(
                base.into_iter()
                    .chain(extra.iter().copied())
                    .map(str::to_owned),
            )
        };
        assert!(!parse(&[]).unwrap().unwrap().system_panel_controls);
        assert!(
            parse(&["--system-panel-controls"])
                .unwrap()
                .unwrap()
                .system_panel_controls
        );
        assert!(parse(&["--system-panel-controls", "--system-panel-controls"]).is_err());
        assert!(
            Options::parse(["--system-panel-controls"].into_iter().map(str::to_owned)).is_err()
        );
    }
    #[test]
    fn system_panel_request_is_not_native_visibility_or_owner_evidence() {
        let mut state = State::new("panel".into());
        assert!(!state.system_panel_open("color"));
        state.status = "running";
        assert!(!state.system_panel_open("font"));
        state.system_panels.enabled = true;
        assert!(!state.system_panel_open("other"));
        assert!(state.system_panel_open("color"));
        assert!(state.system_panel_open("font"));
        assert_eq!(state.system_panels.color.open_actions, 1);
        assert_eq!(state.system_panels.font.open_actions, 1);
        assert_eq!(state.system_panels.color.visible, None);
        assert_eq!(state.system_panels.font.attached_to_own_parent, None);
        assert_eq!(state.system_panels.color.window_number, None);
        assert!(state.records.iter().all(|r| r.mouse.is_none()));
        assert_eq!(state.drag.downs, 0);
        state.close();
        assert!(!state.system_panel_open("color"));
    }
    #[test]
    fn file_panel_request_never_attests_native_response_visibility_or_provider() {
        let mut state = State::new("file-panel".into());
        state.status = "running";
        assert!(!state.system_panel_open("file"));
        state.system_panels.enabled = true;
        assert!(state.system_panel_open("file"));
        assert_eq!(state.system_panels.file.open_actions, 1);
        assert!(state.system_panels.file.visible.is_none());
        assert!(state.system_panels.file.completion_response.is_none());
        assert!(state.system_panels.file.quartz_description.is_none());
        assert!(state.system_panels.file.attached_to_own_parent.is_none());
        state.close();
        assert!(!state.system_panel_open("file"));
    }
    #[test]
    fn file_panel_completion_never_infers_rendered_window_disappearance() {
        let mut state = State::new("file-completion".into());
        state.native_file_panel_completed(0);
        assert_eq!(state.system_panels.file.completion_response, Some(0));
        assert!(state.system_panels.file.visible.is_none());
        assert!(state.system_panels.file.quartz_description.is_none());
        assert!(state.system_panels.file.attached_to_own_parent.is_none());
        assert_eq!(state.drag.downs, 0);
    }
    #[test]
    fn covered_receiver_contract_has_explicit_opt_in_and_rejects_duplicate() {
        let args = [
            "--open-window",
            "--run-id",
            "covered",
            "--state-dir",
            "/tmp/rwi-covered-test",
            "--display-id",
            "4",
            "--cover-controls",
        ];
        assert!(
            Options::parse(args.map(str::to_owned)).is_ok(),
            "explicit own-cover fixture opt-in missing"
        );
        assert!(
            Options::parse(
                args.into_iter()
                    .chain(["--cover-controls"])
                    .map(str::to_owned)
            )
            .is_err()
        );
        assert!(Options::parse(["--cover-controls"].map(str::to_owned)).is_err());
    }
    #[test]
    fn absent_arguments_do_not_authorize_a_window() {
        assert!(Options::parse([]).unwrap().is_none());
        assert!(
            Options::parse(["--run-id", "run", "--state-dir", "/tmp/rwi"].map(str::to_owned))
                .is_err()
        );
    }
    #[test]
    fn native_text_oracle_is_consumed_state_bounded_and_not_a_mouse_receipt() {
        let mut state = State::new("pure".into());
        state.text_changed("RWI-bg-验收-🙂Z");
        assert_eq!(state.text.value, "RWI-bg-验收-🙂Z");
        assert_eq!(state.text.changes, 1);
        assert!(!state.text.truncated);
        assert!(state.records[0].mouse.is_none());
        state.text_changed(&"🙂".repeat(1100));
        assert_eq!(state.text.value.chars().count(), 1024);
        assert!(state.text.truncated);
    }
    #[test]
    fn native_scroll_receipt_keeps_clip_readback_independent_of_received_delta() {
        let mut state = State::new("pure".into());
        state.scroll_received([0., -150.], [0., 0.]);
        assert_eq!(state.scroll.events, 1);
        assert_eq!(state.scroll.clip_origin, Some([0., 0.]));
        assert_eq!(state.scroll.last_delta, Some([0., -150.]));
        assert!(state.records[0].mouse.is_none());
        state.scroll_received([0., f64::NAN], [0., 150.]);
        assert_eq!(state.invalid_events, 1);
        assert_eq!(state.scroll.events, 1);
    }
    #[test]
    fn explicit_display_argument_is_required_and_nonzero_u32() {
        let common = [
            "--open-window",
            "--run-id",
            "run",
            "--state-dir",
            "/tmp/rwi",
        ];
        assert!(Options::parse(common.map(str::to_owned)).is_err());
        for invalid in ["0", "-1", "abc", "4294967296"] {
            assert!(
                Options::parse(
                    common
                        .into_iter()
                        .chain(["--display-id", invalid])
                        .map(str::to_owned)
                )
                .is_err()
            );
        }
        let options = Options::parse(
            common
                .into_iter()
                .chain(["--display-id", "4"])
                .map(str::to_owned),
        )
        .unwrap()
        .unwrap();
        assert_eq!(options.display_id, 4);
    }
    #[test]
    fn lifetime_defaults_to_two_minutes_and_valid_endpoints_are_exact() {
        let common = [
            "--open-window",
            "--run-id",
            "run",
            "--state-dir",
            "/tmp/rwi",
            "--display-id",
            "4",
        ];
        let default = Options::parse(common.map(str::to_owned)).unwrap().unwrap();
        assert_eq!(default.lifetime_ms, 120_000);
        for (literal, expected) in [("1", 1), ("120000", 120_000), ("600000", 600_000)] {
            let options = Options::parse(
                common
                    .into_iter()
                    .chain(["--lifetime-ms", literal])
                    .map(str::to_owned),
            )
            .unwrap()
            .unwrap();
            assert_eq!(options.lifetime_ms, expected);
        }
    }
    #[test]
    fn lifetime_rejects_invalid_missing_or_duplicate_values_without_window() {
        let common = [
            "--open-window",
            "--run-id",
            "run",
            "--state-dir",
            "/tmp/rwi",
            "--display-id",
            "4",
        ];
        for invalid in ["0", "-1", "600001", "4294967296", "abc", "1.5"] {
            assert!(
                Options::parse(
                    common
                        .into_iter()
                        .chain(["--lifetime-ms", invalid])
                        .map(str::to_owned)
                )
                .is_err()
            );
        }
        for suffix in [
            vec!["--lifetime-ms"],
            vec!["--lifetime-ms", "1", "--lifetime-ms", "2"],
        ] {
            assert!(Options::parse(common.into_iter().chain(suffix).map(str::to_owned)).is_err());
        }
        assert!(Options::parse(["--lifetime-ms", "1"].map(str::to_owned)).is_err());
        assert!(Options::parse([]).unwrap().is_none());
        assert!(
            Options::parse(["--help"].map(str::to_owned))
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn lifetime_request_is_not_a_closed_window_receipt() {
        let mut state = State::new("pure".into());
        state.status = "running";
        state.request_lifetime_close();
        assert_eq!(state.lifetime_close_requests, 1);
        assert_eq!(state.status, "running");
        assert_eq!(state.records[0].action, "lifetime_timer_close_requested");
        assert!(state.records[0].mouse.is_none());
        state.close();
        assert_eq!(state.status, "closed");
        assert_eq!(state.records[1].action, "window_closed");
    }
    #[test]
    fn negative_origin_display_placement_is_explicit_and_must_fully_fit() {
        let screen = Rect {
            x: -1920.,
            y: 700.,
            width: 1920.,
            height: 1080.,
        };
        let placed = screen.place(660., 548.).unwrap();
        assert_eq!(placed.x, -1896.);
        assert_eq!(placed.y, 724.);
        assert!(screen.contains(placed));
        assert!(screen.place(1920., 548.).is_err());
        assert!(screen.place(660., f64::NAN).is_err());
    }
    #[test]
    fn safe_action_is_not_fabricated_as_a_mouse_receipt() {
        let mut state = State::new("pure".into());
        state.click();
        assert_eq!(state.safe_button_actions, 1);
        assert!(state.records[0].mouse.is_none());
    }
    #[test]
    fn moved_receipt_does_not_fabricate_tracking_entry_or_visible_tooltip() {
        let mut state = State::new("pure".into());
        state.receive(mouse(MouseKind::Moved, Region::HoverZone, [8., 9.]));
        assert_eq!(state.hover.moves, 1);
        assert_eq!(state.hover.enters, 0);
        assert!(!state.hover.inside);
        assert!(!state.hover.tooltip_registered);
        assert_eq!(state.hover.tooltip_requests, 0);
        assert!(state.hover.last_tooltip_point.is_none());
    }
    #[test]
    fn tooltip_text_request_is_not_a_mouse_or_display_receipt() {
        let mut state = State::new("pure".into());
        state.hover.tooltip_registered = true;
        state.tooltip_requested([8., 9.]);
        assert_eq!(state.hover.tooltip_requests, 1);
        assert_eq!(state.hover.last_tooltip_point, Some([8., 9.]));
        assert_eq!(state.hover.enters, 0);
        assert_eq!(state.hover.moves, 0);
        assert!(!state.hover.inside);
        assert_eq!(state.records[0].action, "nstooltip_owner_text_request");
        assert!(state.records[0].mouse.is_none());
        let json = serde_json::to_value(&state).unwrap();
        assert!(json["hover"].get("tooltip_visible").is_none());
    }
    #[test]
    fn nonfinite_tooltip_request_is_rejected_without_hover_success() {
        let mut state = State::new("pure".into());
        for point in [[f64::NAN, 9.], [8., f64::INFINITY]] {
            state.tooltip_requested(point);
        }
        assert_eq!(state.invalid_events, 2);
        assert_eq!(state.hover.tooltip_requests, 0);
        assert!(state.hover.last_tooltip_point.is_none());
        assert!(!state.hover.inside);
    }
    #[test]
    fn drag_requires_down_move_up_and_records_exact_points() {
        let mut state = State::new("pure".into());
        for (kind, point) in [
            (MouseKind::Down, [10., 20.]),
            (MouseKind::Dragged, [30., 40.]),
            (MouseKind::Up, [50., 60.]),
        ] {
            state.receive(mouse(kind, Region::Canvas, point));
        }
        assert_eq!(state.drag.completed, 1);
        assert!(state.drag.active.is_none());
        let path = state.drag.last_completed.as_ref().unwrap();
        assert_eq!(path.start, [10., 20.]);
        assert_eq!(path.end, [50., 60.]);
        assert_eq!(path.moves, 1);
    }
    #[test]
    fn orphan_drag_and_click_only_do_not_confirm_drag() {
        let mut state = State::new("pure".into());
        for kind in [
            MouseKind::Dragged,
            MouseKind::Up,
            MouseKind::Down,
            MouseKind::Up,
        ] {
            state.receive(mouse(kind, Region::Canvas, [2., 3.]));
        }
        assert_eq!(state.drag.completed, 0);
        assert_eq!(state.drag.orphan_moves, 1);
        assert_eq!(state.drag.orphan_ups, 1);
        assert_eq!(state.drag.click_without_drag, 1);
    }
    #[test]
    fn close_marks_incomplete_drag_cancelled() {
        let mut state = State::new("pure".into());
        state.receive(mouse(MouseKind::Down, Region::Canvas, [2., 3.]));
        state.close();
        assert_eq!(state.drag.cancelled, 1);
        assert_eq!(state.drag.completed, 0);
        assert_eq!(state.status, "closed");
    }
    #[test]
    fn history_and_drag_paths_are_bounded() {
        let mut state = State::new("pure".into());
        state.receive(mouse(MouseKind::Down, Region::Canvas, [2., 3.]));
        for n in 0..400 {
            state.receive(mouse(MouseKind::Dragged, Region::Canvas, [n as f64, 3.]));
        }
        assert_eq!(state.records.len(), HISTORY_LIMIT);
        assert_eq!(state.dropped_records, 401 - HISTORY_LIMIT as u64);
        assert_eq!(state.drag.active.as_ref().unwrap().points.len(), PATH_LIMIT);
        assert_eq!(state.drag.active.as_ref().unwrap().moves, 400);
    }
    #[test]
    fn nonfinite_mouse_values_are_not_recorded_as_receipts() {
        let mut state = State::new("pure".into());
        state.receive(mouse(MouseKind::Down, Region::Canvas, [f64::NAN, 3.]));
        assert_eq!(state.invalid_events, 1);
        assert_eq!(state.drag.downs, 0);
        assert!(serde_json::to_vec(&state).is_ok());
    }
    #[test]
    fn finite_delta_is_serialized_unchanged_for_moved_and_dragged() {
        for (kind, region) in [
            (MouseKind::Moved, Region::HoverZone),
            (MouseKind::Dragged, Region::Canvas),
        ] {
            let mut state = State::new("pure".into());
            let mut event = mouse(kind, region, [2., 3.]);
            event.delta = Some([1.25, -2.5]);
            state.receive(event);
            let json = serde_json::to_value(&state).unwrap();
            assert_eq!(
                json["records"][0]["mouse"]["delta"],
                serde_json::json!([1.25, -2.5])
            );
            assert_eq!(state.invalid_events, 0);
        }
        for kind in [
            MouseKind::Down,
            MouseKind::Up,
            MouseKind::Entered,
            MouseKind::Exited,
        ] {
            assert!(mouse(kind, Region::Canvas, [2., 3.]).delta.is_none());
        }
    }
    #[test]
    fn nonfinite_delta_is_rejected_without_mouse_receipt() {
        for kind in [MouseKind::Moved, MouseKind::Dragged] {
            for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                for delta in [[value, 0.], [0., value]] {
                    let mut state = State::new("pure".into());
                    let mut event = mouse(kind, Region::HoverZone, [2., 3.]);
                    event.delta = Some(delta);
                    state.receive(event);
                    assert_eq!(state.invalid_events, 1);
                    assert_eq!(state.hover.moves, 0);
                    assert_eq!(state.drag.moves, 0);
                    assert!(state.records[0].mouse.is_none());
                    assert!(serde_json::to_vec(&state).is_ok());
                }
            }
        }
    }
    #[test]
    fn atomic_json_is_parseable_and_existing_directory_is_rejected() {
        let parent = tempfile::tempdir().unwrap();
        let directory = parent.path().join("run");
        let store = Store::create(&directory).unwrap();
        let mut state = State::new("pure".into());
        store.persist(&state).unwrap();
        state.click();
        store.persist(&state).unwrap();
        let json: serde_json::Value =
            serde_json::from_slice(&fs::read(&store.path).unwrap()).unwrap();
        assert_eq!(json["safe_button_actions"], 1);
        assert!(Store::create(&directory).is_err());
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&store.path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn temporary_collision_never_removes_an_existing_file() {
        let parent = tempfile::tempdir().unwrap();
        let store = Store::create(&parent.path().join("run")).unwrap();
        let state = State::new("pure".into());
        let collision = store
            .path
            .with_file_name(format!(".state-{}-{}.tmp", state.pid, state.sequence));
        fs::write(&collision, "not ours").unwrap();
        assert!(store.persist(&state).is_err());
        assert_eq!(fs::read_to_string(&collision).unwrap(), "not ours");
    }
}

#[cfg(test)]
#[test]
fn acceptance_environment_sampling_cli_is_explicit_and_bounded() {
    let args = [
        "--open-window",
        "--run-id",
        "sampler-contract",
        "--state-dir",
        "/tmp/rwi-sampler-contract-new",
        "--display-id",
        "4",
        "--lifetime-ms",
        "1000",
        "--environment-sample-ms",
        "100",
    ]
    .map(String::from);
    assert!(
        Options::parse(args).is_ok(),
        "explicit bounded environment sampling must be accepted; parsing alone must not create a window"
    );
}

#[cfg(test)]
#[test]
fn environment_sampling_does_not_fabricate_input_or_run_after_close() {
    let mut s = State::new("sampling-pure".into());
    s.note_environment_probe(1);
    assert_eq!(s.environment_probe_ticks, 0);
    s.environment_sample_ms = Some(100);
    s.note_environment_probe(2);
    assert_eq!(s.environment_probe_ticks, 0);
    s.status = "running";
    s.note_environment_probe(3);
    assert_eq!(s.environment_probe_ticks, 1);
    assert_eq!(s.environment_sampled_unix_ms, Some(3));
    assert_eq!(s.sequence, 0);
    assert_eq!(s.safe_button_actions, 0);
    assert!(s.dispatch_probe.records.is_empty());
    s.close();
    s.note_environment_probe(4);
    assert_eq!(s.environment_probe_ticks, 1);
}
#[cfg(test)]
#[test]
fn environment_sampling_cli_rejects_unbounded_or_implicit_use() {
    let common = [
        "--open-window",
        "--run-id",
        "sampler-parse",
        "--state-dir",
        "/tmp/rwi-sampler-parse-new",
        "--display-id",
        "4",
    ];
    assert_eq!(
        Options::parse(common.map(String::from))
            .unwrap()
            .unwrap()
            .environment_sample_ms,
        None
    );
    for ms in ["0", "24", "2001", "NaN"] {
        let args = common
            .iter()
            .copied()
            .chain(["--environment-sample-ms", ms])
            .map(String::from);
        assert!(Options::parse(args).is_err());
    }
    assert!(Options::parse(["--environment-sample-ms", "100"].map(String::from)).is_err());
}

#[test]
fn acceptance_regular_fixture_cli_requires_allocated_window() {
    let args = [
        "--open-window",
        "--run-id",
        "regular-fixture",
        "--state-dir",
        "/tmp/rwi-regular-test-only",
        "--display-id",
        "4",
        "--regular-app",
    ];
    let parsed = Options::parse(args.into_iter().map(str::to_owned));
    assert!(
        parsed.is_ok(),
        "explicit regular policy unavailable: {parsed:?}"
    );
    assert!(parsed.unwrap().is_some());
}

#[test]
fn regular_fixture_policy_is_explicit_and_never_bypasses_allocation() {
    assert!(Options::parse(["--regular-app".to_owned()]).is_err());
    let base = [
        "--open-window",
        "--run-id",
        "regular-fixture",
        "--state-dir",
        "/tmp/rwi-regular-test-only",
        "--display-id",
        "4",
    ];
    let parsed = Options::parse(base.into_iter().map(str::to_owned))
        .unwrap()
        .unwrap();
    assert!(!parsed.regular_app);
    let mut args = base.into_iter().map(str::to_owned).collect::<Vec<_>>();
    args.push("--regular-app".into());
    assert!(Options::parse(args.clone()).unwrap().unwrap().regular_app);
    args.push("--regular-app".into());
    assert!(Options::parse(args).is_err());
}

#[cfg(test)]
mod allocated_modal_cli_contract {
    use super::*;
    #[test]
    fn modal_controls_need_explicit_allocated_fixture() {
        let options = Options::parse(
            [
                "--open-window",
                "--run-id",
                "own-modal",
                "--state-dir",
                "/tmp/rwi-own-modal",
                "--display-id",
                "4",
                "--modal-controls",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .expect("allocated modal controls must parse")
        .unwrap();
        assert_eq!(options.display_id, 4);
    }
}

#[cfg(test)]
mod modal_oracle_contract {
    use super::*;
    #[test]
    fn modal_requests_never_fabricate_native_visibility_or_identity() {
        let mut state = State::new("modal".into());
        state.modal_action("fixture_open_sheet_action");
        state.modal_action("fixture_sheet_safe_action");
        state.modal_action("fixture_close_sheet_action");
        assert_eq!(state.modal.open_actions, 1);
        assert_eq!(state.modal.safe_actions, 1);
        assert_eq!(state.modal.close_actions, 1);
        assert_eq!(state.modal.window_number, None);
        assert_eq!(state.modal.visible, None);
        assert_eq!(state.modal.is_sheet, None);
        assert_eq!(state.modal.attached_to_own_parent, None);
        assert_eq!(state.safe_button_actions, 0);
    }
    #[test]
    fn modal_controls_are_opt_in_and_never_bypass_window_allocation() {
        assert!(Options::parse(["--modal-controls"].into_iter().map(str::to_owned)).is_err());
        let base = [
            "--open-window",
            "--run-id",
            "modal",
            "--state-dir",
            "/tmp/own-modal",
            "--display-id",
            "4",
        ];
        assert!(
            !Options::parse(base.into_iter().map(str::to_owned))
                .unwrap()
                .unwrap()
                .modal_controls
        );
        assert!(
            Options::parse(
                base.into_iter()
                    .chain(["--modal-controls", "--modal-controls"])
                    .map(str::to_owned)
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod ax_provider_delay_contract {
    use super::*;
    fn parse(extra: &[&str]) -> Result<Option<Options>, String> {
        Options::parse(
            [
                "--open-window",
                "--run-id",
                "own-ax-provider",
                "--state-dir",
                "/tmp/rwi-own-ax-provider",
                "--display-id",
                "4",
            ]
            .into_iter()
            .chain(extra.iter().copied())
            .map(str::to_owned),
        )
    }
    #[test]
    fn delay_is_opt_in_bounded_and_regular_own_app_only() {
        assert!(parse(&[]).unwrap().unwrap().ax_windows_delay_ms.is_none());
        for ms in ["75", "120", "150"] {
            assert!(
                parse(&["--regular-app", "--ax-windows-delay-ms", ms])
                    .unwrap()
                    .unwrap()
                    .ax_windows_delay_ms
                    .is_some()
            );
        }
        assert!(parse(&["--ax-windows-delay-ms", "120"]).is_err());
        for ms in ["0", "74", "151", "-1", "x"] {
            assert!(parse(&["--regular-app", "--ax-windows-delay-ms", ms]).is_err());
        }
        assert!(
            parse(&[
                "--regular-app",
                "--ax-windows-delay-ms",
                "120",
                "--ax-windows-delay-ms",
                "120"
            ])
            .is_err()
        );
    }
    #[test]
    fn provider_delay_does_not_authorize_implicit_window_or_fabricate_error() {
        assert!(
            Options::parse(
                ["--ax-windows-delay-ms", "120"]
                    .into_iter()
                    .map(str::to_owned)
            )
            .is_err()
        );
        let s = State::new("own-ax-provider".into());
        assert_eq!(s.ax_provider_probe["AX_error_attested"], false);
        assert_eq!(s.ax_provider_probe["input_sent"], false);
        assert_eq!(s.ax_provider_probe["getter_calls"], 0);
    }
}

#[cfg(test)]
mod canvas_boundary_geometry_tests {
    use super::*;

    #[test]
    fn boundary_canvas_flag_is_explicit_and_conflicting_geometry_refuses() {
        let args = vec![
            "--open-window",
            "--run-id",
            "parser-only",
            "--state-dir",
            "/tmp/rwi-never-created-parser-probe",
            "--display-id",
            "4",
            "--boundary-canvas-right",
            "--check-only",
        ];
        let parsed = Options::parse(args.iter().map(|s| s.to_string()))
            .unwrap()
            .unwrap();
        assert!(parsed.boundary_canvas_right);
        assert!(!parsed.boundary_right);
        let mut conflict = args;
        conflict.push("--boundary-right");
        assert!(Options::parse(conflict.iter().map(|s| s.to_string())).is_err());
        assert!(
            Options::parse(["--boundary-canvas-right"].into_iter().map(str::to_owned)).is_err()
        );
    }

    #[test]
    fn unchanged_canvas_straddles_edge_without_acceptance_or_screen_substitution() {
        let screen = Rect {
            x: 1800.,
            y: 89.,
            width: 1920.,
            height: 1080.,
        };
        let own = screen.place_boundary_canvas_right(660., 552.).unwrap();
        assert_eq!(own.x, 3456.);
        assert_eq!(own.y, 113.);
        assert_eq!(screen.x + screen.width - own.x, 264.);
        assert!(own.x + 24. < screen.x + screen.width);
        assert!(own.x + 384. > screen.x + screen.width);
        assert!(!screen.contains(own));
        assert!(
            Rect {
                width: 600.,
                ..screen
            }
            .place_boundary_canvas_right(660., 552.)
            .is_err()
        );
        assert!(
            Rect {
                x: f64::NAN,
                ..screen
            }
            .place_boundary_canvas_right(660., 552.)
            .is_err()
        );
    }
}

#[cfg(test)]
mod framework_pointer_tests {
    use super::*;
    #[test]
    fn unread_framework_pointer_stays_unknown_without_a_hover_claim() {
        let p = FrameworkPointerSnapshot::default();
        let j = serde_json::to_value(p).unwrap();
        assert!(j["screen_point"].is_null());
        assert!(j["screen_point_in_own_hover"].is_null());
    }
    #[test]
    fn known_framework_position_is_diagnostic_not_a_received_view_event() {
        let p = FrameworkPointerSnapshot {
            screen_point: Some([10., 20.]),
            screen_point_in_own_hover: Some(false),
            ..Default::default()
        };
        let j = serde_json::to_value(p).unwrap();
        assert_eq!(j["screen_point"], serde_json::json!([10., 20.]));
        assert_eq!(j["screen_point_in_own_hover"], false);
        assert!(j["screen_converted_to_own_window"].is_null());
    }
}

#[cfg(test)]
mod native_dnd_configuration_tests {
    use super::*;
    fn args(extra: &[&str]) -> Vec<String> {
        [
            "--open-window",
            "--check-only",
            "--run-id",
            "dnd-config",
            "--state-dir",
            "/tmp/rwi-native-dnd-check-only-no-write",
            "--display-id",
            "4",
        ]
        .into_iter()
        .chain(extra.iter().copied())
        .map(str::to_owned)
        .collect()
    }
    #[test]
    fn native_drag_is_explicit_and_check_only_keeps_allocation_identity() {
        assert!(
            !Options::parse(args(&[]))
                .unwrap()
                .unwrap()
                .native_dnd_controls
        );
        let o = Options::parse(args(&["--native-dnd-controls", "--right-side"]))
            .unwrap()
            .unwrap();
        assert!(o.native_dnd_controls && o.check_only && o.right_side);
        assert_eq!(o.display_id, 4);
        assert_eq!(o.run_id, "dnd-config");
        assert!(
            Options::parse(["--check-only", "--native-dnd-controls"].map(str::to_owned)).is_err()
        );
    }
    #[test]
    fn duplicate_flag_and_unrelated_panel_or_actor_combinations_are_refused() {
        for extra in [
            vec!["--native-dnd-controls"],
            vec!["--system-panel-controls"],
            vec!["--cover-controls"],
            vec!["--modal-controls"],
            vec!["--boundary-right"],
            vec!["--boundary-canvas-right"],
            vec!["--hover-first-responder"],
            vec!["--regular-app", "--self-foreground-after-ms", "1000"],
        ] {
            let mut flags = vec!["--native-dnd-controls"];
            flags.extend(extra);
            assert!(Options::parse(args(&flags)).is_err(), "{flags:?}");
        }
    }

    #[test]
    fn native_drag_can_arm_only_the_explicit_first_down_own_foreground_actor() {
        let o = Options::parse(args(&[
            "--native-dnd-controls",
            "--regular-app",
            "--lifetime-ms",
            "10000",
            "--self-foreground-on-first-down-after-ms",
            "2000",
        ]))
        .unwrap()
        .unwrap();
        assert!(o.native_dnd_controls && o.regular_app && o.check_only);
        assert_eq!(o.self_foreground_on_first_down_after_ms, Some(2000));
        assert_eq!(o.self_foreground_after_ms, None);
        assert!(o.command_tab.is_none());
    }

    #[test]
    fn native_first_down_actor_keeps_activation_and_lifetime_guards() {
        for extra in [
            vec![],
            vec!["--regular-app", "--lifetime-ms", "2000"],
            vec!["--regular-app", "--self-foreground-after-ms", "1000"],
            vec![
                "--regular-app",
                "--command-tab-on-first-down-after-ms",
                "1000",
                "--command-tab-peer-pid",
                "1234",
                "--command-tab-peer-window",
                "12",
                "--command-tab-peer-bundle",
                "com.destroyedteam.rwi.receiver.allocated-peer",
                "--environment-sample-ms",
                "100",
            ],
            vec![
                "--regular-app",
                "--self-foreground-on-first-down-after-ms",
                "2000",
            ],
        ] {
            let mut flags = vec![
                "--native-dnd-controls",
                "--self-foreground-on-first-down-after-ms",
                "2000",
            ];
            flags.extend(extra);
            assert!(Options::parse(args(&flags)).is_err(), "{flags:?}");
        }
        for delay in ["99", "5001"] {
            assert!(
                Options::parse(args(&[
                    "--native-dnd-controls",
                    "--regular-app",
                    "--self-foreground-on-first-down-after-ms",
                    delay,
                ]))
                .is_err()
            );
        }
    }
}
