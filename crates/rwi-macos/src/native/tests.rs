#[test]
fn capture_consistency_diagnostics_preserve_each_rejection_dimension() {
    for bits in 1u8..8 {
        let result = super::capture_consistency_failure(
            bits & 1 != 0,
            bits & 2 != 0,
            Some(bits & 4 != 0),
            [64, 63],
            3,
        );
        if bits & 3 == 0 {
            assert!(result.is_none());
            continue;
        }
        let error = result.unwrap();
        assert_eq!(error.code, "capture_binding_changed");
        let diagnostic = error.diagnostic.unwrap();
        assert_eq!(diagnostic["binding_changed"], bits & 1 != 0);
        assert_eq!(diagnostic["blocked_changed"], bits & 2 != 0);
        assert_eq!(diagnostic["semantic_content_changed"], bits & 4 != 0);
        assert_eq!(diagnostic["attempt"], 3);
        assert_eq!(diagnostic["before_elements"], 64);
        assert_eq!(diagnostic["after_elements"], 63);
        assert_eq!(diagnostic["native_input_dispatched"], false);
        assert_eq!(diagnostic["replay_allowed"], false);
        assert_eq!(diagnostic.as_object().unwrap().len(), 9);
    }
}

#[test]
fn uncollected_semantic_comparison_keeps_window_and_modal_guards() {
    assert!(super::capture_consistency_failure(false, false, None, [0, 64], 1).is_none());
    for (binding, blocked) in [(true, false), (false, true), (true, true)] {
        let diagnostic = super::capture_consistency_failure(binding, blocked, None, [0, 64], 1)
            .unwrap()
            .diagnostic
            .unwrap();
        assert!(diagnostic["semantic_content_changed"].is_null());
        assert_eq!(diagnostic["native_input_dispatched"], false);
    }
}

#[test]
fn unchanged_capture_metadata_remains_accepted_without_diagnostic_reads() {
    assert!(super::capture_consistency_failure(false, false, Some(false), [64, 64], 1).is_none());
}

