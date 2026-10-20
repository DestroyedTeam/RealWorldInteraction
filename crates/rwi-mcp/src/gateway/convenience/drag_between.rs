//! Resolve both endpoint views before claiming their shared native input domain.
use super::*;

impl Gateway {
    pub(super) fn paired_device(&self, a: &DragBetweenArgs) -> Result<String, DeviceResult> {
        let route = |e: &DragEndpointArgs| {
            self.route_id(
                "computer.resolve_view",
                &json!({"target":e.target,"view_id":e.view_id}),
            )
        };
        let source = route(&a.source)?;
        let destination = route(&a.destination)?;
        if source != destination {
            return Err(DeviceResult::error(
                "drag_domain_mismatch",
                "Paired drag cannot forward observations/input across devices.",
            ));
        }
        Ok(source)
    }
}

pub(super) fn validate_drag_between(a: &DragBetweenArgs) -> Result<(), DeviceResult> {
    if !(16..=5000).contains(&a.duration_ms)
        || !device::valid_id(&a.request_id)
        || a.source.target == a.destination.target
        || [&a.source, &a.destination].iter().any(|e| {
            e.target.is_empty()
                || e.target.len() > 256
                || e.view_id.is_empty()
                || e.view_id.len() > 256
                || !e.point.iter().all(|p| p.is_finite())
        })
    {
        return Err(invalid(
            "Use distinct selected targets, exact endpoint views, finite pixels, stable request_id and duration 16..5000ms.",
        ));
    }
    Ok(())
}

pub(super) async fn paired_drag<F, Fut>(a: DragBetweenArgs, mut call: F) -> DeviceResult
where
    F: FnMut(&'static str, Value, bool) -> Fut,
    Fut: Future<Output = DeviceResult>,
{
    if let Err(error) = validate_drag_between(&a) {
        return error;
    }
    let mut resolved = Vec::with_capacity(2);
    for endpoint in [&a.source, &a.destination] {
        let r = call(
            "computer.resolve_view",
            json!({"target":endpoint.target,"view_id":endpoint.view_id,"x":endpoint.point[0],"y":endpoint.point[1]}),
            false,
        )
        .await;
        if r.is_error {
            return r;
        }
        let point = match serde_json::from_value::<[f64; 2]>(r.data["point"].clone()) {
            Ok(p) if p.iter().all(|n| n.is_finite()) => p,
            _ => {
                return DeviceResult::error("invalid_mapping_reply", "No paired input dispatched.");
            }
        };
        let Some(reference) = r.data["reference"].as_str().filter(|s| !s.is_empty()) else {
            return DeviceResult::error(
                "invalid_mapping_reply",
                "Missing exact endpoint reference; no input.",
            );
        };
        resolved.push((reference.to_owned(), point));
    }
    let offset = [
        resolved[1].1[0] - resolved[0].1[0],
        resolved[1].1[1] - resolved[0].1[1],
    ];
    if !offset.iter().all(|p| p.is_finite()) {
        return DeviceResult::error(
            "invalid_mapping_reply",
            "Mapped endpoint offset overflow; no input.",
        );
    }
    leased(
        &a.source.target,
        &a.request_id,
        "computer.act",
        json!({"reference":resolved[0].0,"intent":{"drag_between":{"target":a.destination.target,"reference":resolved[1].0,"path":{"waypoints":[{"at_ms":0,"offset":[0,0]},{"at_ms":a.duration_ms,"offset":offset}]}}}}),
        call,
    )
    .await
}
