//! Main-thread AppKit UI. No native input events or application activation.
use super::*;
mod accessibility_button;
mod panel;
use objc2::{
    msg_send,
    rc::{Allocated, Retained, autoreleasepool},
    runtime::{AnyClass, AnyObject, ClassBuilder, Sel},
    sel,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use panel::{button_symbol, preview_control_icon};
use std::collections::BTreeMap;

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    static NSAppearanceNameAqua: *mut AnyObject;
}

const CHROME_HEIGHT: f64 = 48.;

#[derive(Default)]
pub(crate) struct Presenter {
    panels: BTreeMap<u64, Panel>,
    actions: Option<Retained<AnyObject>>,
}
struct Panel {
    window: Retained<AnyObject>,
    image: Retained<AnyObject>,
    toggle: Retained<AnyObject>,
    placeholder: Retained<AnyObject>,
    source_label: Retained<AnyObject>,
    source_pixels: Option<[u64; 2]>,
    chrome: Option<(u64, Mode, bool, String)>,
    generation: u64,
    placement_pending_since: Instant,
    last_placement_check: Option<Instant>,
    placement_confirmed: bool,
    placement_hidden: bool,
    placement_failure: Option<Value>,
}
/// A hide/reshow edge gets one new appearance budget even if its previous
/// appearance never reached WindowServer confirmation. Repeated pending ticks
/// must not keep extending that budget.
fn restart_placement_after_hidden(
    hidden: &mut bool,
    pending_since: &mut Instant,
    last_check: &mut Option<Instant>,
    confirmed: &mut bool,
    now: Instant,
) {
    if std::mem::take(hidden) {
        *pending_since = now;
        *last_check = None;
        *confirmed = false;
    }
}
fn placement_stop_reason(error: &str) -> &'static str {
    match error {
        "live_presenter_wrong_display" => "presenter_wrong_display",
        "live_presenter_placement_unconfirmed" => "presenter_placement_unconfirmed",
        "live_preview_display_changed" => "preview_display_changed",
        _ => "presenter_error",
    }
}
fn class(name: &'static std::ffi::CStr) -> std::result::Result<&'static AnyClass, String> {
    AnyClass::get(name)
        .ok_or_else(|| format!("AppKit class {} unavailable", name.to_string_lossy()))
}
extern "C" fn toggle(_this: &AnyObject, _cmd: Sel, sender: &AnyObject) {
    control(sender, false);
}
extern "C" fn stop(_this: &AnyObject, _cmd: Sel, sender: &AnyObject) {
    control(sender, true);
}
fn control(sender: &AnyObject, stop: bool) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let id: isize = unsafe { msg_send![sender, tag] };
        let shared = sessions()
            .lock()
            .ok()
            .and_then(|r| r.get(&(id as u64)).and_then(Weak::upgrade));
        if let Some(shared) = shared {
            if let Ok(mut state) = shared.state.lock() {
                let action = if stop {
                    LiveAction::Stop
                } else if state.mode == Mode::Paused {
                    LiveAction::Resume
                } else {
                    LiveAction::Hide
                };
                state.control(action);
            }
            shared.wake.notify_all();
        }
    }));
}
fn action_target() -> std::result::Result<Retained<AnyObject>, String> {
    let action_class = if let Some(class) = AnyClass::get(c"RWILivePanelActions") {
        class
    } else {
        let mut builder = ClassBuilder::new(c"RWILivePanelActions", class(c"NSObject")?)
            .ok_or("Cannot register Live actions")?;
        unsafe {
            builder.add_method(sel!(toggle:), toggle as extern "C" fn(_, _, _));
            builder.add_method(sel!(stop:), stop as extern "C" fn(_, _, _));
        }
        builder.register()
    };
    Ok(unsafe { msg_send![action_class, new] })
}
unsafe fn view(
    name: &'static std::ffi::CStr,
    frame: NSRect,
) -> std::result::Result<Retained<AnyObject>, String> {
    let allocated: Allocated<AnyObject> = unsafe { msg_send![class(name)?, alloc] };
    Ok(unsafe { msg_send![allocated, initWithFrame: frame] })
}
fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

