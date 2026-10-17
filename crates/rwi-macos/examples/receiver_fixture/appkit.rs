//! Standalone main-thread AppKit receiver. Only public framework selectors.
//! Default passive receiver: no key posting, global monitor, capture or private SPI.
//! The separately opted-in Command+Tab actor pins one allocated peer and existing
//! post/listen authorization; it never grants permission or acts as an RWI fallback.
use super::command_tab;
use super::model::{
    DispatchEvent, DispatchRegion, DispatchStage, FrameworkPointerSnapshot, MouseEvent, MouseKind,
    Options, Rect, Region, State, Store, SystemPanelOracle, activation_policy_ready,
    is_dispatch_mouse_type,
};
use objc2::{
    msg_send,
    rc::{Allocated, Retained, autoreleasepool},
    runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel},
    sel,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use std::{
    cell::{Cell, RefCell},
    ffi::CStr,
    panic::AssertUnwindSafe,
};

#[path = "native_dnd_appkit.rs"]
mod native_dnd;

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}
#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {
    /// Public Foundation NSRunLoopMode constant, not a guessed string or SPI.
    static NSRunLoopCommonModes: *const AnyObject;
}

struct UI {
    state: State,
    store: Store,
    app: Retained<AnyObject>,
    window: Retained<AnyObject>,
    cover: Option<Retained<AnyObject>>,
    modal_sheet: Option<Retained<AnyObject>>,
    modal_button: Option<Retained<AnyObject>>,
    color_panel: Option<Retained<AnyObject>>,
    font_panel: Option<Retained<AnyObject>>,
    file_panel: Option<Retained<AnyObject>>,
    file_panel_directory: Option<std::path::PathBuf>,
    button: Retained<AnyObject>,
    hover: Retained<AnyObject>,
    canvas: Retained<AnyObject>,
    input: Retained<AnyObject>,
    scroll: Retained<AnyObject>,
    summary: Retained<AnyObject>,
    tooltip_text: Retained<NSString>,
    tooltip_tag: isize,
    /// Only this receiver's public addTrackingRect return; never a decoded pointer.
    legacy_tracking_tag: Option<isize>,
    legacy_registration: Option<rwi_macos::cooperative_tracking::OwnedRegistration>,
    cooperative_provider: Option<rwi_macos::cooperative_tracking::OwnerServer>,
    cooperative_timer: Option<Retained<AnyObject>>,
    lifetime_timer: Option<Retained<AnyObject>>,
    environment_timer: Option<Retained<AnyObject>>,
    self_foreground_timer: Option<Retained<AnyObject>>,
    command_tab_timer: Option<Retained<AnyObject>>,
    command_tab_actor: Option<command_tab::native::Actor>,
    native_dnd_objects: Option<native_dnd::Objects>,
    semantic_ui_objects: Option<super::semantic_alternate_ui::native::Objects>,
    _text_client_objects: Option<super::text_client::native::Objects>,
    // AppKit target and delegate are unretained properties. Keep them alive.
    _actions: Retained<AnyObject>,
}
thread_local! {
    static UI_STATE: RefCell<Option<UI>> = const { RefCell::new(None) };
    // Only the opt-in own canvas writes this after its real drawRect callback.
    static LIVE_FRESHNESS_CANVAS_DRAW_TICK: Cell<Option<u64>> = const { Cell::new(None) };
    static COOPERATIVE_CLOSE_GUARD: RefCell<Option<rwi_macos::cooperative_tracking::BusyState>> = const { RefCell::new(None) };
    static COOPERATIVE_CLOSE_PENDING: Cell<bool> = const { Cell::new(false) };
    static DISPATCH_BORROW_SKIPS: Cell<u64> = const { Cell::new(0) };
    // Independent selector diagnostics survive unavailable or reentrant UI_STATE.
    static DEFAULT_RESPONDER_DIAGNOSTICS: Cell<super::model::DefaultResponderDiagnostics> =
        const { Cell::new(super::model::DefaultResponderDiagnostics {
            selector_calls: 0, borrow_skips: 0, no_ui_calls: 0, other_button_calls: 0,
        }) };
    // Selector may be called reentrantly before/inside UI dispatch. No RefCell borrow.
    static FIRST_MOUSE: Cell<super::first_mouse::FirstMouseProbe> =
        const { Cell::new(super::first_mouse::FirstMouseProbe::new(true)) };
    // Record failures even while teardown temporarily owns UI outside this slot.
    static CALLBACK_FAILURES: Cell<u64> = const { Cell::new(0) };
    // Completion can be reentrant during panel teardown; never borrow UI there.
    static FILE_PANEL_RESPONSES: RefCell<Vec<isize>> = const { RefCell::new(Vec::new()) };
    static FILE_PANEL_RESPONSES_DROPPED: Cell<u64> = const { Cell::new(0) };
}

/// AppKit teardown may synchronously draw or notify delegates. Own the value,
/// not a RefCell borrow, across those calls; restore it even during unwinding.
struct DetachedSlot<'a, T> {
    slot: &'a RefCell<Option<T>>,
    value: Option<T>,
}
impl<T> Drop for DetachedSlot<'_, T> {
    fn drop(&mut self) {
        self.slot.replace(self.value.take());
    }
}
fn with_detached_slot<T, R>(
    slot: &RefCell<Option<T>>,
    work: impl FnOnce(&mut T) -> R,
) -> Option<R> {
    let value = slot.borrow_mut().take()?;
    let mut owned = DetachedSlot {
        slot,
        value: Some(value),
    };
    Some(work(owned.value.as_mut().unwrap()))
}
fn with_detached_ui<R>(work: impl FnOnce(&mut UI) -> R) -> Option<R> {
    UI_STATE.with(|slot| with_detached_slot(slot, work))
}

fn class(name: &'static CStr) -> Result<&'static AnyClass, String> {
    AnyClass::get(name).ok_or_else(|| format!("AppKit class unavailable: {name:?}"))
}
fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}
fn geometry(frame: NSRect) -> Rect {
    Rect {
        x: frame.origin.x,
        y: frame.origin.y,
        width: frame.size.width,
        height: frame.size.height,
    }
}
unsafe fn screen_id(screen: *mut AnyObject) -> Option<u32> {
    if screen.is_null() {
        return None;
    }
    let key = NSString::from_str("NSScreenNumber");
    unsafe {
        let description: *mut AnyObject = msg_send![screen, deviceDescription];
        let number: *mut AnyObject = msg_send![description, objectForKey:&*key];
        (!number.is_null()).then(|| msg_send![number, unsignedIntValue])
    }
}
fn explicit_screen_frame(display_id: u32) -> Result<Rect, String> {
    unsafe {
        let screens: Retained<AnyObject> = msg_send![class(c"NSScreen")?, screens];
        let count: usize = msg_send![&*screens, count];
        for index in 0..count {
            let screen: *mut AnyObject = msg_send![&*screens, objectAtIndex:index];
            if screen_id(screen) == Some(display_id) {
                return Ok(geometry(msg_send![screen, frame]));
            }
        }
    }
    Err(format!(
        "explicit display {display_id} is not in NSScreen; no fallback window"
    ))
}
fn frontmost_pid() -> Option<i32> {
    // Public metadata only: no title/text, enumeration, capture or global hook.
    unsafe {
        let workspace: *mut AnyObject = msg_send![class(c"NSWorkspace").ok()?, sharedWorkspace];
        let app: *mut AnyObject = msg_send![workspace, frontmostApplication];
        (!app.is_null()).then(|| msg_send![app, processIdentifier])
    }
}
unsafe fn view(class: &AnyClass, frame: NSRect) -> Retained<AnyObject> {
    let allocated: Allocated<AnyObject> = unsafe { msg_send![class, alloc] };
    unsafe { msg_send![allocated, initWithFrame:frame] }
}

/// No Rust panic may unwind through Objective-C's callback ABI.
fn callback(f: impl FnOnce()) {
    if std::panic::catch_unwind(AssertUnwindSafe(f)).is_err() {
        CALLBACK_FAILURES.with(|count| count.set(count.get().saturating_add(1)));
        eprintln!("receiver_fixture: callback panic; receipt must not be accepted");
        UI_STATE.with(|cell| {
            if let Ok(mut borrowed) = cell.try_borrow_mut()
                && let Some(ui) = borrowed.as_mut()
            {
                ui.state.status = "error";
                ui.state.output_error = Some("callback_panicked".into());
                let _ = ui.store.persist(&ui.state);
            }
        });
    }
}
/// Read runtime class metadata, not a selector on an unknown tracking item.
fn passive_tracking_type(object: &AnyObject, expected: &AnyClass) -> bool {
    let mut current = Some(object.class());
    for _ in 0..64 {
        let Some(class) = current else {
            return false;
        };
        if std::ptr::eq(class, expected) {
            return true;
        }
        current = class.superclass();
    }
    false
}
fn passive_tracking_descriptors(
    areas: &AnyObject,
    hover: &AnyObject,
    hover_window_matches: bool,
    count: usize,
) -> Option<super::model::TrackingAreaProbe> {
    let expected = class(c"NSTrackingArea").ok()?;
    let rows = (0..count.min(super::model::TRACKING_DESCRIPTOR_LIMIT)).map(|index| {
        let mut row = super::model::TrackingAreaDescriptor {
            index,
            options: None,
            rect: None,
            owner_is_hover: None,
            owner_window_matches: None,
            native_type_matches: false,
        };
        // NSArray is retained by the caller. No item/owner pointer is stored in
        // Rust state, survives this main-thread sample, or is serialized.
        let raw: *mut AnyObject = unsafe { msg_send![areas, objectAtIndex:index] };
        let Some(area) = (unsafe { raw.as_ref() }) else {
            return row;
        };
        if !passive_tracking_type(area, expected) {
            return row;
        }
        row.native_type_matches = true;
        unsafe {
            let options: usize = msg_send![area, options];
            row.options = u64::try_from(options).ok();
            row.rect = super::model::passive_visible_rect(geometry(msg_send![area, rect]));
            let owner: *mut AnyObject = msg_send![area, owner];
            if !owner.is_null() {
                let own = std::ptr::eq(owner.cast_const(), hover);
                row.owner_is_hover = Some(own);
                // The known retained hover's window was just read by the caller.
                // Never send window or any selector to a nil/arbitrary weak owner.
                if own {
                    row.owner_window_matches = Some(hover_window_matches);
                }
            }
        }
        row
    });
    Some(super::model::TrackingAreaProbe::bounded(count, rows))
}

