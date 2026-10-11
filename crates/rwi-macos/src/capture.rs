//! ScreenCaptureKit callback ownership is confined to callbacks. Only owned bytes cross threads.
mod pending;
use crate::{ffi, geometry::Rect};
use block2::RcBlock;
use core_graphics::{
    color_space::CGColorSpace,
    context::CGContext,
    geometry::{CGPoint, CGRect, CGSize},
    image::{CGImage, CGImageAlphaInfo, CGImageByteOrderInfo},
};
use foreign_types::ForeignType;
use objc2::{
    msg_send,
    rc::{Allocated, Retained, autoreleasepool},
    runtime::{AnyClass, AnyObject, Bool},
};
use objc2_foundation::NSRect;
use rwi_core::{MAX_FRAME_BYTES, Result, fault};
use std::{
    ffi::c_void,
    io::Write,
    sync::{Arc, atomic::AtomicBool, mpsc},
    time::{Duration, Instant},
};

#[link(name = "ScreenCaptureKit", kind = "framework")]
unsafe extern "C" {}
pub struct Capture {
    pub png: Vec<u8>,
    pub size: [u32; 2],
    /// Native ScreenCaptureKit window pixels before any visual max_edge cap.
    pub native_size: [u32; 2],
    /// Content-region digest: the native title-bar chrome band is excluded so
    /// activation-state repaints cannot invalidate actionable content. The
    /// bottom one logical point is also excluded (compositor edge dithering);
    /// synthetic pointer paths cannot target that excluded edge.
    pub content_sha: String,
}
#[derive(Clone)]
pub struct Capturer {
    busy: Arc<AtomicBool>,
}
impl Default for Capturer {
    fn default() -> Self {
        Self {
            busy: Arc::new(AtomicBool::new(false)),
        }
    }
}
fn class(name: &std::ffi::CStr) -> Result<&'static AnyClass> {
    AnyClass::get(name).ok_or_else(|| {
        fault(
            "capture_api_unavailable",
            "ScreenCaptureKit screenshot API requires macOS 14+.",
        )
    })
}
pub(crate) fn api_available() -> bool {
    [
        c"SCShareableContent",
        c"SCContentFilter",
        c"SCStreamConfiguration",
        c"SCScreenshotManager",
    ]
    .iter()
    .all(|name| AnyClass::get(name).is_some())
}
impl Capturer {
    pub fn capture(&self, pid: u32, window: u32, expected: Rect) -> Result<Capture> {
        self.capture_with_limit(pid, window, expected, None)
    }

    /// Size the response-only visual from ScreenCaptureKit's actual native
    /// pointPixelScale, then cap its longest pixel edge. This does not decode
    /// an observe PNG or create an input-grounding frame.
    pub(crate) fn capture_visual(
        &self,
        pid: u32,
        window: u32,
        expected: Rect,
        max_edge: u32,
    ) -> Result<Capture> {
        if !(64..=2048).contains(&max_edge) {
            return Err(fault(
                "invalid_visual_limit",
                "Use a bounded visual output with max_edge 64..2048 pixels.",
            ));
        }
        self.capture_with_limit(pid, window, expected, Some(max_edge))
    }

