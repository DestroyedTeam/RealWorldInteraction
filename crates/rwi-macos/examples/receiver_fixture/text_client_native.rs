//! Own-window field-editor instrumentation using public AppKit superclass calls.
//! No swizzling, input-context activation, observer injection or text substitution.
use super::{
    Context, CurrentInputContext, Event, InputContextSource, MAX_INPUT_SOURCE_UNITS, MAX_UNITS,
    MarkedTextState, Trace, Units,
};
use objc2::{
    msg_send,
    rc::{Allocated, Retained},
    runtime::{AnyClass, AnyObject, ClassBuilder, Sel},
    sel,
};
use objc2_foundation::{NSRange, NSRect, NSString};
use std::{
    cell::RefCell,
    io::Write,
    panic::AssertUnwindSafe,
    path::{Path, PathBuf},
    time::Instant,
};

struct Recorder {
    app: Retained<AnyObject>,
    window: Retained<AnyObject>,
    input: Retained<AnyObject>,
    editor: Retained<AnyObject>,
    path: PathBuf,
    began: Instant,
    active: bool,
    trace: Trace,
}
thread_local! {
    static RECORDER: RefCell<Option<Recorder>> = const { RefCell::new(None) };
}

pub struct Objects {
    editor: Retained<AnyObject>,
}
fn base() -> &'static AnyClass {
    AnyClass::get(c"NSTextView").expect("own field editor superclass exists")
}
impl Objects {
    pub fn new(
        app: &AnyObject,
        window: &AnyObject,
        input: &AnyObject,
        directory: &Path,
    ) -> Result<Self, String> {
        if RECORDER.with(|cell| cell.borrow().is_some()) {
            return Err("one own field editor trace per fixture process".into());
        }
        let mut class = ClassBuilder::new(c"RWIReceiverFixtureTextClient", base())
            .ok_or("own field editor class already registered")?;
        // Public selectors and exact SDK argument/return signatures. Each
        // override observes around one superclass call, never a replacement.
        unsafe {
            class.add_method(sel!(keyDown:), key_down as extern "C" fn(_, _, _));
            class.add_method(sel!(keyUp:), key_up as extern "C" fn(_, _, _));
            class.add_method(
                sel!(interpretKeyEvents:),
                interpret as extern "C" fn(_, _, _),
            );
            class.add_method(
                sel!(insertText:replacementRange:),
                insert as extern "C" fn(_, _, _, _),
            );
            class.add_method(
                sel!(doCommandBySelector:),
                command as extern "C" fn(_, _, _),
            );
            class.add_method(
                sel!(setMarkedText:selectedRange:replacementRange:),
                marked as extern "C" fn(_, _, _, _, _),
            );
            class.add_method(sel!(unmarkText), unmark as extern "C" fn(_, _));
            let allocated: Allocated<AnyObject> = msg_send![class.register(), alloc];
            let editor: Retained<AnyObject> = msg_send![allocated, initWithFrame:NSRect::ZERO];
            let _: () = msg_send![&*editor, setFieldEditor:true];
            let window_number: isize = msg_send![window, windowNumber];
            let window_number = u32::try_from(window_number)
                .ok()
                .filter(|n| *n > 0)
                .ok_or("own text trace window number unavailable")?;
            RECORDER.with(|cell| {
                *cell.borrow_mut() = Some(Recorder {
                    app: Retained::retain(app as *const _ as *mut _).unwrap(),
                    window: Retained::retain(window as *const _ as *mut _).unwrap(),
                    input: Retained::retain(input as *const _ as *mut _).unwrap(),
                    editor: editor.clone(),
                    path: directory.join("text-client-trace.json"),
                    began: Instant::now(),
                    active: true,
                    trace: Trace::new(std::process::id(), window_number),
                });
            });
            let objects = Self { editor };
            flush()?;
            Ok(objects)
        }
    }
}
impl Drop for Objects {
    fn drop(&mut self) {
        let own = RECORDER.with(|cell| {
            let mut borrowed = cell.borrow_mut();
            let Some(recorder) = borrowed.as_mut() else {
                return false;
            };
            if !std::ptr::eq(&*recorder.editor, &*self.editor) {
                return false;
            }
            recorder.active = false;
            true
        });
        if own {
            if let Err(error) = crate::ime_source_native::restore(&self.editor) {
                eprintln!("own input source restoration unconfirmed: {error}");
            }
            if let Err(error) = flush() {
                eprintln!("own text trace final flush failed: {error}");
            }
            RECORDER.with(|cell| {
                cell.borrow_mut().take();
            });
        }
    }
}

