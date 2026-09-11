//! Read-only evidence for recovery of the native desktop input conflict domain.
use rcu_core::{Result, fault};
pub fn input_resources() -> Result<serde_json::Value> {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    let before = crate::agent_binding::current_session()?;
    let mut down = Vec::new();
    for key in 1..=254 {
        if (unsafe { GetAsyncKeyState(key) } as u16) & 0x8000 != 0 {
            down.push(key);
        }
    }
    before.require_same(&crate::agent_binding::current_session()?)?;
    if !down.is_empty() {
        return Err(fault(
            "physical_input_active",
            "Release physical keys/buttons before explicit input-domain reconciliation.",
        ));
    }
    Ok(
        serde_json::json!({"source":"GetAsyncKeyState/current_native_session","pressed_keys_or_buttons":down,"context":before,"input_sent":false,"business_effect_reconciled":false}),
    )
}
