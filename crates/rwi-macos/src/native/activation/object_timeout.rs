use super::*;

// Timeout policy is local to this retained object, not CFEqual objects or the process.
pub(super) fn checked_foreground_object_timeout<T>(
    object: T,
    set_timeout: impl FnOnce(&T, f32) -> i32,
) -> Result<T> {
    let status = set_timeout(&object, FOREGROUND_OBJECT_TIMEOUT_SECONDS);
    if status != 0 {
        let mut error = fault(
            "foreground_timeout_configuration_failed",
            "Exact foreground AX object timeout setup failed; no retry or fallback.",
        );
        error.diagnostic = Some(Box::new(serde_json::json!({
            "api": "AXUIElementSetMessagingTimeout",
            "native_status": status,
            "timeout_seconds": FOREGROUND_OBJECT_TIMEOUT_SECONDS,
            "scope": "object_local",
            "replay_allowed": false
        })));
        return Err(error);
    }
    Ok(object)
}