impl UI {
    /// Diagnostic oracle for this allocated receiving fixture only. It neither
    /// changes native session state nor acknowledges an application goal.
    pub(super) fn note_native_timeline(
        &mut self,
        phase: crate::native_dnd::Phase,
        native_sequence: Option<isize>,
        operation: Option<usize>,
    ) {
        if !self.state.native_dnd.enabled {
            return;
        }
        static ORIGIN: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        let origin = ORIGIN.get_or_init(std::time::Instant::now);
        self.sample_environment();
        let unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|d| u64::try_from(d.as_millis()).ok());
        // This origin is local to the receiving process, not a clock for sorting
        // callbacks from another application. Unix samples retain that distinction.
        let process_monotonic_ns = u64::try_from(origin.elapsed().as_nanos()).unwrap_or(u64::MAX);
        self.state
            .native_dnd
            .note_timeline(crate::native_dnd::TimelineEvent {
                phase,
                native_sequence,
                active_sequence: self.state.native_dnd.active_sequence,
                operation,
                unix_ms,
                process_monotonic_ns,
                app_active: self.state.app_active,
                window_key: self.state.window_key,
                frontmost_pid: self.state.frontmost_pid,
                app_frontmost: self.state.app_frontmost,
            });
    }
    fn arm_first_down_foreground_timer(&mut self, delay_ms: u32) -> Result<(), String> {
        if self.self_foreground_timer.is_some() {
            return Err("own foreground timer already exists; never overwrite it".into());
        }
        // Explicit fixture-only main-thread actor. The existing callback checks
        // exact timer ownership/running lifetime before own-app public activation.
        unsafe {
            let timer: Retained<AnyObject> = msg_send![class(c"NSTimer")?,
                timerWithTimeInterval:f64::from(delay_ms) / 1000.,
                target:&*self._actions, selector:sel!(ownForegroundRequested:),
                userInfo:std::ptr::null_mut::<AnyObject>(), repeats:false];
            let main: *mut AnyObject = msg_send![class(c"NSRunLoop")?, mainRunLoop];
            let _: () = msg_send![main, addTimer:&*timer, forMode:NSRunLoopCommonModes];
            self.self_foreground_timer = Some(timer);
        }
        Ok(())
    }
    fn arm_command_tab_timer(&mut self, delay_ms: u32) -> Result<(), String> {
        if self.command_tab_timer.is_some() {
            return Err("Command+Tab timer already exists; no replay".into());
        }
        unsafe {
            let timer: Retained<AnyObject> = msg_send![class(c"NSTimer")?, timerWithTimeInterval:f64::from(delay_ms)/1000., target:&*self._actions, selector:sel!(commandTabRequested:), userInfo:std::ptr::null_mut::<AnyObject>(), repeats:false];
            let main: *mut AnyObject = msg_send![class(c"NSRunLoop")?, mainRunLoop];
            let _: () = msg_send![main, addTimer:&*timer, forMode:NSRunLoopCommonModes];
            self.command_tab_timer = Some(timer);
        }
        Ok(())
    }
    fn invalidate_lifetime_timer(&mut self) {
        for slot in [
            &mut self.lifetime_timer,
            &mut self.environment_timer,
            &mut self.self_foreground_timer,
            &mut self.command_tab_timer,
            &mut self.cooperative_timer,
        ] {
            if let Some(timer) = slot.take() {
                // The run loop, timer target and all UI owners are on this main thread.
                unsafe {
                    let _: () = msg_send![&*timer, invalidate];
                }
            }
        }
    }
    fn close_system_panels(&mut self) {
        if let Some(panel) = self.file_panel.take() {
            unsafe {
                let _: () = msg_send![&*panel, cancel:std::ptr::null_mut::<AnyObject>()];
                let _: () = msg_send![&*panel, orderOut:std::ptr::null_mut::<AnyObject>()];
            }
            sample_system_panel(&panel, &self.window, &mut self.state.system_panels.file);
        }
        for (slot, observed) in [
            (&mut self.color_panel, &mut self.state.system_panels.color),
            (&mut self.font_panel, &mut self.state.system_panels.font),
        ] {
            if let Some(panel) = slot.take() {
                unsafe {
                    let _: () = msg_send![&*panel, orderOut:std::ptr::null_mut::<AnyObject>()];
                    let _: () = msg_send![&*panel, close];
                    observed.visible = Some(msg_send![&*panel, isVisible]);
                    observed.window_key = Some(msg_send![&*panel, isKeyWindow]);
                }
            }
        }
    }
    fn close_own_modal(&mut self) {
        if let Some(sheet) = self.modal_sheet.take() {
            // No sheet delegate or input bridge: only our own retained lifecycle.
            unsafe {
                let _: () = msg_send![&*self.window, endSheet:&*sheet];
                let _: () = msg_send![&*sheet, orderOut:std::ptr::null_mut::<AnyObject>()];
                let _: () = msg_send![&*sheet, close];
                self.state.modal.visible = Some(msg_send![&*sheet, isVisible]);
                let parent: *mut AnyObject = msg_send![&*sheet, sheetParent];
                self.state.modal.attached_to_own_parent =
                    Some(std::ptr::eq(parent.cast_const(), &*self.window));
            }
            self.modal_button = None;
        }
    }
    fn close_own_cover(&mut self) {
        if let Some(cover) = self.cover.take() {
            // Only our retained own auxiliary window. It has no delegate, so this
            // does not recursively close or stop the primary receiver window.
            unsafe {
                let _: () = msg_send![&*cover, close];
                let visible: bool = msg_send![&*cover, isVisible];
                self.state.cover_visible = Some(visible);
                if self.state.interactive_sibling_window_key.is_some() {
                    let key: bool = msg_send![&*cover, isKeyWindow];
                    self.state.interactive_sibling_window_key = Some(key);
                }
                if visible {
                    self.state.output_error = Some("own_cover_close_not_observed".into());
                }
            }
        }
    }
    fn sample_environment(&mut self) {
        if self.state.live_freshness_ticks == Some(true) {
            self.state.live_freshness_last_canvas_draw_tick =
                LIVE_FRESHNESS_CANVAS_DRAW_TICK.with(Cell::get);
        }
        self.state.default_responder_mouse.diagnostics =
            DEFAULT_RESPONDER_DIAGNOSTICS.with(Cell::get);
        self.state.system_panels.file.completion_callbacks_dropped =
            FILE_PANEL_RESPONSES_DROPPED.with(Cell::get);
        FILE_PANEL_RESPONSES.with(|queue| {
            for response in queue.borrow_mut().drain(..) {
                self.state.native_file_panel_completed(response);
            }
        });
        if let Some(panel) = &self.file_panel {
            sample_system_panel(panel, &self.window, &mut self.state.system_panels.file);
        }
        if let Some(panel) = &self.color_panel {
            sample_system_panel(panel, &self.window, &mut self.state.system_panels.color);
        }
        if let Some(panel) = &self.font_panel {
            sample_system_panel(panel, &self.window, &mut self.state.system_panels.font);
        }
        // Own AppKit activation/key state and OS frontmost PID are independent.
        // No title/text, app enumeration, capture or global event hook.
        unsafe {
            self.state.app_active = Some(msg_send![&*self.app, isActive]);
            self.state.window_key = Some(msg_send![&*self.window, isKeyWindow]);
            self.state.responder.window_miniaturized =
                Some(msg_send![&*self.window, isMiniaturized]);
            self.state.responder.accepts_mouse_moved =
                Some(msg_send![&*self.window, acceptsMouseMovedEvents]);
            let first: *mut AnyObject = msg_send![&*self.window, firstResponder];
            self.state.responder.first_responder_class = first.as_ref().map(|object| {
                object
                    .class()
                    .name()
                    .to_string_lossy()
                    .chars()
                    .take(128)
                    .collect()
            });
            self.state.responder.first_responder_region =
                first.as_ref().and_then(|object| self.region(object));
            let areas: Retained<AnyObject> = msg_send![&*self.hover, trackingAreas];
            let total: usize = msg_send![&*areas, count];
            self.state.responder.hover_tracking_areas = Some(total);
            self.state.responder.allows_inactive_tooltips = Some(msg_send![
                &*self.window,
                allowsToolTipsWhenApplicationIsInactive
            ]);
            self.state.responder.hover_visible_rect =
                super::model::passive_visible_rect(geometry(msg_send![&*self.hover, visibleRect]));
            let hover_window: *mut AnyObject = msg_send![&*self.hover, window];
            let window_matches = std::ptr::eq(hover_window.cast_const(), &*self.window);
            self.state.responder.hover_window_matches = Some(window_matches);
            self.state.responder.hover_tracking_descriptors =
                passive_tracking_descriptors(&areas, &self.hover, window_matches, total);
            self.state.actual_window_frame = Some(geometry(msg_send![&*self.window, frame]));
            let screen: *mut AnyObject = msg_send![&*self.window, screen];
            self.state.actual_display_id = screen_id(screen);
            if let Some(cover) = &self.cover {
                self.state.cover_window_number = Some(msg_send![&**cover, windowNumber]);
                self.state.cover_frame = Some(geometry(msg_send![&**cover, frame]));
                self.state.cover_visible = Some(msg_send![&**cover, isVisible]);
                if self.state.interactive_sibling_window_key.is_some() {
                    self.state.interactive_sibling_window_key =
                        Some(msg_send![&**cover, isKeyWindow]);
                }
                let screen: *mut AnyObject = msg_send![&**cover, screen];
                self.state.cover_display_id = screen_id(screen);
                let ordered: Retained<AnyObject> = msg_send![&*self.app, orderedWindows];
                let count: usize = msg_send![&*ordered, count];
                let mut primary_index = None;
                let mut cover_index = None;
                for index in 0..count {
                    let item: *mut AnyObject = msg_send![&*ordered, objectAtIndex:index];
                    if std::ptr::eq(item.cast_const(), &*self.window) {
                        primary_index = Some(index);
                    }
                    if std::ptr::eq(item.cast_const(), &**cover) {
                        cover_index = Some(index);
                    }
                }
                self.state.cover_above_primary = cover_index
                    .zip(primary_index)
                    .map(|(cover, primary)| cover < primary);
            }
            if let Some(sheet) = &self.modal_sheet {
                self.state.modal.window_number = Some(msg_send![&**sheet, windowNumber]);
                self.state.modal.visible = Some(msg_send![&**sheet, isVisible]);
                self.state.modal.is_sheet = Some(msg_send![&**sheet, isSheet]);
                self.state.modal.window_key = Some(msg_send![&**sheet, isKeyWindow]);
                self.state.modal.frame = Some(geometry(msg_send![&**sheet, frame]));
                let parent: *mut AnyObject = msg_send![&**sheet, sheetParent];
                self.state.modal.attached_to_own_parent =
                    Some(std::ptr::eq(parent.cast_const(), &*self.window));
                let screen: *mut AnyObject = msg_send![&**sheet, screen];
                self.state.modal.display_id = screen_id(screen);
            }
            let clip: *mut AnyObject = msg_send![&*self.scroll, contentView];
            let bounds: NSRect = msg_send![clip, bounds];
            self.state.scroll.clip_origin = Some([bounds.origin.x, bounds.origin.y]);
        }
        if let Some(objects) = &self.semantic_ui_objects
            && let Some(sample) = objects.snapshot()
        {
            self.state.semantic_alternate_ui = Some(sample);
        }
        self.state.frontmost_pid = frontmost_pid();
        self.state.app_frontmost = self
            .state
            .frontmost_pid
            .map(|pid| pid as u32 == self.state.pid);
    }
    fn flush(&mut self) {
        self.sample_environment();
        self.flush_sampled();
    }
    fn flush_sampled(&mut self) {
        self.state.first_mouse = FIRST_MOUSE.with(Cell::get);
        self.state.ax_provider_probe = super::ax_delay::snapshot();
        if self._text_client_objects.is_some()
            && let Err(error) = super::text_client::native::flush()
        {
            self.state.output_error = Some(error);
            self.state.status = "error";
        }
        if let Err(error) = self.store.persist(&self.state) {
            self.state.output_error = Some(error.to_string());
            self.state.status = "error";
            eprintln!("receiver_fixture JSON output failed: {error}");
        }
        let text = NSString::from_str(&self.state.summary());
        let title = NSString::from_str(&format!("Safe count: {}", self.state.safe_button_actions));
        // SAFETY: UI is retained on the main thread; setters are public AppKit.
        unsafe {
            if let Some(button) = &self.modal_button {
                let title = NSString::from_str(&format!(
                    "Modal safe count: {}",
                    self.state.modal.safe_actions
                ));
                let _: () = msg_send![&**button, setTitle:&*title];
            }
            let _: () = msg_send![&*self.summary, setStringValue:&*text];
            let _: () = msg_send![&*self.button, setTitle:&*title];
            let _: () = msg_send![&*self.hover, setNeedsDisplay:true];
            let _: () = msg_send![&*self.canvas, setNeedsDisplay:true];
            if let Some(objects) = &self.native_dnd_objects {
                let _: () = msg_send![&*objects.destination, setNeedsDisplay:true];
            }
        }
    }
    fn region(&self, view: &AnyObject) -> Option<Region> {
        if std::ptr::eq(view, &*self.hover) {
            Some(Region::HoverZone)
        } else if std::ptr::eq(view, &*self.canvas) {
            Some(Region::Canvas)
        } else {
            None
        }
    }
    unsafe fn dispatch_hit_region(&self, window_point: NSPoint) -> Option<DispatchRegion> {
        unsafe {
            let content: *mut AnyObject = msg_send![&*self.window, contentView];
            if content.is_null() {
                return None;
            }
            let parent: *mut AnyObject = msg_send![content, superview];
            // NSView.hitTest: takes the receiver's superview coordinates. The
            // original NSEvent point is in this window's AppKit coordinates.
            let parent_point: NSPoint = if parent.is_null() {
                window_point
            } else {
                msg_send![parent, convertPoint:window_point,
                    fromView:std::ptr::null_mut::<AnyObject>()]
            };
            let hit: *mut AnyObject = msg_send![content, hitTest:parent_point];
            if hit.is_null() {
                return None;
            }
            for (candidate, region) in [
                (&*self.button, DispatchRegion::SafeButton),
                (&*self.hover, DispatchRegion::HoverZone),
                (&*self.canvas, DispatchRegion::Canvas),
            ] {
                let descendant: bool = msg_send![hit, isDescendantOf:candidate];
                if std::ptr::eq(hit.cast_const(), candidate) || descendant {
                    return Some(region);
                }
            }
            Some(DispatchRegion::OtherOwnContent)
        }
    }
}

fn passive_event_mouse_delta(event: &AnyObject, native_type: usize) -> [Option<f64>; 2] {
    super::model::passive_mouse_move_delta(native_type, || unsafe {
        // Lazy public getters only for actual MouseMoved; absent selectors stay unknown.
        let has_x: bool = msg_send![event, respondsToSelector:sel!(deltaX)];
        let has_y: bool = msg_send![event, respondsToSelector:sel!(deltaY)];
        let x: Option<f64> = has_x.then(|| -> f64 { msg_send![event, deltaX] });
        let y: Option<f64> = has_y.then(|| -> f64 { msg_send![event, deltaY] });
        [x, y]
    })
}

fn dispatch_subtype(event: &AnyObject, native_type: usize) -> Option<i16> {
    // Lazy getter: enter/exit must still be forwarded/recorded, but calling
    // subtype on a tracking NSEvent raises rather than returning an unknown.
    super::model::dispatch_subtype_supported(native_type)
        .then(|| unsafe { msg_send![event, subtype] })
}

