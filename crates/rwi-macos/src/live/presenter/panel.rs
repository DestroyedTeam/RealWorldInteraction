//! Main-thread panel construction and symbols; no input dispatch/activation.
use super::*;

impl Panel {
    pub(super) fn new(
        id: u64,
        state: &State,
        target: &AnyObject,
    ) -> std::result::Result<Self, String> {
        unsafe {
            let source_pixels = source_pixels(state);
            let (width, height) = panel_size(
                state.options.max_edge,
                state.display.rect.w,
                state.display.rect.h,
                source_pixels,
            );
            let allocated: Allocated<AnyObject> = msg_send![class(c"NSPanel")?, alloc];
            let window: Retained<AnyObject> = msg_send![allocated,
                initWithContentRect: rect(0.,0.,width,height), styleMask:128usize, backing:2usize, defer:false];
            let _: () = msg_send![&*window, setReleasedWhenClosed:false];
            // A live/privacy control must not leave a shrinking/fading window
            // server surface after orderOut has acknowledged it as hidden.
            // NSWindowAnimationBehaviorNone, from the AppKit SDK.
            let _: () = msg_send![&*window, setAnimationBehavior:2isize];
            let appearance: *mut AnyObject =
                msg_send![class(c"NSAppearance")?, appearanceNamed:NSAppearanceNameAqua];
            if appearance.is_null() {
                return Err("Light appearance unavailable".into());
            }
            let _: () = msg_send![&*window, setAppearance:appearance];
            let _: () = msg_send![&*window, setFloatingPanel:true];
            let _: () = msg_send![&*window, setHidesOnDeactivate:false];
            let _: () = msg_send![&*window, setBecomesKeyOnlyIfNeeded:true];
            let _: () = msg_send![&*window, setLevel:3isize];
            // Do not attach this panel to a full-screen game's Space. The
            // requested preview display can differ from the source display.
            // FullScreenAuxiliary placed a virtual-screen preview at (0,0) on
            // the user's full-screen game display in live acceptance.
            let _: () = msg_send![&*window, setCollectionBehavior:0usize];
            let _: () = msg_send![&*window, setOpaque:false];
            let _: () = msg_send![&*window, setHasShadow:false];
            let _: () = msg_send![&*window, setMovableByWindowBackground:true];
            let title = NSString::from_str(&format!("RWI Live · {}", state.options.label));
            let _: () = msg_send![&*window, setTitle:&*title];
            let clear: *mut AnyObject = msg_send![class(c"NSColor")?, clearColor];
            let _: () = msg_send![&*window, setBackgroundColor:clear];
            // A semantic floating material follows macOS accessibility/contrast
            // settings; a near-opaque black CALayer produced a heavy matte frame.
            let content = view(c"NSVisualEffectView", rect(0., 0., width, height))?;
            let _: () = msg_send![&*content, setMaterial:6isize]; // Popover
            let _: () = msg_send![&*content, setBlendingMode:0isize]; // BehindWindow
            let _: () = msg_send![&*content, setState:1isize]; // Active
            let _: () = msg_send![&*content, setWantsLayer:true];
            let layer: *mut AnyObject = msg_send![&*content, layer];
            let _: () = msg_send![layer, setCornerRadius:12.0f64];
            let _: () = msg_send![layer, setMasksToBounds:true];
            let label = view(c"NSTextField", rect(12., height - 28., width - 96., 18.))?;
            let _: () = msg_send![&*label,setStringValue:&*title];
            let _: () = msg_send![&*label,setEditable:false];
            let _: () = msg_send![&*label,setSelectable:false];
            let _: () = msg_send![&*label,setBezeled:false];
            let _: () = msg_send![&*label,setDrawsBackground:false];
            let primary: *mut AnyObject = msg_send![class(c"NSColor")?, labelColor];
            let _: () = msg_send![&*label,setTextColor:primary];
            let title_font: *mut AnyObject = msg_send![class(c"NSFont")?,systemFontOfSize:12.0f64];
            let _: () = msg_send![&*label,setFont:title_font];
            let _: () = msg_send![&*label,setToolTip:&*title];
            let _: () = msg_send![&*content,addSubview:&*label];
            let source_label = view(c"NSTextField", rect(12., height - 42., width - 24., 13.))?;
            let _: () = msg_send![&*source_label,setEditable:false];
            let _: () = msg_send![&*source_label,setSelectable:false];
            let _: () = msg_send![&*source_label,setBezeled:false];
            let _: () = msg_send![&*source_label,setDrawsBackground:false];
            let secondary: *mut AnyObject = msg_send![class(c"NSColor")?, secondaryLabelColor];
            let font: *mut AnyObject = msg_send![class(c"NSFont")?,systemFontOfSize:10.0f64];
            let _: () = msg_send![&*source_label,setTextColor:secondary];
            let _: () = msg_send![&*source_label,setFont:font];
            let _: () = msg_send![&*content,addSubview:&*source_label];
            let image = view(
                c"NSImageView",
                rect(8., 6., width - 16., height - CHROME_HEIGHT),
            )?;
            let _: () = msg_send![&*image,setImageScaling:3usize];
            let _: () = msg_send![&*image,setEditable:false];
            let _: () = msg_send![&*content,addSubview:&*image];
            let placeholder = view(
                c"NSTextField",
                rect(
                    20.,
                    6. + (height - CHROME_HEIGHT) / 2. - 13.,
                    width - 40.,
                    26.,
                ),
            )?;
            let _: () = msg_send![&*placeholder,setEditable:false];
            let _: () = msg_send![&*placeholder,setSelectable:false];
            let _: () = msg_send![&*placeholder,setBezeled:false];
            let _: () = msg_send![&*placeholder,setDrawsBackground:false];
            let _: () = msg_send![&*placeholder,setTextColor:primary];
            // NSText.h uses iOS enum values on Apple Silicon; Intel macOS differs.
            let center = if cfg!(target_arch = "x86_64") {
                2isize
            } else {
                1isize
            };
            let _: () = msg_send![&*placeholder,setAlignment:center];
            let _: () = msg_send![&*content,addSubview:&*placeholder];
            let toggle = button(
                &content,
                target,
                id,
                "隐藏预览",
                sel!(toggle:),
                rect(width - 76., height - 34., 28., 28.),
            )?;
            let _close = button(
                &content,
                target,
                id,
                "关闭预览",
                sel!(stop:),
                rect(width - 40., height - 34., 28., 28.),
            )?;
            let _: () = msg_send![&*window,setContentView:&*content];
            let primary_height = core_graphics::display::CGDisplay::main()
                .bounds()
                .size
                .height;
            let frame = rect(
                state.display.rect.x + state.display.rect.w - width - 24.,
                primary_height - state.display.rect.y - state.display.rect.h + 48.,
                width,
                height,
            );
            let _: () = msg_send![&*window,setFrame:frame,display:true,animate:false];
            let _: () = msg_send![&*window, orderFrontRegardless];
            // AppKit may re-place a newly ordered panel for the active Space.
            // Reassert the requested frame before native WindowServer readback.
            let _: () = msg_send![&*window,setFrame:frame,display:true,animate:false];
            Ok(Self {
                window,
                image,
                toggle,
                placeholder,
                source_label,
                source_pixels,
                chrome: None,
                generation: state.generation,
                placement_pending_since: Instant::now(),
                last_placement_check: None,
                placement_confirmed: false,
                placement_hidden: false,
                placement_failure: None,
            })
        }
    }
    /// No image is presented until the exact panel is confirmed on its
    /// allocated display. Continue checking after creation: Spaces/topology
    /// may move an already visible panel without changing the source window.
    pub(super) fn check_placement(
        &mut self,
        display: &Display,
        force: bool,
    ) -> std::result::Result<bool, String> {
        let now = Instant::now();
        if !force
            && self.placement_confirmed
            && self
                .last_placement_check
                .is_some_and(|at| now.duration_since(at) < Duration::from_millis(250))
        {
            return Ok(true);
        }
        self.last_placement_check = Some(now);
        let active = displays().map_err(|e| e.code)?;
        if !active.iter().any(|current| current == display) {
            return Err("live_preview_display_changed".into());
        }
        let number: isize = unsafe { msg_send![&*self.window, windowNumber] };
        let window = u32::try_from(number).map_err(|_| "live_presenter_window_unknown")?;
        let inventory = window_inventory().map_err(|e| e.code)?;
        let bounds =
            presenter_readback(&inventory, Some(window), std::process::id()).map_err(|e| e.code)?;
        let Some(bounds) = bounds else {
            self.placement_confirmed = false;
            if now.duration_since(self.placement_pending_since) >= Duration::from_millis(250) {
                self.placement_failure = Some(json!({
                    "readback":"exact_owned_CGWindow_absent_after_appearance_budget",
                    "pid":std::process::id(), "window":window, "bounds":null,
                    "requested_display":crate::catalog::DisplayRecord::from_native(display),
                    "native_rows":inventory.native_rows,"invalid_rows":inventory.invalid_rows,
                    "cause_attested":false,"input_authority":false,
                }));
                return Err("live_presenter_placement_unconfirmed".into());
            }
            return Ok(false);
        };
        let containing = crate::native::containing_display(&active, bounds);
        if containing != Some(display) {
            self.placement_failure = Some(placement_failure_evidence(
                window, bounds, display, containing,
            ));
            return Err("live_presenter_wrong_display".into());
        }
        self.placement_confirmed = true;
        Ok(true)
    }
    pub(super) fn clear(&self) {
        unsafe {
            let _: () = msg_send![&*self.image,setImage:std::ptr::null_mut::<AnyObject>()];
        }
    }
    pub(super) fn present(&self, png: &[u8]) -> std::result::Result<(), String> {
        unsafe {
            let data: *mut AnyObject = msg_send![class(c"NSData")?, dataWithBytes:png.as_ptr().cast::<std::ffi::c_void>(), length:png.len()];
            let allocated: Allocated<AnyObject> = msg_send![class(c"NSImage")?, alloc];
            let image: Option<Retained<AnyObject>> = msg_send![allocated,initWithData:data];
            let image = image.ok_or("Cannot decode preview PNG")?;
            let _: () = msg_send![&*self.image,setImage:&*image];
        }
        Ok(())
    }
}
unsafe fn button(
    content: &AnyObject,
    target: &AnyObject,
    id: u64,
    title: &str,
    action: Sel,
    frame: NSRect,
) -> std::result::Result<Retained<AnyObject>, String> {
    unsafe {
        let button = accessibility_button::new(frame)?;
        let _: () = msg_send![&*button,setBezelStyle:7usize]; // NSBezelStyleCircular, SDK.
        let font: *mut AnyObject = msg_send![class(c"NSFont")?,systemFontOfSize:12.0f64];
        let _: () = msg_send![&*button,setFont:font];
        let symbol = preview_control_icon(title);
        button_symbol(&button, symbol, title)?;
        let _: () = msg_send![&*button,setTarget:target];
        let _: () = msg_send![&*button,setAction:action];
        let _: () = msg_send![&*button,setTag:id as isize];
        let _: () = msg_send![content,addSubview:&*button];
        Ok(button)
    }
}
pub(super) fn preview_control_icon(title: &str) -> &'static str {
    match title {
        "关闭预览" => "xmark",
        "继续预览" => "play.fill",
        _ => "eye.slash",
    }
}
/// Use native symbols, not copied private Codex assets. Keep the full accessible
/// action name and tooltip when the visible control is only a small glyph.
pub(super) unsafe fn button_symbol(
    button: &AnyObject,
    symbol: &str,
    title: &str,
) -> std::result::Result<(), String> {
    unsafe {
        let name = NSString::from_str(symbol);
        let text = NSString::from_str(title);
        let image: *mut AnyObject = msg_send![class(c"NSImage")?,imageWithSystemSymbolName:&*name,accessibilityDescription:&*text];
        if image.is_null() {
            // No missing-symbol blank button. This changes presentation only,
            // never substitutes an action or target.
            let _: () = msg_send![button,setImage:std::ptr::null_mut::<AnyObject>()];
            let _: () = msg_send![button,setTitle:&*text];
            let _: () = msg_send![button,setImagePosition:0usize];
        } else {
            let _: () = msg_send![button,setImage:image];
            let _: () = msg_send![button,setTitle:&*NSString::from_str("")];
            let _: () = msg_send![button,setImagePosition:1usize]; // NSImageOnly, SDK.
        }
        let _: () = msg_send![button,setAccessibilityLabel:&*text];
        let tooltip = NSString::from_str(match title {
            "关闭预览" => "停止预览并清空画面，不关闭源窗口",
            "继续预览" => "恢复观察，不操作源窗口",
            _ => "隐藏预览并清空画面，不关闭源窗口",
        });
        let _: () = msg_send![button,setToolTip:&*tooltip];
        Ok(())
    }
}
