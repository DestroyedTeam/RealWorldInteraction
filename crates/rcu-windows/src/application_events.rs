// Native event session; mechanically extracted from application.rs.
use super::*;
use std::cell::{Cell, RefCell};
use windows::Win32::UI::Accessibility::*;

const MAX_PUMP_MESSAGES: usize = 256;
const PUMP_BUDGET: Duration = Duration::from_millis(20);

type Hook = ReleaseGuard<HWINEVENTHOOK, fn(HWINEVENTHOOK) -> bool>;

fn close_hook(hook: HWINEVENTHOOK) -> bool {
    let released = unsafe { UnhookWinEvent(hook) }.as_bool();
    if !released {
        resource_release_uncertain();
        HOOK_THREAD_POISONED.with(|p| p.set(true));
    }
    released
}

#[derive(Clone, Copy)]
struct RawEvent {
    event: u32,
    hwnd: usize,
    time: u32,
    thread: u32,
}
struct Recorder {
    hooks: Vec<usize>,
    events: BoundedEvents<RawEvent>,
}
thread_local! {
    static CALLBACK_RECORDER: RefCell<Option<Rc<RefCell<Recorder>>>> = const { RefCell::new(None) };
    static CALLBACK_FAILED: Cell<bool> = const { Cell::new(false) };
    static HOOK_THREAD_POISONED: Cell<bool> = const { Cell::new(false) };
}
unsafe extern "system" fn event_callback(
    hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    thread: u32,
    time: u32,
) {
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if hwnd.0.is_null() || object != OBJID_WINDOW.0 || child != 0 {
            return;
        }
        CALLBACK_RECORDER.with(|slot| {
            let Ok(slot) = slot.try_borrow() else {
                CALLBACK_FAILED.with(|f| f.set(true));
                return;
            };
            let Some(recorder) = slot.as_ref() else {
                return;
            };
            let Ok(mut recorder) = recorder.try_borrow_mut() else {
                CALLBACK_FAILED.with(|f| f.set(true));
                return;
            };
            if !recorder.hooks.contains(&(hook.0 as usize)) {
                recorder.events.complete = false;
                return;
            }
            recorder.events.push(RawEvent {
                event,
                hwnd: hwnd.0 as usize,
                time,
                thread,
            });
        });
    }));
    if caught.is_err() {
        CALLBACK_FAILED.with(|f| f.set(true));
    }
}
fn event_name(event: u32) -> &'static str {
    match event {
        EVENT_OBJECT_CREATE => "create",
        EVENT_OBJECT_DESTROY => "destroy",
        EVENT_OBJECT_SHOW => "show",
        EVENT_OBJECT_NAMECHANGE => "namechange",
        EVENT_SYSTEM_FOREGROUND => "foreground",
        _ => "unknown",
    }
}
pub(super) struct EventSession {
    recorder: Rc<RefCell<Recorder>>,
    hooks: Vec<Hook>,
    released: bool,
}
impl EventSession {
    pub(super) fn begin() -> Result<Self> {
        if HOOK_THREAD_POISONED.with(Cell::get)
            || CALLBACK_RECORDER.with(|r| r.borrow().is_some())
        {
            return Err(fault(
                "lifecycle_event_thread_busy",
                "The current thread has active or uncertain event hooks.",
            ));
        }
        let _call = native_call("SetWinEventHook")?;
        let recorder = Rc::new(RefCell::new(Recorder {
            hooks: Vec::with_capacity(3),
            events: BoundedEvents::new(),
        }));
        CALLBACK_FAILED.with(|f| f.set(false));
        CALLBACK_RECORDER.with(|r| *r.borrow_mut() = Some(recorder.clone()));
        let mut session = Self {
            recorder,
            hooks: Vec::with_capacity(3),
            released: true,
        };
        for (first, last) in [
            (EVENT_OBJECT_CREATE, EVENT_OBJECT_SHOW),
            (EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_NAMECHANGE),
            (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
        ] {
            let h = unsafe {
                SetWinEventHook(
                    first,
                    last,
                    None,
                    Some(event_callback),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                )
            };
            if h.0.is_null() {
                return Err(fault(
                    "lifecycle_event_hook_failed",
                    "Native WinEvent registration failed; no launch is permitted.",
                ));
            }
            session.hooks.push(ReleaseGuard::new(h, close_hook));
            session.recorder.borrow_mut().hooks.push(h.0 as usize);
        }
        Ok(session)
    }
    pub(super) fn incomplete(&self) {
        self.recorder.borrow_mut().events.complete = false;
    }
    pub(super) fn pump(&self, cleanup: bool) -> Result<()> {
        // Cleanup still runs after cancellation/deadline. Native preemption
        // is not claimed: a pending PeekMessage/dispatch owns its call slot.
        let _call = if cleanup {
            None
        } else {
            Some(native_call("lifecycle_event_message_pump")?)
        };
        let began = Instant::now();
        for _ in 0..MAX_PUMP_MESSAGES {
            if !cleanup {
                checkpoint("lifecycle_event_message_pump")?;
            }
            let mut message = MSG::default();
            if !unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
                return Ok(());
            }
            if message.message == WM_QUIT {
                unsafe {
                    PostQuitMessage(message.wParam.0 as i32);
                }
                self.incomplete();
                return Ok(());
            }
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            if began.elapsed() >= PUMP_BUDGET {
                self.incomplete();
                return Ok(());
            }
        }
        self.incomplete();
        Ok(())
    }
    pub(super) fn batch(&self, released: bool) -> EventBatch {
        let recorder = self.recorder.borrow();
        EventBatch {
            events: recorder
                .events
                .events
                .iter()
                .map(|e| NativeEvent {
                    event: event_name(e.event).into(),
                    window: format!("hwnd:{:x}", e.hwnd),
                    event_time_ms: e.time,
                    thread_id: e.thread,
                })
                .collect(),
            complete: recorder.events.complete && !CALLBACK_FAILED.with(Cell::get),
            hooks_released: released,
        }
    }
    pub(super) fn close(&mut self) -> bool {
        for hook in &mut self.hooks {
            self.released &= hook.close();
        }
        CALLBACK_RECORDER.with(|r| {
            let mut r = r.borrow_mut();
            if r.as_ref()
                .is_some_and(|current| Rc::ptr_eq(current, &self.recorder))
            {
                *r = None;
            }
        });
        if !self.released {
            self.incomplete();
        }
        self.released
    }
}
impl Drop for EventSession {
    fn drop(&mut self) {
        self.close();
    }
}