impl UI {
    fn register_legacy_tracking(&mut self) -> Result<(), String> {
        if self.legacy_registration.is_some() || self.legacy_tracking_tag.is_some() {
            return Err("legacy registration exists; no overwrite".into());
        }
        let registration =
            rwi_macos::cooperative_tracking::OwnedRegistration::register(&self.hover, &self.window)
                .map_err(|e| e.to_string())?;
        let tag = registration
            .public_tag_decimal()
            .ok_or("own registration absent")?
            .parse::<isize>()
            .map_err(|_| "own public tag encoding changed")?;
        let number: isize = unsafe { msg_send![&*self.window, windowNumber] };
        self.legacy_tracking_tag = Some(tag);
        COOPERATIVE_CLOSE_GUARD.with(|g| *g.borrow_mut() = Some(registration.busy_state()));
        self.legacy_registration = Some(registration);
        self.state.legacy_tracking = Some(super::model::LegacyTrackingProbe {
            requested: true,
            registration_live: true,
            public_tag_decimal: Some(tag.to_string()),
            registered_window_number: Some(number),
            owner_is_own_hover: Some(true),
            registration_user_data_is_null: Some(true),
            assume_inside: Some(false),
            registration_sequence: Some(self.state.sequence),
            ..Default::default()
        });
        self.flush();
        Ok(())
    }
    fn remove_legacy_tracking(&mut self) {
        if cooperative_busy() {
            COOPERATIVE_CLOSE_PENDING.with(|p| p.set(true));
            return;
        }
        self.cooperative_provider.take();
        let tag = self.legacy_tracking_tag.take();
        let actual_remove = if let Some(mut registration) = self.legacy_registration.take() {
            match registration.unregister() {
                Ok(removed) => removed,
                Err(error) => {
                    self.state.output_error = Some(error.to_string());
                    false
                }
            }
        } else {
            false
        };
        COOPERATIVE_CLOSE_GUARD.with(|g| g.borrow_mut().take());
        if tag.is_some()
            && let Some(probe) = self.state.legacy_tracking.as_mut()
        {
            probe.registration_live = false;
            if actual_remove {
                probe.removal_count = probe.removal_count.saturating_add(1);
            }
        }
        if let Some(status) = self.state.cooperative_tracking.as_mut() {
            status["provider_closed"] = serde_json::json!(true);
        }
    }
    fn framework_pointer(&self) -> Option<FrameworkPointerSnapshot> {
        // Public getters only. Never set a pointer, active/key state or responder,
        // synthesize tracking messages, or count these reads as received events.
        let event_class = AnyClass::get(c"NSEvent")?;
        unsafe {
            let screen: NSPoint = msg_send![event_class, mouseLocation];
            let window: NSPoint = msg_send![&*self.window, convertPointFromScreen:screen];
            let outside: NSPoint = msg_send![&*self.window, mouseLocationOutsideOfEventStream];
            let hover: NSPoint = msg_send![&*self.hover, convertPoint:window, fromView:std::ptr::null_mut::<AnyObject>()];
            let bounds: NSRect = msg_send![&*self.hover, bounds];
            let finite = |p: NSPoint| (p.x.is_finite() && p.y.is_finite()).then_some([p.x, p.y]);
            let hover_point = finite(hover);
            let contains = hover_point
                .filter(|_| {
                    bounds.origin.x.is_finite()
                        && bounds.origin.y.is_finite()
                        && bounds.size.width.is_finite()
                        && bounds.size.height.is_finite()
                        && bounds.size.width > 0.
                        && bounds.size.height > 0.
                })
                .map(|p| {
                    p[0] >= bounds.origin.x
                        && p[0] < bounds.origin.x + bounds.size.width
                        && p[1] >= bounds.origin.y
                        && p[1] < bounds.origin.y + bounds.size.height
                });
            Some(FrameworkPointerSnapshot {
                screen_point: finite(screen),
                screen_converted_to_own_window: finite(window),
                own_window_outside_event_stream: finite(outside),
                screen_converted_to_own_hover: hover_point,
                screen_point_in_own_hover: contains,
            })
        }
    }
}

fn record_dispatch(this: &AnyObject, event: &AnyObject, stage: DispatchStage) -> bool {
    let native_type: usize = unsafe { msg_send![event, type] };
    if !is_dispatch_mouse_type(native_type) {
        return false;
    }
    let mut recorded = false;
    callback(|| {
        UI_STATE.with(|cell| {
            let Ok(mut borrowed) = cell.try_borrow_mut() else {
                DISPATCH_BORROW_SKIPS.with(|skips| skips.set(skips.get() + 1));
                return;
            };
            let Some(ui) = borrowed.as_mut() else {
                return;
            };
            unsafe {
                // Public NSEvent.h properties valid for these mouse types only;
                // No characters or keyboard codes. The optional diagnostic below
                // reads public framework pointer state only for this own receiver;
                // it never moves the cursor or inspects another application's UI.
                let window_number: isize = msg_send![event, windowNumber];
                let point: NSPoint = msg_send![event, locationInWindow];
                let receiver_match = match stage {
                    DispatchStage::Application => std::ptr::eq(this, &*ui.app),
                    DispatchStage::Window => std::ptr::eq(this, &*ui.window),
                };
                let own_window_match = receiver_match
                    && ui
                        .state
                        .window_number
                        .is_some_and(|owned| owned > 0 && owned == window_number);
                let hit_region = if own_window_match && point.x.is_finite() && point.y.is_finite() {
                    ui.dispatch_hit_region(point)
                } else {
                    None
                };
                let framework_pointer = own_window_match.then(|| ui.framework_pointer()).flatten();
                // Passive own-window getters only, before superclass dispatch.
                // Preserve unknown for unmatched events; never make key or activate.
                let own_window_key_at_entry =
                    own_window_match.then(|| msg_send![&*ui.window, isKeyWindow]);
                let own_app_active_at_entry =
                    own_window_match.then(|| msg_send![&*ui.app, isActive]);
                let [delta_x, delta_y] = passive_event_mouse_delta(event, native_type);
                ui.state.dispatch_probe.receive(DispatchEvent {
                    sequence: 0,
                    stage,
                    native_type,
                    window_number,
                    window_point: [point.x, point.y],
                    subtype: dispatch_subtype(event, native_type),
                    timestamp_seconds: msg_send![event, timestamp],
                    delta_x,
                    delta_y,
                    own_window_match,
                    own_window_key_at_entry,
                    own_app_active_at_entry,
                    hit_region,
                    framework_pointer,
                });
            }
            recorded = true;
            // No persist/flush/setters before superclass dispatch. This leaves
            // all receiving-view/tooltip counters and route behavior untouched.
        })
    });
    recorded
}
fn persist_dispatch_only() {
    callback(|| {
        UI_STATE.with(|cell| {
            let Ok(mut borrowed) = cell.try_borrow_mut() else {
                DISPATCH_BORROW_SKIPS.with(|skips| skips.set(skips.get() + 1));
                return;
            };
            let Some(ui) = borrowed.as_mut() else {
                return;
            };
            ui.state.dispatch_probe.borrow_skips = DISPATCH_BORROW_SKIPS.with(Cell::get);
            ui.state.first_mouse = FIRST_MOUSE.with(Cell::get);
            ui.state.default_responder_mouse.diagnostics =
                DEFAULT_RESPONDER_DIAGNOSTICS.with(Cell::get);
            // JSON-only after super returns: no environment query, redraw,
            // selection, activation or control mutation. IO overhead is explicit.
            if let Err(error) = ui.store.persist(&ui.state) {
                ui.state.output_error = Some(error.to_string());
                ui.state.status = "error";
                eprintln!("receiver_fixture dispatch JSON output failed: {error}");
            }
        })
    });
}
fn note_default_responder_diagnostic(
    update: impl FnOnce(&mut super::model::DefaultResponderDiagnostics),
) {
    DEFAULT_RESPONDER_DIAGNOSTICS.with(|cell| {
        let mut value = cell.get();
        update(&mut value);
        cell.set(value);
    });
}
extern "C" fn default_button_mouse_moved(this: &AnyObject, _cmd: Sel, event: &AnyObject) {
    note_default_responder_diagnostic(|d| d.selector_calls = d.selector_calls.saturating_add(1));
    callback(|| {
        UI_STATE.with(|cell| {
            let Ok(mut borrowed) = cell.try_borrow_mut() else {
                note_default_responder_diagnostic(|d| {
                    d.borrow_skips = d.borrow_skips.saturating_add(1)
                });
                return;
            };
            let Some(ui) = borrowed.as_mut() else {
                note_default_responder_diagnostic(|d| {
                    d.no_ui_calls = d.no_ui_calls.saturating_add(1)
                });
                return;
            };
            if !std::ptr::eq(this, &*ui.button) {
                note_default_responder_diagnostic(|d| {
                    d.other_button_calls = d.other_button_calls.saturating_add(1)
                });
                return;
            }
            unsafe {
                let native_type: usize = msg_send![event, type];
                let window_number: isize = msg_send![event, windowNumber];
                let point: NSPoint = msg_send![event, locationInWindow];
                let first: *mut AnyObject = msg_send![&*ui.window, firstResponder];
                let [delta_x, delta_y] = passive_event_mouse_delta(event, native_type);
                ui.state
                    .default_responder_mouse
                    .receive(super::model::DefaultResponderMove {
                        sequence: 0,
                        native_type,
                        window_number,
                        window_point: [point.x, point.y],
                        timestamp_seconds: msg_send![event, timestamp],
                        delta_x,
                        delta_y,
                        own_window_match: ui.state.window_number == Some(window_number),
                        current_first_responder_match: std::ptr::eq(first.cast_const(), this),
                        receiving_app_active: msg_send![&*ui.app, isActive],
                        receiving_window_key: msg_send![&*ui.window, isKeyWindow],
                    });
            }
        });
    });
    // No UI borrow crosses super; never redirect to hover or change its responder.
    let superclass = AnyClass::get(c"NSButton").expect("registered button superclass");
    unsafe {
        let _: () = msg_send![super(this, superclass), mouseMoved:event];
    }
    // Existing JSON-only recorder after super: no setters/redraw/activation.
    persist_dispatch_only();
}

extern "C" fn application_send_event(this: &AnyObject, _cmd: Sel, event: &AnyObject) {
    let recorded = record_dispatch(this, event, DispatchStage::Application);
    // Both base classes are permanent AppKit classes validated before registering
    // this subclass. Always forward every event once, including ignored key/wake.
    let superclass = AnyClass::get(c"NSApplication").expect("registered AppKit superclass");
    unsafe {
        let _: () = msg_send![super(this, superclass), sendEvent:event];
    }
    if recorded {
        persist_dispatch_only();
    }
}
extern "C" fn window_send_event(this: &AnyObject, _cmd: Sel, event: &AnyObject) {
    let recorded = record_dispatch(this, event, DispatchStage::Window);
    let superclass = AnyClass::get(c"NSWindow").expect("registered AppKit superclass");
    unsafe {
        let _: () = msg_send![super(this, superclass), sendEvent:event];
    }
    if recorded {
        persist_dispatch_only();
    }
    record_own_scroll_after_super(this, event);
}

fn record_own_scroll_after_super(this: &AnyObject, event: &AnyObject) {
    // Not an input bridge: normal NSWindow/NSScrollView handling has returned.
    // Read only this own window's scroll viewport, without moving it ourselves.
    let native_type: usize = unsafe { msg_send![event, type] };
    if native_type != 22 {
        return;
    }
    callback(|| {
        UI_STATE.with(|cell| {
            let mut borrowed = cell.borrow_mut();
            let Some(ui) = borrowed.as_mut() else { return };
            if !std::ptr::eq(this, &*ui.window) {
                return;
            }
            let number: isize = unsafe { msg_send![event, windowNumber] };
            if Some(number) != ui.state.window_number {
                return;
            }
            unsafe {
                let clip: *mut AnyObject = msg_send![&*ui.scroll, contentView];
                let bounds: NSRect = msg_send![clip, bounds];
                let dx: f64 = msg_send![event, scrollingDeltaX];
                let dy: f64 = msg_send![event, scrollingDeltaY];
                ui.sample_environment();
                ui.state
                    .scroll_received([dx, dy], [bounds.origin.x, bounds.origin.y]);
            }
            ui.flush();
        });
    });
}

extern "C" fn text_changed(_this: &AnyObject, _cmd: Sel, notification: &AnyObject) {
    callback(|| {
        UI_STATE.with(|cell| {
            let mut borrowed = cell.borrow_mut();
            let Some(ui) = borrowed.as_mut() else { return };
            let control: *mut AnyObject = unsafe { msg_send![notification, object] };
            if !std::ptr::eq(control.cast_const(), &*ui.input) {
                return;
            }
            // Standard nonsecure fixture field, not keyboard event characters,
            // other windows, passwords or a programmatically set target value.
            let text: Retained<NSString> = unsafe { msg_send![&*ui.input, stringValue] };
            ui.sample_environment();
            ui.state.text_changed(&text.to_string());
            ui.flush();
        });
    });
}

unsafe fn stop_own_app(app: &AnyObject, window_number: isize) -> Result<(), String> {
    // stop: requests termination after the current AppKit event. A timer can run
    // while nextEventMatchingMask: waits, so enqueue an own-application wake too.
    // This is not a mouse/key event, native post-to-PID or another application's
    // input. Never record it as a mouse receipt or verified goal.
    unsafe {
        let _: () = msg_send![app, stop:std::ptr::null_mut::<AnyObject>()];
        let wake: Retained<AnyObject> = msg_send![class(c"NSEvent")?,
            otherEventWithType:15usize, // NSEventTypeApplicationDefined
            location:NSPoint::new(0., 0.), modifierFlags:0usize,
            timestamp:0.0f64, windowNumber:window_number,
            context:std::ptr::null_mut::<AnyObject>(), subtype:0i16,
            data1:0isize, data2:0isize];
        let _: () = msg_send![app, postEvent:&*wake, atStart:true];
    }
    Ok(())
}

