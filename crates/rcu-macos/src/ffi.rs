//! Small public CoreFoundation/AX boundary. Create/copy results have RAII release.
use core_foundation::{base::TCFType, string::CFString};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use rcu_core::{Result, fault};
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
    pub fn AXUIElementGetPid(element: Ref, pid: *mut i32) -> i32;
    pub fn AXUIElementSetMessagingTimeout(element: Ref, timeout: f32) -> i32;
    pub fn AXUIElementIsAttributeSettable(element: Ref, name: Ref, settable: *mut u8) -> i32;
    pub fn AXValueGetType(value: Ref) -> u32;
    pub fn AXValueGetValue(value: Ref, kind: u32, value_out: *mut c_void) -> u8;
    pub fn AXValueGetTypeID() -> usize;
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
            return Err(fault(
                "ax_attribute_unavailable",
                "Required accessibility attribute is unavailable; do not guess.",
            ));
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
impl Drop for Cf {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}
pub unsafe fn string(value: Ref) -> Result<String> {
    if value.is_null() || unsafe { CFGetTypeID(value) != CFStringGetTypeID() } {
        return Err(fault("native_type_mismatch", "Expected a native string."));
    }
    if unsafe { CFStringGetLength(value) } > 16384 {
        return Err(fault(
            "ax_value_budget",
            "Native string exceeds the preallocation budget.",
        ));
    }
    let value = unsafe { CFString::wrap_under_get_rule(value.cast()) };
    let result = value.to_string();
    if result.len() > 16384 {
        return Err(fault(
            "ax_value_budget",
            "Accessibility string exceeds 16 KiB.",
        ));
    }
    Ok(result)
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
