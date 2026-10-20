//! Validation of system-hosted descendants, not permission to control a whole helper.
use super::*;
use core_foundation::{
    base::TCFType, dictionary::CFDictionary, number::CFNumber, string::CFString,
};
use std::ffi::c_void;
#[link(name = "Security", kind = "framework")]
unsafe extern "C" {
    static kSecGuestAttributePid: *const c_void;
    fn SecCodeCopyGuestWithAttributes(
        host: *const c_void,
        attributes: *const c_void,
        flags: u32,
        guest: *mut *const c_void,
    ) -> i32;
    fn SecRequirementCreateWithString(
        text: *const c_void,
        flags: u32,
        requirement: *mut *const c_void,
    ) -> i32;
    fn SecCodeCheckValidity(code: *const c_void, flags: u32, requirement: *const c_void) -> i32;
}
const PANEL_REQUIREMENT: &str =
    r#"anchor apple and identifier "com.apple.appkit.xpc.openAndSavePanelService""#;
// Called only on an object freshly found in the exact selected AX subtree.
// A signed process alone is never sufficient to select an object.
pub(super) fn validate_hosted_descendant(bound_pid: u32, actual_pid: u32) -> Result<String> {
    let epoch = process_epoch(actual_pid)?; // Native same-user + birth proof.
    if actual_pid == bound_pid {
        return Ok(epoch);
    }
    unsafe {
        let key = CFString::wrap_under_get_rule(kSecGuestAttributePid.cast());
        let attrs = CFDictionary::from_CFType_pairs(&[(
            key.as_CFType(),
            CFNumber::from(actual_pid as i32).as_CFType(),
        )]);
        let mut code = std::ptr::null();
        let status = SecCodeCopyGuestWithAttributes(
            std::ptr::null(),
            attrs.as_concrete_TypeRef().cast(),
            0,
            &mut code,
        );
        if status != 0 {
            return Err(fault(
                "delegate_code_unknown",
                &format!("Cannot obtain dynamic code for linked PID {actual_pid}: {status}"),
            ));
        }
        let code = Cf::owned(code)?;
        let requirement_text = CFString::new(PANEL_REQUIREMENT);
        let mut requirement = std::ptr::null();
        let status = SecRequirementCreateWithString(
            requirement_text.as_concrete_TypeRef().cast(),
            0,
            &mut requirement,
        );
        if status != 0 {
            return Err(fault(
                "delegate_requirement_failed",
                "Cannot construct system panel identity requirement.",
            ));
        }
        let requirement = Cf::owned(requirement)?;
        let status = SecCodeCheckValidity(code.0, 0, requirement.0);
        require_valid_delegate(status, &epoch, &process_epoch(actual_pid)?)?;
    }
    Ok(epoch)
}
fn require_valid_delegate(status: i32, before: &str, after: &str) -> Result<()> {
    if status != 0 {
        return Err(fault(
            "delegate_identity_rejected",
            &format!(
                "Linked provider does not satisfy the supported system panel signature requirement: {status}. Explicitly select other applications; do not widen automatically."
            ),
        ));
    }
    if before != after {
        return Err(fault(
            "delegate_process_changed",
            "Hosted process changed during validation; no input.",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_signature_and_changed_birth_never_authorize_delegation() {
        assert!(require_valid_delegate(0, "p:1", "p:1").is_ok());
        for code in [-67050, -1, 1] {
            assert!(require_valid_delegate(code, "p:1", "p:1").is_err());
        }
        assert!(require_valid_delegate(0, "p:1", "p:2").is_err());
    }
}