extern "C" fn command_tab_requested(_this: &AnyObject, _cmd: Sel, sender: &AnyObject) {
    callback(|| {
        UI_STATE.with(|cell| {
            let mut borrowed = cell.borrow_mut();
            let Some(ui) = borrowed.as_mut() else { return };
            if ui
                .command_tab_timer
                .as_ref()
                .is_none_or(|timer| !std::ptr::eq(&**timer, sender))
                || ui.state.status != "running"
                || ui.state.command_tab.first_down_sequence.is_none()
            {
                return;
            }
            if let Some(timer) = ui.command_tab_timer.take() {
                unsafe {
                    let _: () = msg_send![&*timer, invalidate];
                }
            }
            if ui.state.drag.active.is_none() {
                ui.state.command_tab.fired = true;
                ui.state.command_tab.error =
                    Some("original gesture already released; no system input".into());
            } else if let Some(actor) = ui.command_tab_actor.take() {
                actor.post_once(&mut ui.state.command_tab, frontmost_pid);
            } else {
                ui.state.command_tab.fired = true;
                if ui.state.command_tab.error.is_none() {
                    ui.state.command_tab.error =
                        Some("allocated peer actor unavailable; no fallback".into());
                }
            }
            ui.sample_environment();
            ui.flush();
        })
    });
}
extern "C" fn sheet_safe_action(_this: &AnyObject, _cmd: Sel, _sender: &AnyObject) {
    callback(|| {
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                ui.state.modal_action("fixture_sheet_safe_action");
                ui.flush();
            }
        })
    });
}
extern "C" fn sheet_close_action(_this: &AnyObject, _cmd: Sel, _sender: &AnyObject) {
    callback(|| {
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                ui.state.modal_action("fixture_close_sheet_action");
                ui.close_own_modal();
                ui.flush();
            }
        })
    });
}
extern "C" fn sheet_open_action(_this: &AnyObject, _cmd: Sel, _sender: &AnyObject) {
    callback(|| {
        // Drop the Rust UI borrow before AppKit may re-enter while creating a sheet.
        let Some((parent, actions)) = UI_STATE.with(|cell| {
            let mut borrowed = cell.borrow_mut();
            let ui = borrowed.as_mut()?;
            if ui.modal_sheet.is_some() || ui.state.status != "running" {
                return None;
            }
            ui.state.modal_action("fixture_open_sheet_action");
            ui.flush();
            Some((ui.window.clone(), ui._actions.clone()))
        }) else {
            return;
        };
        let created = (|| -> Result<(Retained<AnyObject>, Retained<AnyObject>), String> {
            unsafe {
                let frame: NSRect = msg_send![&*parent, frame];
                let allocated: Allocated<AnyObject> = msg_send![class(c"NSPanel")?, alloc];
                let sheet: Retained<AnyObject> = msg_send![allocated,
                    initWithContentRect:rect(frame.origin.x+150., frame.origin.y+270.,360.,150.),
                    styleMask:3usize, backing:2usize, defer:false];
                let _: () = msg_send![&*sheet, setReleasedWhenClosed:false];
                let title = NSString::from_str("RWI own modal sheet");
                let _: () = msg_send![&*sheet, setTitle:&*title];
                let content = view(class(c"NSView")?, rect(0., 0., 360., 150.));
                label(
                    &content,
                    "Own document-modal sheet",
                    rect(16., 105., 328., 24.),
                )?;
                let safe = view(class(c"NSButton")?, rect(16., 58., 208., 34.));
                let title = NSString::from_str("Modal safe count: 0");
                let _: () = msg_send![&*safe, setTitle:&*title];
                let _: () = msg_send![&*safe, setBezelStyle:1usize];
                let _: () = msg_send![&*safe, setTarget:&*actions];
                let _: () = msg_send![&*safe, setAction:sel!(sheetSafeAction:)];
                identify(&safe, "rwi-fixture-modal-safe", "RWI modal safe button");
                let _: () = msg_send![&*content, addSubview:&*safe];
                let close = view(class(c"NSButton")?, rect(230., 16., 112., 30.));
                let title = NSString::from_str("Close sheet");
                let _: () = msg_send![&*close, setTitle:&*title];
                let _: () = msg_send![&*close, setBezelStyle:1usize];
                let _: () = msg_send![&*close, setTarget:&*actions];
                let _: () = msg_send![&*close, setAction:sel!(sheetCloseAction:)];
                identify(&close, "rwi-fixture-modal-close", "RWI close own sheet");
                let _: () = msg_send![&*content, addSubview:&*close];
                let _: () = msg_send![&*sheet, setContentView:&*content];
                Ok((sheet, safe))
            }
        })();
        let (sheet, button) = match created {
            Ok(value) => value,
            Err(error) => {
                UI_STATE.with(|cell| {
                    if let Some(ui) = cell.borrow_mut().as_mut() {
                        ui.state.output_error = Some(error);
                        ui.state.status = "error";
                        ui.flush();
                    }
                });
                return;
            }
        };
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                ui.modal_sheet = Some(sheet.clone());
                ui.modal_button = Some(button);
            }
        });
        unsafe {
            // Public optional completion handler; no fabricated event or global activation.
            let _: () = msg_send![&*parent, beginSheet:&*sheet,
                completionHandler:std::ptr::null_mut::<block2::Block<dyn Fn(isize)>>()];
        }
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                ui.flush();
            }
        });
    });
}
fn sample_system_panel(
    panel: &AnyObject,
    own_parent: &AnyObject,
    observed: &mut SystemPanelOracle,
) {
    // AppKit shared panel readback; no capture, global hook or guessed owner chain.
    unsafe {
        observed.window_number = Some(msg_send![panel, windowNumber]);
        observed.visible = Some(msg_send![panel, isVisible]);
        observed.window_key = Some(msg_send![panel, isKeyWindow]);
        observed.frame = Some(geometry(msg_send![panel, frame]));
        observed.display_id = screen_id(msg_send![panel, screen]);
        observed.is_sheet = Some(msg_send![panel, isSheet]);
        let parent: *mut AnyObject = msg_send![panel, sheetParent];
        observed.attached_to_own_parent = Some(std::ptr::eq(parent.cast_const(), own_parent));
        observed.hides_on_deactivate = Some(msg_send![panel, hidesOnDeactivate]);
        observed.on_active_space = Some(msg_send![panel, isOnActiveSpace]);
        observed.occlusion_state = Some(msg_send![panel, occlusionState]);
    }
    observed.quartz_description = observed.window_number.and_then(|number| {
        u32::try_from(number)
            .ok()
            .filter(|id| *id != 0)
            .map(own_panel_quartz_description)
    });
}

/// Query only the retained panel ID, not a desktop-wide inventory. Neither a
/// property nor this non-atomic read grants capture, activation or input.
fn own_panel_quartz_description(window: u32) -> serde_json::Value {
    use core_foundation::{
        array::CFArray,
        base::{CFType, TCFType},
        boolean::CFBoolean,
        dictionary::CFDictionary,
        number::CFNumber,
        string::CFString,
    };
    use serde_json::json;

    // CGWindowIDs in this CFArray are pointer-sized encoded values, not
    // CFNumbers or a packed u32 slice. Null retain/release callbacks are used.
    let ids = CFArray::from_copyable(&[window as usize]);
    let raw = unsafe {
        core_graphics::window::CGWindowListCreateDescriptionFromArray(ids.as_concrete_TypeRef())
    };
    let mut data = json!({"scope":"exact_own_panel_quartz_metadata","window":window,
        "non_atomic":true,"input_authority":false,"input_sent":false,
        "sampled_unix_ms":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default().as_millis() as u64});
    if raw.is_null() {
        data["read_available"] = json!(false);
        return data;
    }
    let rows: CFArray<CFDictionary<CFString, CFType>> =
        unsafe { TCFType::wrap_under_create_rule(raw) };
    data["read_available"] = json!(true);
    data["records"] = json!(rows.len());
    if rows.len() != 1 {
        return data;
    }
    let row = rows.get(0).expect("one native metadata row");
    let integer = |key: &str| {
        row.find(CFString::new(key))
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|v| v.to_i64())
    };
    let pid = integer("kCGWindowOwnerPID");
    let number = integer("kCGWindowNumber");
    let own = pid == Some(i64::from(std::process::id())) && number == Some(i64::from(window));
    data["owner_matches"] = json!(own);
    if !own {
        return data;
    }
    data["pid"] = json!(pid);
    data["layer"] = json!(integer("kCGWindowLayer"));
    data["onscreen"] = json!(
        row.find(CFString::new("kCGWindowIsOnscreen"))
            .and_then(|v| v.downcast::<CFBoolean>())
            .map(bool::from)
    );
    data["alpha"] = json!(
        row.find(CFString::new("kCGWindowAlpha"))
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|v| v.to_f64())
    );
    data
}
fn open_system_panel(kind: &'static str, sender: &AnyObject) {
    callback(|| {
        let allowed = UI_STATE.with(|cell| {
            let mut b = cell.borrow_mut();
            b.as_mut()
                .is_some_and(|ui| ui.state.system_panel_open(kind))
        });
        if !allowed {
            return;
        }
        // Never hold UI's RefCell borrow over potentially reentrant framework calls.
        let panel: Result<Retained<AnyObject>, String> = (|| unsafe {
            if kind == "color" {
                let p: Retained<AnyObject> = msg_send![class(c"NSColorPanel")?, sharedColorPanel];
                let _: () = msg_send![&*p, orderFront:sender];
                Ok(p)
            } else {
                let manager: Retained<AnyObject> =
                    msg_send![class(c"NSFontManager")?, sharedFontManager];
                let _: () = msg_send![&*manager, orderFrontFontPanel:sender];
                let p: Option<Retained<AnyObject>> = msg_send![&*manager, fontPanel:false];
                p.ok_or_else(|| "shared_font_panel_not_created".into())
            }
        })();
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                match panel {
                    Ok(panel) => {
                        if kind == "color" {
                            ui.color_panel = Some(panel);
                        } else {
                            ui.font_panel = Some(panel);
                        }
                    }
                    Err(error) => ui.state.output_error = Some(error),
                }
                ui.flush();
            }
        });
    });
}
/// Public native NSOpenPanel, opt-in own parent and empty allocated directory.
/// Never opens a selected file, captures other apps, requests OS permissions,
/// synthesizes provider identity, or attributes a request counter to visibility.
extern "C" fn file_panel_open(_this: &AnyObject, _cmd: Sel, _sender: &AnyObject) {
    callback(|| {
        let request = UI_STATE.with(|cell| {
            let mut b = cell.borrow_mut();
            let ui = b.as_mut()?;
            if ui
                .file_panel
                .as_ref()
                .is_some_and(|panel| unsafe { msg_send![&**panel, isVisible] })
                || !ui.state.system_panel_open("file")
            {
                return None;
            }
            Some((ui.window.clone(), ui.file_panel_directory.clone()?))
        });
        let Some((parent, directory)) = request else {
            return;
        };
        // Never hold a UI RefCell borrow across reentrant framework calls.
        let result: Result<Retained<AnyObject>, String> = (|| unsafe {
            let panel: Retained<AnyObject> = msg_send![class(c"NSOpenPanel")?, openPanel];
            let path =
                NSString::from_str(directory.to_str().ok_or("file_panel_directory_not_utf8")?);
            let url: Retained<AnyObject> =
                msg_send![class(c"NSURL")?, fileURLWithPath:&*path, isDirectory:true];
            let _: () = msg_send![&*panel, setDirectoryURL:&*url];
            let _: () = msg_send![&*panel, setCanChooseFiles:true];
            let _: () = msg_send![&*panel, setCanChooseDirectories:false];
            let _: () = msg_send![&*panel, setAllowsMultipleSelection:false];
            let title = NSString::from_str("RWI own empty file chooser — cancel only");
            let _: () = msg_send![&*panel, setTitle:&*title];
            let completion = block2::RcBlock::new(|response: isize| {
                FILE_PANEL_RESPONSES.with(|queue| {
                    let mut values = queue.borrow_mut();
                    if values.len() < 4 {
                        values.push(response);
                    } else {
                        FILE_PANEL_RESPONSES_DROPPED
                            .with(|dropped| dropped.set(dropped.get().saturating_add(1)));
                    }
                });
            });
            let _: () = msg_send![&*panel, beginSheetModalForWindow:&*parent, completionHandler:&*completion];
            Ok(panel)
        })();
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                match result {
                    Ok(panel) => ui.file_panel = Some(panel),
                    Err(error) => ui.state.output_error = Some(error),
                }
                ui.flush();
            }
        });
    });
}
extern "C" fn color_panel_open(_this: &AnyObject, _cmd: Sel, sender: &AnyObject) {
    open_system_panel("color", sender);
}
extern "C" fn font_panel_open(_this: &AnyObject, _cmd: Sel, sender: &AnyObject) {
    open_system_panel("font", sender);
}
extern "C" fn clicked(_this: &AnyObject, _cmd: Sel, _sender: &AnyObject) {
    callback(|| {
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                ui.sample_environment();
                ui.state.click();
                ui.flush();
            }
        })
    });
}
extern "C" fn window_closed(_this: &AnyObject, _cmd: Sel, _notification: &AnyObject) {
    callback(|| {
        with_detached_ui(|ui| {
            ui.sample_environment();
            ui.invalidate_lifetime_timer();
            ui.close_own_modal();
            ui.close_own_cover();
            ui.close_system_panels();
            ui.remove_legacy_tracking();
            let pending_native_drag = ui.state.native_dnd.note_pending_native_shutdown();
            ui.state.close();
            ui.flush();
            // stop: exits NSApplication's run loop after this close event.
            unsafe {
                if pending_native_drag {
                    // A direct-dispatch comparison can leave AppKit's native
                    // drag loop waiting after the owning window is closed.
                    // This dedicated disposable fixture must end its own app;
                    // never fake draggingSession:endedAtPoint: or signal another
                    // process. Flush the unconfirmed native end before quitting.
                    let _: () = msg_send![&*ui.app, terminate:std::ptr::null_mut::<AnyObject>()];
                    return;
                }
                if let Err(error) = stop_own_app(&ui.app, ui.state.window_number.unwrap_or(0)) {
                    ui.state.status = "error";
                    ui.state.output_error = Some(error);
                    ui.flush();
                }
            }
        });
    });
}

