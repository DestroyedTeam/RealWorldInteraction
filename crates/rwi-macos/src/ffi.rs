//! Small public CoreFoundation/AX boundary. Create/copy results have RAII release.
use core_foundation::{
    base::{CFRange, TCFType},
    string::{CFString, CFStringGetBytes, kCFStringEncodingUTF8},
};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use rwi_core::{Result, fault};
use std::ffi::c_void;
pub type Ref = *const c_void;
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    // AuthSession.h: callerSecuritySession = UINT32_MAX; no session creation.
    pub fn SessionGetInfo(session: u32, actual: *mut u32, attributes: *mut u32) -> i32;
}
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    pub fn AXIsProcessTrusted() -> u8;
    pub fn AXUIElementCreateApplication(pid: libc::pid_t) -> Ref;
    pub fn AXUIElementCreateSystemWide() -> Ref;
    pub fn AXUIElementCopyAttributeValue(element: Ref, name: Ref, value: *mut Ref) -> i32;
    pub fn AXUIElementCopyElementAtPosition(element: Ref, x: f32, y: f32, value: *mut Ref) -> i32;
    #[allow(dead_code)]
    pub fn AXUIElementPerformAction(element: Ref, action: Ref) -> i32;
    pub fn AXUIElementCopyActionNames(element: Ref, names: *mut Ref) -> i32;
    pub fn AXUIElementGetPid(element: Ref, pid: *mut i32) -> i32;
    pub fn AXUIElementGetTypeID() -> usize;
    pub fn AXUIElementSetMessagingTimeout(element: Ref, timeout: f32) -> i32;
    pub fn AXUIElementIsAttributeSettable(element: Ref, name: Ref, settable: *mut u8) -> i32;
    pub fn AXValueGetType(value: Ref) -> u32;
    pub fn AXValueGetValue(value: Ref, kind: u32, value_out: *mut c_void) -> u8;
    pub fn AXValueGetTypeID() -> usize;
    pub fn AXUIElementSetAttributeValue(element: Ref, attribute: Ref, value: Ref) -> i32;
    pub fn AXValueCreate(kind: u32, value: *const c_void) -> Ref;
}
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub fn CGSessionCopyCurrentDictionary() -> Ref;
    pub fn CGWindowListCopyWindowInfo(options: u32, relative: u32) -> Ref;
    pub fn CGRectMakeWithDictionaryRepresentation(dict: Ref, rect: *mut CGRect) -> u8;
    pub fn CGPreflightScreenCaptureAccess() -> bool;
    pub fn CGPreflightPostEventAccess() -> bool;
    pub fn CGPreflightListenEventAccess() -> bool;
    pub fn CGEventSourceKeyState(state: i32, key: u16) -> bool;
    pub fn CGEventSourceButtonState(state: i32, button: u32) -> bool;
    pub fn CGEventSourceFlagsState(state: i32) -> u64;
    pub fn CGEventSetLocation(event: Ref, point: CGPoint);
    pub fn CGDisplayIsBuiltin(display: u32) -> i32;
}
#[link(name = "ColorSync", kind = "framework")]
unsafe extern "C" {
    /// Create-rule CFUUIDRef for one currently active CGDirectDisplayID.
    pub fn CGDisplayCreateUUIDFromDisplayID(display: u32) -> Ref;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub fn CFRelease(value: Ref);
    pub fn CFRetain(value: Ref) -> Ref;
    pub fn CFGetTypeID(value: Ref) -> usize;
    pub fn CFEqual(a: Ref, b: Ref) -> u8;
    pub fn CFDictionaryGetTypeID() -> usize;
    pub fn CFDictionaryGetValue(dict: Ref, key: Ref) -> Ref;
    pub fn CFArrayGetTypeID() -> usize;
    pub fn CFArrayGetCount(array: Ref) -> isize;
    pub fn CFArrayGetValueAtIndex(array: Ref, index: isize) -> Ref;
    pub fn CFStringGetTypeID() -> usize;
    pub fn CFStringGetLength(value: Ref) -> isize;
    /// Create-rule CFStringRef; a null allocator selects the default allocator.
    pub fn CFUUIDCreateString(allocator: Ref, uuid: Ref) -> Ref;
    pub fn CFNumberGetTypeID() -> usize;
    pub fn CFNumberGetValue(value: Ref, kind: u32, out: *mut c_void) -> u8;
    pub fn CFBooleanGetTypeID() -> usize;
    pub fn CFBooleanGetValue(value: Ref) -> u8;
}
pub struct Cf(pub Ref);
impl Cf {
    pub fn retained(&self) -> Self {
        Self(unsafe { CFRetain(self.0) })
    }
    /// `value` is a nullable +1 CoreFoundation result from a Create/Copy API.
    pub unsafe fn owned(value: Ref) -> Result<Self> {
        if value.is_null() {
            Err(fault(
                "native_unavailable",
                "Native API returned no object; inspect permissions and session.",
            ))
        } else {
            Ok(Self(value))
        }
    }
    pub fn attr(&self, name: &str) -> Result<Self> {
        let key = CFString::new(name);
        let mut out = std::ptr::null();
        let code = unsafe {
            AXUIElementCopyAttributeValue(self.0, key.as_concrete_TypeRef().cast(), &mut out)
        };
        if code != 0 {
            return Err(attribute_read_fault(name, code));
        }
        unsafe { Self::owned(out) }
    }
    pub fn attribute_settable(&self, name: &str) -> Result<bool> {
        let name = CFString::new(name);
        let mut settable = 0;
        let code = unsafe {
            AXUIElementIsAttributeSettable(self.0, name.as_concrete_TypeRef().cast(), &mut settable)
        };
        if code != 0 {
            return Err(fault(
                "ax_writability_unknown",
                "Native AX attribute writability is unavailable; no retry or input substitution.",
            ));
        }
        Ok(settable != 0)
    }
    pub fn pid(&self) -> Result<u32> {
        let mut pid = 0;
        let code = unsafe { AXUIElementGetPid(self.0, &mut pid) };
        if code != 0 || pid <= 0 {
            Err(fault("pid_unknown", "Accessibility PID is not attested."))
        } else {
            Ok(pid as u32)
        }
    }
    pub fn string(&self) -> Result<String> {
        unsafe { string(self.0) }
    }
    pub fn boolean(&self) -> Option<bool> {
        unsafe { boolean(self.0) }
    }
    pub fn point(&self) -> Result<CGPoint> {
        let mut out = CGPoint::new(0.0, 0.0);
        if unsafe {
            CFGetTypeID(self.0) != AXValueGetTypeID()
                || AXValueGetType(self.0) != 1
                || AXValueGetValue(self.0, 1, (&mut out as *mut CGPoint).cast()) == 0
        } {
            return Err(fault(
                "ax_geometry_unknown",
                "Cannot attest accessibility position.",
            ));
        }
        Ok(out)
    }
    pub fn size(&self) -> Result<CGSize> {
        let mut out = CGSize::new(0.0, 0.0);
        if unsafe {
            CFGetTypeID(self.0) != AXValueGetTypeID()
                || AXValueGetType(self.0) != 2
                || AXValueGetValue(self.0, 2, (&mut out as *mut CGSize).cast()) == 0
        } {
            return Err(fault(
                "ax_geometry_unknown",
                "Cannot attest accessibility size.",
            ));
        }
        Ok(out)
    }
    pub fn children(&self) -> Result<Vec<Cf>> {
        let value = self.attr("AXChildren")?;
        unsafe { array(value.0, 256) }
    }
}
// Preserve read-failure evidence without reading/logging the attribute's value.
// This is not an input receipt and must never authorize replay of earlier input.
pub(crate) fn attribute_is_absent(error: &rwi_core::Fault, name: &str) -> bool {
    error.code == "ax_attribute_unavailable"
        && error.diagnostic.as_ref().is_some_and(|d| {
            d["api"] == "AXUIElementCopyAttributeValue"
                && d["attribute"] == name
                && matches!(d["ax_error"].as_i64(), Some(-25205 | -25212))
        })
}

