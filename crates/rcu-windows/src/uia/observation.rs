//! Shared native node metadata/revision reader; no UIA mutation or cached authority.
use super::*;

pub(super) struct Reader<'a> {
    client: &'a Client,
    w: HWND,
    pid: u32,
    epoch: &'a str,
    start: Instant,
    projection: crate::uia_focus::Projection,
    pub(super) documents: crate::document_context::Documents,
    pub(super) elements: Vec<Element>,
    pub(super) details: Vec<serde_json::Value>,
}
impl<'a> Reader<'a> {
    pub(super) fn new(
        client: &'a Client,
        w: HWND,
        pid: u32,
        epoch: &'a str,
        start: Instant,
        focus: Option<Vec<i32>>,
    ) -> Self {
        Self {
            client,
            w,
            pid,
            epoch,
            start,
            projection: crate::uia_focus::Projection::new(focus),
            documents: crate::document_context::Documents::default(),
            elements: vec![],
            details: vec![],
        }
    }
    pub(super) fn append(&mut self, e: &IUIAutomationElement, depth: Option<usize>) -> Result<()> {
        let (client, w, pid, epoch, start) =
            (self.client, self.w, self.pid, self.epoch, self.start);
        let call = native_call("uia_node_metadata")?;
        let ep = unsafe { e.CachedProcessId() }.ok();
        let pass = unsafe { e.CachedIsPassword() }.ok().map(|b| b.as_bool());
        let enabled = unsafe { e.CachedIsEnabled() }.ok().map(|b| b.as_bool());
        let offscreen = unsafe { e.CachedIsOffscreen() }.ok().map(|b| b.as_bool());
        let cached_focus = unsafe { e.CachedHasKeyboardFocus() }
            .ok()
            .map(|b| b.as_bool());
        let bounds = unsafe { e.CachedBoundingRectangle() }.ok().and_then(rect);
        let rid = runtime_id(e).ok();
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
        let focus = self.projection.focused(rid.as_ref());
        if self.projection.publish(id.as_deref()) {
            let mut value_revision = None;
            let mut classification_error = None;
            if matches!(control, Some(50004 | 50030)) {
                // Read only visible Edit/Document controls; no password or unknown
                // classification is silently converted to an empty document proof.
                if offscreen != Some(true) {
                    if pass != Some(false) || offscreen.is_none() || id.is_none() {
                        self.documents
                            .reject("document visibility/password/runtime identity unknown");
                    } else {
                        match bounded_document_value(e) {
                            Ok(value) => {
                                value_revision = Some(digest(value.as_bytes()));
                                self.documents.push(id.as_deref().unwrap(), &value);
                            }
                            Err(error)
                                if matches!(
                                    error.code.as_str(),
                                    "operation_cancelled" | "operation_deadline"
                                ) =>
                            {
                                return Err(error);
                            }
                            Err(error) => {
                                if crate::value_guard::blocks_input(&error.code) {
                                    classification_error = Some(error.code.clone());
                                }
                                self.documents.reject(&error.code);
                            }
                        }
                    }
                }
            } else if control.is_none() && offscreen != Some(true) {
                self.documents.reject("visible control type unknown");
            }
            if let (Some(id), Some(bounds)) = (&id, bounds) {
                let point = [
                    f64::from(bounds.x) + f64::from(bounds.width) / 2.,
                    f64::from(bounds.y) + f64::from(bounds.height) / 2.,
                ];
                let occluded = match hit_window(point) {
                    Some(h) if h != window_id(w) => Some(true),
                    Some(_) => client.hit_matches(e, point, start)?.map(|hit| !hit),
                    None => None,
                };
                let visible = if classification_error.is_some() {
                    None
                } else {
                    match (offscreen, enabled, pass, ep) {
                        // Control-view descendants can legitimately be hosted by another
                        // process (ApplicationFrameHost/UWP). Keep provider PID distinct
                        // from window PID; input/value use re-proves native root ancestry.
                        (Some(false), Some(true), Some(false), Some(p)) if p > 0 => Some(true),
                        (Some(true), _, _, _) | (_, Some(false), _, _) => Some(false),
                        _ => None,
                    }
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
                self.elements.push(Element {
                    id: id.clone(),
                    name: name.clone().unwrap_or_default(),
                    point,
                    visible,
                    occluded,
                    state_revision: Some(revision),
                });
            }
            self.details.push(serde_json::json!({"element_id":id,"value_revision":value_revision,"classification_error":classification_error,"runtime_id":rid,"provider_pid":ep,"scope_source":"bounded control-view tree plus prioritized exact focus proven under HWND root; provider PID need not equal HWND owner PID; re-prove ancestry on input/value use","control_type":control,"automation_id":automation_id,"rect_physical":bounds,"enabled":enabled,"offscreen":offscreen,"password":pass,"keyboard_focus":focus,"cached_keyboard_focus":cached_focus,"focus_source":"GetFocusedElementBuildCache + runtime ID + native root ancestry; cached flags are diagnostic only","depth":depth,"occlusion_scope":"center WindowFromPoint plus UIA ElementFromPoint and at most 8 native ancestor comparisons; no whole-surface claim"}));
        }
        Ok(())
    }
}
