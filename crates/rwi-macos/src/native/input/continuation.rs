//! Invocation-local continuation authority. No exported reference or renewed TTL.
use super::*;
use std::sync::atomic::AtomicU64;

/// Native K values are retained AX objects compared using CFEqual, not hashes.
pub(super) struct Facts<K> {
    pub identity: Identity,
    pub process: String,
    pub scope: Scope,
    pub window: K,
    pub window_rect: Rect,
    pub hit: K,
    pub hit_rect: Rect,
    pub hit_id: String,
    pub focused: Option<K>,
    pub point: [f64; 2],
    pub layout: String,
    pub mapping_ready: bool,
    pub permissions: bool,
    pub modal: Option<bool>,
    pub occluded: bool,
    pub enabled: Option<bool>,
    pub writable: Option<bool>,
    pub class: TextClass,
}

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Pointer,
    DoubleClick,
    Text,
    Keyboard,
    Scroll,
}
impl Kind {
    pub(super) fn of(intent: &Intent) -> Self {
        match intent {
            Intent::DoubleClick => Self::DoubleClick,
            Intent::Text => Self::Text,
            Intent::KeyChord => Self::Keyboard,
            Intent::Scroll => Self::Scroll,
            _ => Self::Pointer,
        }
    }
    pub(super) fn needs_focus(self) -> bool {
        matches!(self, Self::DoubleClick | Self::Text | Self::Keyboard)
    }
}

impl<K: PartialEq> Facts<K> {
    pub(super) fn validate(&self, expected: &Binding, target: &Resolved, kind: Kind) -> Result<()> {
        let mut changed = Vec::new();
        if self.scope != target.scope {
            changed.push("scope");
        }
        if self.point != target.point {
            changed.push("point");
        }
        if self.layout != expected.layout {
            changed.push("layout");
        }
        if !self.mapping_ready {
            changed.push("mapping");
        }
        if !self.window_rect.contains(self.point) {
            changed.push("window_rect");
        }
        if !self.hit_rect.contains(self.point) {
            changed.push("hit_rect");
        }
        if !self.permissions {
            changed.push("permissions");
        }
        if self.modal != Some(false) {
            changed.push("modal");
        }
        if self.occluded {
            changed.push("occluded");
        }
        if self.enabled == Some(false) {
            changed.push("enabled_disabled");
        }
        if !changed.is_empty() {
            let mut error = fault(
                "input_boundary_changed",
                &format!(
                    "Native facts changed or unknown ({}); stop remaining batches.",
                    changed.join(",")
                ),
            );
            error.diagnostic = Some(Box::new(serde_json::json!({
                "changed":changed,
                "point":self.point,
                "window_bounds":[self.window_rect.x,self.window_rect.y,self.window_rect.w,self.window_rect.h],
                "hit_bounds":[self.hit_rect.x,self.hit_rect.y,self.hit_rect.w,self.hit_rect.h],
                "mapping_ready":self.mapping_ready,
                "modal":self.modal,
                "occluded":self.occluded,
                "permissions":self.permissions,
                "enabled":self.enabled,
                "current_batch_native_input_dispatched":false,
                "prior_batches_may_have_dispatched":true,
                "replay_allowed":false,
            })));
            return Err(error);
        }
        if !self.class.readable()
            || target
                .scope
                .element
                .as_ref()
                .is_some_and(|id| id != &self.hit_id)
        {
            return Err(fault(
                "element_changed",
                "The exact hit element is replaced, secure or unclassified; stop remaining batches.",
            ));
        }
        if kind.needs_focus() && self.focused.is_none() {
            return Err(fault(
                "focus_unknown",
                "No exact native focused AX element; do not infer a continuation focus.",
            ));
        }
        if matches!(kind, Kind::Text | Kind::Keyboard)
            && (self.focused.as_ref() != Some(&self.hit) || target.scope.element.is_none())
        {
            return Err(fault(
                "keyboard_target_not_focused",
                "Keyboard batches require the original exact focused native hit element, not a pixel or same-PID replacement.",
            ));
        }
        if matches!(kind, Kind::Text)
            && (!self.class.input_allowed() || self.writable != Some(true))
        {
            return Err(fault(
                "text_target_not_writable",
                "The exact focused text element must remain explicitly nonsecure and natively writable.",
            ));
        }
        Ok(())
    }
    fn text_resized_in_place(&self, now: &Self, kind: Kind) -> bool {
        matches!(kind, Kind::Text)
            && self.hit_rect.x == now.hit_rect.x
            && self.hit_rect.y == now.hit_rect.y
            && now.hit_rect.valid()
            && now.hit_rect != self.hit_rect
    }
    fn require_continuation(&self, now: &Self, kind: Kind) -> Result<()> {
        let grew = self.text_resized_in_place(now, kind);
        if now.identity != self.identity
            || now.process != self.process
            || now.scope != self.scope
            || now.window != self.window
            || now.hit != self.hit
            || (now.hit_id != self.hit_id && !grew)
            || now.window_rect != self.window_rect
            || (now.hit_rect != self.hit_rect && !grew)
            || now.point != self.point
            || now.layout != self.layout
            || now.class != self.class
        {
            let mut error = fault(
                "input_anchor_changed",
                "Native context, process, AX window/hit object or original geometry changed; continuation cannot authorize a replacement.",
            );
            error.diagnostic = Some(Box::new(serde_json::json!({
                "identity_changed":now.identity != self.identity,
                "process_changed":now.process != self.process,
                "scope_changed":now.scope != self.scope,
                "window_object_changed":now.window != self.window,
                "hit_object_changed":now.hit != self.hit,
                "hit_id_changed":now.hit_id != self.hit_id,
                "window_rect_changed":now.window_rect != self.window_rect,
                "hit_rect_before":[self.hit_rect.x,self.hit_rect.y,self.hit_rect.w,self.hit_rect.h],
                "hit_rect_after":[now.hit_rect.x,now.hit_rect.y,now.hit_rect.w,now.hit_rect.h],
                "point_changed":now.point != self.point,
                "layout_changed":now.layout != self.layout,
                "class_changed":now.class != self.class,
                "text_resize_in_place":grew,
                "replay_allowed":false
            })));
            return Err(error);
        }
        let focus_allowed = if matches!(kind, Kind::DoubleClick) {
            now.focused == self.focused || now.focused.as_ref() == Some(&self.hit)
        } else {
            now.focused == self.focused
        };
        if !focus_allowed {
            return Err(fault(
                "focus_changed",
                "Continuation focus left the original element; only double-click's original focus to original hit transition is allowed.",
            ));
        }
        Ok(())
    }
}