fn attribute_read_fault(name: &str, code: i32) -> rwi_core::Fault {
    let mut error = fault(
        "ax_attribute_unavailable",
        &format!(
            "AX read {name} returned AXError {code}; inspect fresh state, never replay prior input."
        ),
    );
    error.diagnostic = Some(Box::new(serde_json::json!({
        "api": "AXUIElementCopyAttributeValue",
        "attribute": name,
        "ax_error": code,
        "operation_kind": "read",
        "replay_allowed": false
    })));
    error
}

/// Only the observed AXWindows messaging failure is eligible for an observation
/// restart. Never apply this to action dispatch, unsupported attributes or stale
/// native identity. Each new observation still validates its complete binding.
pub(crate) fn observation_read_can_settle(error: &rwi_core::Fault) -> bool {
    error.code == "ax_attribute_unavailable"
        && error.diagnostic.as_ref().is_some_and(|d| {
            d["api"] == "AXUIElementCopyAttributeValue"
                && d["attribute"] == "AXWindows"
                && d["ax_error"] == -25204
                && d["operation_kind"] == "read"
                && d["replay_allowed"] == false
        })
}

#[cfg(test)]
mod attribute_read_tests {
    use super::*;

    #[test]
    fn read_fault_preserves_exact_attribute_and_native_code() {
        for code in [-25204, -25205, -25212] {
            let error = attribute_read_fault("AXWindows", code);
            assert_eq!(error.code, "ax_attribute_unavailable");
            let diagnostic = error.diagnostic.unwrap();
            assert_eq!(diagnostic["attribute"], "AXWindows");
            assert_eq!(diagnostic["ax_error"], code);
            assert_eq!(diagnostic["operation_kind"], "read");
            assert_eq!(diagnostic["replay_allowed"], false);
            assert!(diagnostic.get("value").is_none());
            assert!(error.next_step.contains(&code.to_string()));
        }
    }

