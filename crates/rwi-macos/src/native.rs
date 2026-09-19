mod delegation;
mod input;
pub(crate) use input::NATIVE_SAMPLE_INTERVAL_MS;
mod keyboard;
mod metadata;
use crate::{
    capture::Capturer,
    ffi::{self, Cf},
    geometry::Rect,
    session::console_identity,
};
use core_graphics::{
    display::CGDisplay,
    event::{CGEvent, CGEventFlags, CGEventType, CGMouseButton, EventField, ScrollEventUnit},
    event_source::{CGEventSource, CGEventSourceStateID},
    geometry::{CGPoint, CGRect},
};
use foreign_types::ForeignType;
use keyboard::*;
use rwi_core::*;
use std::{
    collections::VecDeque,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Opt-in native target. Construction only binds a PID/window; it never activates it.
/// Advisory AX window inventory for one process; used by observe-time popup
/// delivery. The list is bounded and never guesses a purpose.
pub(crate) fn popups_for_pid(pid: u32) -> Vec<rwi_core::Popup> {
    metadata::popup_inventory(pid)
}
pub struct NativeMac {
    pub(crate) pid: u32,
    window: u32,
    identity: Identity,
    process: String,
    allow_input: bool,
    capturer: Capturer,
    pub(crate) hit_diagnostics: std::sync::Mutex<Vec<serde_json::Value>>,
    dispatches: std::sync::Arc<std::sync::atomic::AtomicU64>,
}
#[derive(Clone)]
struct AxElement {
    element: Element,
    text_class: TextClass,
    value: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextClass {
    Plain,
    Secure,
    Unknown,
    NotText,
}
impl TextClass {
    fn classify(role: &str, subrole: Option<&str>) -> Self {
        if subrole == Some("AXSecureTextField") {
            return Self::Secure;
        }
        if matches!(role, "AXTextField" | "AXTextArea") {
            if subrole.is_some_and(|s| !s.is_empty() && s != "AXUnknown") {
                Self::Plain
            } else {
                Self::Unknown
            }
        } else {
            Self::NotText
        }
    }
    fn readable(self) -> bool {
        matches!(self, Self::Plain | Self::NotText)
    }
    fn input_allowed(self) -> bool {
        self == Self::Plain
    }
}
struct Metadata {
    identity: Identity,
    rect: Rect,
    binding: Binding,
    elements: Vec<AxElement>,
    title: String,
    modal: Option<bool>,
    blocked: Option<bool>,
    mapping_ready: bool,
}
#[derive(Clone)]
pub(crate) struct Window {
    pub id: u32,
    pub pid: u32,
    pub rect: Rect,
    pub alpha: f64,
    pub title: Option<String>,
}

impl Backend for NativeMac {
    fn input_dispatch_count(&self) -> Option<u64> {
        Some(self.dispatches.load(std::sync::atomic::Ordering::Acquire))
    }
    fn identity(&self) -> Identity {
        self.identity.clone()
    }
    fn capabilities(&self) -> serde_json::Value {
        let mut caps = crate::desktop::native_capabilities(
            self.allow_input,
            crate::session::permissions(),
            crate::session::current_facts()
                .ok()
                .and_then(|f| f.console_seat),
        );
        caps["target_scope"] =
            serde_json::json!("pinned foreground PID + CGWindowID + process start");
        // The legacy pinned constructor does not gain dynamic catalog operations.
        caps["window_discovery"]["implemented"] = serde_json::json!(false);
        caps["pointer_feedback"] = crate::overlay::status_json();
        caps["user_input_recent_seconds"] = match crate::activity::seconds_since_user_input() {
            Some(seconds) => serde_json::json!(seconds),
            None => serde_json::Value::Null,
        };
        caps
    }
    fn observe(&mut self) -> Result<Scene> {
        if !unsafe { ffi::CGPreflightScreenCaptureAccess() } {
            return Err(fault(
                "screen_capture_permission_missing",
                "Screen capture must already be permitted; no permission request is triggered.",
            ));
        }
        // Background observation: activate the target application internally first
        // so its chrome/state is stable for this frame and later revalidation.
        if self.foreground_pid() != Some(self.pid)
            && crate::background::activate_background_window(self.pid, self.window)
        {
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        // A window that is mid-animation repaints between the pre/post reads even
        // though nothing actionable changed. Retry the read-only capture a bounded
        // number of times with a short settle wait; the returned frame is still
        // fully revalidated and no input is ever inferred or replayed.
        let mut settled = None;
        for attempt in 0..3 {
            let result = (|| -> Result<Scene> {
                let before = self.metadata()?;
                let capture = self.capturer.capture(self.pid, self.window, before.rect)?;
                let after = self.metadata()?;
                if before.binding != after.binding
                    || before.blocked != after.blocked
                    || before
                        .elements
                        .iter()
                        .map(|e| (e.element.revision(), &e.value))
                        .collect::<Vec<_>>()
                        != after
                            .elements
                            .iter()
                            .map(|e| (e.element.revision(), &e.value))
                            .collect::<Vec<_>>()
                {
                    return Err(fault(
                        "capture_binding_changed",
                        "Focus/layout/AX content changed during capture; observe again.",
                    ));
                }
                Ok(self.metadata_scene(
                    after,
                    capture.size,
                    capture.png,
                    Some(&capture.content_sha),
                ))
            })();
            match result {
                Ok(scene) => return Ok(scene),
                Err(error) if error.code == "capture_binding_changed" && attempt < 2 => {
                    settled = Some(error);
                    std::thread::sleep(std::time::Duration::from_millis(150));
                }
                Err(error) => return Err(error),
            }
        }
        Err(settled.expect("capture settle attempts exhausted"))
    }
    fn revalidate(&mut self, reference: &Resolved) -> Result<Scene> {
        if reference.source_region.is_some() {
            return Err(fault(
                "capability_unavailable",
                "Native source-region capture/revalidation is not implemented on macOS; no full-window substitution.",
            ));
        }
        if reference.scope.element.is_none() {
            return self.observe();
        }
        Ok(self.metadata_scene(self.metadata()?, [0, 0], Vec::new(), None))
    }

    fn feedback(&mut self, visible: bool, point: Option<[f64; 2]>) -> Result<serde_json::Value> {
        if visible {
            let point = match point {
                Some(point) => point,
                None => crate::overlay::last_point().ok_or_else(|| {
                    fault(
                        "invalid_arguments",
                        "Provide point on the first visible request.",
                    )
                })?,
            };
            if point.iter().any(|v| !v.is_finite()) {
                return Err(fault("invalid_arguments", "Point must be finite."));
            }
            crate::overlay::show_at(point);
        } else {
            crate::overlay::hide();
        }
        Ok(crate::overlay::status_json())
    }
    fn popups(&mut self, _scope: &rwi_core::Scope) -> Result<Vec<rwi_core::Popup>> {
        Ok(crate::native::popups_for_pid(self.pid))
    }
    fn move_window(
        &mut self,
        reference: &Resolved,
        display: &rwi_core::DisplayTarget,
    ) -> Result<serde_json::Value> {
        if reference.scope.pid != self.pid || reference.scope.window != self.window_name() {
            return Err(fault(
                "target_mismatch",
                "Reference belongs to another native window; observe this exact one.",
            ));
        }
        let target = crate::native::displays()?
            .into_iter()
            .find(|candidate| {
                display
                    .built_in
                    .is_none_or(|built_in| candidate.built_in == built_in)
                    && display.id.is_none_or(|id| candidate.id == id)
            })
            .ok_or_else(|| {
                fault(
                    "display_not_found",
                    "No active display matches the requested target; nothing was moved.",
                )
            })?;
        self.place_window_on(&target)?;
        Ok(serde_json::json!({
            "window": self.window_name(),
            "display": {"id": target.id, "built_in": target.built_in},
            "input_sent": false,
            "activated": false,
        }))
    }
    fn act(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        intent: &Intent,
        value: Option<&str>,
    ) -> Result<Option<bool>> {
        let result = self.dispatch_input(expected, target, intent, value);
        if result.is_ok() {
            // Purely visual feedback for the user; never input and never the
            // real cursor. Failure to draw is not an input failure.
            crate::overlay::show_at(target.point);
        }
        result
    }
    fn inspect(&mut self, scope: &Scope, predicate: &Predicate, expected: &str) -> Result<Oracle> {
        if scope.pid != self.pid || scope.window != self.window_name() {
            return Err(fault(
                "verification_scope_mismatch",
                "Oracle is pinned to another window/PID.",
            ));
        }
        let meta = self.metadata()?;
        let matched = match predicate {
            Predicate::WindowTitle => meta.title == expected,
            Predicate::FocusedWindow => meta.binding.focus_window == expected,
            Predicate::ElementValue => {
                let id = scope
                    .element
                    .as_ref()
                    .ok_or_else(|| fault("element_required", "Select an observed element."))?;
                let el = meta
                    .elements
                    .iter()
                    .find(|e| &e.element.id == id)
                    .ok_or_else(|| {
                        fault(
                            "element_changed",
                            "Exact element is absent; same value elsewhere is not success.",
                        )
                    })?;
                verify_value(el, expected)?
            }
            Predicate::Modal => match meta.modal {
                Some(true) => meta.title == expected,
                Some(false) => expected == "none",
                None => {
                    return Err(fault(
                        "modal_state_unknown",
                        "Native AXModal is unavailable; do not infer success from absence.",
                    ));
                }
            },
        };
        Ok(Oracle {
            identity: meta.identity,
            scope: scope.clone(),
            assurance: "native-attested".into(),
            matched,
            source: "independent-AX-readback; no screenshot/input receipt inference".into(),
        })
    }
}
pub(crate) fn process_epoch(pid: u32) -> Result<String> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let bytes = unsafe {
        libc::proc_pidinfo(
            pid as i32,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            std::mem::size_of::<libc::proc_bsdinfo>() as i32,
        )
    };
    if bytes != std::mem::size_of::<libc::proc_bsdinfo>() as i32 {
        return Err(fault(
            "process_identity_unknown",
            "Cannot read the pinned process start identity.",
        ));
    }
    let info = unsafe { info.assume_init() };
    if info.pbi_uid != unsafe { libc::geteuid() } {
        return Err(fault(
            "process_user_mismatch",
            "Target process belongs to another user.",
        ));
    }
    Ok(format!(
        "{}:{}:{}",
        pid, info.pbi_start_tvsec, info.pbi_start_tvusec
    ))
}
pub(crate) fn windows() -> Result<Vec<Window>> {
    Ok(window_inventory()?.windows)
}
pub(crate) struct WindowInventory {
    pub windows: Vec<Window>,
    pub native_rows: usize,
    pub invalid_rows: usize,
}
pub(crate) fn window_inventory() -> Result<WindowInventory> {
    let _call = operation::native_call("macos_window_inventory")?;
    unsafe {
        let array = Cf::owned(ffi::CGWindowListCopyWindowInfo(1 | 16, 0))?;
        let rows = ffi::array(array.0, 2048)?;
        let native_rows = rows.len();
        let mut out = Vec::new();
        for row in rows {
            let number = ffi::number(ffi::field(row.0, "kCGWindowNumber"));
            let pid = ffi::number(ffi::field(row.0, "kCGWindowOwnerPID"));
            let mut rect = CGRect::new(
                &CGPoint::new(0.0, 0.0),
                &core_graphics::geometry::CGSize::new(0.0, 0.0),
            );
            let bounds = ffi::field(row.0, "kCGWindowBounds");
            if bounds.is_null()
                || ffi::CGRectMakeWithDictionaryRepresentation(bounds, &mut rect) == 0
            {
                continue;
            }
            if let (Some(id), Some(pid)) = (number, pid)
                && (1..=i64::from(u32::MAX)).contains(&id)
                && (1..=i64::from(i32::MAX)).contains(&pid)
            {
                out.push(Window {
                    id: id as u32,
                    pid: pid as u32,
                    rect: Rect {
                        x: rect.origin.x,
                        y: rect.origin.y,
                        w: rect.size.width,
                        h: rect.size.height,
                    },
                    alpha: ffi::real(ffi::field(row.0, "kCGWindowAlpha")).unwrap_or(1.0),
                    title: ffi::string(ffi::field(row.0, "kCGWindowName")).ok(),
                });
            }
        }
        Ok(WindowInventory {
            invalid_rows: native_rows - out.len(),
            native_rows,
            windows: out,
        })
    }
}
/// One active display as reported by CoreGraphics, including whether it is the
/// built-in panel. Bounded read-only inventory; no display reconfiguration.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Display {
    pub id: u32,
    pub built_in: bool,
    pub rect: Rect,
    pub pixels: [u64; 2],
    pub rotation: f64,
}
pub(crate) fn displays() -> Result<Vec<Display>> {
    let mut ids = CGDisplay::active_displays()
        .map_err(|_| fault("layout_unknown", "Cannot enumerate native active displays."))?;
    if ids.len() > 32 {
        return Err(fault("display_budget", "More than 32 native displays."));
    }
    ids.sort_unstable();
    let mut out = Vec::new();
    for id in ids {
        let display = CGDisplay::new(id);
        let bounds = display.bounds();
        out.push(Display {
            id,
            built_in: unsafe { ffi::CGDisplayIsBuiltin(id) != 0 },
            rect: Rect {
                x: bounds.origin.x,
                y: bounds.origin.y,
                w: bounds.size.width,
                h: bounds.size.height,
            },
            pixels: [display.pixels_wide(), display.pixels_high()],
            rotation: display.rotation(),
        });
    }
    Ok(out)
}
/// The single unrotated active display that fully contains the window. Windows
/// spanning no display or more than one display stay unassigned rather than
/// being attributed to their largest overlap.
pub(crate) fn containing_display(displays: &[Display], window: Rect) -> Option<&Display> {
    let mut found = None;
    for display in displays {
        if display.rotation.abs() < 0.001 && display.rect.contains_rect(window) {
            if found.is_some() {
                return None;
            }
            found = Some(display);
        }
    }
    found
}
/// Keep the window size; place it near the display origin with a margin,
/// clamped so it stays inside the display whenever the window fits.
pub(crate) fn placement_position(window: Rect, display: Rect) -> (f64, f64) {
    const MARGIN: f64 = 24.0;
    let max_x = display.x + (display.w - window.w).max(0.0);
    let max_y = display.y + (display.h - window.h).max(0.0);
    (
        (display.x + MARGIN).min(max_x.max(display.x)),
        (display.y + MARGIN).min(max_y.max(display.y)),
    )
}
/// Launch an application in the background through the supported AppKit path
/// (`NSWorkspace.openApplicationAtURL:configuration:completionHandler:` with
/// `activates=false`, `hides=true`): the current foreground app is untouched
/// and the child is hidden as soon as it checks in. No synthetic input.
#[allow(dead_code)] // Consumed by the macOS lifecycle driver (next increment).
pub(crate) fn launch_background(path: &str, hide: bool) -> Result<()> {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSString;
    let workspace_class = AnyClass::get(c"NSWorkspace")
        .ok_or_else(|| fault("launch_unavailable", "NSWorkspace is unavailable."))?;
    let configuration_class = AnyClass::get(c"NSWorkspaceOpenConfiguration").ok_or_else(|| {
        fault(
            "launch_unavailable",
            "NSWorkspaceOpenConfiguration is unavailable on this macOS.",
        )
    })?;
    let url_class = AnyClass::get(c"NSURL")
        .ok_or_else(|| fault("launch_unavailable", "NSURL is unavailable."))?;
    let path_string = NSString::from_str(path);
    unsafe {
        let workspace: *mut AnyObject = msg_send![workspace_class, sharedWorkspace];
        let configuration: *mut AnyObject = msg_send![configuration_class, new];
        let url: *mut AnyObject = msg_send![url_class, fileURLWithPath: &*path_string];
        let _: () = msg_send![configuration, setActivates: false];
        // A hidden launch never becomes an on-screen window, so the placement
        // watcher (on-screen scope) could not find it; placement therefore
        // starts visible-but-unfocused and hiding is a separate later step.
        let _: () = msg_send![configuration, setHides: hide];
        let _: () = msg_send![configuration, setAddsToRecentItems: false];
        let handler: *mut AnyObject = std::ptr::null_mut();
        let _: () = msg_send![
            workspace,
            openApplicationAtURL: url,
            configuration: configuration,
            completionHandler: handler
        ];
    }
    Ok(())
}
/// Launch an application in the background and place its first new window onto
/// the authorized display as soon as it appears: 50ms watch cadence, 3s total
/// budget, native placement and native bounds verification. No synthetic input.
#[allow(dead_code)] // Consumed by the macOS lifecycle driver (next increment).
pub(crate) fn launch_background_and_place(
    path: &str,
    display: &Display,
) -> Result<serde_json::Value> {
    let before = windows()?
        .into_iter()
        .map(|window| window.pid)
        .collect::<std::collections::BTreeSet<_>>();
    launch_background(path, false)?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(3000);
    loop {
        for window in windows()? {
            if before.contains(&window.pid) {
                continue;
            }
            let backend = NativeMac::with_shared_resources(
                window.pid,
                window.id,
                false,
                crate::capture::Capturer::default(),
                std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            )?;
            backend.place_window_on(display)?;
            if let Some(moved) = windows()?
                .into_iter()
                .find(|candidate| candidate.id == window.id && candidate.pid == window.pid)
            {
                return Ok(serde_json::json!({
                    "pid": window.pid,
                    "window": format!("cgwindow:{}", window.id),
                    "bounds": [moved.rect.x, moved.rect.y, moved.rect.w, moved.rect.h],
                }));
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(fault(
                "launch_window_timeout",
                "No new window appeared within the placement budget.",
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
fn display_layout(window: Rect) -> Result<(String, bool)> {
    let displays = displays()?;
    let mut text = String::new();
    let mut containing = 0;
    for display in &displays {
        text.push_str(&format!(
            "{}:{:?}:{}:{}:{};",
            display.id, display.rect, display.pixels[0], display.pixels[1], display.rotation
        ));
        if display.rotation.abs() < 0.001 && display.rect.contains_rect(window) {
            containing += 1;
        }
    }
    Ok((digest(text.as_bytes()), containing == 1))
}
fn ax_rect(element: &Cf) -> Result<Rect> {
    let p = element.attr("AXPosition")?.point()?;
    let s = element.attr("AXSize")?.size()?;
    let r = Rect {
        x: p.x,
        y: p.y,
        w: s.width,
        h: s.height,
    };
    if !r.valid() {
        return Err(fault(
            "ax_geometry_unknown",
            "Invalid native element bounds.",
        ));
    }
    Ok(r)
}
fn read_classified_value(
    class: TextClass,
    read: impl FnOnce() -> Option<String>,
) -> Option<String> {
    if class.readable() { read() } else { None }
}
fn require_nonsecure_input(el: &AxElement) -> Result<()> {
    if el.text_class.readable() {
        Ok(())
    } else {
        Err(fault(
            "secure_or_unknown_text_field",
            "Pointer/key/text input to a secure or unclassified text field is unavailable.",
        ))
    }
}
fn require_text_target(el: &AxElement, requested: Option<&str>) -> Result<()> {
    if !el.text_class.input_allowed() || requested != Some(el.element.id.as_str()) {
        Err(fault(
            "text_target_not_focused",
            "Text requires the exact focused, classified non-secure AX text field or text area.",
        ))
    } else {
        Ok(())
    }
}
fn require_keyboard_target(el: &AxElement, requested: Option<&str>) -> Result<()> {
    if requested != Some(el.element.id.as_str()) {
        return Err(fault(
            "keyboard_target_not_focused",
            "Keyboard input requires the exact observed focused native AX element; pixel and other-element substitutions are rejected.",
        ));
    }
    Ok(())
}
fn verify_value(el: &AxElement, expected: &str) -> Result<bool> {
    if !el.text_class.readable() {
        Err(fault(
            "secure_or_unknown_text_field",
            "Native text classification forbids value readback.",
        ))
    } else {
        Ok(el.value.as_deref() == Some(expected))
    }
}
/// Bounded native action-name readback for one element (≤12 names).
pub(crate) fn action_names(el: &Cf) -> Vec<String> {
    unsafe {
        let mut names: crate::ffi::Ref = std::ptr::null();
        if ffi::AXUIElementCopyActionNames(el.0, &mut names) == 0 && !names.is_null() {
            Cf::owned(names)
                .ok()
                .and_then(|array| ffi::array(array.0, 12).ok())
                .map(|rows| {
                    rows.iter()
                        .filter_map(|row| row.string().ok())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    }
}
fn element_description(el: &Cf) -> Result<AxElement> {
    let role = el.attr("AXRole")?.string()?;
    let subrole = el.attr("AXSubrole").ok().and_then(|v| v.string().ok());
    let text_class = TextClass::classify(&role, subrole.as_deref());
    let r = ax_rect(el)?;
    let native_id = el
        .attr("AXIdentifier")
        .ok()
        .and_then(|v| v.string().ok())
        .unwrap_or_default();
    let name = el
        .attr("AXTitle")
        .ok()
        .and_then(|v| v.string().ok())
        .filter(|s| !s.is_empty())
        .or_else(|| el.attr("AXDescription").ok().and_then(|v| v.string().ok()))
        .unwrap_or_default();
    // Bind native object identity to its real process birth, including hosted views.
    let owner_epoch = process_epoch(el.pid()?)?;
    let id = digest(format!("{owner_epoch}:{native_id}:{role}:{subrole:?}:{r:?}").as_bytes());
    let value = read_classified_value(text_class, || {
        el.attr("AXValue").ok().and_then(|v| v.string().ok())
    });
    let actions = action_names(el);
    let enabled = el.attr("AXEnabled").ok().and_then(|v| v.boolean());
    // Selection state is read only for roles that can carry it: an extra AX
    // round-trip per element would blow the bounded walk's time budget and
    // truncate later nodes (the toolbar row). The value reuses the classified
    // text value above rather than asking AXValue a second time.
    // Only anonymous elements pay for this read: icon-only buttons usually
    // expose their function through AXDescription/AXHelp, while named controls
    // already have a name and must not cost an extra AX round-trip.
    let ax_description = if name.trim().is_empty() {
        ["AXDescription", "AXHelp"]
            .iter()
            .find_map(|key| {
                el.attr(key)
                    .ok()
                    .and_then(|v| v.string().ok())
                    .filter(|text| !text.trim().is_empty())
            })
            .map(|text| text.chars().take(64).collect::<String>())
    } else {
        None
    };
    let ax_selected = if matches!(
        role.as_str(),
        "AXRadioButton" | "AXCheckBox" | "AXTab" | "AXMenuItem" | "AXRow" | "AXButton"
    ) {
        el.attr("AXSelected").ok().and_then(|v| v.boolean())
    } else {
        None
    };
    let (ax_value, ax_value_truncated) = match &value {
        Some(text) if !text.is_empty() => {
            let truncated = text.chars().count() > 64;
            (Some(text.chars().take(64).collect::<String>()), truncated)
        }
        _ => (None, false),
    };
    Ok(AxElement {
        element: Element {
            id,
            name,
            role: Some(role),
            actions,
            enabled,
            point: [r.x + r.w / 2.0, r.y + r.h / 2.0],
            visible: None,
            occluded: None,
            state_revision: Some(digest(
                format!("{text_class:?}:{value:?}:{enabled:?}").as_bytes(),
            )),
            selected: ax_selected,
            description: ax_description,
            value: ax_value,
            value_truncated: ax_value_truncated,
        },
        text_class,
        value,
    })
}
fn element_at_point(root: &Cf, point: [f64; 2]) -> Result<Option<Cf>> {
    let mut pending = VecDeque::new();
    pending.push_back((root.retained(), 0usize));
    let mut best: Option<(Cf, usize)> = None;
    let start = Instant::now();
    let mut nodes = 0;
    while let Some((node, depth)) = pending.pop_front() {
        operation::checkpoint("macos_point_element")?;
        if nodes >= 128 || start.elapsed() > Duration::from_millis(3000) {
            break;
        }
        nodes += 1;
        let contains = ax_rect(&node).is_ok_and(|rect| rect.contains(point));
        let children = if depth < 16 {
            node.children().unwrap_or_default()
        } else {
            Vec::new()
        };
        if contains
            && best
                .as_ref()
                .is_none_or(|(_, best_depth)| *best_depth <= depth)
        {
            best = Some((node, depth));
        }
        for child in children {
            if pending.len() >= 256 {
                break;
            }
            pending.push_back((child, depth + 1));
        }
    }
    Ok(best.map(|(node, _)| node))
}
fn hit_test(system: &Cf, point: [f64; 2]) -> Result<Cf> {
    let mut out = std::ptr::null();
    let error = unsafe {
        ffi::AXUIElementCopyElementAtPosition(system.0, point[0] as f32, point[1] as f32, &mut out)
    };
    if error != 0 {
        return Err(fault(
            "hit_test_unknown",
            "Native AX hit test is unavailable.",
        ));
    }
    unsafe { Cf::owned(out) }
}
// Visibility is relative to the selected application's native z-order, not
// the desktop foreground. Failed/unsupported hit tests remain unknown rather
// than fabricating an occlusion; they never positively authorize a control.
fn scoped_visibility(
    inside: bool,
    blocked: Option<bool>,
    exact_hit: Option<bool>,
) -> (Option<bool>, Option<bool>) {
    if !inside || blocked == Some(true) {
        return (Some(false), Some(true));
    }
    if blocked.is_none() {
        return (None, None);
    }
    (exact_hit, exact_hit.map(|hit| !hit))
}

fn ax_tree(
    window: &Cf,
    application: &Cf,
    bounds: Rect,
    blocked: Option<bool>,
    diagnostics: &mut Vec<serde_json::Value>,
) -> Result<Vec<AxElement>> {
    let start = Instant::now();
    let mut pending = VecDeque::new();
    for child in window.children().unwrap_or_default() {
        pending.push_back((child, 1));
    }
    let mut nodes = 0;
    let mut out = Vec::new();
    while let Some((node, depth)) = pending.pop_front() {
        operation::checkpoint("macos_ax_tree")?;
        if nodes >= 128 || start.elapsed() > Duration::from_millis(3000) {
            break;
        }
        nodes += 1;
        if let Ok(mut e) = element_description(&node) {
            let point = e.element.point;
            let inside = bounds.contains(point);
            let matches = if inside && blocked == Some(false) {
                let hit = hit_test(application, point).ok();
                let matches = hit
                    .as_ref()
                    .map(|h| unsafe { ffi::CFEqual(h.0, node.0) } != 0);
                if matches != Some(true) && !e.element.actions.is_empty() && diagnostics.len() < 8 {
                    let rect_json = |v: &Cf| ax_rect(v).ok().map(|r| [r.x, r.y, r.w, r.h]);
                    diagnostics.push(serde_json::json!({
                        "element": e.element.id, "role": e.element.role, "point": point,
                        "node_pid": node.pid().ok(), "node_rect": rect_json(&node),
                        "node_window": ffi::window_number_for_ax_element(&node),
                        "hit_pid": hit.as_ref().and_then(|h| h.pid().ok()),
                        "hit_rect": hit.as_ref().and_then(rect_json),
                        "hit_role": hit.as_ref().and_then(|h| h.attr("AXRole").ok()).and_then(|r| r.string().ok()),
                        "hit_window": hit.as_ref().and_then(ffi::window_number_for_ax_element),
                        "equal": matches
                    }));
                }
                matches
            } else {
                None
            };
            let (visible, occluded) = scoped_visibility(inside, blocked, matches);
            e.element.visible = visible;
            e.element.occluded = occluded;
            out.push(e);
        }
        if depth < 16 {
            for child in node.children().unwrap_or_default() {
                if pending.len() >= 256 {
                    break;
                }
                pending.push_back((child, depth + 1));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoped_visibility_requires_an_exact_native_hit() {
        assert_eq!(
            scoped_visibility(true, Some(false), Some(true)),
            (Some(true), Some(false))
        );
        assert_eq!(
            scoped_visibility(true, Some(false), Some(false)),
            (Some(false), Some(true))
        );
        assert_eq!(scoped_visibility(true, Some(false), None), (None, None));
        assert_eq!(scoped_visibility(true, None, Some(true)), (None, None));
        assert_eq!(
            scoped_visibility(false, Some(false), Some(true)),
            (Some(false), Some(true))
        );
        assert_eq!(
            scoped_visibility(true, Some(true), Some(true)),
            (Some(false), Some(true))
        );
    }
    fn display(id: u32, built_in: bool, x: f64, y: f64, w: f64, h: f64) -> Display {
        Display {
            id,
            built_in,
            rect: Rect { x, y, w, h },
            pixels: [(w as u64) * 2, (h as u64) * 2],
            rotation: 0.0,
        }
    }
    #[test]
    fn placement_keeps_windows_inside_the_authorized_display() {
        let display = Rect {
            x: 282.0,
            y: 1152.0,
            w: 1800.0,
            h: 1169.0,
        };
        let small = Rect {
            x: 232.0,
            y: 55.0,
            w: 656.0,
            h: 422.0,
        };
        assert_eq!(placement_position(small, display), (306.0, 1176.0));
        let huge = Rect {
            x: 0.0,
            y: 0.0,
            w: 2400.0,
            h: 1500.0,
        };
        assert_eq!(placement_position(huge, display), (282.0, 1152.0));
    }
    #[test]
    fn window_display_assignment_requires_one_unrotated_containing_display() {
        let built_in = display(1, true, 0.0, 0.0, 100.0, 80.0);
        let external = display(2, false, 100.0, 0.0, 120.0, 80.0);
        let mut rotated = display(3, false, 0.0, 80.0, 90.0, 60.0);
        rotated.rotation = 90.0;
        let displays = vec![built_in, external, rotated];
        let inside = Rect {
            x: 10.0,
            y: 10.0,
            w: 40.0,
            h: 30.0,
        };
        assert_eq!(containing_display(&displays, inside).map(|d| d.id), Some(1));
        let over_both = Rect {
            x: 80.0,
            y: 10.0,
            w: 40.0,
            h: 30.0,
        };
        assert!(containing_display(&displays, over_both).is_none());
        let on_rotated = Rect {
            x: 10.0,
            y: 90.0,
            w: 40.0,
            h: 30.0,
        };
        assert!(containing_display(&displays, on_rotated).is_none());
        let outside = Rect {
            x: 500.0,
            y: 500.0,
            w: 10.0,
            h: 10.0,
        };
        assert!(containing_display(&displays, outside).is_none());
    }
    #[test]
    #[ignore = "Explicit live AX diagnostic: press the default button of a modal sheet on one pid/window; setup only"]
    fn press_default_button_diagnostic() {
        let pid: u32 = std::env::var("RCU_DIAG_PID")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(18858);
        let want: String = std::env::var("RCU_DIAG_WINDOW").unwrap_or_else(|_| "Untitled 5".into());
        let application =
            unsafe { Cf::owned(ffi::AXUIElementCreateApplication(pid as i32)).unwrap() };
        let focused: bool = std::env::var("RCU_DIAG_FOCUSED").is_ok_and(|v| v == "1");
        let button_title: String = std::env::var("RCU_DIAG_BUTTON").unwrap_or_else(|_| "OK".into());
        let window = if focused {
            application.attr("AXFocusedWindow").expect("focused window")
        } else {
            let value = application.attr("AXWindows").unwrap();
            let windows = unsafe { ffi::array(value.0, 64).unwrap() };
            let mut target = None;
            for w in &windows {
                if w.attr("AXTitle")
                    .ok()
                    .and_then(|v| v.string().ok())
                    .as_deref()
                    == Some(want.as_str())
                {
                    target = Some(w.retained());
                }
            }
            target.expect("window not found")
        };
        fn find_button(node: &Cf, depth: usize, want: &str) -> Option<Cf> {
            if depth > 8 {
                return None;
            }
            let role = node.attr("AXRole").ok().and_then(|v| v.string().ok());
            let title = node.attr("AXTitle").ok().and_then(|v| v.string().ok());
            let default = node.attr("AXDefaultButton").ok().and_then(|v| v.boolean());
            if role.as_deref() == Some("AXButton")
                && (title.as_deref() == Some(want) || default == Some(true))
            {
                return Some(node.retained());
            }
            for child in node.children().unwrap_or_default() {
                if let Some(found) = find_button(&child, depth + 1, want) {
                    return Some(found);
                }
            }
            None
        }
        let button = find_button(&window, 0, &button_title).expect("button not found");
        use core_foundation::base::TCFType;
        let action = core_foundation::string::CFString::new("AXPress");
        let code =
            unsafe { ffi::AXUIElementPerformAction(button.0, action.as_concrete_TypeRef().cast()) };
        println!("AXPress code={code}");
    }
    #[test]
    #[ignore = "Explicit live NSWorkspace background launch of RCU_DIAG_LAUNCH_PATH; setup only"]
    fn launch_background_diagnostic() {
        let path = std::env::var("RCU_DIAG_LAUNCH_PATH")
            .unwrap_or_else(|_| "/System/Applications/Calculator.app".into());
        match launch_background(&path, true) {
            Ok(()) => println!("launch_background ok path={path}"),
            Err(error) => println!("launch_background error code={} next={error}", error.code),
        }
        std::thread::sleep(std::time::Duration::from_millis(1200));
    }
    #[test]
    #[ignore = "Explicit live background launch + placement of RCU_DIAG_LAUNCH_PATH; setup only"]
    fn launch_background_and_place_diagnostic() {
        let path = std::env::var("RCU_DIAG_LAUNCH_PATH")
            .unwrap_or_else(|_| "/System/Applications/Calculator.app".into());
        let displays = displays().expect("displays");
        let target = displays
            .iter()
            .find(|display| display.built_in)
            .expect("built-in display");
        println!("target display id={} rect={:?}", target.id, target.rect);
        match launch_background_and_place(&path, target) {
            Ok(value) => println!("launch_place ok {value}"),
            Err(error) => println!("launch_place error code={} next={error}", error.code),
        }
    }
    #[test]
    #[ignore = "Explicit live AX/CG window identity dump for one pid; read-only, no input"]
    fn dump_ax_windows_for_pid_diagnostic() {
        let pid: u32 = std::env::var("RCU_DIAG_PID")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(18858);
        let application =
            unsafe { Cf::owned(ffi::AXUIElementCreateApplication(pid as i32)).unwrap() };
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 1.0);
        }
        for key in ["AXFocusedWindow", "AXMainWindow"] {
            if let Ok(element) = application.attr(key) {
                println!(
                    "AX {key} number={:?} rect={:?} title={:?} subrole={:?} modal={:?}",
                    ffi::window_number_for_ax_element(&element),
                    ax_rect(&element).ok(),
                    element.attr("AXTitle").ok().and_then(|v| v.string().ok()),
                    element.attr("AXSubrole").ok().and_then(|v| v.string().ok()),
                    element.attr("AXModal").ok().and_then(|v| v.boolean())
                );
            }
        }
        if let Ok(children_value) = application.attr("AXChildren")
            && let Ok(children) = unsafe { ffi::array(children_value.0, 32) }
        {
            for child in &children {
                println!(
                    "AX app-child subrole={:?} number={:?} rect={:?} title={:?}",
                    child.attr("AXSubrole").ok().and_then(|v| v.string().ok()),
                    ffi::window_number_for_ax_element(child),
                    ax_rect(child).ok(),
                    child.attr("AXTitle").ok().and_then(|v| v.string().ok())
                );
            }
        }
        let value = application.attr("AXWindows").unwrap();
        let windows = unsafe { ffi::array(value.0, 64).unwrap() };
        for w in &windows {
            let number = ffi::window_number_for_ax_element(w);
            let rect = ax_rect(w).ok();
            let title = w.attr("AXTitle").ok().and_then(|v| v.string().ok());
            let main = w.attr("AXMain").ok().and_then(|v| v.boolean());
            let focused = w.attr("AXFocused").ok().and_then(|v| v.boolean());
            let minimized = w.attr("AXMinimized").ok().and_then(|v| v.boolean());
            println!(
                "AX window number={number:?} rect={rect:?} title={title:?} main={main:?} focused={focused:?} minimized={minimized:?}"
            );
        }
        for w in &windows {
            let Ok(children_value) = w.attr("AXChildren") else {
                continue;
            };
            let Ok(children) = (unsafe { ffi::array(children_value.0, 32) }) else {
                continue;
            };
            for child in &children {
                let subrole = child.attr("AXSubrole").ok().and_then(|v| v.string().ok());
                let number = ffi::window_number_for_ax_element(child);
                let rect = ax_rect(child).ok();
                let title = child.attr("AXTitle").ok().and_then(|v| v.string().ok());
                println!(
                    "AX child subrole={subrole:?} number={number:?} rect={rect:?} title={title:?}"
                );
            }
        }
        for w in window_inventory()
            .unwrap()
            .windows
            .iter()
            .filter(|w| w.pid == pid)
        {
            println!(
                "CG onscreen id={} rect=({},{},{},{}) title={:?} alpha={}",
                w.id, w.rect.x, w.rect.y, w.rect.w, w.rect.h, w.title, w.alpha
            );
        }
        unsafe {
            let array = Cf::owned(ffi::CGWindowListCopyWindowInfo(0, 0)).unwrap();
            for row in ffi::array(array.0, 2048).unwrap() {
                let owner = ffi::number(ffi::field(row.0, "kCGWindowOwnerPID")).unwrap_or(-1);
                if owner != i64::from(pid) {
                    continue;
                }
                let id = ffi::number(ffi::field(row.0, "kCGWindowNumber")).unwrap_or(-1);
                let onscreen = ffi::number(ffi::field(row.0, "kCGWindowIsOnscreen")).unwrap_or(-1);
                let layer = ffi::number(ffi::field(row.0, "kCGWindowLayer")).unwrap_or(-1);
                println!("CG all id={id} onscreen={onscreen} layer={layer}");
            }
        }
        let capturer = crate::capture::Capturer::default();
        for id in std::env::var("RCU_DIAG_CAPTURE")
            .unwrap_or_default()
            .split(',')
            .filter_map(|v| v.parse::<u32>().ok())
        {
            let rect = window_inventory()
                .unwrap()
                .windows
                .iter()
                .find(|w| w.id == id)
                .map(|w| w.rect);
            let Some(rect) = rect else {
                println!("capture id={id} no on-screen rect");
                continue;
            };
            match capturer.capture(pid, id, rect) {
                Ok(capture) => {
                    let path = format!("/tmp/rcu-diag-window-{id}.png");
                    std::fs::write(&path, &capture.png).unwrap();
                    println!("capture id={id} size={:?} path={path}", capture.size);
                }
                Err(e) => println!("capture id={id} error={}", e.code),
            }
        }
    }
    #[test]
    fn secure_text_subrole_is_rejected_for_description_readback_and_input() {
        let class = TextClass::classify("AXTextField", Some("AXSecureTextField"));
        assert_eq!(class, TextClass::Secure);
        assert!(!class.readable());
        assert!(!class.input_allowed());
        assert_eq!(
            TextClass::classify("AXTextField", Some("AXUnknown")),
            TextClass::Unknown
        );
        let unknown = TextClass::classify("AXTextField", None);
        assert!(!unknown.readable());
        assert!(!unknown.input_allowed());
        let normal = TextClass::classify("AXTextField", Some("AXSearchField"));
        assert!(normal.readable());
        assert!(normal.input_allowed());
    }
    fn classified(class: TextClass) -> AxElement {
        AxElement {
            element: Element {
                enabled: None,
                actions: vec![],
                role: None,
                id: "id".into(),
                name: "Input".into(),
                point: [0.0, 0.0],
                visible: Some(true),
                occluded: Some(false),
                state_revision: None,
                description: None,
                selected: None,
                value: None,
                value_truncated: false,
            },
            text_class: class,
            value: Some("secret".into()),
        }
    }
    #[test]
    fn secure_and_unknown_description_never_calls_ax_value_reader() {
        for class in [TextClass::Secure, TextClass::Unknown] {
            assert!(read_classified_value(class, || panic!("AXValue must not be read")).is_none());
        }
    }
    #[test]
    fn secure_and_unknown_input_is_rejected_even_for_focused_matching_element() {
        for class in [TextClass::Secure, TextClass::Unknown] {
            assert_eq!(
                require_text_target(&classified(class), Some("id"))
                    .unwrap_err()
                    .code,
                "text_target_not_focused"
            );
        }
    }
    #[test]
    fn secure_text_cannot_be_bypassed_using_pointer_or_key_chord() {
        for class in [TextClass::Secure, TextClass::Unknown] {
            assert_eq!(
                require_nonsecure_input(&classified(class))
                    .unwrap_err()
                    .code,
                "secure_or_unknown_text_field"
            );
        }
    }
    #[test]
    fn secure_and_unknown_verification_is_not_success_even_if_equal() {
        for class in [TextClass::Secure, TextClass::Unknown] {
            assert_eq!(
                verify_value(&classified(class), "secret").unwrap_err().code,
                "secure_or_unknown_text_field"
            );
        }
    }
    #[test]
    fn physical_key_aliases_are_explicit() {
        assert_eq!(parse_chord("Shift+Minus").unwrap().0, 27);
        assert_eq!(parse_chord("Return").unwrap().0, 36);
        assert!(parse_chord("Cmd+A+B").is_err());
        assert!(parse_chord("cmd+made-up").is_err());
    }
    #[test]
    fn unicode_chunks_never_split_surrogate_pairs() {
        let text = "\t中文🙂\n  print(\"a b\")🙂 ";
        let chunks = utf16_chunks(text, 5);
        assert!(
            chunks
                .iter()
                .all(|c| c.len() <= 5 && String::from_utf16(c).is_ok())
        );
        assert_eq!(String::from_utf16(&chunks.concat()).unwrap(), text);
    }
}
