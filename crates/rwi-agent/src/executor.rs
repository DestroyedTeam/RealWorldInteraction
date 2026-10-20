//! Device-side domain dispatch. MCP and business workflows are not dependencies.
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rwi_core::{
    Engine, Result, Verification,
    device::{DeviceImage, DeviceResult, api::*},
};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

pub struct Executor {
    pub engine: Engine,
    /// Metadata-only catalog for authenticated target ownership checks. Never
    /// acquire the native executor lock on the async acceptance/query path.
    pub target_catalog: Arc<Mutex<Value>>,
    pub live_controls: Arc<Mutex<rwi_core::LiveControllers>>,
    views: VecDeque<(String, String, String, String, rwi_imaging::ViewMapping)>,
}
fn value(v: impl serde::Serialize) -> Result<DeviceResult> {
    Ok(DeviceResult::value(v))
}
impl Executor {
    pub fn new(engine: Engine) -> Self {
        let target_catalog = Arc::new(Mutex::new(engine.targets()));
        let live_controls = Arc::new(Mutex::new(engine.live_controllers()));
        Self {
            target_catalog,
            live_controls,
            engine,
            views: VecDeque::new(),
        }
    }
    fn publish_targets(&self) {
        let catalog = self.engine.targets();
        *self
            .target_catalog
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = catalog;
        self.publish_live_controls();
    }
    fn publish_live_controls(&self) {
        *self.live_controls.lock().unwrap_or_else(|e| e.into_inner()) =
            self.engine.live_controllers();
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
            "computer.live" => {
                let a = args!(LiveArgs);
                if let Some(update) = a.view {
                    if a.action != rwi_core::LiveAction::Status || a.options.is_some() {
                        return Err(rwi_core::fault(
                            "invalid_live_view",
                            "Visibility updates cannot also control a session.",
                        ));
                    }
                    return value(self.engine.live_view(&a.target, update)?);
                }
                let result = self.engine.live(&a.target, a.action, a.options);
                self.publish_live_controls();
                value(result?)
            }
            "computer.bind_window" => {
                let a = args!(BindArgs);
                let result = self.engine.bind_window(&a.target, &a.selection)?;
                self.publish_targets();
                value(result)
            }
            "computer.attach_window" => {
                let a = args!(BindArgs);
                let result = self.engine.attach_window(&a.target, &a.selection)?;
                self.publish_targets();
                value(result)
            }
            "computer.detach_window" => {
                let a = args!(DetachWindowArgs);
                let result = self.engine.detach_window(&a.target)?;
                self.views.retain(|(_, t, _, _, _)| t != &a.target);
                self.publish_targets();
                value(result)
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
            "computer.set_text" => {
                let a = args!(SetTextArgs);
                let request = rwi_core::set_text::SetTextRequest {
                    target: a.target,
                    frame: a.frame,
                    element: a.element,
                    request_id: a.request_id,
                    text: a.text,
                };
                value(self.engine.set_text(&request, &a.lease)?)
            }
            "computer.focus_element" => {
                let a = args!(FocusElementArgs);
                let request = rwi_core::semantic_focus::FocusElementRequest {
                    target: a.target,
                    frame: a.frame,
                    element: a.element,
                    request_id: a.request_id,
                };
                value(self.engine.focus_element(&request, &a.lease)?)
            }
            "computer.select_window_local" => {
                let a = args!(WindowLocalArgs);
                let request = rwi_core::window_local::WindowLocalRequest {
                    target: a.target,
                    frame: a.frame,
                    kind: a.kind,
                    request_id: a.request_id,
                };
                value(self.engine.select_window_local(&request, &a.lease)?)
            }
            "computer.focus_window" => {
                let a = args!(FocusWindowArgs);
                value(self.engine.focus_window(
                    &a.target,
                    &a.lease,
                    &a.reference,
                    &a.request_id,
                    &a.expected_foreground,
                )?)
            }
            "computer.hotkey" => {
                let a = args!(HotkeyArgs);
                value(self.engine.hotkey(
                    &a.target,
                    &a.lease,
                    &a.request_id,
                    &a.key,
                    &a.expected_foreground,
                )?)
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
                // Keep an explicit opt-in intact through the device executor.
                // Core/native unsupported-marker checks must reject it, never
                // silently substitute the ordinary strict pixel locator.
                let locator = if a.red_marker_cluster {
                    rwi_core::Locator::RedMarkerCluster { x, y }
                } else {
                    rwi_core::Locator::Pixel { x, y }
                };
                value(self.engine.resolve(&a.target, frame, locator)?)
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
        // Image delivery is not input authority. An AX-less macOS scene keeps
        // its original pixels and native tiles, but cannot register a view
        // whose coordinates the Host could later ask to resolve for input.
        let view_id = (scene.binding.is_some() && scene.pixel_to_desktop.is_some())
            .then(|| uuid::Uuid::new_v4().to_string());
        let input_reference_available = view_id.is_some();
        let mut result = DeviceResult::value(
            json!({"view_id":view_id,"frame":a.frame,"target":a.target,"original_artifact_id":a.artifact_id,"original_sha256":rwi_core::digest(original),"view_sha256":rwi_core::digest(&png),"mapping":mapping,"resolve_tool":input_reference_available.then_some("computer.resolve_view"),"input_reference_available":input_reference_available}),
        );
        if let Some(id) = view_id {
            self.views
                .push_back((id, a.target, a.frame, a.artifact_id, mapping));
            while self.views.len() > 64 {
                self.views.pop_front();
            }
        }
        result.images.push(DeviceImage {
            mime: "image/png".into(),
            base64: STANDARD.encode(png),
        });
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rwi_core::{Backend, Binding, Identity, Intent, Oracle, Predicate, Resolved, Scene, Scope};

    struct Raster {
        png: Vec<u8>,
        grounded: bool,
    }
    impl Backend for Raster {
        fn identity(&self) -> Identity {
            Identity {
                host: "offline".into(),
                guest: None,
                session: "fixture".into(),
                seat: "fixture".into(),
                desktop: "fixture".into(),
            }
        }
        fn capabilities(&self) -> Value {
            json!({"input":false})
        }
        fn observe(&mut self) -> Result<Scene> {
            Ok(Scene {
                identity: self.identity(),
                assurance: if self.grounded {
                    "grounded"
                } else {
                    "visual-only"
                }
                .into(),
                binding: self.grounded.then(|| Binding {
                    scope: Scope {
                        window: "fixture-window".into(),
                        pid: 42,
                        element: None,
                    },
                    focus_window: "fixture-window".into(),
                    layout: "fixture".into(),
                    content: rwi_core::digest(&self.png),
                    native_epoch: "fixture-epoch".into(),
                    mutation_context: None,
                }),
                mutation_ready: false,
                blocked: Some(false),
                image_size: [16, 8],
                pixel_to_desktop: self.grounded.then_some([1., 0., 0., 1., 0., 0.]),
                elements: vec![],
                captured_unix_ms: 1,
                atomicity: "offline".into(),
                image_normalization: "offline".into(),
                png: self.png.clone(),
            })
        }
        fn act(
            &mut self,
            _: &Binding,
            _: &Resolved,
            _: &Intent,
            _: Option<&str>,
        ) -> Result<Option<bool>> {
            Err(rwi_core::fault("fixture_read_only", "No input."))
        }
        fn inspect(&mut self, _: &Scope, _: &Predicate, _: &str) -> Result<Oracle> {
            Err(rwi_core::fault("fixture_read_only", "No oracle."))
        }
    }

    // Uses the actual production Executor and preserved synthetic PNG/view
    // mapping. No native backend, screen capture, input lease or GUI is involved.
    fn marker_compat_fixture() -> (tempfile::TempDir, Executor, Value) {
        let dir = tempfile::tempdir().unwrap();
        let mut engine = Engine::new(dir.path().into()).unwrap();
        engine
            .register(
                "fixture".into(),
                Box::new(Raster {
                    png: rwi_imaging::encode_rgb([16, 8], &vec![77; 16 * 8 * 3]).unwrap(),
                    grounded: true,
                }),
            )
            .unwrap();
        let mut executor = Executor::new(engine);
        let observed = executor
            .invoke(
                "computer.observe",
                json!({"target":"fixture", "inline_image":false}),
            )
            .unwrap();
        let image = executor
            .invoke(
                "computer.image",
                json!({"target":"fixture", "frame":observed.data["frame"],
                    "artifact_id":observed.data["artifact_id"], "crop":[2,1,8,4],
                    "max_edge":64}),
            )
            .unwrap();
        assert_eq!(image.data["input_reference_available"], true);
        (dir, executor, image.data)
    }

    #[test]
    fn marker_compat_default_and_false_keep_strict_pixel_mapping() {
        let (_dir, mut executor, image) = marker_compat_fixture();
        for explicit in [None, Some(false)] {
            let mut args = json!({"target":"fixture", "view_id":image["view_id"],
                "x":3, "y":2});
            if let Some(value) = explicit {
                args["red_marker_cluster"] = json!(value);
            }
            let resolved = executor.invoke("computer.resolve_view", args).unwrap();
            assert_eq!(resolved.data["point"], json!([5.0, 3.0]));
            assert_eq!(resolved.data["frame"], image["frame"]);
            assert_eq!(resolved.data["scope"]["window"], "fixture-window");
            assert_eq!(resolved.data["scope"]["pid"], 42);
            assert!(resolved.data["reference"].is_string());
        }
    }

    #[test]
    fn marker_compat_opt_in_on_unsupported_backend_refuses_without_pixel_fallback() {
        let (_dir, mut executor, image) = marker_compat_fixture();
        let error = executor
            .invoke(
                "computer.resolve_view",
                json!({"target":"fixture", "view_id":image["view_id"],
                    "x":3, "y":2, "red_marker_cluster":true}),
            )
            .unwrap_err();
        assert_eq!(error.code, "red_marker_anchor_unavailable");
        assert_eq!(
            error.next_step,
            "This native backend cannot revalidate a local marker cluster; no reference created."
        );
    }

    #[test]
    fn marker_compat_unknown_view_refuses_before_either_locator_is_resolved() {
        let (_dir, mut executor, _) = marker_compat_fixture();
        for opt_in in [false, true] {
            let error = executor
                .invoke(
                    "computer.resolve_view",
                    json!({"target":"fixture", "view_id":"nonexistent-fixture-view",
                        "x":3, "y":2, "red_marker_cluster":opt_in}),
                )
                .unwrap_err();
            assert_eq!(error.code, "unknown_view");
            assert_eq!(error.next_step, "Request a view in this context.");
        }
    }

    #[test]
    fn native_device_image_keeps_visual_only_tiles_but_never_registers_a_view() {
        for grounded in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let mut engine = Engine::new(dir.path().into()).unwrap();
            engine
                .register(
                    "fixture".into(),
                    Box::new(Raster {
                        png: rwi_imaging::encode_rgb([16, 8], &vec![77; 16 * 8 * 3]).unwrap(),
                        grounded,
                    }),
                )
                .unwrap();
            let mut executor = Executor::new(engine);
            let observed = executor.engine.observe("fixture").unwrap();
            let image = executor
                .invoke(
                    "computer.image",
                    json!({"target":"fixture","frame":observed.frame,"artifact_id":observed.artifact_id,"crop":[2,1,8,4],"max_edge":64}),
                )
                .unwrap();
            assert_eq!(image.images.len(), 1);
            let bytes = STANDARD.decode(&image.images[0].base64).unwrap();
            assert_eq!(rwi_imaging::decode_original(&bytes).unwrap().size, [8, 4]);
            assert_eq!(image.data["input_reference_available"], grounded);
            if grounded {
                assert!(image.data["view_id"].is_string());
                assert_eq!(image.data["resolve_tool"], "computer.resolve_view");
                assert_eq!(executor.views.len(), 1);
            } else {
                assert!(image.data["view_id"].is_null());
                assert!(image.data["resolve_tool"].is_null());
                assert!(executor.views.is_empty());
                assert_eq!(
                    executor
                        .invoke(
                            "computer.resolve_view",
                            json!({"target":"fixture","view_id":"invented","x":0,"y":0}),
                        )
                        .unwrap_err()
                        .code,
                    "unknown_view"
                );
            }
        }
    }
}
