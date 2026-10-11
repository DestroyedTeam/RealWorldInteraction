//! AI pointer overlay: the shared A1 lens rendered by the native presenter.
//! Purely visual: it never dispatches input, never moves the real cursor and
//! never takes focus.
//!
//! AppKit work is confined to the process main thread, which pumps commands and
//! animation frames; every other thread only sends bounded commands.
use core_foundation::runloop::{CFRunLoop, kCFRunLoopDefaultMode};
use core_graphics::{
    color_space::CGColorSpace,
    context::CGContext,
    image::{CGImageAlphaInfo, CGImageByteOrderInfo},
};
use foreign_types::ForeignType;
use objc2::{
    msg_send,
    rc::autoreleasepool,
    runtime::{AnyClass, AnyObject, Bool},
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use serde_json::json;
use std::{
    sync::{
        Mutex, OnceLock,
        mpsc::{Receiver, Sender, SyncSender, TryRecvError, channel, sync_channel},
    },
    time::{Duration, Instant},
};

const SIZE_PT: f64 = rwi_pointer::SIZE as f64;
const HOTSPOT_PT: f64 = rwi_pointer::CENTER as f64;
const IDLE: Duration = Duration::from_secs(45);
const TICK: Duration = Duration::from_millis(16);

enum Action {
    Show([f64; 2]),
    Hide,
}

/// Commands carry a one-shot acknowledgement so receipts describe the state
/// after the overlay actually applied them.
struct Command {
    action: Action,
    ack: SyncSender<()>,
}

#[derive(Default)]
struct Status {
    created: bool,
    visible: bool,
    point: Option<[f64; 2]>,
    applied_point: Option<[f64; 2]>,
    window_id: Option<u32>,
    display_id: Option<u32>,
    dpi: Option<u32>,
    last_error: Option<String>,
}

static SENDER: OnceLock<Sender<Command>> = OnceLock::new();
static RECEIVER: OnceLock<Mutex<Option<Receiver<Command>>>> = OnceLock::new();
static STATUS: OnceLock<Mutex<Status>> = OnceLock::new();

fn status() -> &'static Mutex<Status> {
    STATUS.get_or_init(|| Mutex::new(Status::default()))
}

fn set_status(update: impl FnOnce(&mut Status)) {
    if let Ok(mut guard) = status().lock() {
        update(&mut guard);
    }
}

/// Main-thread installation: initialize AppKit once and prepare the command
/// channel. Returns false when a channel is already installed.
pub fn install() -> bool {
    let (sender, receiver) = channel();
    if SENDER.set(sender).is_err() {
        return false;
    }
    let _ = RECEIVER.set(Mutex::new(Some(receiver)));
    unsafe {
        let Some(class) = AnyClass::get(c"NSApplication") else {
            set_status(|s| s.last_error = Some("NSApplication unavailable".into()));
            return false;
        };
        let app: *mut AnyObject = msg_send![class, sharedApplication];
        if app.is_null() {
            set_status(|s| s.last_error = Some("sharedApplication returned null".into()));
            return false;
        }
        // We use a custom event pump rather than NSApplication.run, so complete
        // AppKit launch explicitly. Prohibit activation until initialization
        // finishes; no windows exist yet. Then allow nonactivating panels.
        let prohibited: Bool = msg_send![app, setActivationPolicy: 2isize];
        if !prohibited.as_bool() {
            set_status(|s| s.last_error = Some("cannot prohibit startup activation".into()));
            return false;
        }
        let _: () = msg_send![app, finishLaunching];
        let accessory: Bool = msg_send![app, setActivationPolicy: 1isize];
        if !accessory.as_bool() {
            set_status(|s| s.last_error = Some("accessory presentation unavailable".into()));
            return false;
        }
    }
    crate::live::install();
    true
}

fn dispatch(action: Action) -> bool {
    let Some(sender) = SENDER.get() else {
        return false;
    };
    let (ack, wait) = sync_channel(1);
    if sender.send(Command { action, ack }).is_err() {
        return false;
    }
    // Bounded wait: the overlay pump runs on the main thread and always acks,
    // even when window creation failed.
    wait.recv_timeout(Duration::from_secs(2)).is_ok()
}

/// Any thread: show the marker at a desktop point (global, top-left origin).
/// Blocks until the overlay applied the command (bounded).
pub fn show_at(point: [f64; 2]) -> bool {
    dispatch(Action::Show(point))
}

