//! Read-only image projection and exact-frame view registry.
use super::{CoreResult, Engine, ImageArgs, fault, value};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rmcp::model::{CallToolResult, Content};
use rwi_imaging::{Rect, ViewMapping};
use serde_json::json;
use std::{collections::VecDeque, sync::Mutex};

#[derive(Clone)]
pub(super) struct StoredView {
    pub(super) target: String,
    pub(super) frame: String,
    pub(super) artifact_id: String,
    pub(super) mapping: ViewMapping,
}
pub(super) fn make_view(
    e: &Engine,
    views: &Mutex<VecDeque<(String, StoredView)>>,
    a: &ImageArgs,
) -> CoreResult<CallToolResult> {
    let (original, scene) = e.original(&a.target, &a.frame, &a.artifact_id)?;
    let crop = a
        .crop
        .map(|[x, y, width, height]| -> CoreResult<Rect> {
            Ok(Rect {
                x: i32::try_from(x).map_err(|_| fault("invalid_crop", "Crop x is too large."))?,
                y: i32::try_from(y).map_err(|_| fault("invalid_crop", "Crop y is too large."))?,
                width,
                height,
            })
        })
        .transpose()?;
    let (png, mapping) = rwi_imaging::view(original, crop, a.max_edge, scene.pixel_to_desktop)?;
    if mapping.original_size != scene.image_size {
        return Err(fault(
            "artifact_dimension_mismatch",
            "Original PNG dimensions do not match this frame metadata; do not resolve its pixels.",
        ));
    }
    // A visual-only scene deliberately has no binding or pixel-to-desktop
    // input mapping. Its original artifact and cropped tiles stay readable,
    // but no resolve_view-capable ID is issued or retained.
    let id = if scene.binding.is_some() && scene.pixel_to_desktop.is_some() {
        let id = rwi_core::digest(
            serde_json::to_string(&(&a.target, &a.frame, &a.artifact_id, &mapping))
                .unwrap()
                .as_bytes(),
        );
        let mut views = views.lock().map_err(|_| {
            fault(
                "view_registry_unavailable",
                "Reobserve in a new connection.",
            )
        })?;
        views.retain(|(key, _)| key != &id);
        views.push_back((
            id.clone(),
            StoredView {
                target: a.target.clone(),
                frame: a.frame.clone(),
                artifact_id: a.artifact_id.clone(),
                mapping: mapping.clone(),
            },
        ));
        while views.len() > 64 {
            views.pop_front();
        }
        Some(id)
    } else {
        None
    };
    let input_reference_available = id.is_some();
    let mut r = value(
        json!({"view_id":id,"target":a.target,"frame":a.frame,"original_artifact_id":a.artifact_id,"original_sha256":rwi_core::digest(original),"original_bytes":original.len(),"view_sha256":rwi_core::digest(&png),"view_bytes":png.len(),"mapping":mapping,"original_unchanged":true,"resolve_tool":input_reference_available.then_some("computer.resolve_view"),"input_reference_available":input_reference_available}),
    )?;
    r.content
        .push(Content::image(STANDARD.encode(png), "image/png"));
    Ok(r)
}
