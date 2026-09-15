//! AI pointer overlay: a purely visual, click-through marker for the last
//! dispatched point. It never dispatches input, never moves the real cursor and
//! never takes focus.
//!
//! AppKit window creation is confined to the process main thread, which pumps
//! [`pump_forever`]; every other thread only sends bounded commands.
use core_foundation::runloop::{CFRunLoop, kCFRunLoopDefaultMode};
use objc2::{
    msg_send,
    rc::autoreleasepool,
    runtime::{AnyClass, AnyObject},
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

const SPRITE: f64 = 26.0;
const IDLE: Duration = Duration::from_secs(45);

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
        // Accessory policy: real windows without Dock activation or a menu bar.
        let _: () = msg_send![app, setActivationPolicy: 1i64];
    }
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
    let (created, visible, point, last_error) = snapshot
        .as_ref()
        .map(|s| (s.created, s.visible, s.point, s.last_error.clone()))
        .unwrap_or((false, false, None, None));
    json!({
        "installed": SENDER.get().is_some(),
        "created": created,
        "visible": visible,
        "point": point,
        "last_error": last_error,
    })
}

fn sprite_rect() -> NSRect {
    NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(SPRITE, SPRITE))
}

/// Convert a global top-left desktop point into a Cocoa frame origin for the
/// sprite. Returns None when the primary display height is unknown.
fn cocoa_origin(point: [f64; 2]) -> Option<[f64; 2]> {
    let primary = core_graphics::display::CGDisplay::main().bounds();
    let height = primary.size.height;
    if !height.is_finite() || height <= 0.0 {
        return None;
    }
    Some([point[0] - SPRITE / 2.0, height - point[1] - SPRITE / 2.0])
}

unsafe fn create_window() -> std::result::Result<*mut AnyObject, String> {
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
        let title = NSString::from_str("RCU AI Cursor");
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
        let fill: *mut AnyObject = msg_send![
            color_class,
            colorWithRed: 1.0f64,
            green: 0.35f64,
            blue: 0.12f64,
            alpha: 0.30f64
        ];
        let stroke: *mut AnyObject = msg_send![
            color_class,
            colorWithRed: 1.0f64,
            green: 0.48f64,
            blue: 0.16f64,
            alpha: 0.95f64
        ];
        let fill_cg: *mut AnyObject = msg_send![fill, CGColor];
        let stroke_cg: *mut AnyObject = msg_send![stroke, CGColor];
        let _: () = msg_send![layer, setBackgroundColor: fill_cg];
        let _: () = msg_send![layer, setBorderColor: stroke_cg];
        let _: () = msg_send![layer, setBorderWidth: 2.0f64];
        let _: () = msg_send![layer, setCornerRadius: SPRITE / 2.0];
        let _: () = msg_send![window, setContentView: view];
        Ok(window)
    }
}

/// Main thread: create/show/hide the marker on demand until the process ends.
/// Never returns; call this after `install()` on the main thread.
pub fn pump_forever() -> ! {
    let receiver = RECEIVER
        .get()
        .and_then(|slot| slot.lock().ok().and_then(|mut guard| guard.take()));
    let Some(receiver) = receiver else {
        loop {
            std::thread::sleep(Duration::from_secs(3600));
        }
    };
    let mut window: Option<*mut AnyObject> = None;
    let mut visible = false;
    let mut last_show = Instant::now();
    loop {
        loop {
            match receiver.try_recv() {
                Ok(Command {
                    action: Action::Show(point),
                    ack,
                }) => {
                    autoreleasepool(|_| {
                        if window.is_none() {
                            match unsafe { create_window() } {
                                Ok(created) => {
                                    window = Some(created);
                                    set_status(|s| s.created = true);
                                }
                                Err(error) => {
                                    set_status(|s| s.last_error = Some(error));
                                    let _ = ack.send(());
                                    return;
                                }
                            }
                        }
                        let Some(target) = window else {
                            let _ = ack.send(());
                            return;
                        };
                        let Some(origin) = cocoa_origin(point) else {
                            set_status(|s| s.last_error = Some("primary display unknown".into()));
                            let _ = ack.send(());
                            return;
                        };
                        if let Some(created) = window {
                            unsafe {
                                let _: () = msg_send![created, setFrameOrigin: NSPoint::new(origin[0], origin[1])];
                                let _: () = msg_send![created, orderFrontRegardless];
                            }
                        }
                        visible = true;
                        last_show = Instant::now();
                        set_status(|s| {
                            s.visible = true;
                            s.point = Some(point);
                            s.last_error = None;
                        });
                        let _ = target;
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
        unsafe {
            let _ = CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, Duration::from_millis(50), true);
        }
    }
}
