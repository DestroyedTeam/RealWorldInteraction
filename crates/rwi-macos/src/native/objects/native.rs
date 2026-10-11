//! Only typed AX proxies may be retained across serialized backend operations.
//! General Cf values intentionally remain !Send/!Sync.
use super::{Objects, require_ancestor};
use crate::ffi::{self, Cf};
use crate::native::{AxElement, delegation, element_description};
use rwi_core::{Result, fault, operation};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

pub(in crate::native) struct Ax(Cf);
impl Ax {
    fn copy(element: &Cf) -> Result<Self> {
        if unsafe { ffi::CFGetTypeID(element.0) != ffi::AXUIElementGetTypeID() } {
            return Err(fault(
                "native_type_mismatch",
                "Expected an AXUIElement proxy, not a general CoreFoundation value.",
            ));
        }
        Ok(Self(element.retained()))
    }
}
// SAFETY: Ax owns a retained, type-checked AXUIElement proxy, not an AppKit
// view/observer/run-loop object or arbitrary mutable CF container. The backend
// serializes AX access, including messaging-timeout changes; this wrapper is
// Send, deliberately not Sync. Registry borrows never escape its mutex; copies
// returned to native callers are ordinary !Send Cf handles. CF retain/release
// and immutable proxy identity may move with the exclusively owned backend.
unsafe impl Send for Ax {}
impl PartialEq for Ax {
    fn eq(&self, other: &Self) -> bool {
        unsafe { ffi::CFEqual(self.0.0, other.0.0) != 0 }
    }
}

pub(in crate::native) fn describe(
    element: &Cf,
    root: &Cf,
    objects: &Mutex<Objects<Ax>>,
) -> Result<AxElement> {
    let mut description = element_description(element)?;
    description.element.id = objects
        .lock()
        .map_err(|_| {
            fault(
                "semantic_object_registry_unavailable",
                "Native object registry is unavailable; no substitution.",
            )
        })?
        .identify(Ax::copy(root)?, Ax::copy(element)?, &description.element.id)?;
    Ok(description)
}

pub(in crate::native) fn exact(
    root: &Cf,
    pid: u32,
    id: &str,
    objects: &Mutex<Objects<Ax>>,
) -> Result<Cf> {
    operation::checkpoint("macos_semantic_object_lookup")?;
    let element = objects
        .lock()
        .map_err(|_| {
            fault(
                "semantic_object_registry_unavailable",
                "Native object registry is unavailable; no substitution.",
            )
        })?
        .get(&Ax::copy(root)?, id)?
        .0
        .retained();
    // A previous AXPerformAction may have set 0.5s on this same retained
    // proxy. Restore the short probe budget before any metadata/ancestry read.
    let code = unsafe { ffi::AXUIElementSetMessagingTimeout(element.0, 0.05) };
    if code != 0 {
        return Err(fault(
            "semantic_probe_timeout_configuration_failed",
            "Cannot configure bounded retained-object readback; no action dispatched.",
        ));
    }
    let owner = element.pid()?;
    let epoch = delegation::validate_hosted_descendant(pid, owner)?;
    if element.attr("AXHidden").ok().and_then(|v| v.boolean()) == Some(true) {
        return Err(fault(
            "semantic_target_hidden",
            "The retained native target is explicitly hidden; no action dispatched.",
        ));
    }
    let start = Instant::now();
    let checkpoint = || {
        operation::checkpoint("macos_semantic_target_ancestry")?;
        if start.elapsed() >= Duration::from_millis(500) {
            return Err(fault(
                "semantic_ancestry_deadline",
                "Target ancestry validation exceeded its bounded read budget; no input.",
            ));
        }
        Ok(())
    };
    // AXWindow is an authoritative native relationship for standard windows,
    // not a geometry/PID guess. Sheets/popovers require their own parent chain.
    let same_window = element
        .attr("AXWindow")
        .ok()
        .is_some_and(|w| unsafe { ffi::CFEqual(w.0, root.0) != 0 });
    if !same_window {
        require_ancestor(
            Ax::copy(&element)?,
            &Ax::copy(root)?,
            |node| {
                let parent = node.0.attr("AXParent")?;
                delegation::validate_hosted_descendant(pid, parent.pid()?)?;
                Ax::copy(&parent)
            },
            checkpoint,
        )?;
    }
    checkpoint()?;
    if crate::native::process_epoch(owner)? != epoch {
        return Err(fault(
            "delegate_process_changed",
            "The exact retained provider changed during ancestry validation; no input.",
        ));
    }
    // Attributes/geometry must still match the object-qualified identity. Never
    // resolve a stale ID by registering a replacement object with similar data.
    let fresh = describe(&element, root, objects)?;
    if fresh.element.id != id {
        return Err(fault(
            "semantic_target_changed",
            "The retained object's current identity/geometry changed; no substitution.",
        ));
    }
    Ok(element)
}