/// Explicit own-fixture teardown, independent of screen-sharing titlebar chrome.
/// Closing the source is not a forged drag-ended/cancelled callback.
extern "C" fn native_fixture_close(_this: &AnyObject, _cmd: Sel, _sender: &AnyObject) {
    callback(|| {
        let window = UI_STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            let ui = state.as_mut()?;
            if !ui.state.native_dnd.enabled {
                return None;
            }
            ui.state.native_dnd.explicit_close_requests += 1;
            ui.state.native_dnd.active_at_explicit_close = ui.state.native_dnd.active_sequence;
            ui.flush();
            Some(ui.window.clone())
        });
        if let Some(window) = window {
            // Public close on this retained own window, not another application.
            // Delegate callbacks run without a RefCell borrow spanning AppKit.
            unsafe {
                let _: () = msg_send![&*window, close];
            }
        }
    });
}

fn cooperative_busy() -> bool {
    COOPERATIVE_CLOSE_GUARD.with(|g| g.borrow().as_ref().is_some_and(|b| b.is_busy()))
}
extern "C" fn window_should_close(_this: &AnyObject, _cmd: Sel, _sender: &AnyObject) -> Bool {
    let busy = cooperative_busy();
    if busy {
        COOPERATIVE_CLOSE_PENDING.with(|p| p.set(true));
    }
    if !busy {
        callback(|| {
            with_detached_ui(|ui| ui.remove_legacy_tracking());
        });
    }
    Bool::new(!busy)
}
extern "C" fn cooperative_polled(_this: &AnyObject, _cmd: Sel, timer: &AnyObject) {
    callback(|| {
        // Take only the provider/registration, not the UI. UI stays installed;
        // no RefCell borrow spans NSApplication.sendEvent or its real callbacks.
        let work = UI_STATE.with(|cell| {
            let Ok(mut borrowed) = cell.try_borrow_mut() else {
                return None;
            };
            let ui = borrowed.as_mut()?;
            if ui.state.status != "running"
                || !ui
                    .cooperative_timer
                    .as_ref()
                    .is_some_and(|t| std::ptr::eq(&**t, timer))
            {
                return None;
            }
            Some((
                ui.cooperative_provider.take()?,
                ui.legacy_registration.take()?,
                ui.app.clone(),
            ))
        });
        let Some((mut provider, mut registration, app)) = work else {
            return;
        };
        let result = provider.poll(&mut registration, &app);
        let stats = provider.stats();
        let close = UI_STATE.with(|cell| {
            let mut borrowed = cell.borrow_mut();
            let ui = borrowed.as_mut()?;
            ui.cooperative_provider = Some(provider);
            ui.legacy_registration = Some(registration);
            if let Some(status) = ui.state.cooperative_tracking.as_mut() {
                status["provider_stats"] =
                    serde_json::to_value(stats).unwrap_or(serde_json::Value::Null);
            }
            if let Err(error) = result {
                ui.state.output_error = Some(error.to_string());
            }
            ui.sample_environment();
            ui.flush();
            (COOPERATIVE_CLOSE_PENDING.with(|p| p.replace(false)) && !cooperative_busy())
                .then(|| ui.window.clone())
        });
        if let Some(window) = close {
            unsafe {
                let _: () = msg_send![&*window,performClose:std::ptr::null_mut::<AnyObject>()];
            }
        }
    });
}
extern "C" fn environment_sampled(_this: &AnyObject, _cmd: Sel, timer: &AnyObject) {
    callback(|| {
        UI_STATE.with(|cell| {
            // Nested AppKit loops may re-enter while an event callback owns UI.
            // Skipping that sample is not a state transition or input receipt.
            let Ok(mut borrowed) = cell.try_borrow_mut() else {
                return;
            };
            let Some(ui) = borrowed.as_mut() else { return };
            if ui.state.status != "running"
                || !ui
                    .environment_timer
                    .as_ref()
                    .is_some_and(|owned| std::ptr::eq(&**owned, timer))
            {
                return;
            }
            let unix_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            // Public own-app/window readback; no activation, input or screenshots.
            ui.sample_environment();
            ui.state.note_environment_probe(unix_ms);
            ui.flush_sampled();
        })
    });
}
extern "C" fn own_foreground_requested(_this: &AnyObject, _cmd: Sel, timer: &AnyObject) {
    callback(|| {
        let own = UI_STATE.with(|cell| {
            let mut borrowed = cell.borrow_mut();
            let ui = borrowed.as_mut()?;
            if ui.state.status != "running"
                || !ui
                    .self_foreground_timer
                    .as_ref()
                    .is_some_and(|owned| std::ptr::eq(&**owned, timer))
            {
                return None;
            }
            let owned = ui.self_foreground_timer.take().unwrap();
            unsafe {
                let _: () = msg_send![&*owned, invalidate];
            }
            ui.sample_environment();
            ui.state.request_self_foreground();
            ui.flush();
            Some((
                ui.app.clone(),
                ui.window.clone(),
                ui.state.self_foreground_method,
            ))
        });
        let Some((app, window, method)) = own else {
            return;
        };
        // Public own-app/window lifecycle only. Never send events to another
        // application, synthesize a key/mouse, or claim a human input witness.
        // No RefCell borrow spans potentially reentrant AppKit lifecycle calls.
        unsafe {
            let legacy = method == Some("explicit_legacy_activate_ignoring_other_apps");
            let selector = if legacy {
                sel!(activateIgnoringOtherApps:)
            } else {
                sel!(activate)
            };
            let available: bool = msg_send![&*app, respondsToSelector:selector];
            if available {
                note_own_actor_phase(crate::native_dnd::Phase::ActorBeforeActivate);
                if legacy {
                    let _: () = msg_send![&*app, activateIgnoringOtherApps:true];
                } else {
                    let _: () = msg_send![&*app, activate];
                }
                note_own_actor_phase(crate::native_dnd::Phase::ActorAfterActivate);
                note_own_actor_phase(crate::native_dnd::Phase::ActorBeforeOrder);
                let _: () =
                    msg_send![&*window, makeKeyAndOrderFront:std::ptr::null_mut::<AnyObject>()];
                note_own_actor_phase(crate::native_dnd::Phase::ActorAfterOrder);
            } else {
                note_own_actor_phase(crate::native_dnd::Phase::ActorApiUnavailable);
                UI_STATE.with(|cell| {
                    if let Some(ui) = cell.borrow_mut().as_mut() {
                        ui.state.output_error = Some("own_foreground_api_unavailable".into());
                    }
                });
            }
        }
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                ui.flush();
            }
        });
    });
}
fn note_own_actor_phase(phase: crate::native_dnd::Phase) {
    UI_STATE.with(|cell| {
        if let Some(ui) = cell.borrow_mut().as_mut() {
            ui.note_native_timeline(phase, ui.state.native_dnd.active_sequence, None);
            ui.flush();
        }
    });
}
extern "C" fn lifetime_expired(_this: &AnyObject, _cmd: Sel, timer: &AnyObject) {
    callback(|| {
        if cooperative_busy() {
            COOPERATIVE_CLOSE_PENDING.with(|p| p.set(true));
            return;
        }
        let window = UI_STATE.with(|cell| {
            let mut borrowed = cell.borrow_mut();
            let ui = borrowed.as_mut()?;
            if ui.state.status == "closed"
                || !ui
                    .lifetime_timer
                    .as_ref()
                    .is_some_and(|owned| std::ptr::eq(&**owned, timer))
            {
                return None;
            }
            ui.sample_environment();
            ui.state.request_lifetime_close();
            ui.flush();
            Some(ui.window.clone())
        });
        if let Some(window) = window {
            // No RefCell borrow spans performClose: it synchronously invokes this
            // own window's delegate. Only windowWillClose records state.closed.
            unsafe {
                let _: () = msg_send![&*window, performClose:std::ptr::null_mut::<AnyObject>()];
            }
            let close_unconfirmed = UI_STATE.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .is_some_and(|ui| ui.state.status != "closed")
            });
            if close_unconfirmed {
                // Sharing chrome may disable the close-button action. This is
                // still the uniquely retained own window, never a shared control.
                eprintln!("receiver_fixture: performClose unconfirmed; closing own window");
                unsafe {
                    let _: () = msg_send![&*window, close];
                }
            }
            with_detached_ui(|ui| {
                if ui.state.status != "closed" {
                    // A refused/failed public close is not a fabricated closed
                    // receipt. Bound our run-loop lifetime but preserve the error.
                    ui.invalidate_lifetime_timer();
                    ui.state.status = "error";
                    ui.state.output_error = Some("lifetime_own_window_close_not_confirmed".into());
                    ui.flush();
                    unsafe {
                        if let Err(error) =
                            stop_own_app(&ui.app, ui.state.window_number.unwrap_or(0))
                        {
                            eprintln!("receiver_fixture stop failed: {error}");
                        }
                    }
                }
            });
        }
    });
}
extern "C" fn flipped(_this: &AnyObject, _cmd: Sel) -> Bool {
    Bool::YES
}
extern "C" fn first_mouse(_this: &AnyObject, _cmd: Sel, _event: *mut AnyObject) -> Bool {
    // Default YES calibration remains; explicit NO changes only this own client.
    FIRST_MOUSE.with(|state| {
        let mut probe = state.get();
        let accepted = probe.called();
        state.set(probe);
        Bool::new(accepted)
    })
}

fn receive(this: &AnyObject, event: &AnyObject, kind: MouseKind) {
    callback(|| {
        UI_STATE.with(|cell| {
        if let Some(ui) = cell.borrow_mut().as_mut()
            && let Some(region) = ui.region(this)
        {
            // SAFETY: AppKit invokes these methods with a live mouse NSEvent.
            // Both positions come from public NSView conversion; window points
            // are AppKit bottom-left, local points are this flipped view's top-left.
            ui.sample_environment();
            let frontmost = ui.state.frontmost_pid;
            let receipt = unsafe {
                let point: NSPoint = msg_send![event, locationInWindow];
                let local: NSPoint = msg_send![this, convertPoint:point, fromView:std::ptr::null_mut::<AnyObject>()];
                MouseEvent {
                    region, kind,
                    local_point: [local.x, local.y], window_point: [point.x, point.y],
                    native_type: msg_send![event, type],
                    window_number: msg_send![event, windowNumber],
                    receiving_app_active: msg_send![&*ui.app, isActive],
                    receiving_window_key: msg_send![&*ui.window, isKeyWindow],
                    frontmost_pid: frontmost,
                    receiving_app_frontmost: frontmost.map(|pid| pid as u32 == ui.state.pid),
                    timestamp_seconds: msg_send![event, timestamp],
                    modifier_flags: msg_send![event, modifierFlags],
                    event_number: (kind != MouseKind::Moved)
                        .then(|| msg_send![event, eventNumber]),
                    pressure: matches!(kind, MouseKind::Down | MouseKind::Dragged | MouseKind::Up)
                        .then(|| msg_send![event, pressure]),
                    // NSEvent.h: deltaX/Y are valid for mouse move/drag. Preserve
                    // the raw values; no flipped-view/pixel-unit reinterpretation.
                    delta: matches!(kind, MouseKind::Moved | MouseKind::Dragged)
                        .then(|| [msg_send![event, deltaX], msg_send![event, deltaY]]),
                }
            };
            // Lazy scalar getters only for the actual received 8/9 kind. No
            // trackingArea object decoding and no event-tag/address export.
            if region == Region::HoverZone && matches!(receipt.native_type, 8 | 9)
                && let Some(tag) = ui.legacy_tracking_tag {
                let actual: isize = unsafe { msg_send![event, trackingNumber] };
                let data: *mut std::ffi::c_void = unsafe { msg_send![event, userData] };
                let exact_window = ui.state.window_number == Some(receipt.window_number);
                if let Some(probe) = ui.state.legacy_tracking.as_mut() {
                    probe.note_event(receipt.native_type, exact_window, actual == tag, data.is_null());
                }
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .and_then(|value| u64::try_from(value.as_millis()).ok());
            let actor_delay = ui.state.arm_foreground_on_first_canvas_down(&receipt, now);
            let command_tab_delay = command_tab::arm(&mut ui.state, &receipt, now);
            ui.state.receive(receipt);
            if let Some(delay) = actor_delay
                && let Err(error) = ui.arm_first_down_foreground_timer(delay)
            {
                ui.state.output_error = Some(error);
            }
            if let Some(delay) = command_tab_delay
                && let Err(error) = ui.arm_command_tab_timer(delay)
            {
                ui.state.command_tab.error = Some(error);
            }
            ui.flush();
        }
    })
    });
}
extern "C" fn mouse_down(this: &AnyObject, _cmd: Sel, event: &AnyObject) {
    receive(this, event, MouseKind::Down);
}
extern "C" fn mouse_dragged(this: &AnyObject, _cmd: Sel, event: &AnyObject) {
    receive(this, event, MouseKind::Dragged);
}
extern "C" fn mouse_up(this: &AnyObject, _cmd: Sel, event: &AnyObject) {
    receive(this, event, MouseKind::Up);
}
extern "C" fn mouse_moved(this: &AnyObject, _cmd: Sel, event: &AnyObject) {
    receive(this, event, MouseKind::Moved);
}
extern "C" fn mouse_entered(this: &AnyObject, _cmd: Sel, event: &AnyObject) {
    receive(this, event, MouseKind::Entered);
}
extern "C" fn mouse_exited(this: &AnyObject, _cmd: Sel, event: &AnyObject) {
    receive(this, event, MouseKind::Exited);
}

extern "C" fn tooltip_requested(
    this: &AnyObject,
    _cmd: Sel,
    view: &AnyObject,
    tag: isize,
    point: NSPoint,
    _data: *mut std::ffi::c_void,
) -> *mut NSString {
    // Public NSViewToolTipOwner callback, not a synthesized input event. A +0
    // return is backed by UI's retained string for the whole own-window lifetime.
    let mut text = std::ptr::null_mut();
    callback(|| {
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut()
                && std::ptr::eq(this, view)
                && ui.region(view) == Some(Region::HoverZone)
                && tag == ui.tooltip_tag
            {
                ui.sample_environment();
                // SDK documents this point in the tooltip view's coordinates.
                // Preserve it separately from event/mouse and never infer visible.
                ui.state.tooltip_requested([point.x, point.y]);
                ui.flush();
                text = Retained::as_ptr(&ui.tooltip_text).cast_mut();
            }
        })
    });
    text
}

