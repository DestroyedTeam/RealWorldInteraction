//! Exact engine observation methods, separated by responsibility.
use super::*;

impl Engine {
    pub fn observe(&mut self, target: &str) -> Result<Observation> {
        self.observe_with_region(target, None)
    }
    pub fn observe_with_region(
        &mut self,
        target: &str,
        region: Option<[u32; 4]>,
    ) -> Result<Observation> {
        operation::checkpoint("observe")?;
        let mut scene = self.scene(target, region)?;
        operation::checkpoint("capture_returned")?;
        let frame = id();
        let mut artifact = None;
        let mut hash = None;
        if scene.png.len() > MAX_FRAME_BYTES {
            return Err(fault(
                "screenshot_size_limit",
                "Native PNG exceeds the 32 MiB original-artifact limit; distinguish capture from encoding/transport. Do not change desktop resolution.",
            ));
        }
        if !scene.png.is_empty() {
            let bytes = std::mem::take(&mut scene.png);
            let h = digest(&bytes);
            let aid = id();
            while self.artifact_bytes() + bytes.len() > ARTIFACT_BUDGET
                || self.artifacts.len() >= SNAPSHOT_LIMIT
            {
                self.artifacts.pop_front();
            }
            self.artifacts.push_back(Artifact {
                id: aid.clone(),
                bytes,
                hash: h.clone(),
            });
            artifact = Some(aid);
            hash = Some(h);
        }
        if serde_json::to_vec(&scene)
            .map_err(|_| fault("invalid_scene", "Inspect adapter metadata."))?
            .len()
            > 256 * 1024
        {
            return Err(fault(
                "observation_budget",
                "Narrow the accessibility tree.",
            ));
        }
        self.snapshots.push_back((
            frame.clone(),
            Snapshot {
                target: target.into(),
                at: Instant::now(),
                scene: scene.clone(),
                artifact_id: artifact.clone(),
                source_region: region,
            },
        ));
        while self.snapshots.len() > SNAPSHOT_LIMIT {
            self.snapshots.pop_front();
        }
        let popups = match &scene.binding {
            Some(binding) => self
                .backend(target)?
                .popups(&binding.scope)
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let mut adapter_status = self.backend(target)?.capabilities();
        if let Some(capabilities) = adapter_status.as_object_mut() {
            capabilities.insert(
                "input_proposal_max_age_ms".into(),
                serde_json::json!(INPUT_PROPOSAL_MAX_AGE_SECS * 1000),
            );
        }
        Ok(Observation {
            frame,
            artifact_id: artifact,
            png_sha256: hash,
            scene,
            popups,
            adapter_status,
        })
    }
    pub fn artifact_bytes(&self) -> usize {
        self.artifacts.iter().map(|a| a.bytes.len()).sum()
    }
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }
    pub fn artifact(
        &self,
        aid: &str,
        offset: usize,
        length: usize,
    ) -> Result<(Vec<u8>, String, usize)> {
        let a = self.artifacts.iter().find(|a| a.id == aid).ok_or_else(|| {
            fault(
                "artifact_expired",
                "Capture again; artifacts are held under a 64 MiB total budget.",
            )
        })?;
        if length == 0 || length > INLINE_BYTES || offset > a.bytes.len() {
            return Err(fault(
                "invalid_range",
                "Use a bounded byte range within the raw PNG.",
            ));
        }
        let end = offset.saturating_add(length).min(a.bytes.len());
        Ok((a.bytes[offset..end].to_vec(), a.hash.clone(), a.bytes.len()))
    }
    /// Internal full-original access for bounded view encoding; not a raw transport chunk.
    pub fn original(&self, target: &str, frame: &str, aid: &str) -> Result<(&[u8], &Scene)> {
        let s = self.snapshot(target, frame)?;
        let a = self
            .artifacts
            .iter()
            .find(|a| a.id == aid)
            .ok_or_else(|| fault("artifact_expired", "Observe again."))?;
        if s.artifact_id.as_deref() != Some(aid) {
            return Err(fault(
                "artifact_frame_mismatch",
                "Use the original artifact from this exact frame.",
            ));
        }
        Ok((&a.bytes, &s.scene))
    }
    pub(super) fn snapshot(&self, target: &str, frame: &str) -> Result<&Snapshot> {
        self.snapshots
            .iter()
            .find(|(id, s)| id == frame && s.target == target)
            .map(|(_, s)| s)
            .ok_or_else(|| fault("unknown_frame", "Observe this exact target again."))
    }
    pub fn resolve(&mut self, target: &str, frame: &str, locator: Locator) -> Result<Resolved> {
        self.resolve_action(target, frame, locator, None)
    }
    pub(super) fn resolve_action(
        &mut self,
        target: &str,
        frame: &str,
        locator: Locator,
        native_action: Option<&str>,
    ) -> Result<Resolved> {
        let use_local_anchor = self.backend(target)?.local_pixel_anchor();
        let snapshot = self.snapshot(target, frame)?;
        let scene = &snapshot.scene;
        let binding = scene.binding.as_ref().ok_or_else(|| {
            fault(
                "target_binding_unknown",
                "Native window/PID binding is required.",
            )
        })?;
        let mut scope = binding.scope.clone();
        let mut element_revision = None;
        let placement_only = matches!(locator, Locator::Window);
        let mut pixel = None;
        let mut red_marker_cluster = false;
        let point = match locator {
            locator @ (Locator::Semantic { .. } | Locator::Element { .. }) => {
                let matches = scene
                    .elements
                    .iter()
                    .filter(|e| match &locator {
                        Locator::Semantic { name } => e.name == *name,
                        Locator::Element { id } => e.id == *id,
                        _ => false,
                    })
                    .collect::<Vec<_>>();
                if matches.len() != 1 {
                    return Err(fault(
                        "ambiguous_target",
                        "Choose one grounded visible element.",
                    ));
                }
                let e = matches[0];
                if let Some(action) = native_action {
                    if !e.actions.iter().any(|a| a == action) {
                        return Err(fault(
                            "unsupported_action",
                            "The exact element does not advertise this native action; no input sent. Use its observed actions, not a pointer fallback.",
                        ));
                    }
                    if e.enabled == Some(false) {
                        return Err(fault(
                            "element_disabled",
                            "The exact observed element is disabled; no native action sent.",
                        ));
                    }
                } else if e.visible != Some(true) || e.occluded != Some(false) {
                    return Err(fault(
                        "visibility_unknown",
                        "Resolve occlusion before input.",
                    ));
                }
                scope.element = Some(e.id.clone());
                element_revision = Some(e.action_revision(native_action));
                e.point
            }
            Locator::Window => {
                scope.element = None;
                // This reference is rejected by every input intent. A logical
                // AX dialog has no bitmap or pixel mapping, yet an exact native
                // backend may still place its window. Never invent a mapping:
                // the finite zero is only a non-input placeholder in Resolved.
                scene
                    .pixel_to_desktop
                    .map_or([0.0, 0.0], |[a, b, c, d, e, f]| {
                        let (x, y) = (
                            scene.image_size[0] as f64 / 2.0,
                            scene.image_size[1] as f64 / 2.0,
                        );
                        [a * x + c * y + e, b * x + d * y + f]
                    })
            }
            Locator::Pixel { x, y } => {
                if !x.is_finite()
                    || !y.is_finite()
                    || x < 0.0
                    || y < 0.0
                    || x >= scene.image_size[0] as f64
                    || y >= scene.image_size[1] as f64
                {
                    return Err(fault(
                        "outside_frame",
                        "Use coordinates inside the original image.",
                    ));
                }
                let [a, b, c, d, e, f] = scene.pixel_to_desktop.ok_or_else(|| {
                    fault(
                        "coordinate_mapping_unknown",
                        "Do not assume scale=1 or host coordinates for a guest.",
                    )
                })?;
                scope.element = None;
                pixel = Some([x, y]);
                [a * x + c * y + e, b * x + d * y + f]
            }
            Locator::RedMarkerCluster { x, y } => {
                if !use_local_anchor {
                    return Err(fault(
                        "red_marker_anchor_unavailable",
                        "This native backend cannot revalidate a local marker cluster; no reference created.",
                    ));
                }
                if !x.is_finite()
                    || !y.is_finite()
                    || x < 0.0
                    || y < 0.0
                    || x >= scene.image_size[0] as f64
                    || y >= scene.image_size[1] as f64
                {
                    return Err(fault(
                        "outside_frame",
                        "Use coordinates inside the original image.",
                    ));
                }
                let [a, b, c, d, e, f] = scene.pixel_to_desktop.ok_or_else(|| {
                    fault(
                        "coordinate_mapping_unknown",
                        "Do not assume scale=1 or host coordinates for a guest.",
                    )
                })?;
                scope.element = None;
                pixel = Some([x, y]);
                red_marker_cluster = true;
                [a * x + c * y + e, b * x + d * y + f]
            }
        };
        if !point.iter().all(|p| p.is_finite()) {
            return Err(fault(
                "invalid_transform",
                "Inspect native layout metadata.",
            ));
        }
        let (pixel_anchor, pixel_anchor_creation_reason) = if let Some(pixel) = pixel
            && !red_marker_cluster
        {
            if !use_local_anchor {
                (None, Some("backend_not_opted_in"))
            } else if let Some(artifact_id) = &snapshot.artifact_id {
                if let Some(artifact) = self.artifacts.iter().find(|entry| &entry.id == artifact_id)
                {
                    match PixelAnchor::from_png_diagnosed(
                        &artifact.bytes,
                        scene.image_size,
                        pixel,
                        point,
                    ) {
                        Ok(anchor) => (Some(anchor), None),
                        Err(reason) => (None, Some(reason.as_str())),
                    }
                } else {
                    // The original frame still exists, but its identified bytes
                    // are no longer retained. Do not recreate from a new frame.
                    (None, Some("source_artifact_evicted"))
                }
            } else {
                (None, Some("source_artifact_absent"))
            }
        } else {
            // Semantic, placement and explicit red-marker references are not
            // ordinary pixel-anchor construction failures.
            (None, None)
        };
        let red_marker_anchor = if red_marker_cluster {
            let (pixel, artifact) = pixel.and_then(|pixel| {
                snapshot.artifact_id.as_ref().and_then(|id| {
                    self.artifacts
                        .iter()
                        .find(|artifact| &artifact.id == id)
                        .map(|artifact| (pixel, artifact))
                })
            }).ok_or_else(|| fault(
                "red_marker_anchor_unavailable",
                "A preserved native screenshot is required to anchor a red marker cluster; no reference created.",
            ))?;
            Some(RedMarkerAnchor::from_png(&artifact.bytes, scene.image_size, pixel, point).ok_or_else(|| fault(
                "red_marker_cluster_not_found",
                "The exact point must fall on one compact red marker in a local cluster; no reference created and no fallback was attempted.",
            ))?)
        } else {
            None
        };
        let r = Resolved {
            native_action: native_action.map(str::to_owned),
            reference: id(),
            target: target.into(),
            frame: frame.into(),
            point,
            scope,
            element_revision,
            placement_only,
            source_region: snapshot.source_region,
            pixel_anchor,
            pixel_anchor_used: false,
            red_marker_anchor,
        };
        self.refs.push_back(StoredReference {
            value: r.clone(),
            pixel_anchor_creation_reason,
        });
        while self.refs.len() > 256 {
            self.refs.pop_front();
        }
        Ok(r)
    }
}
