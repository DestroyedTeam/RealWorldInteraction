pub(crate) mod activation;
mod backend;
mod coverage;
mod delegation;
pub(crate) mod hotkey;
mod input;
pub(crate) use input::NATIVE_SAMPLE_INTERVAL_MS;
mod keyboard;
mod live_family;
pub(crate) use live_family::LiveFamilyCapture;
mod metadata;
mod objects;
mod semantic;
pub(crate) use semantic::focused_dialog_window;
mod visual;
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
pub(crate) const AX_TREE_MAX_NODES: usize = 512;
pub(crate) const AX_TREE_MAX_QUEUE: usize = 1024;
/// The previous 16-level walk stopped before deep controls in Chromium's AX
/// projection (the allocated local fixture has 24 explicitly labelled groups).
/// Keep a finite depth cap; node, queue and wall-clock budgets still dominate.
pub(crate) const AX_TREE_MAX_DEPTH: usize = 40;
pub struct NativeMac {
    pub(crate) pid: u32,
    window: u32,
    identity: Identity,
    process: String,
    allow_input: bool,
    pub(crate) semantic_only: bool,
    capturer: Capturer,
    pub(crate) hit_diagnostics: std::sync::Mutex<Vec<serde_json::Value>>,
    dispatches: std::sync::Arc<std::sync::atomic::AtomicU64>,
    objects: std::sync::Mutex<objects::Objects<objects::Ax>>,
    pub(crate) last_observation: std::sync::Mutex<serde_json::Value>,
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
        // Editable combo boxes (for example Paint S's font-size control) use
        // AXValue as their text surface. This classification alone grants no
        // input: the native batch still requires AXValue to be settable and
        // the exact hit object to equal AXFocusedUIElement.
        if matches!(role, "AXTextField" | "AXTextArea" | "AXComboBox") {
            if subrole.is_some_and(|s| s != "AXUnknown") {
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
    coverage: serde_json::Value,
}
#[derive(Clone)]
pub(crate) struct Window {
    pub id: u32,
    pub pid: u32,
    pub rect: Rect,
    pub alpha: f64,
    pub title: Option<String>,
}

/// Window/context changes invalidate the capture. Unrelated semantic changes
/// remain explicit non-atomic read evidence; input revalidates the chosen target.
fn capture_consistency_failure(
    binding_changed: bool,
    blocked_changed: bool,
    semantic_content_changed: Option<bool>,
    element_counts: [usize; 2],
    attempt: u8,
) -> Option<rwi_core::Fault> {
    if !(binding_changed || blocked_changed) {
        return None;
    }
    let mut error = fault(
        "capture_binding_changed",
        "Pre/post native metadata changed during capture; inspect diagnostic dimensions and obtain a fresh observation.",
    );
    error.diagnostic = Some(Box::new(serde_json::json!({
        "stage":"capture_pre_post_metadata",
        "attempt":attempt,
        "binding_changed":binding_changed,
        "blocked_changed":blocked_changed,
        "semantic_content_changed":semantic_content_changed,
        "before_elements":element_counts[0],
        "after_elements":element_counts[1],
        "native_input_dispatched":false,
        "replay_allowed":false,
    })));
    Some(error)
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
    window_inventory_with_options(1 | 16)
}
fn window_inventory_with_options(options: u32) -> Result<WindowInventory> {
    let _call = operation::native_call("macos_window_inventory")?;
    unsafe {
        let array = Cf::owned(ffi::CGWindowListCopyWindowInfo(options, 0))?;
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
    pub uuid: Option<String>,
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
        let uuid = unsafe { Cf::owned(ffi::CGDisplayCreateUUIDFromDisplayID(id)) }
            .ok()
            .and_then(|uuid| {
                unsafe { Cf::owned(ffi::CFUUIDCreateString(std::ptr::null(), uuid.0)) }.ok()
            })
            .and_then(|text| text.string().ok());
        out.push(Display {
            id,
            uuid,
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
// AXValue is not always CFString: checkboxes/radio buttons commonly use
// CFNumber (including mixed state 2), and sliders may expose fractions.
// Keep the existing text-classification gate at the caller; never stringify
// arbitrary CF objects or read secure/unknown text through this helper.
fn scalar_ax_value(value: &Cf) -> Option<String> {
    value
        .string()
        .ok()
        .or_else(|| value.boolean().map(|v| if v { "1" } else { "0" }.into()))
        .or_else(|| unsafe { ffi::number(value.0) }.map(|v| v.to_string()))
        .or_else(|| {
            unsafe { ffi::real(value.0) }
                .filter(|v| v.is_finite())
                .map(|v| v.to_string())
        })
}

#[cfg(test)]
#[test]
fn scalar_ax_values_preserve_off_on_mixed_and_fractional_values() {
    use core_foundation::{base::TCFType, number::CFNumber};
    for (number, expected) in [(0.0, "0"), (1.0, "1"), (2.0, "2"), (0.75, "0.75")] {
        let native = CFNumber::from(number);
        let value = unsafe { Cf::owned(ffi::CFRetain(native.as_CFTypeRef().cast())).unwrap() };
        assert_eq!(scalar_ax_value(&value).as_deref(), Some(expected));
    }
    assert_eq!(
        read_classified_value(TextClass::Secure, || panic!("must not read secure value")),
        None
    );
    assert_eq!(
        read_classified_value(TextClass::Unknown, || panic!("must not read unknown value")),
        None
    );
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
    let subrole = match el.attr("AXSubrole") {
        Ok(value) => value.string().ok(),
        Err(error) if ffi::attribute_is_absent(&error, "AXSubrole") => Some(String::new()),
        Err(_) => None,
    };
    // NSAccessibilitySubrole is nullable. An explicitly absent specialization
    // is not a failed AX read: ordinary NSTextView may have no subrole. Timeout,
    // invalid object/type and AXUnknown remain unknown; secure still wins.
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
        el.attr("AXValue").ok().and_then(|v| scalar_ax_value(&v))
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

fn observed_element(
    node: &Cf,
    window: &Cf,
    application: &Cf,
    bounds: Rect,
    blocked: Option<bool>,
    diagnostics: &mut Vec<serde_json::Value>,
    objects: &std::sync::Mutex<objects::Objects<objects::Ax>>,
) -> Result<AxElement> {
    let mut e = objects::describe(node, window, objects)?;
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
                "node_pid": node.pid().ok(), "node_rect": rect_json(node),
                "node_window": ffi::window_number_for_ax_element(node),
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
    Ok(e)
}

fn ax_tree(
    window: &Cf,
    application: &Cf,
    bounds: Rect,
    blocked: Option<bool>,
    diagnostics: &mut Vec<serde_json::Value>,
    objects: &std::sync::Mutex<objects::Objects<objects::Ax>>,
) -> Result<(Vec<AxElement>, serde_json::Value)> {
    let start = Instant::now();
    let mut pending = VecDeque::new();
    let mut coverage = coverage::Coverage::default();
    match window.children() {
        Ok(children) => pending.extend(children.into_iter().map(|child| (child, 1))),
        Err(_) => coverage.child_reads_failed += 1,
    }
    let mut out = Vec::new();
    while !pending.is_empty() {
        operation::checkpoint("macos_ax_tree")?;
        if coverage.visited >= AX_TREE_MAX_NODES {
            coverage.node_budget_reached = true;
            break;
        }
        if start.elapsed() > Duration::from_millis(3000) {
            coverage.time_budget_reached = true;
            break;
        }
        let (node, depth) = pending.pop_front().expect("nonempty queue");
        coverage.visited += 1;
        coverage.maximum_depth_seen = coverage.maximum_depth_seen.max(depth);
        match observed_element(
            &node,
            window,
            application,
            bounds,
            blocked,
            diagnostics,
            objects,
        ) {
            Ok(e) => {
                coverage.described += 1;
                out.push(e);
            }
            Err(_) => coverage.descriptions_failed += 1,
        }
        if depth >= AX_TREE_MAX_DEPTH {
            // Preserve the same native read budget; do not add a boundary read
            // merely to manufacture 'complete'. Descendants are unknown.
            coverage.unexpanded_depth_nodes += 1;
            continue;
        }
        match node.children() {
            Ok(children) => {
                let capacity = AX_TREE_MAX_QUEUE.saturating_sub(pending.len());
                coverage.dropped_queue_nodes += children.len().saturating_sub(capacity);
                pending.extend(
                    children
                        .into_iter()
                        .take(capacity)
                        .map(|child| (child, depth + 1)),
                );
            }
            Err(_) => coverage.child_reads_failed += 1,
        }
    }
    Ok((out, coverage.evidence(pending.len())))
}

#[cfg(test)]
mod tests;
