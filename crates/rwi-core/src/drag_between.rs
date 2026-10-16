//! Invocation-local paired input authority. Public arguments are opaque references;
//! native endpoints are constructed only after both core revalidations.
use crate::*;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Destination {
    pub target: String,
    pub reference: String,
    pub path: trajectory::DragPath,
}
impl Destination {
    pub fn validate(&self) -> Result<()> {
        if self.target.is_empty()
            || self.target.len() > 256
            || self.reference.is_empty()
            || self.reference.len() > 256
        {
            return Err(fault(
                "invalid_drag_endpoint",
                "Use exact bounded destination target/reference.",
            ));
        }
        self.path.validate()
    }
}
/// Not deserializable: a remote caller cannot supply a native grant.
#[derive(Clone)]
pub struct Endpoint {
    pub identity: Identity,
    pub binding: Binding,
    pub resolved: Resolved,
}
/// Native facts used to reject a changed endpoint during a held gesture.
#[derive(Clone, Debug, PartialEq)]
pub struct Surface {
    pub identity: Identity,
    pub scope: Scope,
    pub process_start: String,
    pub native_epoch: String,
    pub layout: String,
    pub bounds: [f64; 4],
}
pub struct Pair {
    pub source: Endpoint,
    pub destination: Endpoint,
    pub path: trajectory::DragPath,
}

pub(super) fn endpoint_matches(origin: [f64; 2], offset: [f64; 2], point: [f64; 2]) -> bool {
    // Permit only floating-point mapping roundoff, never a pixel-sized alternate
    // drop site. Negative/remote coordinates remain valid in a bound desktop.
    (0..2).all(|i| {
        origin[i].is_finite()
            && offset[i].is_finite()
            && point[i].is_finite()
            && (origin[i] + offset[i] - point[i]).abs() <= 1e-7
    })
}

impl Engine {
    pub(super) fn prepare_drag_destination(
        &mut self,
        source: &str,
        destination: &Destination,
    ) -> Result<Endpoint> {
        if source == destination.target {
            return Err(fault(
                "distinct_drag_endpoints_required",
                "Use ordinary drag within one target; paired drag requires independent targets.",
            ));
        }
        let identity = self.identity(source)?;
        if self.identity(&destination.target)? != identity {
            return Err(fault(
                "drag_domain_mismatch",
                "Both endpoints must have the same host/guest/session/seat/desktop identity.",
            ));
        }
        if self.backend(&destination.target)?.capabilities()["paired_pointer_drag"]["implemented"]
            != true
        {
            return Err(fault(
                "capability_unavailable",
                "Destination backend does not advertise paired native drag; no input or fallback.",
            ));
        }
        let mut resolved = self
            .refs
            .iter()
            .find(|r| r.target == destination.target && r.reference == destination.reference)
            .cloned()
            .ok_or_else(|| {
                fault(
                    "unknown_reference",
                    "Resolve the exact destination target from its own fresh frame.",
                )
            })?;
        if resolved.placement_only
            || resolved.native_action.is_some()
            || resolved.source_region.is_some()
        {
            return Err(fault(
                "invalid_drag_endpoint_class",
                "Paired drag requires input-grade full-window pointer endpoints.",
            ));
        }
        let snapshot = self.snapshot(&destination.target, &resolved.frame)?;
        if snapshot.at.elapsed() > Duration::from_secs(INPUT_PROPOSAL_MAX_AGE_SECS) {
            return Err(fault(
                "stale_reference",
                "Destination proposal expired; no input sent.",
            ));
        }
        let before = snapshot.scene.clone();
        let current = self.backend(&destination.target)?.revalidate(&resolved)?;
        if current.identity != identity || before.identity != identity {
            return Err(fault(
                "drag_domain_mismatch",
                "Native destination identity changed; no cross-device fallback.",
            ));
        }
        if !before.mutation_ready || !current.mutation_ready || current.blocked != Some(false) {
            return Err(fault(
                "drag_destination_unavailable",
                "Destination input, modal or visibility facts are not ready.",
            ));
        }
        let expected = before.binding.as_ref().ok_or_else(|| {
            fault(
                "target_binding_unknown",
                "Destination native binding is required.",
            )
        })?;
        if !evaluate_grounding(expected, &current, &resolved).0 {
            return Err(fault(
                "stale_reference",
                "Destination grounding changed; no source down sent.",
            ));
        }
        resolved.pixel_anchor_used = (resolved.pixel_anchor.is_some()
            || resolved.red_marker_anchor.is_some())
            && current.binding.as_ref().map(|b| &b.content) != Some(&expected.content);
        Ok(Endpoint {
            identity,
            binding: expected.clone(),
            resolved: resolved.value,
        })
    }
    pub(super) fn dispatch_drag_pair(
        &mut self,
        pair: &Pair,
        source_target: &str,
        destination_target: &str,
    ) -> Result<Option<bool>> {
        // One iterator produces disjoint mutable backend borrows; never remove
        // a registered target or expose a temporarily missing destination.
        let mut source = None;
        let mut destination = None;
        for (name, target) in &mut self.targets {
            if name == source_target {
                source = Some(&mut target.backend);
            } else if name == destination_target {
                destination = Some(&mut target.backend);
            }
        }
        let source = source.ok_or_else(|| fault("unknown_target", "Source target retired."))?;
        let destination =
            destination.ok_or_else(|| fault("unknown_target", "Destination target retired."))?;
        source.drag_between(pair, &mut **destination)
    }
}
#[cfg(test)]
mod tests;