/// The owner delegate returns this same editor only for the exact own input.
/// Nil preserves AppKit's default editor when disabled or for any other client.
pub extern "C" fn field_editor(
    _this: &AnyObject,
    _cmd: Sel,
    window: &AnyObject,
    client: *mut AnyObject,
) -> *mut AnyObject {
    RECORDER.with(|cell| {
        let Ok(mut borrowed) = cell.try_borrow_mut() else {
            return std::ptr::null_mut();
        };
        let Some(recorder) = borrowed.as_mut() else {
            return std::ptr::null_mut();
        };
        if !recorder.active
            || !std::ptr::eq(&*recorder.window, window)
            || !std::ptr::eq(&*recorder.input, client.cast_const())
        {
            return std::ptr::null_mut();
        }
        recorder.trace.field_editor_requests =
            recorder.trace.field_editor_requests.saturating_add(1);
        (&*recorder.editor as *const AnyObject).cast_mut()
    })
}

fn observe(this: &AnyObject, phase: &'static str, decorate: impl FnOnce(&mut Event)) {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        // Clone retained context, then release the RefCell BEFORE every native
        // getter. AppKit may reenter; no callback holds a borrow across super.
        let owner = RECORDER.with(|cell| {
            let borrowed = cell.try_borrow().ok()?;
            let r = borrowed.as_ref()?;
            (r.active && std::ptr::eq(&*r.editor, this))
                .then(|| (r.app.clone(), r.window.clone(), r.input.clone(), r.began))
        });
        let Some((app, window, input, began)) = owner else {
            return;
        };
        let context = unsafe {
            let current: *mut AnyObject = msg_send![&*input, currentEditor];
            let actual_window: *mut AnyObject = msg_send![this, window];
            if !std::ptr::eq(current.cast_const(), this)
                || !std::ptr::eq(actual_window.cast_const(), &*window)
            {
                RECORDER.with(|cell| {
                    if let Ok(mut b) = cell.try_borrow_mut()
                        && let Some(r) = b.as_mut()
                    {
                        r.trace.ignored_out_of_scope_callbacks =
                            r.trace.ignored_out_of_scope_callbacks.saturating_add(1);
                    }
                });
                return;
            }
            let first: *mut AnyObject = msg_send![&*window, firstResponder];
            // This exact owned field editor inherits NSTextView. Sample getters
            // only, with no RefCell borrow held and no input-context activation.
            let marked_before: bool = msg_send![this, hasMarkedText];
            let marked_range: NSRange = msg_send![this, markedRange];
            let marked_after: bool = msg_send![this, hasMarkedText];
            Context {
                application_active: msg_send![&*app, isActive],
                window_key: msg_send![&*window, isKeyWindow],
                window_main: msg_send![&*window, isMainWindow],
                first_responder_is_editor: std::ptr::eq(first.cast_const(), this),
                marked_text: MarkedTextState::sample(
                    marked_before,
                    [marked_range.location as u64, marked_range.length as u64],
                    marked_after,
                ),
                // Do not ask for a possibly lazy inputContext before keyDown
                // has completed. This metadata cannot explain an earlier key's
                // internal mode, nor prove that a later key is unperturbed.
                // Sample before the older lazy source getter at keyDown exit.
                current_input_context: matches!(
                    phase,
                    "interpret_enter" | "interpret_exit" | "key_down_exit"
                )
                .then(|| current_input_context(this)),
                input_context_source_after_key: (phase == "key_down_exit")
                    .then(|| input_context_source(this)),
            }
        };
        let elapsed = began.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        let mut event = Event::new(phase, elapsed, context);
        decorate(&mut event);
        RECORDER.with(|cell| {
            if let Ok(mut b) = cell.try_borrow_mut()
                && let Some(r) = b.as_mut()
            {
                r.trace.push(event);
            }
        });
    }));
    if result.is_err() {
        RECORDER.with(|cell| {
            if let Ok(mut b) = cell.try_borrow_mut()
                && let Some(r) = b.as_mut()
            {
                r.trace.observer_panics = r.trace.observer_panics.saturating_add(1);
            }
        });
    }
}
unsafe fn current_input_context(editor: &AnyObject) -> CurrentInputContext {
    // Read the public current activated context only. Do not ask this editor
    // for its potentially lazy inputContext, activate it, or handle any event.
    let Some(class) = AnyClass::get(c"NSTextInputContext") else {
        return CurrentInputContext::sample(false, false, false, false);
    };
    let context: Option<Retained<AnyObject>> = unsafe { msg_send![class, currentInputContext] };
    let Some(context) = context else {
        return CurrentInputContext::sample(true, false, false, false);
    };
    let client: *mut AnyObject = unsafe { msg_send![&*context, client] };
    CurrentInputContext::sample(
        true,
        true,
        !client.is_null(),
        std::ptr::eq(client.cast_const(), editor),
    )
}
unsafe fn input_context_source(editor: &AnyObject) -> InputContextSource {
    // Retain each public getter result across other potentially reentrant calls.
    // Never use the global currentInputContext, activate, a setter or handleEvent.
    let context: Option<Retained<AnyObject>> = unsafe { msg_send![editor, inputContext] };
    let Some(context) = context else {
        return InputContextSource::sample(false, None, None, &[]);
    };
    let before: *mut AnyObject = unsafe { msg_send![&*context, client] };
    if !std::ptr::eq(before.cast_const(), editor) {
        return InputContextSource::sample(true, Some(false), None, &[]);
    }
    let source: Option<Retained<NSString>> =
        unsafe { msg_send![&*context, selectedKeyboardInputSource] };
    let after: *mut AnyObject = unsafe { msg_send![&*context, client] };
    if !std::ptr::eq(after.cast_const(), editor) {
        return InputContextSource::sample(true, Some(false), None, &[]);
    }
    let Some(source) = source else {
        return InputContextSource::sample(true, Some(true), None, &[]);
    };
    let length = source.length();
    let mut units = vec![0u16; length.min(MAX_INPUT_SOURCE_UNITS)];
    if !units.is_empty() {
        unsafe {
            let _: () = msg_send![&*source,
                getCharacters:units.as_mut_ptr(), range:NSRange::new(0, units.len())];
        }
    }
    InputContextSource::sample(true, Some(true), Some(length), &units)
}
unsafe fn units(value: *mut AnyObject) -> Option<Units> {
    if value.is_null() {
        return None;
    }
    let string_class = AnyClass::get(c"NSString")?;
    let attributed_class = AnyClass::get(c"NSAttributedString")?;
    let attributed: bool = unsafe { msg_send![value, isKindOfClass:attributed_class] };
    let string: *mut AnyObject = if attributed {
        unsafe { msg_send![value, string] }
    } else {
        value
    };
    if string.is_null() || !unsafe { msg_send![string, isKindOfClass:string_class] } {
        return None;
    }
    let length: usize = unsafe { msg_send![string, length] };
    let mut raw = vec![0u16; length.min(MAX_UNITS)];
    if !raw.is_empty() {
        unsafe {
            let _: () =
                msg_send![string, getCharacters:raw.as_mut_ptr(), range:NSRange::new(0, raw.len())];
        }
    }
    Some(Units::new(length, raw))
}
fn key(this: &AnyObject, phase: &'static str, native: &AnyObject) {
    observe(this, phase, |e| unsafe {
        let text: *mut AnyObject = msg_send![native, characters];
        e.characters = units(text);
        e.native_event_type = Some(msg_send![native, type]);
        e.key_code = Some(msg_send![native, keyCode]);
        e.modifier_flags = Some(msg_send![native, modifierFlags]);
    });
}
extern "C" fn key_down(this: &AnyObject, _cmd: Sel, event: &AnyObject) {
    key(this, "key_down_enter", event);
    unsafe {
        let _: () = msg_send![super(this, base()), keyDown:event];
    }
    key(this, "key_down_exit", event);
}
extern "C" fn key_up(this: &AnyObject, _cmd: Sel, event: &AnyObject) {
    key(this, "key_up_enter", event);
    unsafe {
        let _: () = msg_send![super(this, base()), keyUp:event];
    }
    key(this, "key_up_exit", event);
}
extern "C" fn interpret(this: &AnyObject, _cmd: Sel, events: &AnyObject) {
    observe(this, "interpret_enter", |_| {});
    unsafe {
        let _: () = msg_send![super(this, base()), interpretKeyEvents:events];
    }
    observe(this, "interpret_exit", |_| {});
}
extern "C" fn insert(this: &AnyObject, _cmd: Sel, string: *mut AnyObject, range: NSRange) {
    let note = |e: &mut Event| {
        e.characters = unsafe { units(string) };
        e.replacement_range = Some([range.location, range.length]);
    };
    observe(this, "insert_text_enter", note);
    unsafe {
        let _: () = msg_send![super(this, base()), insertText:string, replacementRange:range];
    }
    observe(this, "insert_text_exit", note);
}
extern "C" fn command(this: &AnyObject, _cmd: Sel, selector: Sel) {
    let note = |e: &mut Event| {
        e.command = Some(selector.name().to_string_lossy().chars().take(96).collect());
    };
    observe(this, "command_enter", note);
    unsafe {
        let _: () = msg_send![super(this, base()), doCommandBySelector:selector];
    }
    observe(this, "command_exit", note);
}
extern "C" fn marked(
    this: &AnyObject,
    _cmd: Sel,
    string: *mut AnyObject,
    selected: NSRange,
    replacement: NSRange,
) {
    let note = |e: &mut Event| {
        e.characters = unsafe { units(string) };
        e.selected_range = Some([selected.location, selected.length]);
        e.replacement_range = Some([replacement.location, replacement.length]);
    };
    observe(this, "set_marked_enter", note);
    unsafe {
        let _: () = msg_send![super(this, base()), setMarkedText:string, selectedRange:selected, replacementRange:replacement];
    }
    observe(this, "set_marked_exit", note);
}
extern "C" fn unmark(this: &AnyObject, _cmd: Sel) {
    observe(this, "unmark_enter", |_| {});
    unsafe {
        let _: () = msg_send![super(this, base()), unmarkText];
    }
    observe(this, "unmark_exit", |_| {});
}

