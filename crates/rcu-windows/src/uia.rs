//! Per-call MTA and bounded cached UIA walk. COM objects never cross threads.
mod input;
mod value;
use crate::desktop::{api, hit_window, rect, window_id};
pub(crate) use input::InputScope;
use rcu_core::{
    Element, Result, digest, fault,
    operation::{checkpoint, native_call},
};
use serde::Serialize;
use std::{
    mem::size_of,
    time::{Duration, Instant},
};
use value::bounded_document_value;
use windows::{
    Win32::{
        Foundation::*,
        System::{
            Com::*,
            Ole::*,
            Variant::{VT_BOOL, VT_I4},
        },
        UI::Accessibility::*,
    },
    core::{IUnknown, Interface},
};

struct Apartment;
impl Apartment {
    fn new() -> Result<Self> {
        api(
            "uia_apartment_unavailable",
            unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok(),
        )?;
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
struct Array(*mut SAFEARRAY);
impl Drop for Array {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = SafeArrayDestroy(self.0);
            }
        }
    }
}
fn runtime_id(e: &IUIAutomationElement) -> Result<Vec<i32>> {
    let a = Array(api("uia_runtime_id_unavailable", unsafe {
        e.GetRuntimeId()
    })?);
    if a.0.is_null()
        || unsafe { SafeArrayGetDim(a.0) } != 1
        || unsafe { SafeArrayGetElemsize(a.0) } != size_of::<i32>() as u32
        || api("uia_runtime_id_unavailable", unsafe {
            SafeArrayGetVartype(a.0)
        })? != VT_I4
    {
        return Err(fault(
            "uia_runtime_id_unavailable",
            "Expected a one-dimensional VT_I4 native runtime ID.",
        ));
    }
    let lo = api("uia_runtime_id_unavailable", unsafe {
        SafeArrayGetLBound(a.0, 1)
    })?;
    let hi = api("uia_runtime_id_unavailable", unsafe {
        SafeArrayGetUBound(a.0, 1)
    })?;
    if hi < lo || i64::from(hi) - i64::from(lo) >= 64 {
        return Err(fault(
            "uia_runtime_id_unavailable",
            "Runtime ID exceeded 64 integers.",
        ));
    }
    let mut result = vec![];
    for i in lo..=hi {
        let mut v = 0i32;
        api("uia_runtime_id_unavailable", unsafe {
            SafeArrayGetElement(a.0, &i, (&mut v as *mut i32).cast())
        })?;
        result.push(v);
    }
    Ok(result)
}
struct Client {
    automation: IUIAutomation,
    cache: IUIAutomationCacheRequest,
    walker: IUIAutomationTreeWalker,
    _apartment: Apartment,
}
impl Client {
    fn input() -> Result<Self> {
        let mut client = Self::new()?;
        // Ancestry needs live element identities, not names, rectangles or text
        // for every intermediate parent. Keep Full element mode: final leaf
        // enabled/password properties are still read fresh after ancestry.
        client.cache = api("uia_cache_unavailable", unsafe {
            client.automation.CreateCacheRequest()
        })?;
        api("uia_cache_unavailable", unsafe {
            client.cache.SetTreeScope(TreeScope_Element)
        })?;
        Ok(client)
    }
    fn new() -> Result<Self> {
        let _call = native_call("uia_initialize")?;
        let apartment = Apartment::new()?;
        let automation: IUIAutomation = api("uia_unavailable", unsafe {
            CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)
        })?;
        let v2: IUIAutomation2 = api("uia_timeout_control_unavailable", automation.cast())?;
        api("uia_timeout_control_unavailable", unsafe {
            v2.SetAutoSetFocus(false)
        })?;
        api("uia_timeout_control_unavailable", unsafe {
            v2.SetConnectionTimeout(250)
        })?;
        api("uia_timeout_control_unavailable", unsafe {
            v2.SetTransactionTimeout(250)
        })?;
        let cache = api("uia_cache_unavailable", unsafe {
            automation.CreateCacheRequest()
        })?;
        api("uia_cache_unavailable", unsafe {
            cache.SetTreeScope(TreeScope_Element)
        })?;
        // Never cache ValueValue or LegacyIAccessibleValue; unknown/password controls are not value-readable.
        for p in [
            UIA_ProcessIdPropertyId,
            UIA_ControlTypePropertyId,
            UIA_NamePropertyId,
            UIA_AutomationIdPropertyId,
            UIA_BoundingRectanglePropertyId,
            UIA_IsEnabledPropertyId,
            UIA_IsOffscreenPropertyId,
            UIA_IsPasswordPropertyId,
            UIA_HasKeyboardFocusPropertyId,
            UIA_NativeWindowHandlePropertyId,
        ] {
            api("uia_cache_unavailable", unsafe { cache.AddProperty(p) })?;
        }
        let walker = api("uia_walker_unavailable", unsafe {
            automation.ControlViewWalker()
        })?;
        Ok(Self {
            automation,
            cache,
            walker,
            _apartment: apartment,
        })
    }
    fn root(&self, w: HWND) -> Result<IUIAutomationElement> {
        let _call = native_call("uia_root")?;
        api("uia_window_unavailable", unsafe {
            self.automation.ElementFromHandleBuildCache(w, &self.cache)
        })
    }
    fn under_root(
        &self,
        root: &IUIAutomationElement,
        element: &IUIAutomationElement,
    ) -> Result<()> {
        let start = Instant::now();
        let mut current = element.clone();
        for _ in 0..32 {
            checkpoint("uia_root_ancestry")?;
            if start.elapsed() > Duration::from_millis(1500) {
                break;
            }
            let _call = native_call("uia_root_ancestry_native")?;
            if api("uia_ancestry_compare", unsafe {
                self.automation.CompareElements(root, &current)
            })?
            .as_bool()
            {
                return Ok(());
            }
            match self.related(&current, 2)? {
                Some(parent) => current = parent,
                None => {
                    return Err(fault(
                        "uia_wrong_root",
                        "Focused/value element is not a descendant of the allocated native window.",
                    ));
                }
            }
        }
        Err(fault(
            "uia_ancestry_incomplete",
            "Could not attest element ancestry within 32 levels / 1500 ms; no input.",
        ))
    }
    // Preserve S_OK + null (end of tree) separately from E_POINTER/provider
    // errors; the ergonomic Windows wrapper maps both to an Error.
    fn related(&self, e: &IUIAutomationElement, kind: u8) -> Result<Option<IUIAutomationElement>> {
        let mut raw = std::ptr::null_mut();
        let v = self.walker.vtable();
        let method = match kind {
            0 => v.GetFirstChildElementBuildCache,
            1 => v.GetNextSiblingElementBuildCache,
            _ => v.GetParentElementBuildCache,
        };
        let hr = unsafe {
            method(
                self.walker.as_raw(),
                e.as_raw(),
                self.cache.as_raw(),
                &mut raw,
            )
        };
        let element = if raw.is_null() {
            None
        } else {
            Some(unsafe { IUIAutomationElement::from_raw(raw) })
        };
        api("uia_walk_failed", hr.ok())?;
        Ok(element)
    }
    fn hit_matches(
        &self,
        e: &IUIAutomationElement,
        point: [f64; 2],
        start: Instant,
    ) -> Result<Option<bool>> {
        let call = native_call("uia_element_from_point")?;
        let hit = unsafe {
            self.automation.ElementFromPoint(POINT {
                x: point[0].round() as i32,
                y: point[1].round() as i32,
            })
        };
        drop(call);
        let Ok(mut hit) = hit else { return Ok(None) };
        for _ in 0..8 {
            checkpoint("uia_hit_ancestor")?;
            if start.elapsed() > Duration::from_millis(1500) {
                return Ok(None);
            }
            let call = native_call("uia_hit_compare")?;
            let same = unsafe { self.automation.CompareElements(e, &hit) }.ok();
            if same.is_some_and(|b| b.as_bool()) {
                return Ok(Some(true));
            }
            let parent = self.related(&hit, 2);
            drop(call);
            match parent {
                Ok(Some(p)) => hit = p,
                Ok(None) => return Ok(Some(false)),
                Err(_) => return Ok(None),
            }
        }
        Ok(None)
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Walk {
    pub root_runtime_id: Vec<i32>,
    pub elements: Vec<Element>,
    pub details: Vec<serde_json::Value>,
    pub modal: Option<bool>,
    pub complete: bool,
    pub truncation: Option<String>,
    pub elapsed_ms: f64,
    pub document_state: Option<String>,
    pub document_context_unavailable: Option<String>,
}

/// Pixel preflight still attests the native UIA root identity. No descendant
/// enumeration or document values are needed by its full-image hash binding.
/// This is NOT exposed as a full observation or a semantic authority snapshot.
pub(crate) fn root_identity(w: HWND) -> Result<Walk> {
    let start = Instant::now();
    let client = Client::input()?;
    let root = client.root(w)?;
    let _call = native_call("uia_pixel_preflight_root_identity")?;
    let root_runtime_id = runtime_id(&root)?;
    let modal =
        unsafe { root.GetCurrentPatternAs::<IUIAutomationWindowPattern>(UIA_WindowPatternId) }
            .ok()
            .and_then(|p| unsafe { p.CurrentIsModal() }.ok())
            .map(|b| b.as_bool());
    checkpoint("uia_pixel_preflight_root_returned")?;
    Ok(Walk {
        root_runtime_id,
        elements: vec![],
        details: vec![],
        modal,
        complete: false,
        truncation: Some("root identity only; descendant tree not requested".into()),
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.,
        document_state: None,
        document_context_unavailable: Some(
            "not read for native-policy pixel hash revalidation".into(),
        ),
    })
}
pub fn walk(w: HWND, pid: u32, epoch: &str) -> Result<Walk> {
    let start = Instant::now();
    let client = Client::new()?;
    let root = client.root(w)?;
    let call = native_call("uia_root_identity")?;
    let root_id = runtime_id(&root)?;
    let modal =
        unsafe { root.GetCurrentPatternAs::<IUIAutomationWindowPattern>(UIA_WindowPatternId) }
            .ok()
            .and_then(|p| unsafe { p.CurrentIsModal() }.ok())
            .map(|b| b.as_bool());
    drop(call);
    let mut stack = vec![(root, 0usize)];
    let mut elements = vec![];
    let mut details = vec![];
    let mut truncation = None;
    let mut documents = crate::document_context::Documents::default();
    while let Some((e, depth)) = stack.pop() {
        checkpoint("uia_walk_node")?;
        if details.len() >= 128 || start.elapsed() > Duration::from_millis(1500) {
            truncation = Some("128 nodes / 1500 ms cooperative traversal budget".into());
            break;
        }
        let call = native_call("uia_node_metadata")?;
        let ep = unsafe { e.CachedProcessId() }.ok();
        let pass = unsafe { e.CachedIsPassword() }.ok().map(|b| b.as_bool());
        let enabled = unsafe { e.CachedIsEnabled() }.ok().map(|b| b.as_bool());
        let offscreen = unsafe { e.CachedIsOffscreen() }.ok().map(|b| b.as_bool());
        let focus = unsafe { e.CachedHasKeyboardFocus() }
            .ok()
            .map(|b| b.as_bool());
        let bounds = unsafe { e.CachedBoundingRectangle() }.ok().and_then(rect);
        let rid = runtime_id(&e).ok();
        let name = if pass == Some(false) {
            unsafe { e.CachedName() }
                .ok()
                .map(|s| s.to_string().chars().take(256).collect::<String>())
        } else {
            None
        };
        let automation_id = if pass == Some(false) {
            unsafe { e.CachedAutomationId() }
                .ok()
                .map(|s| s.to_string().chars().take(128).collect::<String>())
        } else {
            None
        };
        let control = unsafe { e.CachedControlType() }.ok().map(|x| x.0);
        drop(call);
        let id = rid
            .as_ref()
            .map(|r| digest(format!("{}:{pid}:{epoch}:{r:?}", window_id(w)).as_bytes()));
        let mut value_revision = None;
        if matches!(control, Some(50004 | 50030)) {
            // Read only visible Edit/Document controls; no password or unknown
            // classification is silently converted to an empty document proof.
            if offscreen != Some(true) {
                if pass != Some(false) || offscreen.is_none() || id.is_none() {
                    documents.reject("document visibility/password/runtime identity unknown");
                } else {
                    match bounded_document_value(&e) {
                        Ok(value) => {
                            value_revision = Some(digest(value.as_bytes()));
                            documents.push(id.as_deref().unwrap(), &value);
                        }
                        Err(error) => documents.reject(&error.code),
                    }
                }
            }
        } else if control.is_none() && offscreen != Some(true) {
            documents.reject("visible control type unknown");
        }
        if let (Some(id), Some(bounds)) = (&id, bounds) {
            let point = [
                f64::from(bounds.x) + f64::from(bounds.width) / 2.,
                f64::from(bounds.y) + f64::from(bounds.height) / 2.,
            ];
            let occluded = match hit_window(point) {
                Some(h) if h != window_id(w) => Some(true),
                Some(_) => client.hit_matches(&e, point, start)?.map(|hit| !hit),
                None => None,
            };
            let visible = match (offscreen, enabled, pass, ep) {
                // Control-view descendants can legitimately be hosted by another
                // process (ApplicationFrameHost/UWP). Keep provider PID distinct
                // from window PID; input/value use re-proves native root ancestry.
                (Some(false), Some(true), Some(false), Some(p)) if p > 0 => Some(true),
                (Some(true), _, _, _) | (_, Some(false), _, _) => Some(false),
                _ => None,
            };
            let revision = digest(
                serde_json::to_string(&(
                    ep,
                    rid.as_ref(),
                    &name,
                    &automation_id,
                    bounds,
                    enabled,
                    offscreen,
                    pass,
                    focus,
                    control,
                    &value_revision,
                ))
                .unwrap_or_default()
                .as_bytes(),
            );
            elements.push(Element {
                id: id.clone(),
                name: name.clone().unwrap_or_default(),
                point,
                visible,
                occluded,
                state_revision: Some(revision),
            });
        }
        details.push(serde_json::json!({"element_id":id,"value_revision":value_revision,"runtime_id":rid,"provider_pid":ep,"scope_source":"bounded native control-view descendants of HWND root; provider PID need not equal HWND owner PID; re-prove root ancestry on input/value use","control_type":control,"automation_id":automation_id,"rect_physical":bounds,"enabled":enabled,"offscreen":offscreen,"password":pass,"keyboard_focus":focus,"depth":depth,"occlusion_scope":"center WindowFromPoint plus UIA ElementFromPoint and at most 8 native ancestor comparisons; no whole-surface claim"}));
        if depth != 0 {
            let call = native_call("uia_next_sibling")?;
            let sibling = client.related(&e, 1);
            drop(call);
            match sibling {
                Ok(Some(s)) => stack.push((s, depth)),
                Ok(None) => (),
                Err(err) => {
                    truncation = Some(err.to_string());
                    break;
                }
            }
        }
        if depth < 8 {
            let call = native_call("uia_first_child")?;
            let child = client.related(&e, 0);
            drop(call);
            match child {
                Ok(Some(c)) => stack.push((c, depth + 1)),
                Ok(None) => (),
                Err(err) => {
                    truncation = Some(err.to_string());
                    break;
                }
            }
        } else {
            truncation = Some("depth 8 limit; coverage incomplete".into());
        }
    }
    checkpoint("uia_walk_returned")?;
    let document_state = documents.finish(truncation.is_none());
    Ok(Walk {
        root_runtime_id: root_id,
        elements,
        details,
        modal,
        complete: truncation.is_none(),
        truncation,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.,
        document_state,
        document_context_unavailable: documents.unavailable,
    })
}
pub fn value_matches(w: HWND, pid: u32, epoch: &str, wanted: &str, expected: &str) -> Result<bool> {
    let client = Client::new()?;
    let root = client.root(w)?;
    let start = Instant::now();
    let mut stack = vec![(root.clone(), 0usize)];
    let mut count = 0;
    while let Some((e, depth)) = stack.pop() {
        checkpoint("uia_value_oracle")?;
        count += 1;
        if count > 128 || start.elapsed() > Duration::from_millis(1500) {
            return Err(fault(
                "verification_coverage_incomplete",
                "Exact native element was not reached within the bounded traversal.",
            ));
        }
        let _call = native_call("uia_value_identity")?;
        let rid = runtime_id(&e)?;
        let id = digest(format!("{}:{pid}:{epoch}:{rid:?}", window_id(w)).as_bytes());
        if id == wanted {
            client.under_root(&root, &e)?;
            if api("uia_value_scope", unsafe { e.CurrentProcessId() })? <= 0
                || api("uia_password_classification", unsafe {
                    e.CurrentIsPassword()
                })?
                .as_bool()
            {
                return Err(fault(
                    "sensitive_or_wrong_scope",
                    "No value is read from a password or an unclassified provider.",
                ));
            }
            let actual = if let Some(value) =
                pattern::<IUIAutomationValuePattern>(&e, UIA_ValuePatternId)?
            {
                api("uia_value_read_failed", unsafe { value.CurrentValue() })?
            } else if let Some(text) = pattern::<IUIAutomationTextPattern>(&e, UIA_TextPatternId)? {
                let range = api("uia_text_range_unavailable", unsafe {
                    text.DocumentRange()
                })?;
                api("uia_text_read_failed", unsafe { range.GetText(16385) })?
            } else {
                return Err(fault(
                    "uia_value_pattern_unavailable",
                    "Neither native ValuePattern nor bounded TextPattern is available.",
                ));
            };
            client.under_root(&root, &e)?;
            return Ok(actual.len() <= 16384 && actual == expected);
        }
        if depth != 0
            && let Some(s) = client.related(&e, 1)?
        {
            stack.push((s, depth));
        }
        if depth < 8
            && let Some(c) = client.related(&e, 0)?
        {
            stack.push((c, depth + 1));
        }
    }
    Err(fault(
        "verification_element_gone",
        "Exact runtime ID no longer exists; do not substitute a same-name control.",
    ))
}

/// S_OK + null means pattern absent, not a provider failure to be silently hidden.
fn pattern<T: Interface>(e: &IUIAutomationElement, id: UIA_PATTERN_ID) -> Result<Option<T>> {
    let mut raw = std::ptr::null_mut();
    let hr = unsafe { (e.vtable().GetCurrentPattern)(e.as_raw(), id, &mut raw) };
    let value = if raw.is_null() {
        None
    } else {
        Some(unsafe { IUnknown::from_raw(raw) })
    };
    api("uia_pattern_read_failed", hr.ok())?;
    value
        .map(|p| api("uia_pattern_type_mismatch", p.cast()))
        .transpose()
}

fn require_writable(e: &IUIAutomationElement) -> Result<()> {
    let kind = api("uia_control_type_unknown", unsafe {
        e.CurrentControlType()
    })?;
    if kind != UIA_EditControlTypeId && kind != UIA_DocumentControlTypeId {
        return Err(fault(
            "text_target_not_editable",
            "Ground text in the exact focused native Edit/Document element, not a container.",
        ));
    }
    let readonly = if let Some(v) = pattern::<IUIAutomationValuePattern>(e, UIA_ValuePatternId)? {
        api("uia_readonly_unknown", unsafe { v.CurrentIsReadOnly() })?.as_bool()
    } else if let Some(t) = pattern::<IUIAutomationTextPattern>(e, UIA_TextPatternId)? {
        let range = api("uia_text_range_unavailable", unsafe { t.DocumentRange() })?;
        let value = api("uia_readonly_unknown", unsafe {
            range.GetAttributeValue(UIA_IsReadOnlyAttributeId)
        })?;
        if unsafe { value.Anonymous.Anonymous.vt } != VT_BOOL {
            return Err(fault(
                "uia_readonly_unknown",
                "TextPattern did not attest IsReadOnly as a boolean.",
            ));
        }
        unsafe { value.Anonymous.Anonymous.Anonymous.boolVal }.as_bool()
    } else {
        return Err(fault(
            "text_target_not_editable",
            "No native writable text pattern; no input.",
        ));
    };
    if readonly {
        return Err(fault(
            "text_target_readonly",
            "Native control is read-only; no input.",
        ));
    }
    Ok(())
}