use super::*;
#[test]
fn scoped_visibility_requires_an_exact_native_hit() {
    assert_eq!(
        scoped_visibility(true, Some(false), Some(true)),
        (Some(true), Some(false))
    );
    assert_eq!(
        scoped_visibility(true, Some(false), Some(false)),
        (Some(false), Some(true))
    );
    assert_eq!(scoped_visibility(true, Some(false), None), (None, None));
    assert_eq!(scoped_visibility(true, None, Some(true)), (None, None));
    assert_eq!(
        scoped_visibility(false, Some(false), Some(true)),
        (Some(false), Some(true))
    );
    assert_eq!(
        scoped_visibility(true, Some(true), Some(true)),
        (Some(false), Some(true))
    );
}
fn display(id: u32, built_in: bool, x: f64, y: f64, w: f64, h: f64) -> Display {
    Display {
        id,
        uuid: None,
        built_in,
        rect: Rect { x, y, w, h },
        pixels: [(w as u64) * 2, (h as u64) * 2],
        rotation: 0.0,
    }
}
#[test]
fn placement_keeps_windows_inside_the_authorized_display() {
    let display = Rect {
        x: 282.0,
        y: 1152.0,
        w: 1800.0,
        h: 1169.0,
    };
    let small = Rect {
        x: 232.0,
        y: 55.0,
        w: 656.0,
        h: 422.0,
    };
    assert_eq!(placement_position(small, display), (306.0, 1176.0));
    let huge = Rect {
        x: 0.0,
        y: 0.0,
        w: 2400.0,
        h: 1500.0,
    };
    assert_eq!(placement_position(huge, display), (282.0, 1152.0));
}
#[test]
fn window_display_assignment_requires_one_unrotated_containing_display() {
    let built_in = display(1, true, 0.0, 0.0, 100.0, 80.0);
    let external = display(2, false, 100.0, 0.0, 120.0, 80.0);
    let mut rotated = display(3, false, 0.0, 80.0, 90.0, 60.0);
    rotated.rotation = 90.0;
    let displays = vec![built_in, external, rotated];
    let inside = Rect {
        x: 10.0,
        y: 10.0,
        w: 40.0,
        h: 30.0,
    };
    assert_eq!(containing_display(&displays, inside).map(|d| d.id), Some(1));
    let over_both = Rect {
        x: 80.0,
        y: 10.0,
        w: 40.0,
        h: 30.0,
    };
    assert!(containing_display(&displays, over_both).is_none());
    let on_rotated = Rect {
        x: 10.0,
        y: 90.0,
        w: 40.0,
        h: 30.0,
    };
    assert!(containing_display(&displays, on_rotated).is_none());
    let outside = Rect {
        x: 500.0,
        y: 500.0,
        w: 10.0,
        h: 10.0,
    };
    assert!(containing_display(&displays, outside).is_none());
}
#[test]
#[ignore = "Explicit live AX diagnostic: press the default button of a modal sheet on one pid/window; setup only"]
fn press_default_button_diagnostic() {
    let pid: u32 = std::env::var("RCU_DIAG_PID")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(18858);
    let want: String = std::env::var("RCU_DIAG_WINDOW").unwrap_or_else(|_| "Untitled 5".into());
    let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(pid as i32)).unwrap() };
    let focused: bool = std::env::var("RCU_DIAG_FOCUSED").is_ok_and(|v| v == "1");
    let button_title: String = std::env::var("RCU_DIAG_BUTTON").unwrap_or_else(|_| "OK".into());
    let window = if focused {
        application.attr("AXFocusedWindow").expect("focused window")
    } else {
        let value = application.attr("AXWindows").unwrap();
        let windows = unsafe { ffi::array(value.0, 64).unwrap() };
        let mut target = None;
        for w in &windows {
            if w.attr("AXTitle")
                .ok()
                .and_then(|v| v.string().ok())
                .as_deref()
                == Some(want.as_str())
            {
                target = Some(w.retained());
            }
        }
        target.expect("window not found")
    };
    fn find_button(node: &Cf, depth: usize, want: &str) -> Option<Cf> {
        if depth > 8 {
            return None;
        }
        let role = node.attr("AXRole").ok().and_then(|v| v.string().ok());
        let title = node.attr("AXTitle").ok().and_then(|v| v.string().ok());
        let default = node.attr("AXDefaultButton").ok().and_then(|v| v.boolean());
        if role.as_deref() == Some("AXButton")
            && (title.as_deref() == Some(want) || default == Some(true))
        {
            return Some(node.retained());
        }
        for child in node.children().unwrap_or_default() {
            if let Some(found) = find_button(&child, depth + 1, want) {
                return Some(found);
            }
        }
        None
    }
    let button = find_button(&window, 0, &button_title).expect("button not found");
    use core_foundation::base::TCFType;
    let action = core_foundation::string::CFString::new("AXPress");
    let code =
        unsafe { ffi::AXUIElementPerformAction(button.0, action.as_concrete_TypeRef().cast()) };
    println!("AXPress code={code}");
}
#[test]
#[ignore = "Explicit live NSWorkspace background launch of RCU_DIAG_LAUNCH_PATH; setup only"]
fn launch_background_diagnostic() {
    let path = std::env::var("RCU_DIAG_LAUNCH_PATH")
        .unwrap_or_else(|_| "/System/Applications/Calculator.app".into());
    match launch_background(&path, true) {
        Ok(()) => println!("launch_background ok path={path}"),
        Err(error) => println!("launch_background error code={} next={error}", error.code),
    }
    std::thread::sleep(std::time::Duration::from_millis(1200));
}
#[test]
#[ignore = "Explicit live background launch + placement of RCU_DIAG_LAUNCH_PATH; setup only"]
fn launch_background_and_place_diagnostic() {
    let path = std::env::var("RCU_DIAG_LAUNCH_PATH")
        .unwrap_or_else(|_| "/System/Applications/Calculator.app".into());
    let displays = displays().expect("displays");
    let target = displays
        .iter()
        .find(|display| display.built_in)
        .expect("built-in display");
    println!("target display id={} rect={:?}", target.id, target.rect);
    match launch_background_and_place(&path, target) {
        Ok(value) => println!("launch_place ok {value}"),
        Err(error) => println!("launch_place error code={} next={error}", error.code),
    }
}
#[test]
#[ignore = "Explicit live AX/CG window identity dump for one pid; read-only, no input"]
fn dump_ax_windows_for_pid_diagnostic() {
    let pid: u32 = std::env::var("RCU_DIAG_PID")
        .ok()
        .and_then(|v| v.parse().ok())
        .expect("RCU_DIAG_PID must name the explicitly allocated process");
    let application = unsafe { Cf::owned(ffi::AXUIElementCreateApplication(pid as i32)).unwrap() };
    unsafe {
        ffi::AXUIElementSetMessagingTimeout(application.0, 1.0);
    }
    for key in ["AXFocusedWindow", "AXMainWindow"] {
        if let Ok(element) = application.attr(key) {
            println!(
                "AX {key} number={:?} rect={:?} role={:?} title={:?} subrole={:?} modal={:?} position_settable={:?}",
                ffi::window_number_for_ax_element(&element),
                ax_rect(&element).ok(),
                element.attr("AXRole").ok().and_then(|v| v.string().ok()),
                element.attr("AXTitle").ok().and_then(|v| v.string().ok()),
                element.attr("AXSubrole").ok().and_then(|v| v.string().ok()),
                element.attr("AXModal").ok().and_then(|v| v.boolean()),
                element.attribute_settable("AXPosition").ok()
            );
            if key == "AXFocusedWindow"
                && let Ok(rect) = ax_rect(&element)
            {
                let point = [rect.x + rect.w / 2., rect.y + rect.h / 2.];
                if let Ok(hit) = hit_test(&application, point) {
                    println!(
                        "AX focused center hit point={point:?} hit_window={:?} hit_role={:?} child_window={:?} same_window={}",
                        ffi::window_number_for_ax_element(&hit),
                        hit.attr("AXRole").ok().and_then(|v| v.string().ok()),
                        hit.attr("AXWindow")
                            .ok()
                            .and_then(|v| ffi::window_number_for_ax_element(&v)),
                        unsafe { ffi::CFEqual(hit.0, element.0) } != 0,
                    );
                }
            }
        }
    }
    if let Ok(children_value) = application.attr("AXChildren")
        && let Ok(children) = unsafe { ffi::array(children_value.0, 32) }
    {
        for child in &children {
            println!(
                "AX app-child subrole={:?} number={:?} rect={:?} title={:?}",
                child.attr("AXSubrole").ok().and_then(|v| v.string().ok()),
                ffi::window_number_for_ax_element(child),
                ax_rect(child).ok(),
                child.attr("AXTitle").ok().and_then(|v| v.string().ok())
            );
        }
    }
    let value = application.attr("AXWindows").unwrap();
    let windows = unsafe { ffi::array(value.0, 64).unwrap() };
    for w in &windows {
        let number = ffi::window_number_for_ax_element(w);
        let rect = ax_rect(w).ok();
        let title = w.attr("AXTitle").ok().and_then(|v| v.string().ok());
        let main = w.attr("AXMain").ok().and_then(|v| v.boolean());
        let focused = w.attr("AXFocused").ok().and_then(|v| v.boolean());
        let minimized = w.attr("AXMinimized").ok().and_then(|v| v.boolean());
        let children_count = w
            .attr("AXChildren")
            .ok()
            .and_then(|value| unsafe { ffi::array(value.0, 64).ok() })
            .map(|children| children.len());
        println!(
            "AX window number={number:?} rect={rect:?} title={title:?} main={main:?} focused={focused:?} minimized={minimized:?} children_count={children_count:?} modal={:?} position_settable={:?}",
            w.attr("AXModal").ok().and_then(|v| v.boolean()),
            w.attribute_settable("AXPosition").ok()
        );
        if let Some(rect) = rect {
            let point = [rect.x + rect.w / 2.0, rect.y + rect.h / 2.0];
            if let Ok(hit) = hit_test(&application, point) {
                println!(
                    "AX hit point={point:?} pid={:?} window={:?} containing_window={:?} role={:?}",
                    hit.pid().ok(),
                    ffi::window_number_for_ax_element(&hit),
                    hit.attr("AXWindow")
                        .ok()
                        .and_then(|w| ffi::window_number_for_ax_element(&w)),
                    hit.attr("AXRole").ok().and_then(|v| v.string().ok())
                );
            }
        }
    }
    for w in &windows {
        let Ok(children_value) = w.attr("AXChildren") else {
            continue;
        };
        let Ok(children) = (unsafe { ffi::array(children_value.0, 32) }) else {
            continue;
        };
        for child in &children {
            let subrole = child.attr("AXSubrole").ok().and_then(|v| v.string().ok());
            let number = ffi::window_number_for_ax_element(child);
            let rect = ax_rect(child).ok();
            let title = child.attr("AXTitle").ok().and_then(|v| v.string().ok());
            println!(
                "AX child subrole={subrole:?} number={number:?} rect={rect:?} title={title:?}"
            );
        }
    }
    for w in window_inventory()
        .unwrap()
        .windows
        .iter()
        .filter(|w| w.pid == pid)
    {
        println!(
            "CG onscreen id={} rect=({},{},{},{}) title={:?} alpha={}",
            w.id, w.rect.x, w.rect.y, w.rect.w, w.rect.h, w.title, w.alpha
        );
    }
    unsafe {
        let array = Cf::owned(ffi::CGWindowListCopyWindowInfo(0, 0)).unwrap();
        for row in ffi::array(array.0, 2048).unwrap() {
            let owner = ffi::number(ffi::field(row.0, "kCGWindowOwnerPID")).unwrap_or(-1);
            if owner != i64::from(pid) {
                continue;
            }
            let id = ffi::number(ffi::field(row.0, "kCGWindowNumber")).unwrap_or(-1);
            let onscreen = ffi::boolean(ffi::field(row.0, "kCGWindowIsOnscreen"));
            let layer = ffi::number(ffi::field(row.0, "kCGWindowLayer")).unwrap_or(-1);
            let alpha = ffi::real(ffi::field(row.0, "kCGWindowAlpha"));
            let mut rect = core_graphics::geometry::CGRect::default();
            let has_rect = ffi::CGRectMakeWithDictionaryRepresentation(
                ffi::field(row.0, "kCGWindowBounds"),
                &mut rect,
            ) != 0;
            println!(
                "CG all id={id} onscreen={onscreen:?} layer={layer} alpha={alpha:?} rect={has_rect}:{rect:?}"
            );
        }
    }
    let capturer = crate::capture::Capturer::default();
    for id in std::env::var("RCU_DIAG_CAPTURE")
        .unwrap_or_default()
        .split(',')
        .filter_map(|v| v.parse::<u32>().ok())
    {
        let rect = window_inventory()
            .unwrap()
            .windows
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.rect);
        let Some(rect) = rect else {
            println!("capture id={id} no on-screen rect");
            continue;
        };
        match capturer.capture(pid, id, rect) {
            Ok(capture) => {
                let path = format!("/tmp/rcu-diag-window-{id}.png");
                std::fs::write(&path, &capture.png).unwrap();
                println!("capture id={id} size={:?} path={path}", capture.size);
            }
            Err(e) => println!("capture id={id} error={}", e.code),
        }
    }
}
#[test]
fn secure_text_subrole_is_rejected_for_description_readback_and_input() {
    let class = TextClass::classify("AXTextField", Some("AXSecureTextField"));
    assert_eq!(class, TextClass::Secure);
    assert!(!class.readable());
    assert!(!class.input_allowed());
    assert_eq!(
        TextClass::classify("AXTextField", Some("AXUnknown")),
        TextClass::Unknown
    );
    let unknown = TextClass::classify("AXTextField", None);
    assert!(!unknown.readable());
    assert!(!unknown.input_allowed());
    let normal = TextClass::classify("AXTextField", Some("AXSearchField"));
    assert!(normal.readable());
    assert!(normal.input_allowed());
    for role in ["AXTextField", "AXTextArea", "AXComboBox"] {
        assert_eq!(TextClass::classify(role, Some("")), TextClass::Plain);
        assert_eq!(TextClass::classify(role, None), TextClass::Unknown);
        assert_eq!(
            TextClass::classify(role, Some("AXUnknown")),
            TextClass::Unknown
        );
        assert_eq!(
            TextClass::classify(role, Some("AXSecureTextField")),
            TextClass::Secure
        );
    }
}
fn classified(class: TextClass) -> AxElement {
    AxElement {
        element: Element {
            enabled: None,
            actions: vec![],
            role: None,
            id: "id".into(),
            name: "Input".into(),
            point: [0.0, 0.0],
            visible: Some(true),
            occluded: Some(false),
            state_revision: None,
            description: None,
            selected: None,
            value: None,
            value_truncated: false,
        },
        text_class: class,
        value: Some("secret".into()),
        focus_availability: None,
    }
}
#[test]
fn secure_and_unknown_description_never_calls_ax_value_reader() {
    for class in [TextClass::Secure, TextClass::Unknown] {
        assert!(read_classified_value(class, || panic!("AXValue must not be read")).is_none());
    }
}
#[test]
fn secure_and_unknown_input_is_rejected_even_for_focused_matching_element() {
    for class in [TextClass::Secure, TextClass::Unknown] {
        assert_eq!(
            require_text_target(&classified(class), Some("id"))
                .unwrap_err()
                .code,
            "text_target_not_focused"
        );
    }
}
#[test]
fn secure_text_cannot_be_bypassed_using_pointer_or_key_chord() {
    for class in [TextClass::Secure, TextClass::Unknown] {
        assert_eq!(
            require_nonsecure_input(&classified(class))
                .unwrap_err()
                .code,
            "secure_or_unknown_text_field"
        );
    }
}
#[test]
fn secure_and_unknown_verification_is_not_success_even_if_equal() {
    for class in [TextClass::Secure, TextClass::Unknown] {
        assert_eq!(
            verify_value(&classified(class), "secret").unwrap_err().code,
            "secure_or_unknown_text_field"
        );
    }
}
#[test]
fn physical_key_aliases_are_explicit() {
    assert_eq!(parse_chord("Shift+Minus").unwrap().0, 27);
    assert_eq!(parse_chord("Return").unwrap().0, 36);
    assert!(parse_chord("Cmd+A+B").is_err());
    assert!(parse_chord("cmd+made-up").is_err());
}
#[test]
fn unicode_chunks_never_split_surrogate_pairs() {
    let text = "\t中文🙂\n  print(\"a b\")🙂 ";
    let chunks = utf16_chunks(text, 5);
    assert!(
        chunks
            .iter()
            .all(|c| c.len() <= 5 && String::from_utf16(c).is_ok())
    );
    assert_eq!(String::from_utf16(&chunks.concat()).unwrap(), text);
}