/// Flush outside key callbacks using the own fixture's existing bounded timer.
pub fn flush() -> Result<(), String> {
    if crate::ime_source_native::configured() {
        let owner = RECORDER.with(|cell| {
            let borrowed = cell.try_borrow().ok()?;
            let r = borrowed.as_ref()?;
            r.active.then(|| {
                (
                    r.app.clone(),
                    r.editor.clone(),
                    r.window.clone(),
                    r.input.clone(),
                )
            })
        });
        if let Some((app, editor, window, input)) = owner {
            crate::ime_source_native::poll(&app, &editor, &window, &input)?;
        }
    }
    let dump = RECORDER.with(|cell| {
        let b = cell
            .try_borrow()
            .map_err(|_| "own text trace busy".to_owned())?;
        b.as_ref()
            .map(|r| {
                serde_json::to_vec_pretty(&serde_json::json!({
                    "active":r.active,"trace":r.trace,"superclass":"NSTextView",
                    "input_posted_by_recorder":false,"context_properties_are_atomic":false
                }))
                .map(|bytes| (r.path.clone(), bytes))
                .map_err(|e| e.to_string())
            })
            .transpose()
    })?;
    let Some((path, bytes)) = dump else {
        return Ok(());
    };
    let pending = path.with_extension("pending");
    let mut out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)
        .map_err(|e| e.to_string())?;
    let result = out
        .write_all(&bytes)
        .and_then(|()| out.flush())
        .and_then(|()| std::fs::rename(&pending, &path));
    if result.is_err() {
        let _ = std::fs::remove_file(&pending);
    }
    result.map_err(|e| e.to_string())
}