    fn capture_with_limit(
        &self,
        pid: u32,
        window: u32,
        expected: Rect,
        max_edge: Option<u32>,
    ) -> Result<Capture> {
        if !unsafe { ffi::CGPreflightScreenCaptureAccess() } {
            return Err(fault(
                "screen_capture_permission_missing",
                "Native screenshot permission is unavailable; no permission request is issued.",
            ));
        }
        // If a timed-out native callback never completes, keep this target busy rather
        // than accumulating unbounded asynchronous captures on every retry.
        let pending = pending::Pending::begin(self.busy.clone())?;
        let (tx, rx) = mpsc::sync_channel(1);
        let callback_pending = pending.clone();
        let start = (|| -> Result<()> {
            let share = class(c"SCShareableContent")?;
            let filter_class = class(c"SCContentFilter")?;
            let screenshot_configuration = AnyClass::get(c"SCScreenshotConfiguration");
            let modern = screenshot_configuration.is_some();
            let configuration_class = match screenshot_configuration {
                Some(class) => class,
                None => class(c"SCStreamConfiguration")?,
            };
            let screenshot_class = class(c"SCScreenshotManager")?;
            let callback = RcBlock::new(move |content: *mut AnyObject, error: *mut AnyObject| {
                let _keep_native_scope = &callback_pending;
                autoreleasepool(|_| {
                    let result = callback_result(|| -> Result<()> {
                        callback_pending.require_active()?;
                        if content.is_null() || !error.is_null() {
                            return Err(fault(
                                "screen_capture_denied",
                                "Screen capture failed; inspect existing permission and target state.",
                            ));
                        }
                        // SAFETY: selectors/signatures are taken from the installed SDK's
                        // SCShareableContent.h, SCStream.h and SCScreenshotManager.h.
                        unsafe {
                            let windows: *mut AnyObject = msg_send![content, windows];
                            let count: usize = msg_send![windows, count];
                            if count > 2048 {
                                return Err(fault(
                                    "window_list_budget",
                                    "Too many shareable windows.",
                                ));
                            }
                            let mut selected = std::ptr::null_mut::<AnyObject>();
                            for i in 0..count {
                                let w: *mut AnyObject = msg_send![windows,objectAtIndex:i];
                                let wid: u32 = msg_send![w, windowID];
                                if wid == window {
                                    selected = w;
                                    break;
                                }
                            }
                            if selected.is_null() {
                                return Err(fault(
                                    "window_missing",
                                    "Pinned CGWindowID is not shareable; do not substitute another window.",
                                ));
                            }
                            let app: *mut AnyObject = msg_send![selected, owningApplication];
                            if app.is_null() {
                                return Err(fault("pid_unknown", "Shareable window has no owner."));
                            }
                            let owner: i32 = msg_send![app, processID];
                            let rect: NSRect = msg_send![selected, frame];
                            let rect = Rect {
                                x: rect.origin.x,
                                y: rect.origin.y,
                                w: rect.size.width,
                                h: rect.size.height,
                            };
                            if owner != pid as i32 || !expected.same(rect) {
                                return Err(fault(
                                    "capture_binding_changed",
                                    "PID or bounds changed before capture.",
                                ));
                            }
                            let allocated: Allocated<AnyObject> = msg_send![filter_class, alloc];
                            let filter: Retained<AnyObject> =
                                msg_send![allocated,initWithDesktopIndependentWindow:selected];
                            let scale: f32 = msg_send![&*filter, pointPixelScale];
                            let content_rect: NSRect = msg_send![&*filter, contentRect];
                            if !scale.is_finite()
                                || scale <= 0.0
                                || (content_rect.size.width - rect.w).abs() > 0.01
                                || (content_rect.size.height - rect.h).abs() > 0.01
                            {
                                return Err(fault(
                                    "capture_geometry_unknown",
                                    "Filter content does not match the pinned full window.",
                                ));
                            }
                            let native_width = (rect.w * scale as f64).round() as usize;
                            let native_height = (rect.h * scale as f64).round() as usize;
                            validate_pixels(native_width, native_height)?;
                            let [width, height] = max_edge
                                .map(|limit| {
                                    bounded_native_size([native_width, native_height], limit)
                                })
                                .unwrap_or([native_width, native_height]);
                            validate_pixels(width, height)?;
                            if width > native_width || height > native_height {
                                return Err(fault(
                                    "visual_upscale_unavailable",
                                    "Visual output cannot exceed native window pixels.",
                                ));
                            }
                            let configuration = screenshot_configuration_for(
                                configuration_class,
                                rect,
                                width,
                                height,
                                modern,
                            );
                            let tx = tx.clone();
                            let completed = callback_pending.clone();
                            let keep_filter = filter.clone();
                            let keep_configuration = configuration.clone();
                            let completion = RcBlock::new(
                                move |output: *const c_void, error: *mut AnyObject| {
                                    let _ = (&keep_filter, &keep_configuration, &completed);
                                    let result = if output.is_null() || !error.is_null() {
                                        Err(fault(
                                            "capture_failed",
                                            "Native screenshot callback failed; no synthetic substitute was used.",
                                        ))
                                    } else {
                                        callback_result(|| {
                                            completed.require_active()?;
                                            let image: *const c_void = if modern {
                                                msg_send![output.cast::<AnyObject>(), sdrImage]
                                            } else {
                                                output
                                            };
                                            if image.is_null() {
                                                return Err(fault(
                                                    "capture_failed",
                                                    "Native screenshot has no SDR image.",
                                                ));
                                            }
                                            let mut capture =
                                                encode_native_image(image, width, height, rect.h)?;
                                            capture.native_size =
                                                [native_width as u32, native_height as u32];
                                            Ok(capture)
                                        })
                                    };
                                    let _ = tx.send(result);
                                },
                            );
                            callback_pending.require_active()?;
                            if modern {
                                let _: () = msg_send![screenshot_class,captureScreenshotWithFilter:&*filter,configuration:&*configuration,completionHandler:&*completion];
                            } else {
                                let _: () = msg_send![screenshot_class,captureImageWithFilter:&*filter,configuration:&*configuration,completionHandler:&*completion];
                            }
                        }
                        Ok(())
                    });
                    if let Err(e) = result {
                        let _ = tx.send(Err(e));
                    }
                });
            });
            unsafe {
                let _: () = msg_send![share,getShareableContentExcludingDesktopWindows:Bool::YES,onScreenWindowsOnly:Bool::YES,completionHandler:&*callback];
            }
            Ok(())
        })();
        start?;
        let started = Instant::now();
        loop {
            if let Err(error) = rwi_core::operation::checkpoint("macos_capture_wait") {
                pending.abandon();
                return Err(error);
            }
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Timeout)
                    if started.elapsed() < Duration::from_secs(6) => {}
                Err(_) => {
                    pending.abandon();
                    return Err(fault(
                        "capture_timeout",
                        "Native capture did not complete. Its callback retains the native scope and shared capture slot; do not replay input.",
                    ));
                }
            }
        }
    }
}
/// macOS 26 screenshot configuration uses an explicit display-logical source
/// rectangle. A sheet must not inherit a parent/window-group capture extent.
/// Older systems retain their existing SCStreamConfiguration capture path.
unsafe fn screenshot_configuration_for(
    class: &AnyClass,
    _rect: Rect,
    width: usize,
    height: usize,
    modern: bool,
) -> Retained<AnyObject> {
    unsafe {
        let config: Retained<AnyObject> = msg_send![class, new];
        let _: () = msg_send![&*config, setWidth:width as isize];
        let _: () = msg_send![&*config, setHeight:height as isize];
        let _: () = msg_send![&*config, setShowsCursor:Bool::NO];
        if modern {
            // The filter already selects the complete window. A display-space
            // crop here can crop the window a second time (observed on macOS 27).
            // Leave sourceRect at its native full-frame default.

            let _: () = msg_send![&*config, setIgnoreShadows:Bool::YES];
            let _: () = msg_send![&*config, setIncludeChildWindows:Bool::NO];
            let _: () = msg_send![&*config, setDynamicRange:0isize];
        } else {
            let _: () = msg_send![&*config, setIgnoreShadowsSingleWindow:Bool::YES];
            let _: () = msg_send![&*config, setScalesToFit:Bool::NO];
        }
        config
    }
}
fn callback_result<T>(operation: impl FnOnce() -> Result<T>) -> Result<T> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)).unwrap_or_else(|_| {
        Err(fault(
            "native_callback_failed",
            "Rust callback failed before returning native data; no input retry is performed.",
        ))
    })
}
fn bounded_native_size(native: [usize; 2], max_edge: u32) -> [usize; 2] {
    let edge = native[0].max(native[1]);
    let limit = max_edge as usize;
    if edge <= limit {
        native
    } else {
        native.map(|value| (value * limit / edge).max(1))
    }
}
fn validate_pixels(w: usize, h: usize) -> Result<()> {
    if w == 0
        || h == 0
        || w > 16384
        || h > 16384
        || w.checked_mul(h).is_none_or(|pixels| pixels > 16_000_000)
    {
        Err(fault(
            "screenshot_pixel_budget",
            "Capture a smaller window; no automatic downscaling or resolution change.",
        ))
    } else {
        Ok(())
    }
}
unsafe fn encode_native_image(
    ptr: *const c_void,
    w: usize,
    h: usize,
    logical_height: f64,
) -> Result<Capture> {
    // Borrowed callback image is retained exactly once, then released by CGImage.
    let image = unsafe { CGImage::from_ptr(ffi::CFRetain(ptr).cast_mut().cast()) };
    if image.width() != w || image.height() != h {
        return Err(fault(
            "capture_geometry_changed",
            "Native image size differs from the attested filter configuration.",
        ));
    }
    validate_pixels(w, h)?;
    let color = CGColorSpace::create_device_rgb();
    let mut context = CGContext::create_bitmap_context(
        None,
        w,
        h,
        8,
        w * 4,
        &color,
        CGImageAlphaInfo::CGImageAlphaPremultipliedLast as u32
            | CGImageByteOrderInfo::CGImageByteOrder32Big as u32,
    );
    context.draw_image(
        CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(w as f64, h as f64)),
        &image,
    );
    let pixels = context.data();
    for rgba in pixels.chunks_exact_mut(4) {
        let a = rgba[3] as u32;
        if a != 0 && a != 255 {
            for channel in &mut rgba[..3] {
                *channel = ((*channel as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    let content_sha = content_digest(pixels, w, h, logical_height);
    Ok(Capture {
        png: encode_rgba(w, h, pixels)?,
        size: [w as u32, h as u32],
        native_size: [w as u32, h as u32],
        content_sha,
    })
}

fn content_digest(pixels: &[u8], w: usize, h: usize, logical_height: f64) -> String {
    let band = (h / 16).clamp(16, 96).min(h / 4);
    let bottom = (h as f64 / logical_height).ceil() as usize;
    // Tiny captures retain all pixels rather than accepting an empty digest.
    let range = if band.saturating_add(bottom) < h {
        band * w * 4..(h - bottom) * w * 4
    } else {
        0..pixels.len()
    };
    rwi_core::digest(&pixels[range])
}
struct Limited(Vec<u8>);
impl Write for Limited {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.0.len().saturating_add(buf.len()) > MAX_FRAME_BYTES {
            return Err(std::io::Error::other("PNG budget"));
        }
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encode_rgba(w: usize, h: usize, pixels: &[u8]) -> Result<Vec<u8>> {
    validate_pixels(w, h)?;
    if pixels.len() != w * h * 4 {
        return Err(fault("invalid_pixels", "RGBA buffer size mismatch."));
    }
    let mut out = Limited(Vec::new());
    {
        let mut encoder = png::Encoder::new(&mut out, w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|_| fault("png_encode_failed", "PNG header failed."))?;
        writer.write_image_data(pixels).map_err(|_|fault("screenshot_size_limit","PNG exceeded bounded output or failed encoding; no degraded image was substituted."))?;
        writer
            .finish()
            .map_err(|_| fault("png_encode_failed", "PNG did not finish."))?;
    }
    Ok(out.0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modern_full_window_capture_keeps_native_source_default() {
        let Some(class) = AnyClass::get(c"SCScreenshotConfiguration") else {
            return;
        };
        unsafe {
            let rect = Rect {
                x: -1200.0,
                y: 283.0,
                w: 880.0,
                h: 448.0,
            };
            let config = screenshot_configuration_for(class, rect, 1760, 896, true);
            let source: NSRect = msg_send![&*config, sourceRect];
            let defaults: Retained<AnyObject> = msg_send![class, new];
            let default_source: NSRect = msg_send![&*defaults, sourceRect];
            assert_eq!(source, default_source);
            let width: isize = msg_send![&*config, width];
            let height: isize = msg_send![&*config, height];
            let children: Bool = msg_send![&*config, includeChildWindows];
            let cursor: Bool = msg_send![&*config, showsCursor];
            assert_eq!((width, height), (1760, 896));
            assert!(!children.as_bool());
            assert!(!cursor.as_bool());
        }
    }
    #[test]
    fn callback_panic_is_contained_before_foreign_boundary() {
        let result: Result<()> = callback_result(|| panic!("synthetic callback failure"));
        assert_eq!(result.unwrap_err().code, "native_callback_failed");
    }
    #[test]
    fn native_bitmap_encoder_preserves_orientation_and_pixels_without_screen_capture() {
        use core_graphics::data_provider::CGDataProvider;
        let pixels = vec![
            255, 0, 0, 255, 0, 255, 128, 255, 0, 0, 255, 255, 8, 16, 24, 255,
        ];
        let provider = CGDataProvider::from_buffer(Arc::new(pixels.clone()));
        let image = CGImage::new(
            2,
            2,
            8,
            32,
            8,
            &CGColorSpace::create_device_rgb(),
            CGImageAlphaInfo::CGImageAlphaPremultipliedLast as u32
                | CGImageByteOrderInfo::CGImageByteOrder32Big as u32,
            &provider,
            false,
            0,
        );
        let capture = unsafe { encode_native_image(image.as_ptr().cast(), 2, 2, 2.0) }.unwrap();
        let mut reader = png::Decoder::new(std::io::Cursor::new(capture.png))
            .read_info()
            .unwrap();
        let mut actual = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut actual).unwrap();
        assert_eq!(&actual[..info.buffer_size()], pixels);
    }
    #[test]
    fn native_bitmap_alpha_is_explicitly_normalized_without_resize() {
        use core_graphics::data_provider::CGDataProvider;
        // Source is premultiplied device RGB, output is straight RGBA PNG.
        let source = vec![
            64, 32, 16, 128, 0, 0, 0, 0, 255, 16, 32, 255, 16, 32, 64, 128,
        ];
        let expected = [
            128u8, 64, 32, 128, 0, 0, 0, 0, 255, 16, 32, 255, 32, 64, 128, 128,
        ];
        let provider = CGDataProvider::from_buffer(Arc::new(source));
        let image = CGImage::new(
            2,
            2,
            8,
            32,
            8,
            &CGColorSpace::create_device_rgb(),
            CGImageAlphaInfo::CGImageAlphaPremultipliedLast as u32
                | CGImageByteOrderInfo::CGImageByteOrder32Big as u32,
            &provider,
            false,
            0,
        );
        let capture = unsafe { encode_native_image(image.as_ptr().cast(), 2, 2, 2.0) }.unwrap();
        assert_eq!(capture.size, [2, 2]);
        let mut reader = png::Decoder::new(std::io::Cursor::new(capture.png))
            .read_info()
            .unwrap();
        let mut actual = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut actual).unwrap();
        assert_eq!((info.width, info.height), (2, 2));
        for (a, e) in actual.iter().zip(expected) {
            assert!(
                a.abs_diff(e) <= 1,
                "expected normalized channel {e}, got {a}"
            );
        }
    }
    #[test]
    fn original_pixels_roundtrip_without_resizing() {
        let pixels = [
            255, 0, 0, 255, 0, 255, 128, 255, 0, 0, 255, 255, 8, 16, 24, 255,
        ];
        let data = encode_rgba(2, 2, &pixels).unwrap();
        let decoder = png::Decoder::new(std::io::Cursor::new(data));
        let mut reader = decoder.read_info().unwrap();
        let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut bytes).unwrap();
        assert_eq!((info.width, info.height), (2, 2));
        assert_eq!(&bytes[..info.buffer_size()], pixels);
    }
    #[test]
    fn unreasonable_capture_rejected_before_allocation() {
        assert_eq!(
            validate_pixels(20000, 20000).unwrap_err().code,
            "screenshot_pixel_budget"
        );
    }

    #[test]
    fn visual_limit_uses_native_point_pixel_scale_without_upscaling() {
        assert_eq!(bounded_native_size([2868, 1320], 2048), [2048, 942]);
        assert_eq!(bounded_native_size([1434, 660], 2048), [1434, 660]);
        assert_eq!(bounded_native_size([2868, 1320], 1024), [1024, 471]);
        assert_eq!(bounded_native_size([256, 128], 2048), [256, 128]);
    }

    #[test]
    fn compositor_bottom_fringe_is_excluded_but_body_changes_are_exact() {
        for scale in [1usize, 2] {
            let (w, h) = (20 * scale, 100 * scale);
            let mut pixels = vec![245; w * h * 4];
            let original = content_digest(&pixels, w, h, 100.0);
            for channel in &mut pixels[(h - scale) * w * 4..] {
                *channel = 246;
            }
            assert_eq!(content_digest(&pixels, w, h, 100.0), original);
            pixels[((h - scale - 1) * w + 10) * 4] += 1;
            assert_ne!(content_digest(&pixels, w, h, 100.0), original);
        }
    }
}