/// On Retina, roughly two captured pixels fill each panel point, up to a compact
/// 320-point overview. Capture detail must not enlarge the panel indefinitely.
/// Display margins and compact chrome remain bounded. Match the source aspect
/// ratio instead of surrounding wide windows with a dark 16:9 letterbox.
fn panel_size(
    max_edge: u32,
    display_width: f64,
    display_height: f64,
    source_pixels: Option<[u64; 2]>,
) -> (f64, f64) {
    let aspect = source_pixels
        .filter(|[w, h]| *w > 0 && *h > 0)
        .map(|[w, h]| w as f64 / h as f64)
        .unwrap_or(2.);
    let max_height = (display_height - 96.).min(360.);
    let mut width = (f64::from(max_edge) / 2. + 16.)
        .clamp(200., 320.)
        .min(display_width - 48.);
    if (width - 16.) / aspect + CHROME_HEIGHT > max_height {
        width = ((max_height - CHROME_HEIGHT) * aspect + 16.)
            .max(200.)
            .min(display_width - 48.);
    }
    let height = ((width - 16.) / aspect + CHROME_HEIGHT).min(max_height);
    (width, height)
}
fn source_pixels(state: &State) -> Option<[u64; 2]> {
    let size = state.metadata["image_size"].as_array()?;
    let [w, h] = size.as_slice() else {
        return None;
    };
    Some([w.as_u64()?, h.as_u64()?]).filter(|[w, h]| *w > 0 && *h > 0)
}