/// Any thread: hide the marker. Blocks until the overlay applied the command.
pub fn hide() -> bool {
    dispatch(Action::Hide)
}

/// The most recent shown point, if any.
pub fn last_point() -> Option<[f64; 2]> {
    status().lock().ok().and_then(|s| s.point)
}

pub fn status_json() -> serde_json::Value {
    let snapshot = status().lock().ok();
    let (created, visible, point, applied_point, window_id, display_id, dpi, last_error) = snapshot
        .as_ref()
        .map(|s| {
            (
                s.created,
                s.visible,
                s.point,
                s.applied_point,
                s.window_id,
                s.display_id,
                s.dpi,
                s.last_error.clone(),
            )
        })
        .unwrap_or((false, false, None, None, None, None, None, None));
    json!({
        "implemented": true,
        "installed": SENDER.get().is_some(),
        "created": created,
        "visible": visible,
        "point": point,
        "applied_point": applied_point,
        "window_id": window_id,
        "display_id": display_id,
        "dpi": dpi,
        "material": "a1_diamond_rcu",
        "last_error": last_error,
    })
}

fn sprite_rect() -> NSRect {
    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(SIZE_PT, SIZE_PT))
}

/// Effective dpi from the window's actual screen backing scale. Read after the
/// window is ordered onto the target display so Retina rasters stay 1:1.
unsafe fn window_dpi(window: *mut AnyObject) -> u32 {
    let scale: f64 = unsafe { msg_send![window, backingScaleFactor] };
    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    (96.0 * scale)
        .round()
        .clamp(96.0, f64::from(rwi_pointer::MAX_DPI)) as u32
}

/// Convert a global top-left desktop point into a Cocoa frame origin so the
/// fixed lens hotspot lands exactly on the point.
fn cocoa_origin(point: [f64; 2]) -> Option<[f64; 2]> {
    let primary = core_graphics::display::CGDisplay::main().bounds();
    cocoa_origin_with_height(point, primary.size.height)
}

fn cocoa_origin_with_height(point: [f64; 2], height: f64) -> Option<[f64; 2]> {
    if !height.is_finite() || height <= 0.0 || !point.iter().all(|v| v.is_finite()) {
        return None;
    }
    Some([point[0] - HOTSPOT_PT, height - point[1] - HOTSPOT_PT])
}

/// Rasterize one material frame into the layer contents. The lens hotspot is
/// the raster center; the RWI label sits below it inside the same raster.
unsafe fn present(
    layer: *mut AnyObject,
    dpi: u32,
    frame: rwi_pointer::motion::Frame,
) -> std::result::Result<(), String> {
    unsafe {
        let material = rwi_pointer::material::Material::new(
            dpi,
            rwi_pointer::material::Style::default(),
            rwi_pointer::material::Label::fallback(dpi),
        );
        let scale = rwi_pointer::scale_for_dpi(dpi);
        let raster = rwi_pointer::raster_size(dpi);
        let label = rwi_pointer::material::Label::fallback(dpi);
        let label_offset = [
            HOTSPOT_PT * scale - label.width as f64 / 2.0,
            HOTSPOT_PT * scale + 21.0 * scale + label.padding * scale,
        ];
        let bgra = material.raster(frame, label_offset);
        if bgra.len() != raster * raster * 4 {
            return Err("material raster size mismatch".into());
        }
        let color = CGColorSpace::create_device_rgb();
        let mut context = CGContext::create_bitmap_context(
            None,
            raster,
            raster,
            8,
            raster * 4,
            &color,
            CGImageAlphaInfo::CGImageAlphaPremultipliedFirst as u32
                | CGImageByteOrderInfo::CGImageByteOrder32Little as u32,
        );
        std::ptr::copy_nonoverlapping(bgra.as_ptr(), context.data().as_mut_ptr(), bgra.len());
        let image = context
            .create_image()
            .ok_or("CGImage unavailable for the material raster")?;
        let contents = image.as_ptr() as *mut AnyObject;
        let _: () = msg_send![layer, setContentsScale: f64::from(dpi) / 96.0];
        let _: () = msg_send![layer, setContents: contents];
        Ok(())
    }
}

