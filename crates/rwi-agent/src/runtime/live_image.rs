//! Authorized cached pixels, independent of native input and execution retention.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rwi_core::{
    device::api::LiveImageArgs,
    live_image::{LiveImageQuery, LiveImageReply, MAX_LIVE_IMAGE_BYTES},
};
use std::time::Duration;
pub(super) const METHOD: &str = "computer.live_image";
pub(super) const READERS: u32 = 2;
const INTERVAL: Duration = Duration::from_millis(250);
fn image_result(reply: LiveImageReply, query: &LiveImageQuery) -> rwi_core::Result<DeviceResult> {
    let (cursor, image) = match reply {
        LiveImageReply::Ready(v) => (v.cursor, Some(v.image)),
        LiveImageReply::Unchanged(c) => (c, None),
    };
    LiveImageQuery {
        view_id: query.view_id.clone(),
        after: Some(cursor),
    }
    .validate()?;
    let mut result = DeviceResult::value(
        json!({"state":"unchanged","cursor":cursor,"frame_created":false,"input_authority":false,"input_sent":false,"goal_verified":false,"capture_on_read":false,"presence_renewal":false,"retained_in_execution_registry":false}),
    );
    if query.after == Some(cursor) {
        return Ok(result);
    }
    let Some(image) = image else {
        return Ok(result);
    };
    let [w, h] = image.image_size;
    let png = &image.png;
    if png.len() < 24
        || png.len() > MAX_LIVE_IMAGE_BYTES
        || !png.starts_with(b"\x89PNG\r\n\x1a\n")
        || &png[12..16] != b"IHDR"
        || w == 0
        || h == 0
        || w > 2048
        || h > 2048
        || u32::from_be_bytes(png[16..20].try_into().unwrap()) != w
        || u32::from_be_bytes(png[20..24].try_into().unwrap()) != h
    {
        return Err(rwi_core::fault(
            "live_image_bounds",
            "Cached PNG exceeds the image-only transport bounds; no conversion, capture or fallback.",
        ));
    }
    result.data["state"] = json!("ready");
    result.data["image_size"] = json!(image.image_size);
    result.data["captured_unix_ms"] = json!(image.captured_unix_ms);
    result.data["png_sha256"] = json!(rwi_core::digest(png));
    result.data["png_bytes"] = json!(png.len());
    result.images.push(DeviceImage {
        mime: "image/png".into(),
        base64: STANDARD.encode(png),
    });
    Ok(result)
}
impl Runtime {
    pub(super) async fn cached_live_image(
        &self,
        peer: &PeerPin,
        policy: &Principal,
        token: Option<&str>,
        arguments: serde_json::Value,
    ) -> DeviceResult {
        if !policy.observe {
            return DeviceResult::error(
                "permission_denied",
                "No cached image observation authorization.",
            );
        }
        let args: LiveImageArgs = match serde_json::from_value(arguments) {
            Ok(v) => v,
            Err(_) => {
                return DeviceResult::error(
                    "invalid_arguments",
                    "Use target and a strict cached-image query, without input or presence fields.",
                );
            }
        };
        if let Err(e) = args.query.validate() {
            return dispatch::fault_result(e);
        }
        let Some(context) = self.context(peer, token) else {
            return DeviceResult::error("context_unknown", "Use the authenticated owning context.");
        };
        if !context.owns_target(&json!(args.target)) {
            return DeviceResult::error(
                "foreign_target",
                "Cached pixels belong to this context's selected target only.",
            );
        }
        if context.closed.load(Ordering::Acquire) {
            return DeviceResult::error("context_closed", "No pixels after context closure.");
        }
        let controller = match context.live_controls.lock() {
            Ok(c) => c.get(&args.target).cloned(),
            Err(_) => {
                return DeviceResult::error(
                    "live_state_unavailable",
                    "Owned preview registry unavailable.",
                );
            }
        };
        let controller = match controller {
            Some(Ok(v)) => v,
            Some(Err(e)) => return dispatch::fault_result(e),
            None => {
                return DeviceResult::error("unknown_target", "The preview binding was retired.");
            }
        };
        let Ok(permit) = context.image_gate.clone().try_acquire_owned() else {
            return DeviceResult::error(
                "live_image_busy",
                "Bounded image readers are busy; no input or new capture.",
            );
        };
        {
            let Ok(mut rates) = context.image_rate.lock() else {
                return DeviceResult::error(
                    "live_image_rate_unavailable",
                    "Image admission state unavailable.",
                );
            };
            let now = Instant::now();
            rates.retain(|_, until| *until > now);
            if let Some(until) = rates.get(&args.target) {
                let mut r = DeviceResult::error(
                    "live_image_rate_limited",
                    "Read no faster than one image request per target per250ms.",
                );
                r.data["retry_after_ms"] = json!(until.saturating_duration_since(now).as_millis());
                return r;
            }
            if rates.len() >= 16 {
                return DeviceResult::error(
                    "live_image_rate_capacity",
                    "Finite target read budget reached.",
                );
            }
            rates.insert(args.target.clone(), now + INTERVAL);
        }
        match tokio::task::spawn_blocking(move || {
            let _permit = permit;
            if context.closed.load(Ordering::Acquire) {
                return DeviceResult::error("context_closed", "No pixels after context closure.");
            }
            let result = controller
                .latest_image(args.query.clone())
                .and_then(|v| image_result(v, &args.query));
            if context.closed.load(Ordering::Acquire) {
                return DeviceResult::error(
                    "context_closed",
                    "Context closed before cached pixels could be returned.",
                );
            }
            let mut r = match result {
                Ok(v) => v,
                Err(e) => dispatch::fault_result(e),
            };
            r.data["result_kind"] = json!("cached_live_image");
            r.data["native_dispatch_entered"] = json!(false);
            r.data["input_domain_fenced"] = json!(context.domain.fenced.load(Ordering::Acquire));
            r
        })
        .await
        {
            Ok(r) => r,
            Err(_) => DeviceResult::error(
                "live_image_worker_failed",
                "Cached image read failed; no input or capture was started.",
            ),
        }
    }
}
#[cfg(all(test, feature = "fixture"))]
mod tests {
    use super::*;
    use crate::runtime::dispatch_tests::{tracked_backend_context, wire};
    use rwi_core::{
        Backend, Binding, Identity, Intent, LiveAction, LiveViewUpdate, Oracle, Predicate,
        Resolved, Scene, Scope,
        live_image::{CachedLiveImage, LiveImageCursor},
    };
    use std::sync::{Weak, atomic::AtomicUsize};
    const SAMPLE: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGMQMYr6DwACngGgfe0KRgAAAABJRU5ErkJggg==";
    struct Pixels {
        reads: AtomicUsize,
        mode: AtomicUsize,
        close_hook: Mutex<Option<Weak<Context>>>,
    }
    impl Default for Pixels {
        fn default() -> Self {
            Self {
                reads: AtomicUsize::new(0),
                mode: AtomicUsize::new(0),
                close_hook: Mutex::new(None),
            }
        }
    }
    fn frame() -> Arc<rwi_core::VisualCapture> {
        let mut v:rwi_core::VisualCapture=serde_json::from_value(json!({"identity":{"host":"fixture","guest":null,"session":"s","seat":"s","desktop":"d"},"window":{"window":"w","pid":1,"process_created":1,"native_epoch":"e"},"source_rect":{"x":0,"y":0,"width":1,"height":1},"source_pixel_size":[1,1],"image_size":[1,1],"visual_to_desktop":[1,0,0,1,0,0],"captured_unix_ms":1,"assurance":"synthetic","atomicity":"synthetic","normalization":"fixture","feedback":{"requested":"exclude","suspension_ticket":null,"exclusion":"fixture","before_copy":null,"after_copy":null,"after_release":null},"details":{}})).unwrap();
        v.png = STANDARD.decode(SAMPLE).unwrap();
        Arc::new(v)
    }
    impl rwi_core::LiveController for Pixels {
        fn control(&self, _: LiveAction) -> rwi_core::Result<serde_json::Value> {
            panic!("image must not control Live")
        }
        fn view_update(&self, _: LiveViewUpdate) -> rwi_core::Result<serde_json::Value> {
            panic!("image must not renew presence")
        }
        fn latest_image(&self, q: LiveImageQuery) -> rwi_core::Result<LiveImageReply> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if q.view_id != "a".repeat(32) {
                return Err(rwi_core::fault("live_view_mismatch", "fixture"));
            }
            let mut image = frame();
            let mut cursor = LiveImageCursor {
                generation: 1,
                sequence: 1,
            };
            match self.mode.load(Ordering::SeqCst) {
                1 => Arc::make_mut(&mut image)
                    .png
                    .resize(MAX_LIVE_IMAGE_BYTES + 1, 0),
                2 => Arc::make_mut(&mut image).image_size = [2, 1],
                3 => cursor.sequence = 0,
                4 => return Ok(LiveImageReply::Unchanged(cursor)),
                _ => {}
            }
            if let Some(c) = self
                .close_hook
                .lock()
                .unwrap()
                .as_ref()
                .and_then(Weak::upgrade)
            {
                c.closed.store(true, Ordering::Release);
            }
            Ok(LiveImageReply::Ready(CachedLiveImage { cursor, image }))
        }
    }
    struct Preview {
        fixture: rwi_core::fixture::Fixture,
        pixels: Arc<Pixels>,
    }
    impl Backend for Preview {
        fn identity(&self) -> Identity {
            self.fixture.identity()
        }
        fn capabilities(&self) -> serde_json::Value {
            self.fixture.capabilities()
        }
        fn observe(&mut self) -> rwi_core::Result<Scene> {
            self.fixture.observe()
        }
        fn act(
            &mut self,
            b: &Binding,
            r: &Resolved,
            i: &Intent,
            v: Option<&str>,
        ) -> rwi_core::Result<Option<bool>> {
            self.fixture.act(b, r, i, v)
        }
        fn inspect(&mut self, s: &Scope, p: &Predicate, e: &str) -> rwi_core::Result<Oracle> {
            self.fixture.inspect(s, p, e)
        }
        fn live_controller(&self) -> rwi_core::Result<Arc<dyn rwi_core::LiveController>> {
            Ok(self.pixels.clone())
        }
    }
    fn setup() -> (
        tempfile::TempDir,
        Arc<Runtime>,
        Principal,
        Principal,
        Arc<Pixels>,
    ) {
        let t = tempfile::tempdir().unwrap();
        let a = Principal {
            journal_identity: None,
            certificate: PeerPin::parse(&"a".repeat(64)).unwrap(),
            observe: true,
            input: true,
        };
        let b = Principal {
            certificate: PeerPin::parse(&"b".repeat(64)).unwrap(),
            ..a.clone()
        };
        let r = Runtime::new(Config {
            version: 1,
            device_id: "device".into(),
            bind: "127.0.0.1:0".parse().unwrap(),
            identity: rwi_link_trust::IdentityPaths::in_profile(t.path()),
            principals: vec![a.clone(), b.clone()],
            state_root: t.path().join("state"),
            platform: crate::config::PlatformPolicy::Fixture,
        })
        .unwrap();
        (t, r, a, b, Arc::new(Pixels::default()))
    }
    async fn bind(r: &Arc<Runtime>, p: &Principal, v: Arc<Pixels>) -> (String, String) {
        tracked_backend_context(
            r,
            p,
            Box::new(Preview {
                fixture: rwi_core::fixture::Fixture::default(),
                pixels: v,
            }),
        )
        .await
    }
    fn args(target: &str) -> serde_json::Value {
        json!({"target":target,"query":{"view_id":"a".repeat(32)}})
    }
    async fn get(r: &Arc<Runtime>, p: &Principal, c: &str, a: serde_json::Value) -> DeviceResult {
        wire(
            r,
            p,
            Some(c),
            Call::Cached {
                tool: METHOD.into(),
                arguments: a,
            },
        )
        .await
    }
    #[tokio::test]
    async fn f31_live_image_pixels_are_exact_and_not_retained_as_executions() {
        let (_t, r, p, _b, v) = setup();
        let (c, target) = bind(&r, &p, v.clone()).await;
        let x = get(&r, &p, &c, args(&target)).await;
        assert!(!x.is_error, "{:?}", x.data);
        assert_eq!(
            STANDARD.decode(&x.images[0].base64).unwrap(),
            STANDARD.decode(SAMPLE).unwrap()
        );
        for field in [
            "target",
            "frame",
            "reference",
            "view_id",
            "source_rect",
            "lease",
        ] {
            assert!(x.data.get(field).is_none());
        }
        assert_eq!(x.data["input_authority"], false);
        assert_eq!(x.data["retained_in_execution_registry"], false);
        assert!(
            r.context(p.identity(), Some(&c))
                .unwrap()
                .executions
                .lock()
                .unwrap()
                .is_empty()
        );
        assert_eq!(v.reads.load(Ordering::SeqCst), 1);
    }
    #[tokio::test]
    async fn f31_live_image_cached_read_continues_while_input_executor_is_held() {
        let (_t, r, p, _b, v) = setup();
        let (c, target) = bind(&r, &p, v.clone()).await;
        let context = r.context(p.identity(), Some(&c)).unwrap();
        let _native = context.gate.clone().try_acquire_owned().unwrap();
        let _input = context.domain.gate.clone().try_acquire_owned().unwrap();
        context.entered.store(true, Ordering::Release);
        let exec = context.executor.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let (release, done) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let _held = exec.lock().unwrap();
            tx.send(()).unwrap();
            let _ = done.recv_timeout(Duration::from_secs(5));
        });
        rx.recv().unwrap();
        let result =
            tokio::time::timeout(Duration::from_secs(2), get(&r, &p, &c, args(&target))).await;
        release.send(()).unwrap();
        thread.join().unwrap();
        assert!(!result.unwrap().is_error);
        assert!(context.entered.load(Ordering::Acquire));
        assert_eq!(v.reads.load(Ordering::SeqCst), 1);
        assert!(context.executions.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn f31_live_image_unchanged_and_same_ready_cursor_have_no_png() {
        for mode in [0, 4] {
            let (_t, r, p, _b, v) = setup();
            v.mode.store(mode, Ordering::SeqCst);
            let (c, target) = bind(&r, &p, v).await;
            let mut a = args(&target);
            a["query"]["after"] = json!({"generation":1,"sequence":1});
            let x = get(&r, &p, &c, a).await;
            assert!(!x.is_error);
            assert_eq!(x.data["state"], "unchanged");
            assert!(x.images.is_empty());
        }
    }
    #[tokio::test]
    async fn f31_live_image_foreign_peer_context_target_and_no_observe_refuse() {
        let (_t, r, p, b, v) = setup();
        let (c, target) = bind(&r, &p, v.clone()).await;
        assert!(get(&r, &b, &c, args(&target)).await.is_error);
        let (other, _) = bind(&r, &p, Arc::new(Pixels::default())).await;
        assert!(get(&r, &p, &other, args(&target)).await.is_error);
        let denied = Principal {
            observe: false,
            ..p.clone()
        };
        assert!(
            r.cached(
                p.identity(),
                &denied,
                Some(&c),
                METHOD.into(),
                args(&target)
            )
            .await
            .is_error
        );
        assert_eq!(v.reads.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn f31_live_image_strict_arguments_cannot_publish_presence_or_input() {
        let (_t, r, p, _b, v) = setup();
        let (c, target) = bind(&r, &p, v.clone()).await;
        for key in ["visible", "close", "capture", "input", "frame"] {
            let mut a = args(&target);
            a[key] = json!(true);
            assert!(get(&r, &p, &c, a).await.is_error);
        }
        let mut q = args(&target);
        q["query"]["after"] = json!({"generation":0,"sequence":1});
        assert!(get(&r, &p, &c, q).await.is_error);
        assert_eq!(v.reads.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn f31_live_image_rate_and_parallel_budgets_are_bounded() {
        let (_t, r, p, _b, v) = setup();
        let (c, target) = bind(&r, &p, v.clone()).await;
        let ctx = r.context(p.identity(), Some(&c)).unwrap();
        let held = ctx
            .image_gate
            .clone()
            .try_acquire_many_owned(READERS)
            .unwrap();
        let x = get(&r, &p, &c, args(&target)).await;
        assert_eq!(x.data["code"], "live_image_busy");
        assert_eq!(v.reads.load(Ordering::SeqCst), 0);
        drop(held);
        assert!(!get(&r, &p, &c, args(&target)).await.is_error);
        let x = get(&r, &p, &c, args(&target)).await;
        assert_eq!(x.data["code"], "live_image_rate_limited");
        assert!(x.images.is_empty());
        assert_eq!(v.reads.load(Ordering::SeqCst), 1);
        assert!(ctx.image_rate.lock().unwrap().len() <= 16);
    }
    #[tokio::test]
    async fn f31_live_image_oversize_bad_dimensions_or_cursor_never_emit_pixels() {
        for mode in [1, 2, 3] {
            let (_t, r, p, _b, v) = setup();
            v.mode.store(mode, Ordering::SeqCst);
            let (c, target) = bind(&r, &p, v).await;
            let x = get(&r, &p, &c, args(&target)).await;
            assert!(x.is_error);
            assert!(x.images.is_empty());
        }
    }
    #[tokio::test]
    async fn f31_live_image_close_waits_for_readers_then_revokes() {
        let (_t, r, p, _b, v) = setup();
        let (c, target) = bind(&r, &p, v).await;
        let ctx = r.context(p.identity(), Some(&c)).unwrap();
        let held = ctx.image_gate.clone().try_acquire_owned().unwrap();
        let x = wire(&r, &p, Some(&c), Call::CloseContext).await;
        assert!(x.is_error);
        assert!(!ctx.closed.load(Ordering::Acquire));
        drop(held);
        assert!(!wire(&r, &p, Some(&c), Call::CloseContext).await.is_error);
        assert!(get(&r, &p, &c, args(&target)).await.is_error);
    }
    #[tokio::test]
    async fn f31_live_image_closure_after_controller_read_discards_response() {
        let (_t, r, p, _b, v) = setup();
        let (c, target) = bind(&r, &p, v.clone()).await;
        let ctx = r.context(p.identity(), Some(&c)).unwrap();
        *v.close_hook.lock().unwrap() = Some(Arc::downgrade(&ctx));
        let x = get(&r, &p, &c, args(&target)).await;
        assert_eq!(x.data["code"], "context_closed");
        assert!(x.images.is_empty());
    }
    #[tokio::test]
    async fn f31_live_image_is_cached_only_not_a_model_catalog_tool() {
        let (_t, r, p, _b, v) = setup();
        let (c, target) = bind(&r, &p, v).await;
        assert!(
            !rwi_core::device::catalog::definitions()
                .iter()
                .any(|d| d.name == METHOD)
        );
        let x = wire(
            &r,
            &p,
            Some(&c),
            Call::Prepare {
                tool: METHOD.into(),
                arguments: args(&target),
            },
        )
        .await;
        assert!(x.is_error);
        assert!(x.data["operation_ref"].is_null());
        let d = wire(&r, &p, None, Call::Describe).await;
        assert_eq!(
            d.data["executor_features"]["cached_live_image_v1"]["implemented"],
            true
        );
    }
    #[tokio::test]
    async fn f31_live_image_never_clears_input_fence_or_writer_marker() {
        let (_t, r, p, _b, v) = setup();
        let (c, target) = bind(&r, &p, v).await;
        let ctx = r.context(p.identity(), Some(&c)).unwrap();
        ctx.domain.fenced.store(true, Ordering::Release);
        ctx.entered.store(true, Ordering::Release);
        let x = get(&r, &p, &c, args(&target)).await;
        assert!(!x.is_error);
        assert_eq!(x.data["input_domain_fenced"], true);
        assert!(ctx.domain.fenced.load(Ordering::Acquire));
        assert!(ctx.entered.load(Ordering::Acquire));
    }
}