    #[test]
    fn subrole_absence_is_distinct_from_timeout_invalid_object_and_other_attributes() {
        for code in [-25205, -25212] {
            assert!(attribute_is_absent(
                &attribute_read_fault("AXSubrole", code),
                "AXSubrole"
            ));
        }
        for code in [-25204, -25202, -25200] {
            assert!(!attribute_is_absent(
                &attribute_read_fault("AXSubrole", code),
                "AXSubrole"
            ));
        }
        assert!(!attribute_is_absent(
            &attribute_read_fault("AXValue", -25212),
            "AXSubrole"
        ));
        assert!(!attribute_is_absent(
            &fault("native_type_mismatch", "not a string"),
            "AXSubrole"
        ));
    }

    #[test]
    fn only_exact_window_list_read_failure_may_restart_observation() {
        assert!(observation_read_can_settle(&attribute_read_fault(
            "AXWindows",
            -25204
        )));
        for (name, code) in [
            ("AXWindows", -25205),
            ("AXWindows", -25202),
            ("AXValue", -25204),
        ] {
            assert!(!observation_read_can_settle(&attribute_read_fault(
                name, code
            )));
        }
        assert!(!observation_read_can_settle(&fault(
            "ax_attribute_unavailable",
            "missing evidence"
        )));
        let mut error = attribute_read_fault("AXWindows", -25204);
        error.diagnostic.as_mut().unwrap()["api"] = serde_json::json!("AXUIElementPerformAction");
        assert!(!observation_read_can_settle(&error));
    }
}
impl Drop for Cf {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}
pub unsafe fn string(value: Ref) -> Result<String> {
    if value.is_null() || unsafe { CFGetTypeID(value) != CFStringGetTypeID() } {
        return Err(fault("native_type_mismatch", "Expected a native string."));
    }
    let chars = unsafe { CFStringGetLength(value) };
    if chars > 16384 {
        return Err(fault(
            "ax_value_budget",
            "Native string exceeds the preallocation budget.",
        ));
    }
    let encoding_error = || {
        fault(
            "native_string_encoding",
            "Native string is not a complete, lossless UTF-8 value.",
        )
    };
    if chars < 0 {
        return Err(encoding_error());
    }
    // Keep the original get-rule retain/release lifetime. Never borrow a C string:
    // embedded NUL is data, while invalid UTF-16 must not panic or become lossy text.
    let value = unsafe { CFString::wrap_under_get_rule(value.cast()) };
    let range = CFRange {
        location: 0,
        length: chars,
    };
    let mut required = 0;
    let converted = unsafe {
        CFStringGetBytes(
            value.as_concrete_TypeRef(),
            range,
            kCFStringEncodingUTF8,
            0,
            0,
            std::ptr::null_mut(),
            0,
            &mut required,
        )
    };
    if converted != chars || required < 0 {
        return Err(encoding_error());
    }
    if required > 16384 {
        return Err(fault(
            "ax_value_budget",
            "Accessibility string exceeds 16 KiB.",
        ));
    }
    let bytes_required = usize::try_from(required).map_err(|_| encoding_error())?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(bytes_required).map_err(|_| {
        fault(
            "ax_value_budget",
            "Native string allocation exceeded the bounded capacity.",
        )
    })?;
    bytes.resize(bytes_required, 0);
    let mut used = 0;
    let converted = unsafe {
        CFStringGetBytes(
            value.as_concrete_TypeRef(),
            range,
            kCFStringEncodingUTF8,
            0,
            0,
            bytes.as_mut_ptr(),
            required,
            &mut used,
        )
    };
    if converted != chars || used != required {
        return Err(encoding_error());
    }
    String::from_utf8(bytes).map_err(|_| encoding_error())
}