unsafe fn create_window(dpi: u32) -> std::result::Result<(*mut AnyObject, *mut AnyObject), String> {
    unsafe {
        let window_class = AnyClass::get(c"NSWindow").ok_or("NSWindow unavailable")?;
        let view_class = AnyClass::get(c"NSView").ok_or("NSView unavailable")?;
        let color_class = AnyClass::get(c"NSColor").ok_or("NSColor unavailable")?;
        let rect = sprite_rect();
        let allocated: *mut AnyObject = msg_send![window_class, alloc];
        let window: *mut AnyObject = msg_send![
            allocated,
            initWithContentRect: rect,
            styleMask: 0usize,
            backing: 2usize,
            defer: false
        ];
        if window.is_null() {
            return Err("NSWindow init returned null".into());
        }
        let title = NSString::from_str("Real World Interaction AI Cursor");
        let _: () = msg_send![window, setTitle: &*title];
        let _: () = msg_send![window, setOpaque: false];
        let _: () = msg_send![window, setHasShadow: false];
        let _: () = msg_send![window, setIgnoresMouseEvents: true];
        let _: () = msg_send![window, setLevel: 1000i64];
        let _: () = msg_send![window, setCollectionBehavior: 1usize | 16 | 256];
        let clear: *mut AnyObject = msg_send![color_class, clearColor];
        let _: () = msg_send![window, setBackgroundColor: clear];
        let view_allocated: *mut AnyObject = msg_send![view_class, alloc];
        let view: *mut AnyObject = msg_send![view_allocated, initWithFrame: rect];
        if view.is_null() {
            return Err("NSView init returned null".into());
        }
        let _: () = msg_send![view, setWantsLayer: true];
        let layer: *mut AnyObject = msg_send![view, layer];
        if layer.is_null() {
            return Err("content view has no layer".into());
        }
        present(layer, dpi, rwi_pointer::motion::Frame::idle())?;
        let _: () = msg_send![window, setContentView: view];
        Ok((window, layer))
    }
}

