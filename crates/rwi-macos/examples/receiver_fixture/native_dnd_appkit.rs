//! Public AppKit native drag/drop receiving fixture. No global event producer,
//! activation, clipboard-general access, hidden transport or production fallback.
use super::*;
use crate::native_dnd::{PASTEBOARD_TYPE, Payload, Phase};
use objc2::runtime::AnyProtocol;

pub(super) struct Objects {
    pub source: Retained<AnyObject>,
    pub destination: Retained<AnyObject>,
    down: Option<Retained<AnyObject>>,
    session: Option<Retained<AnyObject>>,
}
impl Objects {
    pub(super) unsafe fn new() -> Result<Self, String> {
        let mut source = ClassBuilder::new(c"RWINativeDragSource", class(c"NSView")?)
            .ok_or("native source class already registered")?;
        let mut destination = ClassBuilder::new(c"RWINativeDropDestination", class(c"NSView")?)
            .ok_or("native destination class already registered")?;
        // SDK NSDragging.h signatures. These are this process's own classes.
        unsafe {
            source.add_protocol(
                AnyProtocol::get(c"NSDraggingSource").ok_or("NSDraggingSource unavailable")?,
            );
            destination.add_protocol(
                AnyProtocol::get(c"NSDraggingDestination")
                    .ok_or("NSDraggingDestination unavailable")?,
            );
            for b in [&mut source, &mut destination] {
                b.add_method(sel!(isFlipped), flipped as extern "C" fn(_, _) -> _);
                b.add_method(
                    sel!(acceptsFirstMouse:),
                    first_mouse as extern "C" fn(_, _, _) -> _,
                );
                b.add_method(sel!(drawRect:), draw as extern "C" fn(_, _, _));
            }
            source.add_method(
                sel!(shouldDelayWindowOrderingForEvent:),
                first_mouse as extern "C" fn(_, _, _) -> _,
            );
            source.add_method(sel!(mouseDown:), down as extern "C" fn(_, _, _));
            source.add_method(sel!(mouseDragged:), dragged as extern "C" fn(_, _, _));
            source.add_method(sel!(mouseUp:), up as extern "C" fn(_, _, _));
            source.add_method(
                sel!(draggingSession:sourceOperationMaskForDraggingContext:),
                mask as extern "C" fn(_, _, _, _) -> _,
            );
            source.add_method(
                sel!(draggingSession:willBeginAtPoint:),
                began as extern "C" fn(_, _, _, _),
            );
            source.add_method(
                sel!(draggingSession:movedToPoint:),
                moved as extern "C" fn(_, _, _, _),
            );
            source.add_method(
                sel!(draggingSession:endedAtPoint:operation:),
                ended as extern "C" fn(_, _, _, _, _),
            );
            source.add_method(
                sel!(ignoreModifierKeysForDraggingSession:),
                first_mouse as extern "C" fn(_, _, _) -> _,
            );
            destination.add_method(
                sel!(draggingEntered:),
                entered as extern "C" fn(_, _, _) -> _,
            );
            destination.add_method(
                sel!(draggingUpdated:),
                updated as extern "C" fn(_, _, _) -> _,
            );
            destination.add_method(sel!(draggingExited:), exited as extern "C" fn(_, _, _));
            destination.add_method(
                sel!(prepareForDragOperation:),
                prepare as extern "C" fn(_, _, _) -> _,
            );
            destination.add_method(
                sel!(performDragOperation:),
                perform as extern "C" fn(_, _, _) -> _,
            );
            destination.add_method(
                sel!(concludeDragOperation:),
                conclude as extern "C" fn(_, _, _),
            );
            let source = view(source.register(), rect(24., 100., 360., 206.));
            let destination = view(destination.register(), rect(0., 0., 210., 1000.));
            identify(
                &source,
                "rwi-native-dnd-source",
                "RWI native pasteboard drag source",
            );
            identify(
                &destination,
                "rwi-native-dnd-destination",
                "RWI native pasteboard drop destination",
            );
            let kind = NSString::from_str(PASTEBOARD_TYPE);
            let types: Retained<AnyObject> = msg_send![class(c"NSArray")?, arrayWithObject:&*kind];
            let _: () = msg_send![&*destination, registerForDraggedTypes:&*types];
            label(
                &source,
                "Drag this native payload",
                rect(16., 24., 320., 28.),
            )?;
            Ok(Self {
                source,
                destination,
                down: None,
                session: None,
            })
        }
    }
}