pub unsafe fn boolean(value: Ref) -> Option<bool> {
    if value.is_null() || unsafe { CFGetTypeID(value) != CFBooleanGetTypeID() } {
        None
    } else {
        Some(unsafe { CFBooleanGetValue(value) } != 0)
    }
}
pub unsafe fn number(value: Ref) -> Option<i64> {
    if value.is_null() || unsafe { CFGetTypeID(value) != CFNumberGetTypeID() } {
        return None;
    }
    let mut out = 0i64;
    if unsafe { CFNumberGetValue(value, 4, (&mut out as *mut i64).cast()) } == 0 {
        None
    } else {
        Some(out)
    }
}
pub unsafe fn real(value: Ref) -> Option<f64> {
    if value.is_null() || unsafe { CFGetTypeID(value) != CFNumberGetTypeID() } {
        return None;
    }
    let mut out = 0f64;
    if unsafe { CFNumberGetValue(value, 13, (&mut out as *mut f64).cast()) } == 0 {
        None
    } else {
        Some(out)
    }
}
pub unsafe fn field(dict: Ref, key: &str) -> Ref {
    if dict.is_null() || unsafe { CFGetTypeID(dict) != CFDictionaryGetTypeID() } {
        return std::ptr::null();
    }
    let key = CFString::new(key);
    unsafe { CFDictionaryGetValue(dict, key.as_concrete_TypeRef().cast()) }
}
pub unsafe fn array(value: Ref, limit: usize) -> Result<Vec<Cf>> {
    if value.is_null() || unsafe { CFGetTypeID(value) != CFArrayGetTypeID() } {
        return Err(fault("native_type_mismatch", "Expected a native array."));
    }
    let count = unsafe { CFArrayGetCount(value) };
    if count < 0 || count as usize > limit {
        return Err(fault(
            "native_array_budget",
            "Narrow the native tree/window list.",
        ));
    }
    (0..count)
        .map(|i| unsafe { Cf::owned(CFRetain(CFArrayGetValueAtIndex(value, i))) })
        .collect()
}
/// Private SPI used only for exact window identity and window-local event
/// addressing. Missing symbols fail closed; no substitution by title.
mod private_spi {
    use super::{Cf, Ref};
    use core_graphics::geometry::CGPoint;
    use std::sync::OnceLock;
    type SetWindowLocationFn = unsafe extern "C" fn(Ref, CGPoint);
    type GetAXWindowFn = unsafe extern "C" fn(Ref, *mut u32) -> i32;
    fn symbol(framework: &std::ffi::CStr, name: &std::ffi::CStr) -> Option<*mut std::ffi::c_void> {
        let handle = unsafe { libc::dlopen(framework.as_ptr(), libc::RTLD_LAZY) };
        if handle.is_null() {
            return None;
        }
        let found = unsafe { libc::dlsym(handle, name.as_ptr()) };
        (!found.is_null()).then_some(found)
    }
    fn set_window_location_fn() -> &'static Option<SetWindowLocationFn> {
        static RESOLVED: OnceLock<Option<SetWindowLocationFn>> = OnceLock::new();
        RESOLVED.get_or_init(|| {
            symbol(
                c"/System/Library/PrivateFrameworks/SkyLight.framework/SkyLight",
                c"CGEventSetWindowLocation",
            )
            .map(|found| unsafe {
                std::mem::transmute::<*mut std::ffi::c_void, SetWindowLocationFn>(found)
            })
        })
    }
    pub fn window_location_spi_available() -> bool {
        set_window_location_fn().is_some()
    }
    pub fn set_window_location(event: Ref, point: CGPoint) -> bool {
        let Some(function) = set_window_location_fn() else {
            return false;
        };
        unsafe { function(event, point) };
        true
    }
    pub fn window_number_for_ax_element(element: &Cf) -> Option<u32> {
        static RESOLVED: OnceLock<Option<GetAXWindowFn>> = OnceLock::new();
        let resolved = RESOLVED.get_or_init(|| {
            symbol(
                c"/System/Library/Frameworks/ApplicationServices.framework/Frameworks/HIServices.framework/HIServices",
                c"_AXUIElementGetWindow",
            )
            .map(|found| unsafe {
                std::mem::transmute::<*mut std::ffi::c_void, GetAXWindowFn>(found)
            })
        });
        let function = resolved.as_ref()?;
        let mut window = 0u32;
        if unsafe { function(element.0, &mut window) } == 0 && window != 0 {
            Some(window)
        } else {
            None
        }
    }
}
pub use private_spi::{
    set_window_location, window_location_spi_available, window_number_for_ax_element,
};

