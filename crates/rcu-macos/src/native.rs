mod input;
use crate::{
    capture::Capturer,
    ffi::{self, Cf},
    geometry::Rect,
};
use core_graphics::{
    display::CGDisplay,
    event::{
        CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton, EventField,
        ScrollEventUnit,
    },
    event_source::{CGEventSource, CGEventSourceStateID},
    geometry::{CGPoint, CGRect},
};
use foreign_types::ForeignType;
use rcu_core::*;
use std::{
    collections::VecDeque,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Opt-in native target. Construction only binds a PID/window; it never activates it.
pub struct NativeMac {
    pid: u32,
    window: u32,
    identity: Identity,
    process: String,
    allow_input: bool,
    capturer: Capturer,
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
struct Window {
    id: u32,
    pid: u32,
    rect: Rect,
    alpha: f64,
}

impl NativeMac {
    pub fn new(pid: u32, window: u32, allow_input: bool) -> Result<Self> {
        if pid == 0 || pid > i32::MAX as u32 || window == 0 {
            return Err(fault(
                "invalid_target",
                "Pin a positive PID and CGWindowID.",
            ));
        }
        Ok(Self {
            pid,
            window,
            identity: console_identity()?,
            process: process_epoch(pid)?,
            allow_input,
            capturer: Capturer::default(),
        })
    }
    fn window_name(&self) -> String {
        format!("cgwindow:{}", self.window)
    }
    fn metadata_scene(&self, meta: Metadata, size: [u32; 2], png: Vec<u8>) -> Scene {
        let mut binding = meta.binding;
        if !png.is_empty() {
            binding.content = digest(&png);
        }
        Scene {identity:meta.identity,assurance:"native-attested".into(),binding:Some(binding),
            mutation_ready:self.allow_input&&meta.mapping_ready&&meta.blocked==Some(false)&&unsafe{ffi::CGPreflightPostEventAccess()},
            blocked:meta.blocked,image_size:size,pixel_to_desktop:if meta.mapping_ready{meta.rect.map(size)}else{None},
            elements:meta.elements.into_iter().map(|e|e.element).collect(),
            captured_unix_ms:SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            atomicity:"pre/post native validation; non-atomic; live acceptance pending".into(),
            image_normalization:"CGImage -> device RGB bitmap -> straight RGBA PNG; native dimensions; no resizing; color/alpha normalization, not source-byte identity".into(),png}
    }
    fn metadata(&self) -> Result<Metadata> {
        let identity = console_identity()?;
        if identity != self.identity || process_epoch(self.pid)? != self.process {
            return Err(fault(
                "session_or_process_changed",
                "Pinned console session/process instance changed. Rebind explicitly.",
            ));
        }
        if unsafe { ffi::AXIsProcessTrusted() } == 0 {
            return Err(fault(
                "accessibility_permission_missing",
                "Grant permission to the native binary through an authorized setup, not this observation call.",
            ));
        }
        if !unsafe { ffi::CGPreflightScreenCaptureAccess() } {
            return Err(fault(
                "screen_capture_permission_missing",
                "Screen capture must already be permitted; no permission prompt is triggered automatically.",
            ));
        }
        let all = windows()?;
        let index = all
            .iter()
            .position(|w| w.id == self.window && w.pid == self.pid)
            .ok_or_else(|| {
                fault(
                    "window_missing",
                    "Exact pinned on-screen window/PID is absent.",
                )
            })?;
        let window = &all[index];
        let system = unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide()) }?;
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(system.0, 0.05);
        }
        let application = system.attr("AXFocusedApplication")?;
        if application.pid()? != self.pid {
            return Err(fault(
                "focus_mismatch",
                "Pinned application is not focused. Do not silently focus another app.",
            ));
        }
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(application.0, 0.05);
        }
        let focused = application.attr("AXFocusedWindow")?;
        let ax_rect = ax_rect(&focused)?;
        if !window.rect.same(ax_rect)
            || all
                .iter()
                .filter(|w| w.pid == self.pid && w.rect.same(ax_rect))
                .count()
                != 1
        {
            return Err(fault(
                "window_binding_ambiguous",
                "Focused AX window does not uniquely match the pinned CGWindowID/PID/bounds.",
            ));
        }
        let title = focused.attr("AXTitle")?.string()?;
        let modal = focused.attr("AXModal").ok().and_then(|v| v.boolean());
        let covered = all[..index]
            .iter()
            .any(|w| w.alpha > 0.01 && w.rect.overlaps(window.rect));
        let blocked = if covered || modal == Some(true) {
            Some(true)
        } else {
            modal.map(|_| false)
        };
        let (layout, mapping_ready) = display_layout(window.rect)?;
        let elements = ax_tree(&focused, &system, window.rect, blocked)?;
        let focused_element = application
            .attr("AXFocusedUIElement")
            .ok()
            .and_then(|e| element_description(&e).ok());
        let native_epoch = digest(
            format!(
                "{}|{:?}|{}|{:?}|{:?}",
                self.process,
                window.rect,
                title,
                modal,
                focused_element.as_ref().map(|v| &v.element.id)
            )
            .as_bytes(),
        );
        let binding = Binding {
            mutation_context: None,
            scope: Scope {
                window: self.window_name(),
                pid: self.pid,
                element: None,
            },
            focus_window: self.window_name(),
            layout,
            content: native_epoch.clone(),
            native_epoch,
        };
        Ok(Metadata {
            identity,
            rect: window.rect,
            binding,
            elements,
            title,
            modal,
            blocked,
            mapping_ready,
        })
    }
    fn focus_and_hit(&self, target: &Resolved, text: bool, keyboard: bool) -> Result<()> {
        if console_identity()? != self.identity || process_epoch(self.pid)? != self.process {
            return Err(fault(
                "session_or_process_changed",
                "No further input is sent.",
            ));
        }
        let system = unsafe { Cf::owned(ffi::AXUIElementCreateSystemWide()) }?;
        unsafe {
            ffi::AXUIElementSetMessagingTimeout(system.0, 0.05);
        }
        let app = system.attr("AXFocusedApplication")?;
        if app.pid()? != self.pid {
            return Err(fault(
                "focus_mismatch",
                "Focus changed immediately before dispatch.",
            ));
        }
        let focused = app.attr("AXFocusedWindow")?;
        let rect = ax_rect(&focused)?;
        let all = windows()?;
        let index = all
            .iter()
            .position(|w| w.id == self.window && w.pid == self.pid && w.rect.same(rect))
            .ok_or_else(|| {
                fault(
                    "window_binding_changed",
                    "Window binding changed immediately before dispatch.",
                )
            })?;
        if all[..index]
            .iter()
            .any(|w| w.alpha > 0.01 && w.rect.contains(target.point))
        {
            return Err(fault(
                "target_occluded",
                "Another native window owns the action point.",
            ));
        }
        if !rect.contains(target.point) {
            return Err(fault(
                "outside_target",
                "Action point left the pinned native window.",
            ));
        }
        let hit = hit_test(&system, target.point)?;
        if hit.pid()? != self.pid {
            return Err(fault(
                "target_occluded",
                "Native hit test points to another PID.",
            ));
        }
        let hit_window = hit.attr("AXWindow")?;
        if unsafe { ffi::CFEqual(hit_window.0, focused.0) } == 0 {
            return Err(fault(
                "target_occluded",
                "Native hit test belongs to another window.",
            ));
        }
        let hit_description = element_description(&hit)?;
        require_nonsecure_input(&hit_description)?;
        if let Some(id) = &target.scope.element
            && hit_description.element.id != *id
        {
            return Err(fault(
                "element_changed",
                "The grounded element moved or was replaced.",
            ));
        }
        if keyboard {
            let el = app.attr("AXFocusedUIElement")?;
            let description = element_description(&el)?;
            require_nonsecure_input(&description)?;
            if text {
                require_text_target(&description, target.scope.element.as_deref())?;
            }
        }
        Ok(())
    }
}
impl Backend for NativeMac {
    fn identity(&self) -> Identity {
        self.identity.clone()
    }
    fn capabilities(&self) -> serde_json::Value {
        serde_json::json!({"adapter":"rust-macos-native","capture":"ScreenCaptureKit.SCScreenshotManager","input":"CoreGraphics.CGEvent","oracle":"Accessibility.AXUIElement","input_opt_in":self.allow_input,"permissions":"checked without prompts on observation","target_scope":"pinned foreground PID + CGWindowID + process start","ax_tree":"bounded_partial","mapping":"single containing unrotated display only","unsupported":["cross-window modal rebinding","mixed-display windows","rotated displays","drag","clipboard","unattended unlock"],"live_acceptance":"pending","atomicity":"pre/post validation; OS does not offer a transactional capture-and-input primitive"})
    }
    fn observe(&mut self) -> Result<Scene> {
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
        Ok(self.metadata_scene(after, capture.size, capture.png))
    }
    fn revalidate(&mut self, reference: &Resolved) -> Result<Scene> {
        if reference.scope.element.is_none() {
            return self.observe();
        }
        Ok(self.metadata_scene(self.metadata()?, [0, 0], Vec::new()))
    }