extern "C" fn draw(this: &AnyObject, _cmd: Sel, _dirty: NSRect) {
    callback(|| {
        UI_STATE.with(|cell| {
            let borrowed = cell.borrow();
            let Some(ui) = borrowed.as_ref() else {
                return;
            };
            let Some(region) = ui.region(this) else {
                return;
            };
            // Public AppKit drawing runs only inside this view's drawRect: callback.
            unsafe {
                let color = match region {
                    Region::HoverZone if ui.state.hover.inside => sel!(systemGreenColor),
                    Region::HoverZone => sel!(controlBackgroundColor),
                    Region::Canvas => sel!(textBackgroundColor),
                };
                let color: *mut AnyObject = match color {
                    s if s == sel!(systemGreenColor) => {
                        msg_send![class(c"NSColor").unwrap(), systemGreenColor]
                    }
                    s if s == sel!(controlBackgroundColor) => {
                        msg_send![class(c"NSColor").unwrap(), controlBackgroundColor]
                    }
                    _ => msg_send![class(c"NSColor").unwrap(), textBackgroundColor],
                };
                let _: () = msg_send![color, setFill];
                let bounds: NSRect = msg_send![this, bounds];
                let _: () = msg_send![class(c"NSBezierPath").unwrap(), fillRect:bounds];
                if region == Region::Canvas && ui.state.live_freshness_ticks == Some(true) {
                    // Eight large black/white bits survive compact Live scaling. The
                    // low byte wraps; use the independently exported full tick too.
                    let tick = ui.state.environment_probe_ticks;
                    for bit in 0..8 {
                        let value = if (tick >> bit) & 1 == 0 {
                            0.08f64
                        } else {
                            0.95f64
                        };
                        let ink: *mut AnyObject = msg_send![class(c"NSColor").unwrap(),
                            colorWithSRGBRed:value, green:value, blue:value, alpha:1.0f64];
                        let _: () = msg_send![ink, setFill];
                        let _: () = msg_send![class(c"NSBezierPath").unwrap(),
                            fillRect:rect(f64::from(bit) * 45., 24., 45., 150.)];
                    }
                    LIVE_FRESHNESS_CANVAS_DRAW_TICK.with(|last| last.set(Some(tick)));
                }
                if region == Region::Canvas {
                    let path = ui.state.drag.active.as_ref().or(ui
                        .state
                        .drag
                        .last_completed
                        .as_ref());
                    if let Some(path) = path {
                        let curve: Retained<AnyObject> =
                            msg_send![class(c"NSBezierPath").unwrap(), new];
                        let blue: *mut AnyObject =
                            msg_send![class(c"NSColor").unwrap(), systemBlueColor];
                        let _: () = msg_send![blue, setStroke];
                        let _: () = msg_send![&*curve, setLineWidth:3.0f64];
                        for (index, point) in path.points.iter().enumerate() {
                            let point = NSPoint::new(point[0], point[1]);
                            if index == 0 {
                                let _: () = msg_send![&*curve, moveToPoint:point];
                            } else {
                                let _: () = msg_send![&*curve, lineToPoint:point];
                            }
                        }
                        let _: () = msg_send![&*curve, stroke];
                    }
                }
            }
        })
    });
}

struct FixtureClasses {
    actions: &'static AnyClass,
    receiver: &'static AnyClass,
    button: &'static AnyClass,
    application: &'static AnyClass,
    window: &'static AnyClass,
}
fn classes() -> Result<FixtureClasses, String> {
    let mut actions = ClassBuilder::new(c"RWIReceiverFixtureActions", class(c"NSObject")?)
        .ok_or("receiver action class already registered")?;
    let mut receiver = ClassBuilder::new(c"RWIReceiverFixtureView", class(c"NSView")?)
        .ok_or("receiver view class already registered")?;
    let mut button = ClassBuilder::new(c"RWIReceiverFixtureButton", class(c"NSButton")?)
        .ok_or("receiver button class already registered")?;
    let mut application =
        ClassBuilder::new(c"RWIReceiverFixtureApplication", class(c"NSApplication")?)
            .ok_or("receiver application class already registered")?;
    let mut window = ClassBuilder::new(c"RWIReceiverFixtureWindow", class(c"NSWindow")?)
        .ok_or("receiver window class already registered")?;
    // SAFETY: selector signatures exactly match the public AppKit SDK headers.
    unsafe {
        actions.add_method(sel!(clicked:), clicked as extern "C" fn(_, _, _));
        actions.add_method(
            sel!(nativeFixtureClose:),
            native_fixture_close as extern "C" fn(_, _, _),
        );
        actions.add_method(
            sel!(systemFileOpen:),
            file_panel_open as extern "C" fn(_, _, _),
        );
        actions.add_method(
            sel!(systemColorOpen:),
            color_panel_open as extern "C" fn(_, _, _),
        );
        actions.add_method(
            sel!(systemFontOpen:),
            font_panel_open as extern "C" fn(_, _, _),
        );
        actions.add_method(
            sel!(sheetOpenAction:),
            sheet_open_action as extern "C" fn(_, _, _),
        );
        actions.add_method(
            sel!(sheetSafeAction:),
            sheet_safe_action as extern "C" fn(_, _, _),
        );
        actions.add_method(
            sel!(sheetCloseAction:),
            sheet_close_action as extern "C" fn(_, _, _),
        );
        actions.add_method(
            sel!(controlTextDidChange:),
            text_changed as extern "C" fn(_, _, _),
        );
        actions.add_method(
            sel!(windowWillReturnFieldEditor:toObject:),
            super::text_client::native::field_editor as extern "C" fn(_, _, _, _) -> _,
        );
        actions.add_method(
            sel!(windowShouldClose:),
            window_should_close as extern "C" fn(_, _, _) -> _,
        );
        actions.add_method(
            sel!(cooperativePolled:),
            cooperative_polled as extern "C" fn(_, _, _),
        );
        actions.add_method(
            sel!(windowWillClose:),
            window_closed as extern "C" fn(_, _, _),
        );
        actions.add_method(
            sel!(lifetimeExpired:),
            lifetime_expired as extern "C" fn(_, _, _),
        );
        actions.add_method(
            sel!(environmentSampled:),
            environment_sampled as extern "C" fn(_, _, _),
        );
        actions.add_method(
            sel!(ownForegroundRequested:),
            own_foreground_requested as extern "C" fn(_, _, _),
        );
        actions.add_method(
            sel!(commandTabRequested:),
            command_tab_requested as extern "C" fn(_, _, _),
        );
        receiver.add_method(sel!(isFlipped), flipped as extern "C" fn(_, _) -> _);
        receiver.add_method(
            sel!(acceptsFirstMouse:),
            first_mouse as extern "C" fn(_, _, _) -> _,
        );
        button.add_method(
            sel!(acceptsFirstMouse:),
            first_mouse as extern "C" fn(_, _, _) -> _,
        );
        button.add_method(
            sel!(mouseMoved:),
            default_button_mouse_moved as extern "C" fn(_, _, _),
        );
        receiver.add_method(sel!(drawRect:), draw as extern "C" fn(_, _, _));
        receiver.add_method(sel!(mouseDown:), mouse_down as extern "C" fn(_, _, _));
        receiver.add_method(sel!(mouseDragged:), mouse_dragged as extern "C" fn(_, _, _));
        receiver.add_method(sel!(mouseUp:), mouse_up as extern "C" fn(_, _, _));
        receiver.add_method(sel!(mouseMoved:), mouse_moved as extern "C" fn(_, _, _));
        receiver.add_method(sel!(mouseEntered:), mouse_entered as extern "C" fn(_, _, _));
        receiver.add_method(sel!(mouseExited:), mouse_exited as extern "C" fn(_, _, _));
        receiver.add_method(
            sel!(view:stringForToolTip:point:userData:),
            tooltip_requested as extern "C" fn(_, _, _, _, _, _) -> _,
        );
        super::ax_delay::register(&mut application);
        application.add_method(
            sel!(sendEvent:),
            application_send_event as extern "C" fn(_, _, _),
        );
        window.add_method(
            sel!(sendEvent:),
            window_send_event as extern "C" fn(_, _, _),
        );
    }
    Ok(FixtureClasses {
        actions: actions.register(),
        receiver: receiver.register(),
        button: button.register(),
        application: application.register(),
        window: window.register(),
    })
}

unsafe fn label(
    content: &AnyObject,
    text: &str,
    frame: NSRect,
) -> Result<Retained<AnyObject>, String> {
    let label = unsafe { view(class(c"NSTextField")?, frame) };
    let string = NSString::from_str(text);
    unsafe {
        let _: () = msg_send![&*label, setStringValue:&*string];
        let _: () = msg_send![&*label, setEditable:false];
        let _: () = msg_send![&*label, setSelectable:false];
        let _: () = msg_send![&*label, setBezeled:false];
        let _: () = msg_send![&*label, setDrawsBackground:false];
        let _: () = msg_send![content, addSubview:&*label];
    }
    Ok(label)
}
unsafe fn identify(view: &AnyObject, identifier: &str, label: &str) {
    let identifier = NSString::from_str(identifier);
    let label = NSString::from_str(label);
    unsafe {
        let _: () = msg_send![view, setAccessibilityIdentifier:&*identifier];
        let _: () = msg_send![view, setAccessibilityLabel:&*label];
    }
}

extern "C" fn calibration_accepts_first_responder(_this: &AnyObject, _cmd: Sel) -> Bool {
    Bool::YES
}
fn hover_calibration_class(
    base: &'static AnyClass,
    enabled: bool,
) -> Result<&'static AnyClass, String> {
    if !enabled {
        return Ok(base);
    }
    let mut builder = ClassBuilder::new(c"RWIHoverFirstResponderCalibration", base)
        .ok_or("own hover calibration class already registered")?;
    // SAFETY: public NSResponder BOOL acceptsFirstResponder signature. The
    // subclass inherits normal receiver callbacks; no sendEvent override,
    // tracking-number synthesis, input sender or external application mutation.
    unsafe {
        builder.add_method(
            sel!(acceptsFirstResponder),
            calibration_accepts_first_responder as extern "C" fn(_, _) -> _,
        );
    }
    Ok(builder.register())
}

