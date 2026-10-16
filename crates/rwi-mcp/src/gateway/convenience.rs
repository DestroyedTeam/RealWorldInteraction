//! Typed, platform-neutral convenience calls. No JS and no native input replay.
use super::*;
use schemars::JsonSchema;
use serde::Deserialize;
use std::{future::Future, time::Instant};

mod bring_to_front;
mod click;
mod controls;
mod dispatch;
mod drag_between;
mod focus_element;
mod inputs;
mod schema;
mod set_text;
mod snapshots;
mod window_local;
use bring_to_front::*;
use click::*;
use controls::*;
#[cfg(test)]
use dispatch::CallTimings;
use drag_between::*;
use focus_element::*;
use inputs::*;
use schema::*;
use set_text::*;
use snapshots::*;
use window_local::*;

pub(super) fn tools() -> Vec<Tool> {
    vec![
        focus_element_tool(),
        set_text_tool(),
        window_local_tool(),
        tool::<BringToFrontArgs>(
            "computer.bring_to_front",
            "Explicitly bring the already selected native window to foreground. Supply target and stable request_id; fresh observation, exact current foreground tuple, window reference, short lease and release are handled internally. Uses the existing computer.focus_window guards; unknown foreground or unsupported device refuses. No discovery, keys, mouse, unhide/restore, background fallback or retry. Receipt is not goal proof; inspect a fresh snapshot afterward.",
        ),
        tool::<SnapshotArgs>(
            "computer.snapshot",
            "Inspect selected window: compact controls and screenshot in one call. Default max_edge=2048 for text detail; use computer.image with an original-pixel crop for a readable tile. AX-incompatible macOS windows may return visual_only=true with image but no controls, input binding, or resolvable view_id; computer.image remains read-only. Semantic-only dialogs have no image or pixel mapping. Terminal, released AXWindows read errors may include a separate exact-current-bound-window visual read; the AX error stays explicit and no frame, view or input authority is created. Full metadata via computer.result. No JS, activation or permission grant.",
        ),
        tool::<ControlsArgs>(
            "computer.controls",
            "Find or page compact controls from an existing snapshot: observation_ref + frame, optional query and offset. No JS, JSON pointers, device RPC, screenshot or input. Returns original element IDs for computer.click. Preserves ambiguity and unknown states; does not refresh frame lifetime. Use next_offset to continue.",
        ),
        tool::<ClickArgs>(
            "computer.click",
            "Click one exact snapshot element or delivered-view coordinate. Stable request_id; click_count=1 (default) or 2 for native double-click. Optional delivery={delivery:background_skylight_experimental} is macOS-only, single pointer click, no AX action; requires the exact selected target's pointer_click route and executor support. Missing capability or older peer refuses, never downgrades. Omit delivery for unchanged native default. red_marker_cluster=true is only for a visible compact red-dot cluster; default false is strict. Receipt/requested_delivery is not goal or delivery proof. No JS, retry, activation or fallback; observe afterward.",
        ),
        tool::<HoverArgs>(
            "computer.hover",
            "Send one checked mouse-move to an exact snapshot element or delivered-view coordinate; no button or AX action. Omit options for unchanged addressed native default. Independent background_cooperative_tracking_experimental uses {phase,owner_endpoint,registration_id}: a live cooperative owner authenticated by native socket PID/process birth/audit token; the owner dispatches its own registered-tag Enter/Exit synchronously on its UI thread via public NSApplication, never a caller pointer/tag or trusted witness. Ordinary third-party/modern cold tracking remains unsupported by this route; no activation, automatic fallback or input retry. Explicit background_appkit_tracking_experimental instead constructs Entered/Exited (8/9) with phase and tracking_number: a canonical nonzero signed i64 decimal STRING from a currently registered public legacy addTrackingRect tag bound to this exact process/window; NULL userData, never guessed modern area/foreign ObjC pointer or an automatic fallback. Caller must establish current tag owner/lifetime independently; scalar parsing and the lease do not attest registration. This carrier is currently unavailable for Native input until authenticated owner registration/lifetime support is implemented; it is not generic cold hover or tooltip success. Optional options={delivery,delta}: experimental background routes require selected-device capability; delta only for background_appkit_experimental, a nonzero integer pair within +/-128, not history. An explicit foreground_system requires expected_foreground and sends a checked system-cursor event. Unsupported Windows/older peer refuses; no downgrade. Stable request_id; short lease/release inside one call. No JS, dwell, activation, automatic global fallback or retry. Motion history unknown. Receipt is not tooltip success; inspect app state afterward.",
        ),
        tool::<RevealArgs>(
            "computer.reveal",
            "Ask one exact observed AX element to make itself visible using its advertised AXScrollToVisible action. This is an explicit native UI action (it may scroll the selected app's content), not a click, focus, activation, or pointer fallback. The device must advertise AXScrollToVisible and the element must advertise that exact action; otherwise no lease/input is issued. Supply the original frame/element and a stable request_id. One durable action only; no retry. Returns a dispatch receipt, not proof of visibility; inspect a fresh computer.snapshot afterward.",
        ),
        tool::<TextArgs>(
            "computer.type_text",
            "Type Unicode into one exact currently focused editable element (target/frame/element). Stable request_id; text 1..4096 UTF-8 bytes. Uses native checked text input, not setValue or clipboard. App selection determines insertion/replacement. No auto-focus, secure-field bypass, global keyboard fallback or retry; snapshot afterward.",
        ),
        tool::<KeyArgs>(
            "computer.press_key",
            "Send one key or shortcut to the exact currently focused native element (target/frame/element). Stable request_id; key e.g. escape or cmd+s on macOS, ctrl+s on Windows. Uses existing platform chord parser; unsupported keys are rejected. No auto-focus, shell, global fallback or retry; snapshot afterward.",
        ),
        tool::<DragArgs>(
            "computer.drag",
            "Drag from/to pixels in one delivered view_id; native mapping and short lease handled internally. No JS or desktop-coordinate arithmetic. duration_ms 16..5000, default 500. Stable request_id required. No replay or activation; inspect computer.snapshot afterward.",
        ),
        tool::<DragBetweenArgs>(
            "computer.drag_between",
            "Explicit native drag between two independently selected windows on one device/desktop. source/destination each use their own target, delivered view_id and pixel point. Short shared lease, endpoint guards and paired release are handled internally. macOS only when paired_pointer_drag is advertised; no ordinary-drag widening, activation/global-pointer fallback, clipboard insertion or retry. Stable request_id; duration_ms 16..5000. Receipt is not a drop goal; independently inspect source and destination.",
        ),
        tool::<ScrollArgs>(
            "computer.scroll",
            "Scroll once at one exact snapshot element or delivered-view coordinate. delta_y is signed native vertical units: positive up/negative down; macOS pixels +/-2000, Windows wheel units multiples of 120 +/-1200; nonzero. This is not a cross-platform pixel conversion. Short lease, native guards and release handled internally, no JS, activation, fallback, replay or automatic screenshot. Read back application state afterward.",
        ),
        tool::<PlaceWindowArgs>(
            "computer.place_window",
            "Move the already selected window to one explicit display (prefer macOS UUID). Fresh observation, window reference, short lease and release are handled in one call, avoiding model-side timing gaps. Supply stable request_id. Uses computer.move_window capability, returns native placement confirmation and durable receipt. Does not bind or activate another window, resize it, send keyboard/mouse input or replay a move.",
        ),
    ]
}
pub(super) fn handles(name: &str) -> bool {
    matches!(
        name,
        "computer.snapshot"
            | "computer.focus_element"
            | "computer.set_text"
            | "computer.select_window_local"
            | "computer.click"
            | "computer.hover"
            | "computer.reveal"
            | "computer.drag"
            | "computer.drag_between"
            | "computer.scroll"
            | "computer.controls"
            | "computer.type_text"
            | "computer.press_key"
            | "computer.place_window"
            | "computer.bring_to_front"
    )
}
fn invalid(message: &str) -> DeviceResult {
    DeviceResult::error("invalid_arguments", message)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod r20_degraded_observation_tests {
    use super::*;
    fn observed() -> DeviceResult {
        let mut r = DeviceResult::error("ax_attribute_unavailable", "AXWindows cannotComplete");
        r.data["diagnostic"] = json!({"api":"AXUIElementCopyAttributeValue","attribute":"AXWindows","ax_error":-25204,"operation_kind":"read"});
        r.data["native_dispatch_entered"] = json!(false);
        r.data["operation"] = json!({"terminal":true,"native_call_pending":false,"resources_released":true,"resource_release_uncertain":false});
        r
    }
    fn visual() -> DeviceResult {
        DeviceResult {
            is_error: false,
            data: json!({"frame_created":false,"input_authority_granted":false,"native_dispatch_entered":false,"operation":{"terminal":true,"native_call_pending":false,"resources_released":true,"resource_release_uncertain":false},"capture":{"window":{"window":"cgwindow:77","pid":7}}}),
            images: vec![rwi_core::device::DeviceImage {
                mime: "image/png".into(),
                base64: "fixture-only".into(),
            }],
        }
    }
    #[test]
    fn exact_terminal_released_ax_read_is_the_only_candidate() {
        assert!(read_only_ax_visual_candidate(&observed()));
        for (ptr, value) in [
            ("/code", json!("screen_capture_permission_missing")),
            ("/diagnostic/api", json!("AXUIElementPerformAction")),
            ("/diagnostic/attribute", json!("AXValue")),
            ("/diagnostic/ax_error", json!(-25205)),
            ("/diagnostic/operation_kind", json!("input")),
            ("/native_dispatch_entered", json!(true)),
            ("/operation/terminal", json!(false)),
            ("/operation/native_call_pending", json!(true)),
            ("/operation/resources_released", json!(false)),
            ("/operation/resource_release_uncertain", json!(true)),
        ] {
            let mut r = observed();
            *r.data.pointer_mut(ptr).unwrap() = value;
            assert!(!read_only_ax_visual_candidate(&r), "unsafe {ptr}");
        }
        let mut r = observed();
        r.is_error = false;
        assert!(!read_only_ax_visual_candidate(&r));
    }
    #[test]
    fn missing_cleanup_facts_do_not_become_false_or_authorized() {
        for name in [
            "terminal",
            "native_call_pending",
            "resources_released",
            "resource_release_uncertain",
        ] {
            let mut r = observed();
            r.data["operation"].as_object_mut().unwrap().remove(name);
            assert!(!read_only_ax_visual_candidate(&r));
        }
    }
    #[test]
    fn good_visual_still_keeps_ax_failure_unknown_controls_and_no_authority() {
        let old = observed();
        let data = old.data.clone();
        let r = degraded_ax_snapshot("same-target", old, visual());
        assert!(r.is_error);
        assert_eq!(r.data["observation_error"], data);
        assert_eq!(r.images.len(), 1);
        assert_eq!(r.data["visual_available"], true);
        assert!(
            r.data["frame"].is_null()
                && r.data["view_id"].is_null()
                && r.data["available_controls"].is_null()
        );
        for key in [
            "input_reference_available",
            "input_authority_granted",
            "mutation_ready",
            "native_complete",
            "goal_verified",
            "input_replayed",
        ] {
            assert_eq!(r.data[key], false);
        }
        assert_eq!(r.data["coverage"]["absence_proven"], false);
    }
    #[test]
    fn failed_or_unsettled_visual_cannot_leak_even_attached_media() {
        for (ptr, value) in [
            ("/native_dispatch_entered", json!(true)),
            ("/operation/native_call_pending", json!(true)),
            ("/operation/resources_released", json!(false)),
            ("/operation/resource_release_uncertain", json!(true)),
            ("/frame_created", json!(true)),
            ("/input_authority_granted", json!(true)),
        ] {
            let mut v = visual();
            *v.data.pointer_mut(ptr).unwrap() = value;
            let r = degraded_ax_snapshot("t", observed(), v);
            assert!(r.images.is_empty(), "bad {ptr}");
            assert_eq!(r.data["visual_available"], false);
        }
        let mut v = visual();
        v.is_error = true;
        let r = degraded_ax_snapshot("t", observed(), v);
        assert!(r.images.is_empty());
    }
}
