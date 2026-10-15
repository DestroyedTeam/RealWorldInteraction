use super::*;

#[test]
fn r92_exact_desktop_selection_advertises_the_pinned_native_click_shape() {
    for input_enabled in [false, true] {
        let mut caps = json!({"window_selected":true,"pointer_move":{"sentinel":"unchanged"}});
        apply_selected_click_capabilities(&mut caps, input_enabled, Some(false));
        assert_eq!(
            caps["pointer_click"],
            NativeMac::pointer_click_capabilities(input_enabled, false)
        );
        assert_eq!(caps["pointer_click"]["implemented"], true);
        assert_eq!(
            caps["pointer_click"]["default_delivery"],
            "background_window"
        );
        let route = &caps["pointer_click"]["background_skylight_experimental"];
        assert_eq!(
            route["available"],
            input_enabled && crate::background::skylight::available()
        );
        assert_eq!(route["fallback"], false);
        assert_eq!(route["activation_requested"], false);
        assert_eq!(route["application_effect_attested"], false);
        assert_eq!(caps["pointer_move"]["sentinel"], "unchanged");
    }
}

#[test]
fn r92_semantic_only_desktop_selection_never_advertises_synthetic_click_availability() {
    for input_enabled in [false, true] {
        let mut caps = json!({"window_selected":true,"semantic_only":true});
        apply_selected_click_capabilities(&mut caps, input_enabled, Some(true));
        assert_eq!(
            caps["pointer_click"],
            NativeMac::pointer_click_capabilities(input_enabled, true)
        );
        let route = &caps["pointer_click"]["background_skylight_experimental"];
        assert_eq!(route["implemented"], true);
        assert_eq!(route["available"], false);
        assert_eq!(route["fallback"], false);
    }
}

#[test]
fn r92_unselected_desktop_cannot_borrow_click_authority_from_pointer_move() {
    let mut caps = json!({"window_selected":false,"pointer_move":{"background_skylight_experimental":{"available":true}}});
    apply_selected_click_capabilities(&mut caps, true, None);
    assert!(caps.get("pointer_click").is_none());
    assert_eq!(caps["window_selected"], false);
    assert_eq!(
        caps["pointer_move"]["background_skylight_experimental"]["available"],
        true
    );
}