#[cfg(test)]
mod cf_string_checked_tests {
    use super::*;
    use core_foundation::{boolean::CFBoolean, string::CFStringCreateWithCharacters};

    fn own_utf16(units: &[u16]) -> Cf {
        let value = unsafe {
            CFStringCreateWithCharacters(std::ptr::null(), units.as_ptr(), units.len() as isize)
        };
        unsafe { Cf::owned(value.cast()) }.expect("own memory CFString creation")
    }

    #[test]
    fn cf_string_checked_empty_and_unicode_are_exact() {
        for expected in ["", "ASCII", "汉字 e\u{301} 🙂"] {
            let native = own_utf16(&expected.encode_utf16().collect::<Vec<_>>());
            let actual = unsafe { string(native.0) }.unwrap();
            assert_eq!(actual, expected);
            println!(
                "{}",
                serde_json::json!({"probe":"cf_string_checked", "case":"valid", "utf8_bytes":actual.len(), "exact":actual == expected, "window_calls":0, "input_post_calls":0})
            );
        }
    }

    #[test]
    fn cf_string_checked_embedded_nul_preserves_the_entire_value() {
        for expected in ["ok\0suffix", "\0", "a\0b\0c"] {
            let native = own_utf16(&expected.encode_utf16().collect::<Vec<_>>());
            let actual = unsafe { string(native.0) }.unwrap();
            assert_eq!(actual.as_bytes(), expected.as_bytes());
            println!(
                "{}",
                serde_json::json!({"probe":"cf_string_checked", "case":"embedded_nul", "utf8_bytes":actual.len(), "expected_bytes":expected.len(), "exact":actual == expected, "window_calls":0, "input_post_calls":0})
            );
        }
    }

    #[test]
    fn cf_string_checked_invalid_utf16_returns_encoding_fault_without_panic() {
        for units in [&[0xd800, 0x0058, 0xdc00][..], &[0xd800], &[0xdc00]] {
            let native = own_utf16(units);
            let error = unsafe { string(native.0) }.unwrap_err();
            assert_eq!(error.code, "native_string_encoding");
            println!(
                "{}",
                serde_json::json!({"probe":"cf_string_checked", "case":"invalid_utf16", "utf16_units":units.len(), "error":error.code, "window_calls":0, "input_post_calls":0})
            );
        }
    }

    #[test]
    fn cf_string_checked_null_and_wrong_type_keep_type_fault() {
        assert_eq!(
            unsafe { string(std::ptr::null()) }.unwrap_err().code,
            "native_type_mismatch"
        );
        let boolean = CFBoolean::true_value();
        assert_eq!(
            unsafe { string(boolean.as_concrete_TypeRef().cast()) }
                .unwrap_err()
                .code,
            "native_type_mismatch"
        );
    }

    #[test]
    fn cf_string_checked_character_budget_keeps_16384_unit_boundary() {
        let exact = own_utf16(&vec![0x0061; 16384]);
        assert_eq!(unsafe { string(exact.0) }.unwrap().len(), 16384);
        let over = own_utf16(&vec![0x0061; 16385]);
        assert_eq!(
            unsafe { string(over.0) }.unwrap_err().code,
            "ax_value_budget"
        );
    }

    #[test]
    fn cf_string_checked_byte_budget_is_independent_of_character_budget() {
        let mut exact_units = vec![0x4e00; 5461];
        exact_units.push(0x0061);
        let exact = own_utf16(&exact_units);
        assert_eq!(unsafe { string(exact.0) }.unwrap().len(), 16384);
        let over_units = vec![0x4e00; 5462];
        assert!(over_units.len() < 16384);
        let over = own_utf16(&over_units);
        assert_eq!(
            unsafe { string(over.0) }.unwrap_err().code,
            "ax_value_budget"
        );
    }
}