extern "C" fn down(this: &AnyObject, _cmd: Sel, event: &AnyObject) {
    receive(this, event, MouseKind::Down);
    callback(|| {
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut()
                && let Some(objects) = ui.native_dnd_objects.as_mut()
                && objects.session.is_none()
                && std::ptr::eq(this, &*objects.source)
            {
                // Real delivered AppKit mouse-down only, never a fabricated NSEvent.
                objects.down = unsafe { Retained::retain(std::ptr::from_ref(event).cast_mut()) };
            }
        })
    });
}
extern "C" fn up(this: &AnyObject, _cmd: Sel, event: &AnyObject) {
    receive(this, event, MouseKind::Up);
    callback(|| {
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut()
                && let Some(objects) = ui.native_dnd_objects.as_mut()
            {
                objects.down = None;
            }
        })
    });
}
extern "C" fn dragged(this: &AnyObject, _cmd: Sel, event: &AnyObject) {
    receive(this, event, MouseKind::Dragged);
    callback(|| {
        let start = UI_STATE.with(|cell| {
            let mut guard = cell.borrow_mut();
            let ui = guard.as_mut()?;
            let objects = ui.native_dnd_objects.as_mut()?;
            if objects.session.is_some() || !std::ptr::eq(this, &*objects.source) {
                return None;
            }
            let down = objects.down.take()?; // one attempt, never replay after failure
            ui.state.native_dnd.source_requests += 1;
            let payload = Payload::own(
                ui.state.pid,
                &ui.state.run_id,
                ui.state.native_dnd.source_requests,
            );
            let source = objects.source.clone();
            ui.note_native_timeline(Phase::SourceRequest, None, None);
            ui.flush();
            Some((source, down, payload))
        });
        let Some((source, down, payload)) = start else {
            return;
        };
        // No RefCell borrow crosses AppKit's synchronous callbacks/run-loop work.
        let result = autoreleasepool(|_| unsafe { begin(&source, &down, &payload) });
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                match result {
                    Ok(session) => {
                        let seq: isize = unsafe { msg_send![&*session, draggingSequenceNumber] };
                        if ui.state.native_dnd.active_sequence == Some(seq)
                            && let Some(objects) = ui.native_dnd_objects.as_mut()
                        {
                            objects.session = Some(session);
                        }
                    }
                    Err(error) => ui.state.native_dnd.error = Some(error),
                }
                ui.flush();
            }
        });
    });
}
unsafe fn begin(
    source: &AnyObject,
    event: &AnyObject,
    payload: &Payload,
) -> Result<Retained<AnyObject>, String> {
    let text = NSString::from_str(&serde_json::to_string(payload).map_err(|e| e.to_string())?);
    let kind = NSString::from_str(PASTEBOARD_TYPE);
    unsafe {
        let writer: Retained<AnyObject> = msg_send![class(c"NSPasteboardItem")?, new];
        let accepted: Bool = msg_send![&*writer, setString:&*text, forType:&*kind];
        if !accepted.as_bool() {
            return Err("own native pasteboard writer refused payload".into());
        }
        let allocated: Allocated<AnyObject> = msg_send![class(c"NSDraggingItem")?, alloc];
        let item: Option<Retained<AnyObject>> =
            msg_send![allocated, initWithPasteboardWriter:&*writer];
        let item = item.ok_or("NSDraggingItem initialization failed")?;
        let allocated: Allocated<AnyObject> = msg_send![class(c"NSImage")?, alloc];
        let image: Retained<AnyObject> = msg_send![allocated, initWithSize:NSSize::new(120., 40.)];
        let _: () = msg_send![&*item, setDraggingFrame:rect(16., 24., 120., 40.), contents:&*image];
        let items: Retained<AnyObject> = msg_send![class(c"NSArray")?, arrayWithObject:&*item];
        let session: Option<Retained<AnyObject>> =
            msg_send![source, beginDraggingSessionWithItems:&*items, event:event, source:source];
        let session = session.ok_or("native drag session not returned")?;
        let _: () = msg_send![&*session, setAnimatesToStartingPositionsOnCancelOrFail:false];
        Ok(session)
    }
}
extern "C" fn mask(_this: &AnyObject, _cmd: Sel, _session: &AnyObject, _context: isize) -> usize {
    1 // NSDragOperationCopy, NSDragging.h, both within/outside own app.
}
extern "C" fn began(_this: &AnyObject, _cmd: Sel, session: &AnyObject, _point: NSPoint) {
    callback(|| {
        let sequence = unsafe { msg_send![session, draggingSequenceNumber] };
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                ui.state.native_dnd.source_began += 1;
                ui.state.native_dnd.active_sequence = Some(sequence);
                ui.note_native_timeline(Phase::SourceBegan, Some(sequence), None);
                ui.flush();
            }
        })
    });
}
extern "C" fn moved(_this: &AnyObject, _cmd: Sel, session: &AnyObject, _point: NSPoint) {
    callback(|| {
        let sequence = unsafe { msg_send![session, draggingSequenceNumber] };
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                ui.state.native_dnd.source_moved += 1;
                ui.note_native_timeline(Phase::SourceMoved, Some(sequence), None);
                ui.flush();
            }
        })
    });
}
extern "C" fn ended(
    _this: &AnyObject,
    _cmd: Sel,
    session: &AnyObject,
    _point: NSPoint,
    operation: usize,
) {
    callback(|| {
        let sequence = unsafe { msg_send![session, draggingSequenceNumber] };
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                ui.state.native_dnd.source_ended += 1;
                ui.state.native_dnd.source_cancelled += u64::from(operation == 0);
                ui.state.native_dnd.last_source_operation = Some(operation);
                // Keep the real session and active state in the end receipt before clearing them.
                ui.note_native_timeline(Phase::SourceEnded, Some(sequence), Some(operation));
                ui.state.native_dnd.active_sequence = None;
                if let Some(o) = ui.native_dnd_objects.as_mut() {
                    o.down = None;
                    o.session = None;
                }
                ui.flush();
            }
        })
    });
}