/// Main thread: create/show/hide the marker and animate its highlight until the
/// process ends. Never returns; call this after `install()`.
pub fn pump_forever() -> ! {
    let receiver = RECEIVER
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|mut guard| guard.take()));
    let Some(receiver) = receiver else {
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    };
    let start = Instant::now();
    let mut window: Option<*mut AnyObject> = None;
    let mut layer: Option<*mut AnyObject> = None;
    let mut dpi = 192u32;
    let mut visible = false;
    let mut last_show = Instant::now();
    let mut motion = rwi_pointer::motion::Motion::default();
    let mut animating_until: Option<Instant> = None;
    let mut live = crate::live::Presenter::default();
    loop {
        loop {
            match receiver.try_recv() {
                Ok(Command {
                    action: Action::Show(point),
                    ack,
                }) => {
                    autoreleasepool(|_| {
                        if window.is_none() {
                            match unsafe { create_window(dpi) } {
                                Ok((created, view_layer)) => {
                                    window = Some(created);
                                    layer = Some(view_layer);
                                    set_status(|s| s.created = true);
                                }
                                Err(error) => {
                                    set_status(|s| s.last_error = Some(error));
                                    let _ = ack.send(());
                                    return;
                                }
                            }
                        }
                        let (Some(created), Some(view_layer)) = (window, layer) else {
                            let _ = ack.send(());
                            return;
                        };
                        let Some(origin) = cocoa_origin(point) else {
                            set_status(|s| s.last_error = Some("primary display unknown".into()));
                            let _ = ack.send(());
                            return;
                        };
                        // Order onto the target display first, then read its real
                        // backing scale so the Retina raster stays 1:1.
                        unsafe {
                            let frame = NSRect::new(
                                NSPoint::new(origin[0], origin[1]),
                                NSSize::new(SIZE_PT, SIZE_PT),
                            );
                            let _: () =
                                msg_send![created, setFrame: frame, display: true, animate: false];
                            let _: () = msg_send![created, orderFrontRegardless];
                            // This presenter pumps CFRunLoop, not NSApplication.run.
                            // Commit pending layer/window transactions explicitly.
                            if let Some(transaction) = AnyClass::get(c"CATransaction") {
                                let _: () = msg_send![transaction, flush];
                            }
                        }
                        let wanted = unsafe { window_dpi(created) };
                        if wanted != dpi {
                            dpi = wanted;
                        }
                        let now = start.elapsed().as_secs_f64();
                        motion.sample(point, dpi, false, now);
                        motion.click(now);
                        match unsafe { present(view_layer, dpi, motion.frame(now, true)) } {
                            Ok(()) => {
                                set_status(|s| s.last_error = None);
                            }
                            Err(error) => set_status(|s| s.last_error = Some(error)),
                        }
                        visible = true;
                        last_show = Instant::now();
                        animating_until = Some(
                            Instant::now() + Duration::from_millis(rwi_pointer::HIGHLIGHT_MS + 300),
                        );
                        set_status(|s| {
                            s.visible = true;
                            s.point = Some(point);
                            s.dpi = Some(dpi);
                        });
                        // Requested coordinates are not evidence of placement.
                        // Read the window server's actual window and display back.
                        let window_number: isize = unsafe { msg_send![created, windowNumber] };
                        let mut actual = u32::try_from(window_number).ok().and_then(|id| {
                            crate::native::windows()
                                .ok()?
                                .into_iter()
                                .find(|w| w.id == id)
                        });
                        // The window server can commit a move just after the
                        // AppKit call returns. Settle reads only; never repeat
                        // the placement command or manufacture an applied point.
                        let settle = Instant::now();
                        while actual
                            .as_ref()
                            .is_none_or(|w| !hotspot_matches(w.rect, point))
                            && settle.elapsed() < Duration::from_millis(50)
                        {
                            std::thread::sleep(Duration::from_millis(5));
                            actual = u32::try_from(window_number).ok().and_then(|id| {
                                crate::native::windows()
                                    .ok()?
                                    .into_iter()
                                    .find(|w| w.id == id)
                            });
                        }
                        set_status(|s| {
                            s.window_id = u32::try_from(window_number).ok();
                            s.applied_point = actual
                                .as_ref()
                                .map(|w| [w.rect.x + w.rect.w / 2.0, w.rect.y + w.rect.h / 2.0]);
                            s.display_id = actual.as_ref().and_then(|w| {
                                crate::native::displays().ok().and_then(|ds| {
                                    crate::native::containing_display(&ds, w.rect).map(|d| d.id)
                                })
                            });
                            if s.applied_point.is_none_or(|p| {
                                (p[0] - point[0]).abs() > 1.0 || (p[1] - point[1]).abs() > 1.0
                            }) {
                                s.last_error =
                                    Some("overlay placement not confirmed by window server".into());
                            }
                        });
                    });
                    let _ = ack.send(());
                }
                Ok(Command {
                    action: Action::Hide,
                    ack,
                }) => {
                    if let Some(created) = window {
                        unsafe {
                            let _: () =
                                msg_send![created, orderOut: std::ptr::null_mut::<AnyObject>()];
                        }
                    }
                    visible = false;
                    set_status(|s| s.visible = false);
                    let _ = ack.send(());
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        // Animate the highlight while it is active, then settle to idle.
        if let Some(until) = animating_until {
            if let Some(view_layer) = layer {
                let now = start.elapsed().as_secs_f64();
                let frame = motion.frame(now, true);
                if Instant::now() >= until && !frame.animating {
                    animating_until = None;
                }
                let _ = unsafe { present(view_layer, dpi, frame) };
            } else {
                animating_until = None;
            }
        }
        if visible && last_show.elapsed() > IDLE {
            if let Some(created) = window {
                unsafe {
                    let _: () = msg_send![created, orderOut: std::ptr::null_mut::<AnyObject>()];
                }
            }
            visible = false;
            set_status(|s| s.visible = false);
        }
        // Pump the main run loop so AppKit can service its own sources: a
        // borderless window is only ordered on screen while the loop runs.
        live.tick();
        unsafe {
            let _ = CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, TICK, true);
        }
    }
}

fn hotspot_matches(frame: crate::geometry::Rect, point: [f64; 2]) -> bool {
    (frame.x + frame.w / 2.0 - point[0]).abs() <= 1.0
        && (frame.y + frame.h / 2.0 - point[1]).abs() <= 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_hotspot_conversion_preserves_non_primary_display_coordinates() {
        for point in [
            [3751.0, 372.0],
            [-800.0, 300.0],
            [300.0, -500.0],
            [100.0, 1300.0],
        ] {
            let origin = cocoa_origin_with_height(point, 982.0).unwrap();
            assert_eq!(
                [origin[0] + HOTSPOT_PT, 982.0 - origin[1] - HOTSPOT_PT],
                point
            );
        }
        assert!(cocoa_origin_with_height([f64::NAN, 0.0], 982.0).is_none());
        assert!(cocoa_origin_with_height([0.0, 0.0], 0.0).is_none());
    }
}
