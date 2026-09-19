//! Device-side domain dispatch. MCP and business workflows are not dependencies.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rwi_core::{
    Engine, Result, Verification,
    device::{DeviceImage, DeviceResult, api::*},
};
use serde_json::{Value, json};
use std::collections::VecDeque;

pub struct Executor {
    pub engine: Engine,
    views: VecDeque<(String, String, String, String, rwi_imaging::ViewMapping)>,
}
fn value(v: impl serde::Serialize) -> Result<DeviceResult> {
    Ok(DeviceResult::value(v))
}
impl Executor {
    pub fn new(engine: Engine) -> Self {
        Self {
            engine,
            views: VecDeque::new(),
        }
    }
    pub fn invoke(&mut self, tool: &str, args: Value) -> Result<DeviceResult> {
        macro_rules! args {
            ($t:ty) => {
                serde_json::from_value::<$t>(args).map_err(|_| {
                    rwi_core::fault("invalid_arguments", "Use the advertised capability schema.")
                })?
            };
        }
        match tool {
            "computer.targets" => value(json!({
                "targets": self.engine.targets(),
                "recent_operations": self.engine.recent_operations(5),
            })),
            "computer.observe" => {
                let a = args!(ObserveArgs);
                let observation = self.engine.observe_with_region(&a.target, a.region)?;
                let mut result = DeviceResult::value(&observation);
                if a.inline_image
                    && let Some(artifact) = &observation.artifact_id
                {
                    let (bytes, _, total) =
                        self.engine.artifact(artifact, 0, rwi_core::INLINE_BYTES)?;
                    if bytes.len() == total {
                        result.images.push(DeviceImage {
                            mime: "image/png".into(),
                            base64: STANDARD.encode(bytes),
                        });
                    }
                }
                Ok(result)
            }
            "computer.capture_visual" => {
                let a = args!(CaptureVisualArgs);
                let visual = self.engine.capture_visual(
                    &a.target,
                    rwi_core::VisualOptions {
                        max_edge: a.max_edge,
                        feedback: a.feedback,
                    },
                )?;
                let encoded = STANDARD.encode(&visual.capture.png);
                if encoded.len() != visual.base64_bytes
                    || encoded.len() > rwi_core::MAX_VISUAL_BASE64_BYTES
                {
                    return Err(rwi_core::fault(
                        "visual_transport_budget",
                        "Visual base64 exceeded its independent transport budget.",
                    ));
                }
                rwi_core::operation::checkpoint("capture_visual_base64_encoded")?;
                let mut result = DeviceResult::value(&visual);
                result.images.push(DeviceImage {
                    mime: "image/png".into(),
                    base64: encoded,
                });
                Ok(result)
            }
            "computer.discover" => {
                let a = args!(TargetArgs);
                value(self.engine.discover_with(
                    &a.target,
                    rwi_core::DiscoveryQuery {
                        cursor: a.cursor,
                        pid: a.pid,
                        title_contains: a.title_contains,
                    },
                )?)
            }
            "computer.bind_window" => {
                let a = args!(BindArgs);
                value(self.engine.bind_window(&a.target, &a.selection)?)
            }
            "computer.resolve" => {
                let a = args!(ResolveArgs);
                value(self.engine.resolve(&a.target, &a.frame, a.locator)?)
            }
            "computer.claim" => {
                let a = args!(ClaimArgs);
                value(json!({"lease":self.engine.claim(&a.target,a.ttl_ms)?,"ttl_ms":a.ttl_ms}))
            }
            "computer.release" => {
                let a = args!(ReleaseArgs);
                self.engine.release(&a.lease)?;
                value(json!({"released":true}))
            }
            "computer.act" => {
                let a = args!(ActArgs);
                value(self.engine.act(
                    &a.target,
                    &a.reference,
                    &a.lease,
                    &a.request_id,
                    a.intent,
                    a.value,
                )?)
            }
            "computer.pointer_feedback" => {
                let a = args!(PointerFeedbackArgs);
                value(self.engine.feedback(&a.target, a.visible, a.point)?)
            }
            "computer.move_window" => {
                let a = args!(MoveWindowArgs);
                value(self.engine.move_window(
                    &a.target,
                    &a.lease,
                    &a.reference,
                    &a.request_id,
                    &a.display,
                )?)
            }
            "computer.act_checked" => {
                let a = args!(CheckedActArgs);
                value(self.engine.act_checked(rwi_core::CheckedAction {
                    target: &a.target,
                    frame: &a.frame,
                    element: &a.element,
                    lease: &a.lease,
                    request: &a.request_id,
                    intent: a.intent,
                    value: a.value,
                })?)
            }
            "computer.verify" => {
                let a = args!(VerifyArgs);
                value(self.engine.verify(Verification {
                    target: &a.target,
                    frame: &a.frame,
                    element: a.element,
                    request: a.request_id,
                    predicate: a.predicate,
                    expected: &a.expected,
                    timeout_ms: a.timeout_ms,
                    stable_samples: a.stable_samples,
                })?)
            }
            "computer.lifecycle_preflight" => {
                let a = args!(LifecyclePreflightArgs);
                value(
                    self.engine
                        .lifecycle_preflight(&a.target, &a.authorization_id)?,
                )
            }
            "computer.launch_application" => {
                let a = args!(LaunchArgs);
                value(self.engine.launch_application(
                    &a.target,
                    &a.lease,
                    rwi_core::lifecycle::LaunchRequest {
                        authorization_id: &a.authorization_id,
                        request_id: &a.request_id,
                        timeout_ms: a.timeout_ms,
                    },
                )?)
            }
            "computer.activate_window" => {
                let a = args!(ActivateArgs);
                value(self.engine.activate_window(
                    &a.target,
                    &a.lease,
                    rwi_core::lifecycle::ActivateRequest {
                        authorization_id: &a.authorization_id,
                        request_id: &a.request_id,
                        expected_foreground: &a.expected_foreground,
                    },
                )?)
            }
            "computer.lifecycle_receipt" => {
                let a = args!(LifecycleReceiptArgs);
                value(self.engine.lifecycle_receipt(
                    &a.target,
                    &a.kind,
                    &a.authorization_id,
                    &a.request_id,
                )?)
            }
            "computer.artifact" => {
                let a = args!(ArtifactArgs);
                let (bytes, hash, total) =
                    self.engine.artifact(&a.artifact_id, a.offset, a.length)?;
                if a.as_image && (a.offset != 0 || bytes.len() != total) {
                    return Err(rwi_core::fault(
                        "incomplete_image",
                        "Image requires complete original bytes.",
                    ));
                }
                let mut r = DeviceResult::value(
                    json!({"artifact_id":a.artifact_id,"offset":a.offset,"total_bytes":total,"sha256":hash,"base64":if a.as_image {Value::Null} else {json!(STANDARD.encode(&bytes))}}),
                );
                if a.as_image {
                    r.images.push(DeviceImage {
                        mime: "image/png".into(),
                        base64: STANDARD.encode(bytes),
                    });
                }
                Ok(r)
            }
            "computer.image" => self.image(args!(ImageArgs)),
            "computer.resolve_view" => {
                let a = args!(ResolveViewArgs);
                let (_, _, frame, artifact, map) = self
                    .views
                    .iter()
                    .find(|(id, target, ..)| id == &a.view_id && target == &a.target)
                    .ok_or_else(|| {
                        rwi_core::fault("unknown_view", "Request a view in this context.")
                    })?;
                self.engine.original(&a.target, frame, artifact)?;
                let [x, y] = map.original_point([a.x, a.y])?;
                value(
                    self.engine
                        .resolve(&a.target, frame, rwi_core::Locator::Pixel { x, y })?,
                )
            }
            _ => Err(rwi_core::fault(
                "unsupported_capability",
                "Capability is not dispatched by this native executor.",
            )),
        }
    }
    fn image(&mut self, a: ImageArgs) -> Result<DeviceResult> {
        let (original, scene) = self.engine.original(&a.target, &a.frame, &a.artifact_id)?;
        let crop = a
            .crop
            .map(|[x, y, width, height]| -> Result<_> {
                Ok(rwi_imaging::Rect {
                    x: i32::try_from(x).map_err(|_| {
                        rwi_core::fault("crop_overflow", "Use original image coordinates.")
                    })?,
                    y: i32::try_from(y).map_err(|_| {
                        rwi_core::fault("crop_overflow", "Use original image coordinates.")
                    })?,
                    width,
                    height,
                })
            })
            .transpose()?;
        let (png, mapping) = rwi_imaging::view(original, crop, a.max_edge, scene.pixel_to_desktop)?;
        let id = uuid::Uuid::new_v4().to_string();
        let mut result = DeviceResult::value(
            json!({"view_id":id,"frame":a.frame,"target":a.target,"original_artifact_id":a.artifact_id,"original_sha256":rwi_core::digest(original),"view_sha256":rwi_core::digest(&png),"mapping":mapping,"resolve_tool":"computer.resolve_view"}),
        );
        self.views
            .push_back((id, a.target, a.frame, a.artifact_id, mapping));
        while self.views.len() > 64 {
            self.views.pop_front();
        }
        result.images.push(DeviceImage {
            mime: "image/png".into(),
            base64: STANDARD.encode(png),
        });
        Ok(result)
    }
}