    fn act(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        intent: &Intent,
        value: Option<&str>,
    ) -> Result<Option<bool>> {
        self.dispatch_input(expected, target, intent, value)
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
fn source() -> Result<CGEventSource> {
    CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
        fault(
            "input_source_unavailable",
            "Native event source is unavailable.",
        )
    })
}
fn key_event(key: u16, flags: CGEventFlags, text: Option<&[u16]>) -> Result<()> {
    let down = CGEvent::new_keyboard_event(source()?, key, true)
        .map_err(|_| fault("input_allocation_failed", "Key event was not allocated."))?;
    let up = CGEvent::new_keyboard_event(source()?, key, false)
        .map_err(|_| fault("input_allocation_failed", "Key event was not allocated."))?;
    for event in [&down, &up] {
        event.set_flags(flags);
        if let Some(text) = text {
            event.set_string_from_utf16_unchecked(text);
        }
    }
    down.post(CGEventTapLocation::HID);
    up.post(CGEventTapLocation::HID);
    Ok(())
}
fn utf16_chunks(text: &str, limit: usize) -> Vec<Vec<u16>> {
    let mut out = Vec::new();
    let mut chunk = Vec::new();
    for c in text.chars() {
        let mut buf = [0u16; 2];
        let units = c.encode_utf16(&mut buf);
        if chunk.len() + units.len() > limit && !chunk.is_empty() {
            out.push(std::mem::take(&mut chunk));
        }
        chunk.extend_from_slice(units);
    }
    if !chunk.is_empty() {
        out.push(chunk);
    }
    out
}
fn parse_chord(value: &str) -> Result<(u16, CGEventFlags)> {
    let mut flags = CGEventFlags::empty();
    let mut key = None;
    for token in value.split('+') {
        let code = match token.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "super" | "meta" => {
                flags |= CGEventFlags::CGEventFlagCommand;
                continue;
            }
            "shift" => {
                flags |= CGEventFlags::CGEventFlagShift;
                continue;
            }
            "ctrl" | "control" => {
                flags |= CGEventFlags::CGEventFlagControl;
                continue;
            }
            "alt" | "option" => {
                flags |= CGEventFlags::CGEventFlagAlternate;
                continue;
            }
            "a" => 0,
            "s" => 1,
            "d" => 2,
            "f" => 3,
            "h" => 4,
            "g" => 5,
            "z" => 6,
            "x" => 7,
            "c" => 8,
            "v" => 9,
            "b" => 11,
            "q" => 12,
            "w" => 13,
            "e" => 14,
            "r" => 15,
            "y" => 16,
            "t" => 17,
            "o" => 31,
            "u" => 32,
            "i" => 34,
            "p" => 35,
            "l" => 37,
            "j" => 38,
            "k" => 40,
            "n" => 45,
            "m" => 46,
            "enter" | "return" => 36,
            "tab" => 48,
            "space" => 49,
            "backspace" => 51,
            "escape" | "esc" => 53,
            "left" => 123,
            "right" => 124,
            "down" => 125,
            "up" => 126,
            "minus" | "-" => 27,
            _ => {
                return Err(fault(
                    "unsupported_key",
                    "Use documented physical key names; do not substitute shell commands.",
                ));
            }
        };
        if key.replace(code).is_some() {
            return Err(fault(
                "invalid_chord",
                "Exactly one non-modifier key is required.",
            ));
        }
    }
    Ok((
        key.ok_or_else(|| fault("invalid_chord", "Chord contains no key."))?,
        flags,
    ))
}
fn process_epoch(pid: u32) -> Result<String> {
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
fn console_identity() -> Result<Identity> {
    unsafe {
        let dict = Cf::owned(ffi::CGSessionCopyCurrentDictionary())?;
        if ffi::boolean(ffi::field(dict.0, "kCGSSessionOnConsoleKey")) != Some(true)
            || ffi::boolean(ffi::field(dict.0, "kCGSessionLoginDoneKey")) != Some(true)
        {
            return Err(fault(
                "session_not_interactive",
                "No authenticated console GUI session.",
            ));
        }
        let user = ffi::number(ffi::field(dict.0, "kCGSSessionUserIDKey"))
            .ok_or_else(|| fault("session_unknown", "Missing console UID."))?;
        let console = ffi::number(ffi::field(dict.0, "kCGSSessionConsoleSetKey"))
            .ok_or_else(|| fault("session_unknown", "Missing console seat identity."))?;
        if user != libc::geteuid() as i64 {
            return Err(fault(
                "session_user_mismatch",
                "MCP process is not the console user.",
            ));
        }
        let mut host = [0u8; 16];
        let timeout = libc::timespec {
            tv_sec: 1,
            tv_nsec: 0,
        };
        if libc::gethostuuid(host.as_mut_ptr(), &timeout) != 0 {
            return Err(fault(
                "host_identity_unknown",
                "Cannot attest local host UUID.",
            ));
        }
        let mut boot = libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        };
        let mut len = std::mem::size_of_val(&boot);
        if libc::sysctlbyname(
            c"kern.boottime".as_ptr(),
            (&mut boot as *mut libc::timeval).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return Err(fault("session_unknown", "Cannot bind current boot epoch."));
        }
        Ok(Identity {
            host: format!("macos-{}", digest(&host)),
            guest: None,
            session: format!("uid:{user}:console:{console}:boot:{}", boot.tv_sec),
            seat: format!("console:{console}"),
            desktop: "quartz-console".into(),
        })
    }
}
fn windows() -> Result<Vec<Window>> {
    unsafe {
        let array = Cf::owned(ffi::CGWindowListCopyWindowInfo(1 | 16, 0))?;
        let rows = ffi::array(array.0, 2048)?;
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
            if let (Some(id), Some(pid)) = (number, pid) {
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
                });
            }
        }
        Ok(out)
    }
}
fn display_layout(window: Rect) -> Result<(String, bool)> {
    let mut ids = CGDisplay::active_displays()
        .map_err(|_| fault("layout_unknown", "Cannot enumerate native active displays."))?;
    if ids.len() > 32 {
        return Err(fault("display_budget", "More than 32 native displays."));
    }
    ids.sort_unstable();
    let mut text = String::new();
    let mut containing = 0;
    for id in ids {
        let d = CGDisplay::new(id);
        let b = d.bounds();
        let r = Rect {
            x: b.origin.x,
            y: b.origin.y,
            w: b.size.width,
            h: b.size.height,
        };
        text.push_str(&format!(
            "{id}:{r:?}:{}:{}:{};",
            d.pixels_wide(),
            d.pixels_high(),
            d.rotation()
        ));
        if r.contains_rect(window) && d.rotation().abs() < 0.001 {
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
    let id = digest(format!("{}:{native_id}:{role}:{subrole:?}:{r:?}", el.pid()?).as_bytes());
    let value = read_classified_value(text_class, || {
        el.attr("AXValue").ok().and_then(|v| v.string().ok())
    });
    Ok(AxElement {
        element: Element {
            id,
            name,
            point: [r.x + r.w / 2.0, r.y + r.h / 2.0],
            visible: None,
            occluded: None,
            state_revision: Some(digest(
                format!(
                    "{text_class:?}:{value:?}:{:?}",
                    el.attr("AXEnabled").ok().and_then(|v| v.boolean())
                )
                .as_bytes(),
            )),
        },
        text_class,
        value,
    })
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
fn ax_tree(
    window: &Cf,
    system: &Cf,
    bounds: Rect,
    blocked: Option<bool>,
) -> Result<Vec<AxElement>> {
    let start = Instant::now();
    let mut pending = VecDeque::new();
    for child in window.children().unwrap_or_default() {
        pending.push_back((child, 1));
    }
    let mut nodes = 0;
    let mut out = Vec::new();
    while let Some((node, depth)) = pending.pop_front() {
        if nodes >= 128 || start.elapsed() > Duration::from_millis(1500) {
            break;
        }
        nodes += 1;
        if let Ok(mut e) = element_description(&node) {
            let point = e.element.point;
            let hit = if bounds.contains(point) && blocked == Some(false) {
                hit_test(system, point).ok()
            } else {
                None
            };
            let visible = hit.is_some_and(|h| unsafe { ffi::CFEqual(h.0, node.0) } != 0);
            e.element.visible = Some(visible);
            e.element.occluded = Some(!visible);
            out.push(e);
        }
        if depth < 8 {
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
                id: "id".into(),
                name: "Input".into(),
                point: [0.0, 0.0],
                visible: Some(true),
                occluded: Some(false),
                state_revision: None,
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
