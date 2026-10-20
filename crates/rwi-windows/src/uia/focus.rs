//! Query the authoritative focused element, not every provider's cached flag.
use super::*;

pub(super) struct Sample {
    pub element: IUIAutomationElement,
    pub runtime: Vec<i32>,
}
pub(super) fn read(client: &Client, root: &IUIAutomationElement) -> Result<Sample> {
    let element = {
        let _call = native_call("uia_observe_focused_element")?;
        api("uia_focus_unavailable", unsafe {
            client.automation.GetFocusedElementBuildCache(&client.cache)
        })?
    };
    client.under_root(root, &element)?;
    let _call = native_call("uia_observe_focused_identity")?;
    let runtime = runtime_id(&element)?;
    if !api("uia_focus_unknown", unsafe {
        element.CurrentHasKeyboardFocus()
    })?
    .as_bool()
        || api("uia_provider_unknown", unsafe {
            element.CurrentProcessId()
        })? <= 0
    {
        return Err(fault(
            "uia_focus_unconfirmed",
            "Focused element changed during observation.",
        ));
    }
    // Password/enabled/value classification is still performed by the normal
    // node reader and again by InputScope immediately before every input batch.
    Ok(Sample { element, runtime })
}