pub(super) struct Sequence<K> {
    kind: Kind,
    anchor: Option<Facts<K>>,
}
impl<K: PartialEq> Sequence<K> {
    pub(super) fn new(intent: &Intent) -> Self {
        Self {
            kind: Kind::of(intent),
            anchor: None,
        }
    }
    /// Anchor is committed only AFTER the first down/up pair returns. Any error
    /// aborts the caller's invocation; no external token can reuse this object.
    pub(super) fn pair<T>(
        &mut self,
        expected: &Binding,
        target: &Resolved,
        counter: &AtomicU64,
        events: (T, T),
        read: impl FnOnce(bool) -> Result<Facts<K>>,
        post: impl FnMut(&T),
    ) -> Result<()> {
        let mut candidate = None;
        dispatch::pair(
            counter,
            events.0,
            events.1,
            || {
                operation::checkpoint("macos_continuation_guard")?;
                let mut facts = read(self.anchor.is_none())?;
                if let Some(anchor) = &self.anchor {
                    anchor.require_continuation(&facts, self.kind)?;
                    if anchor.text_resized_in_place(&facts, self.kind) {
                        // Local normalization only after exact native object,
                        // focus, process and layout continuity is proven. No new
                        // public reference, geometry authority or replay token.
                        facts.hit_id = anchor.hit_id.clone();
                    }
                    facts.validate(expected, target, self.kind)?;
                } else {
                    facts.validate(expected, target, self.kind)?;
                    candidate = Some(facts);
                }
                operation::checkpoint("macos_continuation_ready")
            },
            post,
        )?;
        if let Some(first) = candidate {
            self.anchor = Some(first);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