unsafe fn read(info: &AnyObject) -> Option<String> {
    unsafe {
        let mask: usize = msg_send![info, draggingSourceOperationMask];
        if mask & 1 == 0 {
            return None;
        }
        let board: *mut AnyObject = msg_send![info, draggingPasteboard];
        if board.is_null() {
            return None;
        }
        let kind = NSString::from_str(PASTEBOARD_TYPE);
        let text: Option<Retained<NSString>> = msg_send![board, stringForType:&*kind];
        let text = text?;
        let bytes: usize = msg_send![&*text, lengthOfBytesUsingEncoding:4usize]; // NSUTF8StringEncoding
        if bytes > crate::native_dnd::MAX_PAYLOAD_BYTES {
            return None;
        }
        let text = text.to_string();
        Payload::decode(&text).map(|_| text)
    }
}
fn destination(info: &AnyObject, update: bool) -> usize {
    let mut operation = 0;
    callback(|| {
        let sequence = unsafe { msg_send![info, draggingSequenceNumber] };
        let valid = unsafe { read(info) }.is_some();
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                if update {
                    ui.state.native_dnd.destination_updated += 1;
                } else {
                    ui.state.native_dnd.destination_entered += 1;
                }
                operation = usize::from(ui.state.native_dnd.enabled && valid);
                ui.note_native_timeline(
                    if update {
                        Phase::DestinationUpdated
                    } else {
                        Phase::DestinationEntered
                    },
                    Some(sequence),
                    Some(operation),
                );
                ui.flush();
            }
        });
    });
    operation
}
extern "C" fn entered(_this: &AnyObject, _cmd: Sel, info: &AnyObject) -> usize {
    destination(info, false)
}
extern "C" fn updated(_this: &AnyObject, _cmd: Sel, info: &AnyObject) -> usize {
    destination(info, true)
}
extern "C" fn exited(_this: &AnyObject, _cmd: Sel, info: *mut AnyObject) {
    callback(|| {
        let sequence = unsafe {
            info.as_ref()
                .map(|info| msg_send![info, draggingSequenceNumber])
        };
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                ui.state.native_dnd.destination_exited += 1;
                ui.note_native_timeline(Phase::DestinationExited, sequence, None);
                ui.flush();
            }
        })
    });
}
extern "C" fn prepare(_this: &AnyObject, _cmd: Sel, info: &AnyObject) -> Bool {
    let mut accepted = false;
    callback(|| {
        let sequence = unsafe { msg_send![info, draggingSequenceNumber] };
        let valid = unsafe { read(info) }.is_some();
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                ui.state.native_dnd.destination_prepared += 1;
                accepted = ui.state.native_dnd.enabled && valid;
                ui.note_native_timeline(
                    Phase::DestinationPrepared,
                    Some(sequence),
                    Some(usize::from(accepted)),
                );
                ui.flush();
            }
        });
    });
    Bool::from(accepted)
}
extern "C" fn perform(_this: &AnyObject, _cmd: Sel, info: &AnyObject) -> Bool {
    let mut consumed = false;
    callback(|| {
        let text = unsafe { read(info) }.unwrap_or_default();
        let (seq, external, window) = unsafe {
            let source: *mut AnyObject = msg_send![info, draggingSource];
            let window: *mut AnyObject = msg_send![info, draggingDestinationWindow];
            (
                msg_send![info, draggingSequenceNumber],
                source.is_null(),
                if window.is_null() {
                    0
                } else {
                    msg_send![window, windowNumber]
                },
            )
        };
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                consumed = ui.state.native_dnd.consume(seq, &text, external, window);
                ui.note_native_timeline(
                    Phase::DestinationPerformed,
                    Some(seq),
                    Some(usize::from(consumed)),
                );
                ui.flush();
            }
        });
    });
    Bool::from(consumed)
}
extern "C" fn conclude(_this: &AnyObject, _cmd: Sel, info: *mut AnyObject) {
    callback(|| {
        let sequence = unsafe {
            info.as_ref()
                .map(|info| msg_send![info, draggingSequenceNumber])
        };
        UI_STATE.with(|cell| {
            if let Some(ui) = cell.borrow_mut().as_mut() {
                ui.state.native_dnd.destination_concluded += 1;
                ui.note_native_timeline(Phase::DestinationConcluded, sequence, None);
                ui.flush();
            }
        })
    });
}
extern "C" fn draw(this: &AnyObject, _cmd: Sel, _dirty: NSRect) {
    callback(|| {
        UI_STATE.with(|cell| {
            let guard = cell.borrow();
            let Some(ui) = guard.as_ref() else {
                return;
            };
            let Some(objects) = ui.native_dnd_objects.as_ref() else {
                return;
            };
            unsafe {
                let color: *mut AnyObject = if std::ptr::eq(this, &*objects.source) {
                    msg_send![class(c"NSColor").unwrap(), systemBlueColor]
                } else if ui.state.native_dnd.consumed > 0 {
                    msg_send![class(c"NSColor").unwrap(), systemGreenColor]
                } else {
                    msg_send![class(c"NSColor").unwrap(), controlBackgroundColor]
                };
                let _: () = msg_send![color, setFill];
                let bounds: NSRect = msg_send![this, bounds];
                let _: () = msg_send![class(c"NSBezierPath").unwrap(), fillRect:bounds];
            }
        })
    });
}
