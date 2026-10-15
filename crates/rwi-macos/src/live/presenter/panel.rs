//! Main-thread panel construction and symbols; no input dispatch/activation.
use super::*;

impl Panel {
    pub(super) fn new(
        id: u64,
        state: &State,
        target: &AnyObject,
    ) -> std::result::Result<Self, String> {
        if state.options.host_image_only {
            return Err("host_image_native_panel_forbidden".into());
        }
        let display = state
            .display
            .as_ref()
            .ok_or("native_preview_display_missing")?;
        unsafe {
            let source_pixels = source_pixels(state);
            let (width, height) = panel_size(
                state.options.max_edge,
                display.rect.w,
                display.rect.h,
                source_pixels,
            );
            let layout = preview_layout(width, height).ok_or("live_preview_layout_unsupported")?;
            let allocated: Allocated<AnyObject> = msg_send![class(c"NSPanel")?, alloc];
            let window: Retained<AnyObject> = msg_send![allocated,
                initWithContentRect: rect(0.,0.,width,height), styleMask:128usize, backing:2usize, defer:false];
            let _: () = msg_send![&*window, setReleasedWhenClosed:false];
            // A live/privacy control must not leave a shrinking/fading window
            // server surface after orderOut has acknowledged it as hidden.
            // NSWindowAnimationBehaviorNone, from the AppKit SDK.
            let _: () = msg_send![&*window, setAnimationBehavior:2isize];
            // Nil clears only this owned panel's override. AppKit inherits the
            // app/system effective appearance; semantic colors/materials remain native.
            let _: () = msg_send![&*window, setAppearance:std::ptr::null_mut::<AnyObject>()];
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
            let title = NSString::from_str(&preview_window_title(&state.options.label));
            let _: () = msg_send![&*window, setTitle:&*title];
            let clear: *mut AnyObject = msg_send![class(c"NSColor")?, clearColor];
            let _: () = msg_send![&*window, setBackgroundColor:clear];
            // Keep proportional-fit body gutters transparent. Only the compact
            // header uses the native material and accessibility/contrast settings.
            let content = view(c"NSView", rect(0., 0., width, height))?;
            let _: () = msg_send![&*content, setWantsLayer:true];
            let layer: *mut AnyObject = msg_send![&*content, layer];
            let clear_cg: *mut AnyObject = msg_send![clear, CGColor];
            let _: () = msg_send![layer, setBackgroundColor:clear_cg];
            let _: () = msg_send![layer, setCornerRadius:12.0f64];
            let _: () = msg_send![layer, setMasksToBounds:true];
            let header = view(c"NSVisualEffectView", layout.chrome_background)?;
            let _: () = msg_send![&*header, setMaterial:6isize]; // Popover
            let _: () = msg_send![&*header, setBlendingMode:0isize]; // BehindWindow
            let _: () = msg_send![&*header, setState:1isize]; // Active
            let _: () = msg_send![&*content,addSubview:&*header];
            let label = view(c"NSTextField", layout.title)?;
            let heading = NSString::from_str(PREVIEW_SHORT_HEADING);
            let _: () = msg_send![&*label,setStringValue:&*heading];
            let _: () = msg_send![&*label,setEditable:false];
            let _: () = msg_send![&*label,setSelectable:false];
            let _: () = msg_send![&*label,setBezeled:false];
            let _: () = msg_send![&*label,setDrawsBackground:false];
            let primary: *mut AnyObject = msg_send![class(c"NSColor")?, labelColor];
            let _: () = msg_send![&*label,setTextColor:primary];
            let title_font: *mut AnyObject = msg_send![class(c"NSFont")?,systemFontOfSize:12.0f64];
            let _: () = msg_send![&*label,setFont:title_font];
            let _: () = msg_send![&*label,setToolTip:&*title];
            // Keep the complete normalized task title accessible, not the compact glyph-width heading.
            let _: () = msg_send![&*label,setAccessibilityLabel:&*title];
            let _: () = msg_send![&*content,addSubview:&*label];
            let source_label = view(c"NSTextField", layout.source)?;
            let _: () = msg_send![&*source_label,setEditable:false];
            let _: () = msg_send![&*source_label,setSelectable:false];
            let _: () = msg_send![&*source_label,setBezeled:false];
            let _: () = msg_send![&*source_label,setDrawsBackground:false];
            // Display-only ellipsis: preserve the full provided StringValue, tooltip
            // and native AX value. A missing cosmetic cell never stops the Live session.
            let source_cell: *mut AnyObject = msg_send![&*source_label, cell];
            if !source_cell.is_null() {
                let _: () = msg_send![source_cell,setUsesSingleLineMode:true];
                let _: () = msg_send![source_cell,setLineBreakMode:SOURCE_CAPTION_LINE_BREAK_MODE];
            }
            let secondary: *mut AnyObject = msg_send![class(c"NSColor")?, secondaryLabelColor];
            let font: *mut AnyObject = msg_send![class(c"NSFont")?,systemFontOfSize:10.0f64];
            let _: () = msg_send![&*source_label,setTextColor:secondary];
            let _: () = msg_send![&*source_label,setFont:font];
            let _: () = msg_send![&*content,addSubview:&*source_label];
            let image = view(c"NSImageView", layout.image)?;
            let _: () = msg_send![&*image,setImageScaling:3usize];
            let _: () = msg_send![&*image,setEditable:false];
            let _: () = msg_send![&*content,addSubview:&*image];
            let placeholder = view(c"NSTextField", layout.placeholder)?;
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
                layout.toggle,
            )?;
            let _close = button(&content, target, id, "关闭预览", sel!(stop:), layout.close)?;
            let _: () = msg_send![&*window,setContentView:&*content];
            let primary_height = core_graphics::display::CGDisplay::main()
                .bounds()
                .size
                .height;
            let frame = rect(
                display.rect.x + display.rect.w - width - 24.,
                primary_height - display.rect.y - display.rect.h + 48.,
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

/// Short visible product heading; the complete normalized label stays in window title/tooltip.
const PREVIEW_HEADING: &str = "RWI Live";
// The 160pt portrait preview leaves a 40pt text column; intentionally show just
// the product initials instead of letting "RWI Live" look accidentally clipped.
const PREVIEW_SHORT_HEADING: &str = "RWI";
// Public NSParagraphStyle.h: NSLineBreakByTruncatingTail, NSUInteger enum.
const SOURCE_CAPTION_LINE_BREAK_MODE: usize = 4;
fn preview_window_title(label: &str) -> String {
    let mut scope = label.trim();
    // Strip only anchored product prefixes, never a meaningful occurrence inside a label.
    while let Some(rest) = scope
        .strip_prefix("RWI Live ·")
        .or_else(|| scope.strip_prefix("RWI ·"))
    {
        scope = rest.trim_start();
    }
    if scope.is_empty() || scope == "RWI Live" || scope == "RWI" {
        PREVIEW_HEADING.into()
    } else {
        format!("{PREVIEW_HEADING} · {scope}")
    }
}
#[cfg(test)]
mod appearance_title_tests {
    use super::*;
    #[test]
    fn live_title_has_one_product_prefix_and_keeps_the_full_scope_label() {
        for (label, expected) in [
            ("RWI Live", "RWI Live"),
            ("RWI", "RWI Live"),
            ("RWI · 240pt控件验收", "RWI Live · 240pt控件验收"),
            ("RWI Live · RWI · 任务", "RWI Live · 任务"),
            ("RWI Live · RWI · RWI Live · 任务", "RWI Live · 任务"),
            ("我的RWI · 项目", "RWI Live · 我的RWI · 项目"),
            ("RWI · ", "RWI Live"),
        ] {
            assert_eq!(preview_window_title(label), expected);
        }
    }
    #[test]
    fn compact_heading_is_constant_and_window_title_keeps_non_prefix_information() {
        assert_eq!(PREVIEW_HEADING, "RWI Live");
        let label = "RWI · 非常长的完整任务标签 · 不应被截断丢弃";
        assert_eq!(
            preview_window_title(label),
            "RWI Live · 非常长的完整任务标签 · 不应被截断丢弃"
        );
    }
    #[test]
    fn live_header_caption_short_text_keeps_complete_task_title() {
        assert_eq!(PREVIEW_SHORT_HEADING, "RWI");
        assert_eq!(PREVIEW_HEADING, "RWI Live");
        assert_eq!(SOURCE_CAPTION_LINE_BREAK_MODE, 4);
        let scope = "非常长的完整任务标签 · 来源与emoji🙂都不能从完整标题丢失";
        assert_eq!(
            preview_window_title(&format!("RWI · {scope}")),
            format!("RWI Live · {scope}")
        );
    }
    #[test]
    fn live_header_caption_keeps_portrait_geometry_and_44pt_controls() {
        let layout = preview_layout(160., 220.).unwrap();
        assert_eq!(layout.title.size.width, 40.);
        assert_eq!(layout.source.size.width, 40.);
        assert_eq!(
            (layout.toggle.size.width, layout.toggle.size.height),
            (44., 44.)
        );
        assert_eq!(
            (layout.close.size.width, layout.close.size.height),
            (44., 44.)
        );
        assert_eq!(layout.image, rect(8., 2., 144., 172.));
        assert_eq!(preview_control_icon("隐藏预览"), "eye.slash");
        assert_eq!(preview_control_icon("继续预览"), "play.fill");
        assert_eq!(preview_control_icon("关闭预览"), "xmark");
    }
}

/// Actual view/hit frames, not a nearest-button/fuzzy hit-test workaround.
struct PreviewLayout {
    chrome_background: NSRect,
    title: NSRect,
    source: NSRect,
    image: NSRect,
    placeholder: NSRect,
    toggle: NSRect,
    close: NSRect,
}
fn preview_layout(width: f64, height: f64) -> Option<PreviewLayout> {
    if !width.is_finite() || !height.is_finite() || width < 160. || height <= CHROME_HEIGHT {
        return None;
    }
    let image_height = height - CHROME_HEIGHT;
    let placeholder_height = image_height.min(26.);
    Some(PreviewLayout {
        chrome_background: rect(0., height - CHROME_HEIGHT, width, CHROME_HEIGHT),
        title: rect(12., height - 28., width - 120., 18.),
        source: rect(12., height - 42., width - 120., 13.),
        image: rect(8., 2., width - 16., image_height),
        placeholder: rect(
            20.,
            2. + (image_height - placeholder_height) / 2.,
            width - 40.,
            placeholder_height,
        ),
        toggle: rect(width - 100., height - 46., 44., 44.),
        close: rect(width - 52., height - 46., 44., 44.),
    })
}
#[cfg(test)]
mod hit_region_tests {
    use super::*;
    fn right(r: NSRect) -> f64 {
        r.origin.x + r.size.width
    }
    fn top(r: NSRect) -> f64 {
        r.origin.y + r.size.height
    }
    fn overlaps(a: NSRect, b: NSRect) -> bool {
        a.origin.x < right(b) && b.origin.x < right(a) && a.origin.y < top(b) && b.origin.y < top(a)
    }
    #[test]
    fn live44_actual_frames_keep_gap_text_image_and_placeholder_disjoint() {
        for width in [160., 200., 240., 280.] {
            for height in [49., 100., 180., 300.] {
                let l = preview_layout(width, height).unwrap();
                assert_eq!(
                    l.chrome_background,
                    rect(0., height - CHROME_HEIGHT, width, CHROME_HEIGHT)
                );
                assert_eq!(l.toggle.size, NSSize::new(44., 44.));
                assert_eq!(l.close.size, NSSize::new(44., 44.));
                assert_eq!(l.close.origin.x - right(l.toggle), 4.);
                assert_eq!(height - top(l.close), 2.);
                for hit in [l.toggle, l.close] {
                    assert!(hit.origin.x >= 0. && hit.origin.y >= 0.);
                    assert!(right(hit) <= width && top(hit) <= height);
                    assert!(hit.origin.y >= l.chrome_background.origin.y);
                    for other in [l.title, l.source, l.image, l.placeholder] {
                        assert!(!overlaps(hit, other));
                    }
                }
                assert!(!overlaps(l.toggle, l.close));
                assert!(right(l.title) < l.toggle.origin.x);
                assert!(right(l.source) < l.toggle.origin.x);
                assert!(l.placeholder.origin.y >= l.image.origin.y);
                assert!(top(l.placeholder) <= top(l.image));
            }
        }
    }
    #[test]
    fn live44_rejects_nonfinite_and_unusable_frames_without_native_objects() {
        for (w, h) in [
            (f64::NAN, 180.),
            (280., f64::INFINITY),
            (159., 180.),
            (280., 48.),
            (0., 0.),
        ] {
            assert!(preview_layout(w, h).is_none());
        }
    }
    #[test]
    fn unusable_display_geometry_is_rejected_before_panel_allocation() {
        for [width, height] in [
            [207., 316.],
            [500., 144.],
            [f64::NAN, 1080.],
            [1920., f64::INFINITY],
        ] {
            let (width, height) = panel_size(2048, width, height, Some([1600, 1920]));
            assert!(preview_layout(width, height).is_none());
        }
    }
    #[test]
    fn live44_naive_expansion_of_old_centers_would_overlap() {
        assert!(overlaps(
            rect(196., 138., 44., 44.),
            rect(232., 138., 44., 44.)
        ));
    }
}

#[cfg(test)]
#[test]
fn f39_smaller_preview_preserves_accessible_hit_frames_and_actions() {
    for (width, height) in [(160., 200.), (197.7391304347826, 200.), (200., 132.6328125)] {
        let layout = preview_layout(width, height).unwrap();
        assert_eq!(layout.toggle.size, NSSize::new(44., 44.));
        assert_eq!(layout.close.size, NSSize::new(44., 44.));
        assert_eq!(layout.close.origin.x - layout.toggle.origin.x - 44., 4.);
        assert!(layout.title.origin.x + layout.title.size.width < layout.toggle.origin.x);
        assert!(layout.image.origin.y + layout.image.size.height <= layout.toggle.origin.y);
    }
    assert_eq!(preview_control_icon("隐藏预览"), "eye.slash");
    assert_eq!(preview_control_icon("继续预览"), "play.fill");
    assert_eq!(preview_control_icon("关闭预览"), "xmark");
    println!(
        "F39_CONTROLS hide=eye.slash resume=play.fill close=xmark hit=44x44 gap=4 input_authority_unchanged"
    );
}
