//! Fresh exact UIA input scope; COM proxies never cross threads.
use super::*;

/// Kept on the native worker/MTA only. Focus is checked; never SetFocus/Invoke/SetValue.
pub(crate) struct InputScope {
    root: IUIAutomationElement,
    window: HWND,
    pid: u32,
    epoch: String,
    wanted: Option<String>,
    point: [f64; 2],
    keyboard: bool,
    text: bool,
    provider_pid: Option<i32>,
    // Drop the root and all other proxies before Client uninitializes COM.
    client: Client,
}
impl InputScope {
    pub(crate) fn set_point(&mut self, point: [f64; 2]) {
        self.point = point;
    }
    pub fn new(
        w: HWND,
        pid: u32,
        epoch: &str,
        wanted: Option<&str>,
        point: [f64; 2],
        keyboard: bool,
        text: bool,
    ) -> Result<Self> {
        if keyboard && wanted.is_none() {
            return Err(fault(
                "focused_element_required",
                "Keyboard input requires the exact observed focused UIA element, not a pixel reference.",
            ));
        }
        let client = if keyboard {
            Client::new()?
        } else {
            Client::input()?
        };
        let root = client.root(w)?;
        let mut scope = Self {
            client,
            root,
            window: w,
            pid,
            epoch: epoch.into(),
            wanted: wanted.map(str::to_owned),
            point,
            keyboard,
            text,
            provider_pid: None,
        };
        scope.check()?;
        Ok(scope)
    }
    pub fn check(&mut self) -> Result<()> {
        checkpoint("uia_input_scope")?;
        if self.keyboard {
            let e = {
                let _call = native_call("uia_focused_element")?;
                api("uia_focus_unavailable", unsafe {
                    self.client.automation.GetFocusedElement()
                })?
            };
            self.client.under_root(&self.root, &e)?;
            let _call = native_call("uia_focused_properties")?;
            let provider = api("uia_provider_unknown", unsafe { e.CurrentProcessId() })?;
            let rid = runtime_id(&e)?;
            let id = digest(
                format!(
                    "{}:{}:{}:{rid:?}",
                    window_id(self.window),
                    self.pid,
                    self.epoch
                )
                .as_bytes(),
            );
            if provider <= 0
                || self.provider_pid.is_some_and(|p| p != provider)
                || self.wanted.as_deref() != Some(&id)
                || !api("uia_focus_unknown", unsafe { e.CurrentHasKeyboardFocus() })?.as_bool()
                || !api("uia_enabled_unknown", unsafe { e.CurrentIsEnabled() })?.as_bool()
                || api("uia_password_unknown", unsafe { e.CurrentIsPassword() })?.as_bool()
            {
                return Err(fault(
                    "keyboard_focus_changed",
                    "Exact focused runtime ID/provider/enabled/password classification did not match; no further input.",
                ));
            }
            if self.text {
                require_writable(&e)?;
            }
            self.provider_pid = Some(provider);
        } else {
            let leaf = {
                let _call = native_call("uia_pointer_element")?;
                api("uia_pointer_unavailable", unsafe {
                    self.client.automation.ElementFromPoint(POINT {
                        x: self.point[0].round() as i32,
                        y: self.point[1].round() as i32,
                    })
                })?
            };
            let mut e = leaf.clone();
            let start = Instant::now();
            let mut proof = crate::uia_ancestry::PointerAncestry::new(self.wanted.is_some());
            let mut root_proven = false;
            for _ in 0..32 {
                checkpoint("uia_pointer_ancestry")?;
                if start.elapsed() > Duration::from_millis(1500) {
                    break;
                }
                let _call = native_call("uia_pointer_ancestry_native")?;
                let root = api("uia_ancestry_compare", unsafe {
                    self.client.automation.CompareElements(&self.root, &e)
                })?
                .as_bool();
                let matches = if proof.needs_target() {
                    let rid = runtime_id(&e)?;
                    let id = digest(
                        format!(
                            "{}:{}:{}:{rid:?}",
                            window_id(self.window),
                            self.pid,
                            self.epoch
                        )
                        .as_bytes(),
                    );
                    self.wanted.as_deref() == Some(&id)
                } else {
                    false
                };
                if proof.visit(root, matches)? {
                    root_proven = true;
                    break;
                }
                match self.client.related(&e, 2)? {
                    Some(parent) => e = parent,
                    None => {
                        return Err(fault(
                            "uia_wrong_root",
                            "Pointer element is not under the allocated root; no input.",
                        ));
                    }
                }
            }
            if !root_proven {
                return Err(fault(
                    "uia_ancestry_incomplete",
                    "Could not attest pointer root within 32 levels / 1500 ms; no input.",
                ));
            }
            // These remain current reads AFTER potentially blocking ancestry,
            // not stale classifications cached before walking the tree.
            if api("uia_password_unknown", unsafe { leaf.CurrentIsPassword() })?.as_bool()
                || !api("uia_enabled_unknown", unsafe { leaf.CurrentIsEnabled() })?.as_bool()
            {
                return Err(fault(
                    "pointer_sensitive_or_disabled",
                    "Pointer input cannot bypass password/enabled classification through a pixel locator.",
                ));
            }
        }
        Ok(())
    }
}
