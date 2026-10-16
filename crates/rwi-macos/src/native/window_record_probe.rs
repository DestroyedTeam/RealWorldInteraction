//! Advisory exact-ID WindowServer read after an onscreen lookup failed.
//! Never supplies a Window, input reference, pixels, restoration or replay.
use super::*;
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    boolean::CFBoolean,
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};
use serde_json::json;

pub(super) struct Record {
    pid: u32,
    window: u32,
    onscreen: Option<bool>,
}

pub(super) fn read(window: u32) -> Result<Option<Record>> {
    let _call = operation::native_call("macos_exact_missing_window_description")?;
    // The SDK encodes these IDs in pointer-sized slots, with no CF callbacks.
    // A packed u32 slice would have the wrong stride on 64-bit macOS.
    let ids = CFArray::from_copyable(&[window as usize]);
    let raw = unsafe {
        core_graphics::window::CGWindowListCreateDescriptionFromArray(ids.as_concrete_TypeRef())
    };
    if raw.is_null() {
        return Err(fault(
            "window_record_probe_unavailable",
            "Exact native description returned no array.",
        ));
    }
    let rows: CFArray<CFDictionary<CFString, CFType>> =
        unsafe { TCFType::wrap_under_create_rule(raw) };
    if rows.is_empty() {
        return Ok(None);
    }
    if rows.len() != 1 {
        return Err(fault(
            "window_record_probe_ambiguous",
            "Exact native description did not return one row.",
        ));
    }
    let row = rows.get(0).expect("one native description row");
    let number = |name: &str| -> Option<u32> {
        row.find(CFString::new(name))
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|v| v.to_i64())
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| *v != 0)
    };
    let (Some(pid), Some(id)) = (number("kCGWindowOwnerPID"), number("kCGWindowNumber")) else {
        return Err(fault(
            "window_record_probe_invalid",
            "Native owner/window identifiers are unavailable.",
        ));
    };
    // This optional key can be omitted for an offscreen description. Missing
    // is unknown, not false; no inference about hidden/minimized state is made.
    let onscreen = row
        .find(CFString::new("kCGWindowIsOnscreen"))
        .and_then(|v| v.downcast::<CFBoolean>())
        .map(bool::from);
    Ok(Some(Record {
        pid,
        window: id,
        onscreen,
    }))
}

pub(super) fn missing(pid: u32, window: u32, probe: Result<Option<Record>>) -> Fault {
    let mut diagnostic = json!({
        "scope":"exact_id_read_after_onscreen_miss", "expected_pid":pid,
        "expected_window":window, "on_screen_inventory_match":false,
        "native_record_present":null, "owner_matches":null, "native_onscreen":null,
        "destroyed":null, "hidden":null, "absence_proven":false,
        "non_atomic":true, "input_authority":false, "replay_allowed":false
    });
    let message = match probe {
        Ok(Some(record)) if record.pid == pid && record.window == window => {
            diagnostic["native_record_present"] = json!(true);
            diagnostic["owner_matches"] = json!(true);
            diagnostic["native_onscreen"] = json!(record.onscreen);
            "The exact ID/PID still has a native record but was absent from the onscreen lookup. Hidden/offscreen state is not fully classified; independently refresh discovery or explicitly request supported focus/placement. No input fallback or replay."
        }
        Ok(Some(_)) => {
            diagnostic["owner_matches"] = json!(false);
            "The onscreen target is absent and the exact-ID description does not match its owner. Re-discover without substituting another window or replaying input."
        }
        Ok(None) => {
            diagnostic["native_record_present"] = json!(false);
            "No onscreen target or current description for the selected ID was found. This is not continuous proof of window destruction; re-discover before new input, never replay."
        }
        Err(error) => {
            diagnostic["probe_error"] = json!(error);
            "The onscreen target is absent and its exact native record is unknown. Preserve the read failure; do not assume destruction, substitute input, or replay."
        }
    };
    let mut error = fault("window_missing", message);
    error.diagnostic = Some(Box::new(diagnostic));
    error
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assert_no_authority(error: &Fault) {
        assert_eq!(error.code, "window_missing");
        let d = error.diagnostic.as_ref().unwrap();
        assert_eq!(d["input_authority"], false);
        assert_eq!(d["replay_allowed"], false);
        assert_eq!(d["absence_proven"], false);
        assert!(d["destroyed"].is_null());
        assert!(d["hidden"].is_null());
        assert!(!super::super::visual::ax_unavailable_for_visual(error));
    }
    #[test]
    fn exact_present_record_explains_onscreen_absence_without_granting_input() {
        for onscreen in [None, Some(false), Some(true)] {
            let e = missing(
                42,
                123,
                Ok(Some(Record {
                    pid: 42,
                    window: 123,
                    onscreen,
                })),
            );
            assert_no_authority(&e);
            let d = e.diagnostic.as_ref().unwrap();
            assert_eq!(d["native_record_present"], true);
            assert_eq!(d["owner_matches"], true);
            assert_eq!(d["native_onscreen"], json!(onscreen));
            assert_eq!(d["non_atomic"], true);
        }
    }
    #[test]
    fn absent_record_is_only_current_read_evidence_not_destroyed_or_hidden() {
        let e = missing(42, 123, Ok(None));
        assert_no_authority(&e);
        assert_eq!(
            e.diagnostic.as_ref().unwrap()["native_record_present"],
            false
        );
    }
    #[test]
    fn owner_or_id_mismatch_never_becomes_target_presence() {
        for record in [
            Record {
                pid: 43,
                window: 123,
                onscreen: Some(true),
            },
            Record {
                pid: 42,
                window: 124,
                onscreen: Some(true),
            },
        ] {
            let e = missing(42, 123, Ok(Some(record)));
            assert_no_authority(&e);
            let d = e.diagnostic.as_ref().unwrap();
            assert_eq!(d["owner_matches"], false);
            assert!(d["native_record_present"].is_null());
            assert!(d["native_onscreen"].is_null());
        }
    }
    #[test]
    fn unavailable_native_probe_retains_fault_and_unknown_presence() {
        let original = fault(
            "window_record_probe_unavailable",
            "original native read failure",
        );
        let e = missing(42, 123, Err(original));
        assert_no_authority(&e);
        let d = e.diagnostic.as_ref().unwrap();
        assert!(d["native_record_present"].is_null());
        assert_eq!(d["probe_error"]["code"], "window_record_probe_unavailable");
        assert_eq!(
            d["probe_error"]["next_step"],
            "original native read failure"
        );
    }
}