#[test]
fn unnamed_standard_window_controls_get_exact_subrole_names() {
    for (subrole, expected) in [
        ("AXCloseButton", "Close window"),
        ("AXMinimizeButton", "Minimize window"),
        ("AXZoomButton", "Zoom window"),
        ("AXFullScreenButton", "Toggle full screen"),
    ] {
        assert_eq!(
            window_control_name("AXButton", Some(subrole), String::new()),
            expected
        );
    }
}
#[test]
fn window_control_names_do_not_override_provider_or_guess_unknown_roles() {
    assert_eq!(
        window_control_name("AXButton", Some("AXCloseButton"), "Provider title".into()),
        "Provider title"
    );
    for subrole in [None, Some(""), Some("AXUnknown"), Some("AXGenericButton")] {
        assert_eq!(window_control_name("AXButton", subrole, String::new()), "");
    }
    assert_eq!(
        window_control_name("AXTextField", Some("AXCloseButton"), String::new()),
        ""
    );
    assert_eq!(
        window_control_name("AXButton", Some("AXSecureTextField"), String::new()),
        ""
    );
}

#[test]
fn window_local_input_mapping_preserves_visible_identity_without_full_monitor_containment() {
    let screens = vec![display(4, false, 1800., 0., 1920., 1080.)];
    let own = Rect {
        x: 3225.,
        y: 504.,
        w: 660.,
        h: 552.,
    };
    assert!(containing_display(&screens, own).is_none());
    assert!(
        window_mapping_ready(&screens, own),
        "full-window SCK/AX point geometry is known; single-display assignment is descriptive"
    );
    assert!(window_mapping_ready(&screens, Rect { x: 1824., ..own }));
    assert!(!window_mapping_ready(&screens, Rect { x: 4000., ..own }));
    assert!(!window_mapping_ready(&[], own));
    assert!(!window_mapping_ready(&screens, Rect { w: f64::NAN, ..own }));
}

#[test]
fn window_local_input_mapping_supports_spanning_but_not_invalid_or_rotated_intersections() {
    let mut screens = vec![
        display(1, true, 0., 0., 100., 80.),
        display(2, false, 100., 0., 120., 80.),
    ];
    let cross = Rect {
        x: 80.,
        y: 10.,
        w: 40.,
        h: 30.,
    };
    assert!(window_mapping_ready(&screens, cross));
    screens[1].rotation = 90.;
    assert!(!window_mapping_ready(&screens, cross));
    screens[1].rotation = 0.;
    screens[1].rect.w = f64::NAN;
    assert!(!window_mapping_ready(&screens, cross));
    assert!(!window_mapping_ready(&[], cross));
    assert!(!window_mapping_ready(
        &[display(4, false, 0., 0., 100., 80.)],
        Rect {
            x: f64::MAX,
            w: f64::MAX,
            ..cross
        }
    ));
}