/// Bounded metadata from the exact owned panel, retained before removing it.
/// A missing containing display is explicit; do not infer a move's cause.
fn placement_failure_evidence(
    window: u32,
    bounds: crate::geometry::Rect,
    requested: &Display,
    containing: Option<&Display>,
) -> Value {
    json!({
        "readback":"exact_owned_CGWindow_geometry",
        "pid":std::process::id(), "window":window,
        "bounds":[bounds.x,bounds.y,bounds.w,bounds.h],
        "requested_display":crate::catalog::DisplayRecord::from_native(requested),
        "containing_display":containing.map(crate::catalog::DisplayRecord::from_native),
        "cause_attested":false,"input_authority":false,
    })
}
impl Drop for Panel {
    fn drop(&mut self) {
        self.clear();
        unsafe {
            let _: () = msg_send![&*self.window,orderOut:std::ptr::null_mut::<AnyObject>()];
        }
    }
}
impl Presenter {
    pub(crate) fn tick(&mut self) {
        autoreleasepool(|_| {
            if !self.panels.is_empty() {
                pump_events();
            }
            let active = sessions()
                .lock()
                .map(|mut r| {
                    // Drop dead contexts even if their final worker ended between UI ticks.
                    r.retain(|_, weak| weak.strong_count() > 0);
                    r.iter()
                        .filter_map(|(id, w)| w.upgrade().map(|s| (*id, s)))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            for (id, shared) in active {
                let mut state = match shared.state.lock() {
                    Ok(state) => state,
                    Err(poisoned) => {
                        let mut state = poisoned.into_inner();
                        state.stop("state_poisoned");
                        self.panels.remove(&id);
                        state.visible = false;
                        state.image_present = false;
                        continue;
                    }
                };
                state.expire();
                if state.mode == Mode::Stopped {
                    self.panels.remove(&id);
                    state.visible = false;
                    state.image_present = false;
                    continue;
                }
                if state.mode == Mode::Hidden || !state.presentation_allowed() {
                    if let Some(panel) = self.panels.get_mut(&id) {
                        panel.placement_hidden = true;
                        panel.clear();
                        if state.visible {
                            unsafe {
                                let _: () = msg_send![&*panel.window, orderOut:std::ptr::null_mut::<AnyObject>()];
                            }
                        }
                    }
                    state.visible = false;
                    state.image_present = false;
                    continue;
                }
                let pixels = source_pixels(&state);
                if self
                    .panels
                    .get(&id)
                    .is_some_and(|panel| pixels.is_some() && panel.source_pixels != pixels)
                {
                    self.panels.remove(&id);
                    state.visible = false;
                    state.image_present = false;
                }
                if state.mode == Mode::Running && pixels.is_none() && !self.panels.contains_key(&id)
                {
                    continue;
                }
                if self.actions.is_none() {
                    match action_target() {
                        Ok(target) => self.actions = Some(target),
                        Err(error) => {
                            state.last_error = Some(error);
                            state.stop("presenter_error");
                            continue;
                        }
                    }
                }
                if let std::collections::btree_map::Entry::Vacant(entry) = self.panels.entry(id) {
                    match Panel::new(id, &state, self.actions.as_deref().unwrap()) {
                        Ok(panel) => {
                            entry.insert(panel);
                        }
                        Err(error) => {
                            state.last_error = Some(error);
                            state.stop("presenter_error");
                            continue;
                        }
                    }
                }
                let panel = self.panels.get_mut(&id).unwrap();
                if !state.visible {
                    restart_placement_after_hidden(
                        &mut panel.placement_hidden,
                        &mut panel.placement_pending_since,
                        &mut panel.last_placement_check,
                        &mut panel.placement_confirmed,
                        Instant::now(),
                    );
                    unsafe {
                        let _: () = msg_send![&*panel.window, orderFrontRegardless];
                    }
                }
                match panel.check_placement(&state.display, !state.visible) {
                    Ok(true) => {}
                    Ok(false) => {
                        state.visible = unsafe { msg_send![&*panel.window, isVisible] };
                        state.image_present = false;
                        continue;
                    }
                    Err(error) => {
                        panel.clear();
                        state.presenter_failure = panel.placement_failure.clone();
                        let reason = placement_stop_reason(&error);
                        state.last_error = Some(error);
                        state.stop(reason);
                        self.panels.remove(&id);
                        state.visible = false;
                        state.image_present = false;
                        state.window_id = None;
                        continue;
                    }
                }
                if panel.generation != state.generation
                    || matches!(state.mode, Mode::Paused | Mode::Recovering)
                {
                    panel.clear();
                    state.image_present = false;
                    panel.generation = state.generation;
                }
                if state.mode == Mode::Running
                    && let Some(frame) = state.latest.take()
                {
                    match panel.present(&frame.png) {
                        Ok(()) => {
                            state.presented += 1;
                            state.image_present = true;
                        }
                        Err(error) => {
                            state.last_error = Some(error);
                            state.mode = Mode::Paused;
                            state.clear();
                            panel.clear();
                            state.image_present = false;
                        }
                    }
                }
                state.visible = unsafe { msg_send![&*panel.window, isVisible] };
                let source_title = state.metadata["title"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .unwrap_or("当前窗口");
                let source_screen = state.metadata["display"]["id"]
                    .as_u64()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".into());
                let caption = format!(
                    "屏幕 {} · {}",
                    source_screen,
                    source_title.chars().take(48).collect::<String>()
                );
                let chrome = (state.generation, state.mode, state.image_present, caption);
                if panel.chrome.as_ref() != Some(&chrome) {
                    let message = NSString::from_str(if state.mode == Mode::Recovering {
                        "来源暂不可用 · 正在有界重新观察"
                    } else if state.last_error.is_some() {
                        "来源暂不可用 · 请重新选择窗口"
                    } else if state.mode == Mode::Paused {
                        "预览已暂停 · 画面已清空"
                    } else {
                        "正在读取已选择的窗口…"
                    });
                    let caption = NSString::from_str(&chrome.3);
                    unsafe {
                        let title = if state.mode == Mode::Paused {
                            "继续预览"
                        } else {
                            "隐藏预览"
                        };
                        let symbol = preview_control_icon(title);
                        if let Err(error) = button_symbol(&panel.toggle, symbol, title) {
                            state.last_error = Some(error);
                            state.stop("presenter_error");
                            continue;
                        }
                        let _: () = msg_send![&*panel.source_label,setStringValue:&*caption];
                        let _: () = msg_send![&*panel.source_label,setToolTip:&*caption];
                        let _: () = msg_send![&*panel.placeholder,setStringValue:&*message];
                        let _: () = msg_send![&*panel.placeholder,setHidden:state.image_present];
                    }
                    panel.chrome = Some(chrome);
                }
                let number: isize = unsafe { msg_send![&*panel.window, windowNumber] };
                state.window_id = u32::try_from(number).ok();
            }
            // Closed contexts can disappear before the UI observes their stop.
            let ids = sessions()
                .lock()
                .map(|r| r.keys().copied().collect::<Vec<_>>())
                .unwrap_or_default();
            self.panels.retain(|id, _| ids.contains(id));
            if !self.panels.is_empty()
                && let Some(class) = AnyClass::get(c"CATransaction")
            {
                unsafe {
                    let _: () = msg_send![class, flush];
                }
            }
        });
    }
}
fn pump_events() {
    unsafe {
        let Some(app_class) = AnyClass::get(c"NSApplication") else {
            return;
        };
        let Some(date_class) = AnyClass::get(c"NSDate") else {
            return;
        };
        let app: *mut AnyObject = msg_send![app_class, sharedApplication];
        let date: *mut AnyObject = msg_send![date_class, distantPast];
        let mode = NSString::from_str("kCFRunLoopDefaultMode");
        for _ in 0..16 {
            let event: *mut AnyObject = msg_send![app,nextEventMatchingMask:usize::MAX,untilDate:date,inMode:&*mode,dequeue:true];
            if event.is_null() {
                break;
            }
            let _: () = msg_send![app,sendEvent:event];
        }
    }
}

#[cfg(test)]
mod panel_size_tests {
    use super::{
        panel_size, placement_failure_evidence, placement_stop_reason,
        restart_placement_after_hidden,
    };
    use std::time::{Duration, Instant};
    #[test]
    fn unconfirmed_appearance_gets_one_fresh_budget_after_real_hide() {
        let now = Instant::now();
        let mut hidden = true;
        let mut since = now - Duration::from_secs(10);
        let mut last = Some(since);
        let mut confirmed = false;
        restart_placement_after_hidden(&mut hidden, &mut since, &mut last, &mut confirmed, now);
        assert_eq!(since, now);
        assert!(last.is_none());
        assert!(!hidden && !confirmed);
        restart_placement_after_hidden(
            &mut hidden,
            &mut since,
            &mut last,
            &mut confirmed,
            now + Duration::from_secs(1),
        );
        assert_eq!(
            since, now,
            "pending ticks cannot extend the appearance budget"
        );
    }
    #[test]
    fn reshow_invalidates_old_placement_confirmation_and_cached_check() {
        let now = Instant::now();
        let mut hidden = true;
        let mut since = now - Duration::from_secs(10);
        let mut last = Some(since);
        let mut confirmed = true;
        restart_placement_after_hidden(&mut hidden, &mut since, &mut last, &mut confirmed, now);
        assert_eq!(since, now);
        assert!(last.is_none());
        assert!(!confirmed);
    }
    #[test]
    fn missing_panel_is_not_a_wrong_display_observation() {
        assert_eq!(
            placement_stop_reason("live_presenter_placement_unconfirmed"),
            "presenter_placement_unconfirmed"
        );
        assert_eq!(
            placement_stop_reason("live_presenter_wrong_display"),
            "presenter_wrong_display"
        );
        assert_eq!(
            placement_stop_reason("live_preview_display_changed"),
            "preview_display_changed"
        );
        assert_eq!(
            placement_stop_reason("live_presenter_readback_unknown"),
            "presenter_error"
        );
    }
    #[test]
    fn placement_failure_keeps_exact_geometry_and_unknown_cause() {
        let requested = crate::native::Display {
            id: 1,
            uuid: Some("fixture-built-in".into()),
            built_in: true,
            rect: crate::geometry::Rect {
                x: 0.,
                y: 0.,
                w: 1800.,
                h: 1169.,
            },
            pixels: [1800, 1169],
            rotation: 0.,
        };
        let bounds = crate::geometry::Rect {
            x: 1824.,
            y: 504.,
            w: 640.,
            h: 577.,
        };
        let evidence = placement_failure_evidence(99, bounds, &requested, None);
        assert_eq!(evidence["window"], 99);
        assert_eq!(
            evidence["bounds"],
            serde_json::json!([1824., 504., 640., 577.])
        );
        assert_eq!(evidence["requested_display"]["uuid"], "fixture-built-in");
        assert!(evidence["containing_display"].is_null());
        assert_eq!(evidence["cause_attested"], false);
        assert_eq!(evidence["input_authority"], false);
    }
    #[test]
    fn preview_follows_capture_aspect_without_leaving_allocated_display() {
        let wide = panel_size(2048, 1920., 1080., Some([2048, 942]));
        assert_eq!(wide.0, 320.);
        assert!((wide.1 - 187.828125).abs() < 0.000001);
        assert_eq!(panel_size(2048, 1920., 1080., None), (320., 200.));
        assert_eq!(panel_size(1024, 1920., 1080., None), (320., 200.));
        assert_eq!(panel_size(640, 1920., 1080., None), (320., 200.));
        assert_eq!(panel_size(2048, 300., 300., None), (252., 166.));
        assert_eq!(panel_size(2048, 500., 350., None), (320., 200.));
    }
}

#[cfg(test)]
mod compact_preview_acceptance {
    use super::panel_size;
    #[test]
    fn detailed_capture_keeps_native_preview_compact() {
        let (width, height) = panel_size(2048, 1800., 1169., Some([1320, 1104]));
        assert!(
            width <= 320.,
            "high-detail capture must not occupy most of the screen: {width}"
        );
        assert!(
            height <= 360.,
            "source-aspect overview must remain compact: {height}"
        );
        let wide = panel_size(2048, 1920., 1080., Some([2048, 942]));
        assert!(wide.0 <= 320. && wide.1 <= 200.);
    }
    #[test]
    fn portrait_preview_shrinks_without_extra_letterbox() {
        let (width, height) = panel_size(2048, 1800., 1169., Some([252, 420]));
        assert!(width <= 320. && height <= 360.);
        assert!(((width - 16.) / (height - super::CHROME_HEIGHT) - 0.6).abs() < 0.000001);
    }
    #[test]
    fn preview_buttons_distinguish_hide_resume_and_close_without_minimize() {
        assert_eq!(super::preview_control_icon("隐藏预览"), "eye.slash");
        assert_eq!(super::preview_control_icon("继续预览"), "play.fill");
        assert_eq!(super::preview_control_icon("关闭预览"), "xmark");
    }
}
