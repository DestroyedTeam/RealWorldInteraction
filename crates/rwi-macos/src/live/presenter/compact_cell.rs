//! Public AppKit cell drawing only, confined to RWILiveAccessibilityButton.
//! No global cellClass setter, input forwarding, activation, hitTest or tracking override.
use super::*;
use objc2::runtime::Bool;

/// Idempotent: normal redraw may call interior/bezel again with already compact bounds.
fn compact_frame(frame: NSRect) -> Option<NSRect> {
    let [x, y, w, h] = [
        frame.origin.x,
        frame.origin.y,
        frame.size.width,
        frame.size.height,
    ];
    if ![x, y, w, h].iter().all(|v| v.is_finite()) || w <= 0. || h <= 0. {
        return None;
    }
    let width = w.min(28.);
    let height = h.min(28.);
    Some(rect(
        x + (w - width) / 2.,
        y + (h - height) / 2.,
        width,
        height,
    ))
}
fn parent() -> Option<&'static AnyClass> {
    AnyClass::get(c"NSButtonCell")
}

// Forward every designated initializer. No new ivars, borrowed objects or ownership
// escape: superclass preserves init-family +1 ownership, and NSButton owns its cell.
extern "C" fn init(this: &AnyObject, _cmd: Sel) -> *mut AnyObject {
    let Some(parent) = parent() else {
        return std::ptr::null_mut();
    };
    unsafe { msg_send![super(this, parent), init] }
}
macro_rules! init_callback {
    ($name:ident,$selector:ident) => {
        extern "C" fn $name(this:&AnyObject,_cmd:Sel,value:*mut AnyObject)->*mut AnyObject {
            let Some(parent)=parent() else {return std::ptr::null_mut()};
            unsafe {msg_send![super(this,parent),$selector:value]}
        }
    };
}
init_callback!(init_text, initTextCell);
init_callback!(init_image, initImageCell);
init_callback!(init_coder, initWithCoder);

// These are AppKit-driven display callbacks, not direct out-of-band drawing calls.
// Actual bounds and all hitTest/trackMouse/mouseDown/release behavior stay in NSButton.
macro_rules! paint_callback {
    ($name:ident,$selector:ident) => {
        extern "C" fn $name(this:&AnyObject,_cmd:Sel,frame:NSRect,view:*mut AnyObject) {
            let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let (Some(parent),Some(frame))=(parent(),compact_frame(frame)) else {return};
                if view.is_null() {return}
                unsafe {let _:()=msg_send![super(this,parent),$selector:frame,inView:view];}
            }));
            if result.is_err() {eprintln!("rwi-live: compact cell display callback failed");}
        }
    };
}
paint_callback!(draw, drawWithFrame);
paint_callback!(bezel, drawBezelWithFrame);
paint_callback!(interior, drawInteriorWithFrame);
paint_callback!(focus_mask, drawFocusRingMaskWithFrame);
extern "C" fn highlight(
    this: &AnyObject,
    _cmd: Sel,
    flag: Bool,
    frame: NSRect,
    view: *mut AnyObject,
) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (Some(parent), Some(frame)) = (parent(), compact_frame(frame)) else {
            return;
        };
        if view.is_null() {
            return;
        }
        unsafe {
            let _: () = msg_send![super(this,parent),highlight:flag,withFrame:frame,inView:view];
        }
    }));
    if result.is_err() {
        eprintln!("rwi-live: compact cell highlight callback failed");
    }
}
extern "C" fn focus_bounds(
    this: &AnyObject,
    _cmd: Sel,
    frame: NSRect,
    view: *mut AnyObject,
) -> NSRect {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (Some(parent), Some(frame)) = (parent(), compact_frame(frame)) else {
            return rect(0., 0., 0., 0.);
        };
        if view.is_null() {
            return rect(0., 0., 0., 0.);
        }
        unsafe { msg_send![super(this,parent),focusRingMaskBoundsForFrame:frame,inView:view] }
    }))
    .unwrap_or_else(|_| {
        eprintln!("rwi-live: compact cell focus callback failed");
        rect(0., 0., 0., 0.)
    })
}

pub(super) extern "C" fn cell_class(_class: &AnyClass, _cmd: Sel) -> *const AnyClass {
    // register() completes BEFORE our NSButton subclass can be allocated.
    AnyClass::get(c"RWILiveCompactButtonCell").map_or(std::ptr::null(), |c| c as *const AnyClass)
}
pub(super) fn register() -> std::result::Result<&'static AnyClass, String> {
    if let Some(class) = AnyClass::get(c"RWILiveCompactButtonCell") {
        return Ok(class);
    }
    let mut builder = ClassBuilder::new(c"RWILiveCompactButtonCell", class(c"NSButtonCell")?)
        .ok_or("Cannot register compact Live button cell")?;
    unsafe {
        builder.add_method(sel!(init), init as extern "C" fn(_, _) -> _);
        builder.add_method(
            sel!(initTextCell:),
            init_text as extern "C" fn(_, _, _) -> _,
        );
        builder.add_method(
            sel!(initImageCell:),
            init_image as extern "C" fn(_, _, _) -> _,
        );
        builder.add_method(
            sel!(initWithCoder:),
            init_coder as extern "C" fn(_, _, _) -> _,
        );
        builder.add_method(
            sel!(drawWithFrame:inView:),
            draw as extern "C" fn(_, _, _, _),
        );
        builder.add_method(
            sel!(drawBezelWithFrame:inView:),
            bezel as extern "C" fn(_, _, _, _),
        );
        builder.add_method(
            sel!(drawInteriorWithFrame:inView:),
            interior as extern "C" fn(_, _, _, _),
        );
        builder.add_method(
            sel!(highlight:withFrame:inView:),
            highlight as extern "C" fn(_, _, _, _, _),
        );
        builder.add_method(
            sel!(drawFocusRingMaskWithFrame:inView:),
            focus_mask as extern "C" fn(_, _, _, _),
        );
        builder.add_method(
            sel!(focusRingMaskBoundsForFrame:inView:),
            focus_bounds as extern "C" fn(_, _, _, _) -> _,
        );
    }
    Ok(builder.register())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live44_compact_paint_is_centered_28_and_idempotent_not_a_hit_frame() {
        let full = rect(0., 0., 44., 44.);
        let visual = compact_frame(full).unwrap();
        assert_eq!(visual, rect(8., 8., 28., 28.));
        assert_eq!(compact_frame(visual), Some(visual));
        assert_eq!(
            full,
            rect(0., 0., 44., 44.),
            "borrowed control bounds remain untouched"
        );
        assert_eq!(
            compact_frame(rect(10., 20., 20., 18.)),
            Some(rect(10., 20., 20., 18.))
        );
    }
    #[test]
    fn live44_compact_paint_refuses_nonfinite_and_nonpositive_geometry() {
        for r in [
            rect(f64::NAN, 0., 44., 44.),
            rect(0., 0., f64::INFINITY, 44.),
            rect(0., 0., 0., 44.),
            rect(0., 0., 44., -1.),
        ] {
            assert!(compact_frame(r).is_none());
        }
    }
}