pub fn run(options: Options) -> Result<(), String> {
    // AppKit application lifetime and every retained UI object stay on main.
    if unsafe { libc::pthread_main_np() } != 1 {
        return Err("AppKit receiver must run on the main thread".into());
    }
    let store = Store::create(&options.state_dir).map_err(|e| e.to_string())?;
    let file_panel_directory = if options.system_panel_controls {
        let directory = options.state_dir.join("empty-native-file-panel");
        std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
        Some(directory)
    } else {
        None
    };
    // Fix the own-client policy before any own NSView/NSButton is constructed.
    FIRST_MOUSE.with(|probe| {
        probe.set(super::first_mouse::FirstMouseProbe::new(
            options.first_mouse_acceptance,
        ))
    });
    let mut state = State::new(options.run_id);
    state.first_mouse = FIRST_MOUSE.with(Cell::get);
    state.requested_display_id = Some(options.display_id);
    state.lifetime_ms = Some(options.lifetime_ms);
    state.environment_sample_ms = options.environment_sample_ms;
    state.live_freshness_ticks = options.live_freshness_ticks.then_some(true);
    state.system_panels.enabled = options.system_panel_controls;
    state.native_dnd.enabled = options.native_dnd_controls;
    state.self_foreground_on_first_down_after_ms = options.self_foreground_on_first_down_after_ms;
    state.command_tab.config = options.command_tab.clone();
    if options.self_foreground_after_ms.is_some()
        || options.self_foreground_on_first_down_after_ms.is_some()
    {
        state.self_foreground_method = Some(if options.legacy_self_foreground {
            "explicit_legacy_activate_ignoring_other_apps"
        } else {
            "public_modern_activate"
        });
    }
    state.responder.calibration_requested = options.hover_first_responder;
    store.persist(&state).map_err(|e| e.to_string())?;
    println!(
        "fixture prepared: title={:?} pid={} state={}",
        state.window_title,
        state.pid,
        store.path.display()
    );
    autoreleasepool(|_| {
        let FixtureClasses {
            actions: actions_class,
            receiver: receiver_class,
            button: button_class,
            application: application_class,
            window: window_class,
        } = classes()?;
        // SAFETY: this is the dedicated fixture process, initialized on main.
        // Public AppKit methods only; never activate another application.
        unsafe {
            // First sharedApplication is sent to our registered subclass. Do not
            // swizzle an existing singleton or override sharedApplication itself.
            let app: Retained<AnyObject> = msg_send![application_class, sharedApplication];
            let own_application: bool = msg_send![&*app, isKindOfClass:application_class];
            if !own_application {
                return Err("shared application does not contain own dispatch probe".into());
            }
            // Explicit own-fixture opt-in exposes a normal Cmd+Tab/Dock app for
            // real foreground takeover tests; the default remains Accessory.
            // Setting policy does not request activation of any application.
            let policy = if options.regular_app { 0isize } else { 1isize };
            let before: isize = msg_send![&*app, activationPolicy];
            let accepted: bool = msg_send![&*app, setActivationPolicy:policy];
            let observed: isize = msg_send![&*app, activationPolicy];
            state.application_identity.activation_policy_before = Some(before);
            state
                .application_identity
                .activation_policy_request_accepted = Some(accepted);
            state.application_identity.activation_policy = Some(observed);
            let own: Retained<AnyObject> =
                msg_send![class(c"NSRunningApplication")?, currentApplication];
            let identifier: Option<Retained<NSString>> = msg_send![&*own, bundleIdentifier];
            let name: Option<Retained<NSString>> = msg_send![&*own, localizedName];
            state.application_identity.bundle_identifier =
                identifier.map(|s| s.to_string().chars().take(256).collect());
            state.application_identity.localized_name =
                name.map(|s| s.to_string().chars().take(128).collect());
            store.persist(&state).map_err(|e| e.to_string())?;
            if !activation_policy_ready(before, policy, accepted, observed) {
                return Err(format!(
                    "explicit fixture activation policy not observed: before={before}, requested={policy}, accepted={accepted}, observed={observed}"
                ));
            }
            let _: () = msg_send![&*app, finishLaunching];
            // Ground the caller-selected NSScreen before creating any window.
            // No default/main-screen creation followed by a later move.
            let screen_frame = explicit_screen_frame(options.display_id)?;
            // Public NSWindowStyleMask: titled | closable | miniaturizable.
            const ROOT_WINDOW_STYLE: usize = 1 | 2 | 4;
            // Existing default layout is unchanged; opt-in semantic control uses its own extra row.
            let content_height = if options.semantic_alternate_ui {
                580.
            } else {
                520.
            };
            let decoration: NSRect = msg_send![class(c"NSWindow")?,
                frameRectForContentRect:rect(0., 0., 660., content_height), styleMask:ROOT_WINDOW_STYLE];
            let placed = if options.boundary_canvas_right {
                screen_frame
                    .place_boundary_canvas_right(decoration.size.width, decoration.size.height)?
            } else if options.boundary_right {
                screen_frame.place_boundary_right(decoration.size.width, decoration.size.height)?
            } else if options.right_side {
                screen_frame.place_right(decoration.size.width, decoration.size.height)?
            } else {
                screen_frame.place(decoration.size.width, decoration.size.height)?
            };
            let requested_frame = rect(placed.x, placed.y, placed.width, placed.height);
            let content_frame: NSRect = msg_send![class(c"NSWindow")?,
                contentRectForFrameRect:requested_frame, styleMask:ROOT_WINDOW_STYLE];
            state.requested_window_frame = Some(placed);
            let actions: Retained<AnyObject> = msg_send![actions_class, new];
            let allocated: Allocated<AnyObject> = msg_send![window_class, alloc];
            let window: Retained<AnyObject> = msg_send![allocated,
                initWithContentRect:content_frame, styleMask:ROOT_WINDOW_STYLE,
                backing:2usize, defer:false];
            let title = NSString::from_str(&state.window_title);
            let _: () = msg_send![&*window, setTitle:&*title];
            let _: () = msg_send![&*window, setReleasedWhenClosed:false];
            let _: () = msg_send![&*window, setDelegate:&*actions];
            let _: () = msg_send![&*window, setAcceptsMouseMovedEvents:true];
            let _: () = msg_send![&*window, setFrame:requested_frame, display:false, animate:false];
            let content = view(class(c"NSView")?, rect(0., 0., 660., content_height));
            let semantic_ui_objects = if options.semantic_alternate_ui {
                let objects = super::semantic_alternate_ui::native::Objects::new()?;
                let _: () = msg_send![&*content, addSubview:&*objects.control];
                Some(objects)
            } else {
                None
            };
            label(
                &content,
                "RWI receiver · harmless local controls only",
                rect(
                    24.,
                    478.,
                    if options.system_panel_controls {
                        216.
                    } else if options.modal_controls {
                        420.
                    } else {
                        612.
                    },
                    24.,
                ),
            )?;
            if options.modal_controls {
                let open = view(class(c"NSButton")?, rect(464., 474., 172., 30.));
                let title = NSString::from_str("Open own sheet");
                let _: () = msg_send![&*open, setTitle:&*title];
                let _: () = msg_send![&*open, setBezelStyle:1usize];
                let _: () = msg_send![&*open, setTarget:&*actions];
                let _: () = msg_send![&*open, setAction:sel!(sheetOpenAction:)];
                identify(&open, "rwi-fixture-modal-open", "RWI open own modal sheet");
                let _: () = msg_send![&*content, addSubview:&*open];
            }
            if options.system_panel_controls {
                for (x, title, identifier, action) in [
                    (
                        252.,
                        "Colors",
                        "rwi-system-color-open",
                        sel!(systemColorOpen:),
                    ),
                    (360., "Fonts", "rwi-system-font-open", sel!(systemFontOpen:)),
                    (468., "Files", "rwi-system-file-open", sel!(systemFileOpen:)),
                ] {
                    let control = view(class(c"NSButton")?, rect(x, 474., 98., 30.));
                    let title = NSString::from_str(title);
                    let _: () = msg_send![&*control,setTitle:&*title];
                    let _: () = msg_send![&*control,setBezelStyle:1usize];
                    let _: () = msg_send![&*control,setTarget:&*actions];
                    let _: () = msg_send![&*control,setAction:action];
                    identify(&control, identifier, &title.to_string());
                    let _: () = msg_send![&*content,addSubview:&*control];
                }
            }
            let button = view(button_class, rect(24., 430., 180., 34.));
            let button_title = NSString::from_str("Safe count: 0");
            let _: () = msg_send![&*button, setTitle:&*button_title];
            let _: () = msg_send![&*button, setBezelStyle:1usize];
            let _: () = msg_send![&*button, setTarget:&*actions];
            let _: () = msg_send![&*button, setAction:sel!(clicked:)];
            identify(&button, "rwi-fixture-safe-button", "RWI safe count button");
            let _: () = msg_send![&*content, addSubview:&*button];
            let input = view(
                class(c"NSTextField")?,
                rect(
                    230.,
                    432.,
                    if options.native_dnd_controls {
                        214.
                    } else {
                        406.
                    },
                    30.,
                ),
            );
            let _: () = msg_send![&*input, setEditable:true];
            let _: () = msg_send![&*input, setSelectable:true];
            let _: () = msg_send![&*input, setDelegate:&*actions];
            let placeholder = NSString::from_str("RWI test text · nonsecure own field");
            let _: () = msg_send![&*input, setPlaceholderString:&*placeholder];
            identify(&input, "rwi-fixture-input", "RWI fixture text input");
            let _: () = msg_send![&*content, addSubview:&*input];
            let text_client_objects = if options.text_client_trace {
                Some(super::text_client::native::Objects::new(
                    &app,
                    &window,
                    &input,
                    &options.state_dir,
                )?)
            } else {
                None
            };
            if options.native_dnd_controls {
                let stop = view(button_class, rect(464., 430., 172., 34.));
                let title = NSString::from_str("End own fixture");
                let _: () = msg_send![&*stop, setTitle:&*title];
                let _: () = msg_send![&*stop, setBezelStyle:1usize];
                let _: () = msg_send![&*stop, setTarget:&*actions];
                let _: () = msg_send![&*stop, setAction:sel!(nativeFixtureClose:)];
                identify(
                    &stop,
                    "rwi-native-dnd-close",
                    "RWI end own native drag fixture",
                );
                let _: () = msg_send![&*content, addSubview:&*stop];
            }
            label(
                &content,
                "Hover zone (native tooltip, tracking receipt below)",
                rect(24., 398., 612., 22.),
            )?;
            let hover = view(
                hover_calibration_class(receiver_class, options.hover_first_responder)?,
                rect(24., 342., 612., 50.),
            );
            identify(&hover, "rwi-fixture-hover", "RWI hover zone");
            let tooltip =
                NSString::from_str("RWI fixture hover tooltip · harmless acceptance target");
            // AppKit's dynamic string request is a separate observable callback.
            // owner is weak: the hover view is retained by UI and the own window.
            let tooltip_tag: isize = msg_send![&*hover,
                addToolTipRect:rect(0., 0., 612., 50.),
                owner:&*hover, userData:std::ptr::null_mut::<std::ffi::c_void>()];
            if !options.legacy_tracking_calibration {
                let allocated: Allocated<AnyObject> = msg_send![class(c"NSTrackingArea")?, alloc];
                // MouseEnteredAndExited | MouseMoved | ActiveAlways | InVisibleRect.
                // ActiveAlways tests receiving while inactive, not a promise that any
                // producer's process-directed mouse event will create tracking events.
                let tracking: Retained<AnyObject> = msg_send![allocated,
                initWithRect:rect(0., 0., 612., 50.), options:0x283usize,
                owner:&*hover, userInfo:std::ptr::null_mut::<AnyObject>()];
                let _: () = msg_send![&*hover, addTrackingArea:&*tracking];
            }
            state.hover.tooltip_registered = true;
            let _: () = msg_send![&*content, addSubview:&*hover];
            label(
                &content,
                if options.native_dnd_controls {
                    "Native pasteboard drag source → native drop destination"
                } else {
                    "Drag canvas (blue path requires received down → drag → up)"
                },
                rect(24., 312., 612., 22.),
            )?;
            let native_dnd_objects = if options.native_dnd_controls {
                Some(native_dnd::Objects::new()?)
            } else {
                None
            };
            let canvas = native_dnd_objects
                .as_ref()
                .map(|o| o.source.clone())
                .unwrap_or_else(|| view(receiver_class, rect(24., 100., 360., 206.)));
            identify(&canvas, "rwi-fixture-canvas", "RWI drag canvas");
            let _: () = msg_send![&*content, addSubview:&*canvas];
            let scroll = view(class(c"NSScrollView")?, rect(404., 100., 232., 206.));
            let _: () = msg_send![&*scroll, setHasVerticalScroller:true];
            let _: () = msg_send![&*scroll, setHasHorizontalScroller:false];
            let _: () = msg_send![&*scroll, setAutohidesScrollers:false];
            let _: () = msg_send![&*scroll, setBorderType:1usize];
            identify(&scroll, "rwi-fixture-scroll", "RWI native scroll viewport");
            let document = native_dnd_objects
                .as_ref()
                .map(|o| o.destination.clone())
                .unwrap_or_else(|| view(receiver_class, rect(0., 0., 210., 1000.)));
            for n in 0..if options.native_dnd_controls { 4 } else { 30 } {
                label(
                    &document,
                    &if options.native_dnd_controls {
                        format!("Native drop target {n:02}")
                    } else {
                        format!("Native scroll row {n:02}")
                    },
                    rect(12., 10. + f64::from(n) * 30., 190., 24.),
                )?;
            }
            let _: () = msg_send![&*scroll, setDocumentView:&*document];
            let _: () = msg_send![&*content, addSubview:&*scroll];
            let summary = label(&content, &state.summary(), rect(24., 20., 612., 72.))?;
            identify(&summary, "rwi-fixture-state", "RWI receiver state");
            let _: () = msg_send![&*window, setContentView:&*content];
            if options.hover_first_responder {
                // Explicit fixture setup only. Neither the backend nor a hover
                // operation changes the first responder or invokes a callback.
                let accepted: Bool = msg_send![&*window, makeFirstResponder:&*hover];
                state.responder.calibration_accepted = Some(accepted.as_bool());
            }
            let command_tab_actor = if let Some(config) = options.command_tab.as_ref() {
                command_tab::native::existing_permissions(&mut state.command_tab);
                match command_tab::native::Actor::new(config) {
                    Ok(actor) => Some(actor),
                    Err(error) => {
                        state.command_tab.error = Some(error);
                        None
                    }
                }
            } else {
                None
            };
            UI_STATE.with(|cell| {
                *cell.borrow_mut() = Some(UI {
                    state,
                    store,
                    app: app.clone(),
                    window: window.clone(),
                    cover: None,
                    modal_sheet: None,
                    modal_button: None,
                    color_panel: None,
                    font_panel: None,
                    file_panel: None,
                    file_panel_directory,
                    button,
                    hover,
                    canvas,
                    input,
                    scroll,
                    summary,
                    tooltip_text: tooltip,
                    tooltip_tag,
                    legacy_tracking_tag: None,
                    legacy_registration: None,
                    cooperative_provider: None,
                    cooperative_timer: None,
                    lifetime_timer: None,
                    environment_timer: None,
                    self_foreground_timer: None,
                    command_tab_timer: None,
                    command_tab_actor,
                    native_dnd_objects,
                    semantic_ui_objects,
                    _text_client_objects: text_client_objects,
                    _actions: actions,
                })
            });
            // Deliberately no makeKeyAndOrderFront / application activation.
            let _: () = msg_send![&*window, orderFront:std::ptr::null_mut::<AnyObject>()];
            let _: () = msg_send![&*window, setFrame:requested_frame, display:true, animate:false];
            let actual: NSRect = msg_send![&*window, frame];
            let screen: *mut AnyObject = msg_send![&*window, screen];
            let allocation_matches = if options.boundary_right || options.boundary_canvas_right {
                placed.same(geometry(actual))
            } else {
                screen_frame.contains(geometry(actual))
            };
            if !allocation_matches || screen_id(screen) != Some(options.display_id) {
                UI_STATE.with(|cell| {
                    let mut borrowed = cell.borrow_mut();
                    let ui = borrowed.as_mut().unwrap();
                    ui.state.status = "error";
                    ui.state.output_error = Some("initial_window_placement_mismatch".into());
                    ui.flush();
                });
                let _: () = msg_send![&*window, orderOut:std::ptr::null_mut::<AnyObject>()];
                UI_STATE.with(|cell| {
                    cell.borrow_mut().take();
                });
                return Err(
                    "fixture window did not match its explicit display/geometry allocation; stopped without fallback".into(),
                );
            }
            if options.legacy_tracking_calibration {
                let registered = with_detached_ui(|ui| ui.register_legacy_tracking())
                    .ok_or("own UI missing before legacy registration")?;
                if let Err(error) = registered {
                    with_detached_ui(|ui| {
                        ui.state.status = "error";
                        ui.state.output_error = Some(error.clone());
                        ui.remove_legacy_tracking();
                        ui.flush();
                    });
                    let _: () = msg_send![&*window, orderOut:std::ptr::null_mut::<AnyObject>()];
                    let released = UI_STATE.with(|cell| cell.borrow_mut().take());
                    drop(released);
                    return Err(error);
                }
            }
            if let Some(config) = &options.cooperative_owner {
                let provider = rwi_macos::cooperative_tracking::OwnerServer::start(
                    &config.directory,
                    config.agent_pid,
                    &config.agent_birth,
                )
                .map_err(|e| e.to_string())?;
                UI_STATE.with(|cell| {
                    let mut borrowed=cell.borrow_mut();let ui=borrowed.as_mut().unwrap();
                    let id=ui.legacy_registration.as_ref().unwrap().registration_id().to_owned();
                    ui.state.cooperative_tracking=Some(serde_json::json!({"provider_configured":true,"provider_closed":false,
                        "owner_endpoint":provider.endpoint(),"registration_id":id,"allowed_agent_pid":config.agent_pid,
                        "allowed_agent_birth":config.agent_birth,"Native_peer_authenticated_by_controller_JSON":false,
                        "provider_stats":provider.stats(),"goal_verified":false,"generic_modern_hover_verified":false}));
                    ui.cooperative_provider=Some(provider);ui.flush();
                });
            }
            if options.cover_controls {
                // A real second own window covers the harmless button. Ground
                // it in the same allocated NSScreen before ordering; no main
                // display fallback, makeKey, activation, input, or global hook.
                let cover_rect = rect(
                    content_frame.origin.x + 16.,
                    content_frame.origin.y + 418.,
                    205.,
                    58.,
                );
                if !screen_frame.contains(geometry(cover_rect)) {
                    let _: () = msg_send![&*window, close];
                    return Err("own cover does not fit allocated display".into());
                }
                let allocated: Allocated<AnyObject> = msg_send![window_class, alloc];
                let cover: Retained<AnyObject> = msg_send![allocated, initWithContentRect:cover_rect, styleMask:0usize, backing:2usize, defer:false];
                let _: () = msg_send![&*cover, setReleasedWhenClosed:false];
                let _: () = msg_send![&*cover, setOpaque:true];
                let title = NSString::from_str(&format!("RWI own cover · {}", options.display_id));
                let _: () = msg_send![&*cover, setTitle:&*title];
                let cover_content = view(class(c"NSView")?, rect(0., 0., 205., 58.));
                label(&cover_content, "RWI own cover", rect(12., 18., 180., 24.))?;
                let _: () = msg_send![&*cover, setContentView:&*cover_content];
                let base_number: isize = msg_send![&*window, windowNumber];
                let _: () = msg_send![&*cover, orderWindow:1isize, relativeTo:base_number];
                let frame: NSRect = msg_send![&*cover, frame];
                let cover_screen: *mut AnyObject = msg_send![&*cover, screen];
                let key: bool = msg_send![&*cover, isKeyWindow];
                if !screen_frame.contains(geometry(frame))
                    || screen_id(cover_screen) != Some(options.display_id)
                    || key
                {
                    let _: () = msg_send![&*cover, close];
                    let _: () = msg_send![&*window, close];
                    return Err("own cover placement/key readback mismatch; no fallback".into());
                }
                UI_STATE.with(|cell| {
                    cell.borrow_mut().as_mut().unwrap().cover = Some(cover);
                });
            }
            if options.interactive_sibling_window {
                // Ordinary key-capable own sibling, not the legacy borderless cover.
                // Build on the same allocated screen; never makeKey, activate,
                // send input, install an actor/global hook or fall back to main.
                let sibling_result = (|| -> Result<(Retained<AnyObject>, bool), String> {
                    const SIBLING_STYLE: usize = 1 | 2 | 4 | 8;
                    let decoration: NSRect = msg_send![class(c"NSWindow")?,
                        frameRectForContentRect:rect(0., 0., 320., 180.), styleMask:SIBLING_STYLE];
                    let sibling_frame = rect(
                        actual.origin.x + actual.size.width + 24.,
                        actual.origin.y,
                        decoration.size.width,
                        decoration.size.height,
                    );
                    if !screen_frame.contains(geometry(sibling_frame)) {
                        return Err("interactive sibling does not fit allocated display".into());
                    }
                    let sibling_content = view(class(c"NSView")?, rect(0., 0., 320., 180.));
                    label(
                        &sibling_content,
                        "RWI same-PID sibling · ready",
                        rect(16., 80., 288., 24.),
                    )?;
                    let content_rect: NSRect = msg_send![class(c"NSWindow")?,
                        contentRectForFrameRect:sibling_frame, styleMask:SIBLING_STYLE];
                    let allocated: Allocated<AnyObject> = msg_send![class(c"NSWindow")?, alloc];
                    let sibling: Retained<AnyObject> = msg_send![allocated,
                        initWithContentRect:content_rect, styleMask:SIBLING_STYLE,
                        backing:2usize, defer:false];
                    let _: () = msg_send![&*sibling, setReleasedWhenClosed:false];
                    let title = UI_STATE.with(|cell| {
                        let borrowed = cell.borrow();
                        NSString::from_str(&format!(
                            "RWI interactive sibling · {}",
                            borrowed.as_ref().unwrap().state.run_id
                        ))
                    });
                    let _: () = msg_send![&*sibling, setTitle:&*title];
                    let _: () = msg_send![&*sibling, setContentView:&*sibling_content];
                    let _: () = msg_send![&*sibling,
                        setFrame:sibling_frame, display:false, animate:false];
                    let _: () = msg_send![&*sibling, orderFront:std::ptr::null_mut::<AnyObject>()];
                    let observed: NSRect = msg_send![&*sibling, frame];
                    let sibling_screen: *mut AnyObject = msg_send![&*sibling, screen];
                    let key: bool = msg_send![&*sibling, isKeyWindow];
                    if !geometry(sibling_frame).same(geometry(observed))
                        || !screen_frame.contains(geometry(observed))
                        || screen_id(sibling_screen) != Some(options.display_id)
                        || observed.origin.x < actual.origin.x + actual.size.width + 23.99
                    {
                        let _: () = msg_send![&*sibling, close];
                        return Err(
                            "interactive sibling placement readback mismatch; no fallback".into(),
                        );
                    }
                    // Record actual initial Key, including true; it is not proof of
                    // background startup, continuous identity or absence of effects.
                    Ok((sibling, key))
                })();
                match sibling_result {
                    Ok((sibling, key)) => UI_STATE.with(|cell| {
                        let mut borrowed = cell.borrow_mut();
                        let ui = borrowed.as_mut().unwrap();
                        ui.cover = Some(sibling);
                        ui.state.interactive_sibling_window_key = Some(key);
                    }),
                    Err(error) => {
                        let _: () = msg_send![&*window, close];
                        return Err(error);
                    }
                }
            }
            UI_STATE.with(|cell| {
                let mut borrowed = cell.borrow_mut();
                let ui = borrowed.as_mut().unwrap();
                ui.state.window_number = Some(msg_send![&*window, windowNumber]);
                ui.state.status = "running";
                ui.flush();
            });
            // A nonrepeating timer is scoped to this validated own window/run.
            // No timer is made on help/no arguments or failed initial placement.
            let timer_class = class(c"NSTimer")?;
            let loop_class = class(c"NSRunLoop")?;
            let timer: Retained<AnyObject> = UI_STATE.with(|cell| {
                let borrowed = cell.borrow();
                let ui = borrowed.as_ref().unwrap();
                msg_send![timer_class,
                    timerWithTimeInterval:f64::from(options.lifetime_ms) / 1000.,
                    target:&*ui._actions, selector:sel!(lifetimeExpired:),
                    userInfo:std::ptr::null_mut::<AnyObject>(), repeats:false]
            });
            let main_loop: *mut AnyObject = msg_send![loop_class, mainRunLoop];
            let _: () = msg_send![main_loop, addTimer:&*timer, forMode:NSRunLoopCommonModes];
            UI_STATE.with(|cell| {
                cell.borrow_mut().as_mut().unwrap().lifetime_timer = Some(timer);
            });
            if options.cooperative_owner.is_some() {
                let timer:Retained<AnyObject>=UI_STATE.with(|cell| {
                    let borrowed=cell.borrow();let ui=borrowed.as_ref().unwrap();
                    msg_send![timer_class,timerWithTimeInterval:0.025f64,target:&*ui._actions,selector:sel!(cooperativePolled:),userInfo:std::ptr::null_mut::<AnyObject>(),repeats:true]
                });
                let _: () = msg_send![main_loop,addTimer:&*timer,forMode:NSRunLoopCommonModes];
                UI_STATE.with(|cell| {
                    cell.borrow_mut().as_mut().unwrap().cooperative_timer = Some(timer)
                });
            }
            if let Some(ms) = options.environment_sample_ms {
                let timer: Retained<AnyObject> = UI_STATE.with(|cell| {
                    let borrowed = cell.borrow();
                    let ui = borrowed.as_ref().unwrap();
                    msg_send![timer_class,
                        timerWithTimeInterval:f64::from(ms) / 1000.,
                        target:&*ui._actions, selector:sel!(environmentSampled:),
                        userInfo:std::ptr::null_mut::<AnyObject>(), repeats:true]
                });
                let _: () = msg_send![main_loop, addTimer:&*timer, forMode:NSRunLoopCommonModes];
                UI_STATE.with(|cell| {
                    cell.borrow_mut().as_mut().unwrap().environment_timer = Some(timer);
                });
            }
            if let Some(ms) = options.self_foreground_after_ms {
                let timer: Retained<AnyObject> = UI_STATE.with(|cell| {
                    let mut borrowed = cell.borrow_mut();
                    let ui = borrowed.as_mut().unwrap();
                    ui.state.self_foreground_after_ms = Some(ms);
                    ui.state.self_foreground_method = Some(if options.legacy_self_foreground {
                        "explicit_legacy_activate_ignoring_other_apps"
                    } else {
                        "public_modern_activate"
                    });
                    ui.flush();
                    msg_send![timer_class, timerWithTimeInterval:f64::from(ms) / 1000.,
                        target:&*ui._actions, selector:sel!(ownForegroundRequested:),
                        userInfo:std::ptr::null_mut::<AnyObject>(), repeats:false]
                });
                let _: () = msg_send![main_loop, addTimer:&*timer, forMode:NSRunLoopCommonModes];
                UI_STATE.with(|cell| {
                    cell.borrow_mut().as_mut().unwrap().self_foreground_timer = Some(timer);
                });
            }
            super::ax_delay::configure(
                options.ax_windows_delay_ms,
                options.ax_focused_window_once_delay_ms,
            );
            let _: () = msg_send![&*app, run];
            let result: Result<(), String> = with_detached_ui(|ui| {
                ui.invalidate_lifetime_timer();
                ui.close_own_modal();
                ui.close_own_cover();
                ui.close_system_panels();
                ui.remove_legacy_tracking();
                if ui.state.status != "closed" {
                    ui.state.status = "error";
                    if ui.state.output_error.is_none() {
                        ui.state.output_error =
                            Some("run_loop_returned_without_window_closed".into());
                    }
                    ui.flush();
                }
                let _: () = msg_send![&*ui.window, orderOut:std::ptr::null_mut::<AnyObject>()];
                if CALLBACK_FAILURES.with(Cell::get) != 0 {
                    ui.state
                        .output_error
                        .get_or_insert_with(|| "callback_panicked".into());
                    ui.flush();
                }
                if ui.state.status != "closed" || ui.state.output_error.is_some() {
                    return Err("own window closed state was not verified without error".into());
                }
                Ok(())
            })
            .unwrap_or(Ok(()));
            // Drop retained AppKit objects with no temporary RefCell borrow.
            let released = UI_STATE.with(|cell| cell.borrow_mut().take());
            drop(released);
            result?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod detached_teardown_tests {
    use super::*;

    #[test]
    fn reentrant_teardown_reads_an_empty_slot_without_a_live_refcell_borrow() {
        let slot = RefCell::new(Some(7u8));
        let result = with_detached_slot(&slot, |value| {
            assert!(slot.borrow().is_none());
            assert!(with_detached_slot(&slot, |_| ()).is_none());
            *value = 9;
            11
        });
        assert_eq!(result, Some(11));
        assert_eq!(*slot.borrow(), Some(9));
    }

    #[test]
    fn detached_slot_restores_owned_state_even_when_work_unwinds() {
        let slot = RefCell::new(Some(3u8));
        let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
            with_detached_slot(&slot, |value| {
                *value = 4;
                panic!("synthetic teardown failure");
            });
        }));
        assert!(result.is_err());
        // An interrupted callback is not a rollback of prior state changes.
        assert_eq!(*slot.borrow(), Some(4));
    }

    #[test]
    fn callback_failure_is_recorded_without_an_installed_ui() {
        assert!(UI_STATE.with(|cell| cell.borrow().is_none()));
        let before = CALLBACK_FAILURES.with(Cell::get);
        callback(|| panic!("synthetic callback without UI"));
        assert_eq!(CALLBACK_FAILURES.with(Cell::get), before.saturating_add(1));
        CALLBACK_FAILURES.with(|count| count.set(before));
    }
}
