//! Independent macOS coordinate-model probe (read-only, no input, no capture).
//!
//! Prints the native facts required to map image pixels -> window points ->
//! global desktop points, so a calibration never depends on remembered
//! constants. Conventions used here:
//!   * CG global display space: origin = top-left of the MAIN display, y grows
//!     downward, units = points (kCGWindowBounds / AX position / CGEvent
//!     locations all live in this space).
//!   * Cocoa/AppKit screen space: origin = bottom-left of the primary screen,
//!     y grows upward. Flip: cocoa_y = primary_height - cg_y.
//!   * Retina: points != pixels; scale = CGDisplayPixelsWide / bounds.width.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use core_graphics::display::CGDisplay;

    let main = CGDisplay::main();
    let main_bounds = main.bounds();
    let scale = main.pixels_wide() as f64 / main_bounds.size.width;
    println!(
        "main id={} bounds_pt=({:.1},{:.1},{:.1},{:.1}) pixels={}x{} scale={:.4} primary_height_pt={:.1}",
        main.id,
        main_bounds.origin.x,
        main_bounds.origin.y,
        main_bounds.size.width,
        main_bounds.size.height,
        main.pixels_wide(),
        main.pixels_high(),
        scale,
        main_bounds.size.height
    );

    for id in CGDisplay::active_displays().unwrap_or_default() {
        let display = CGDisplay::new(id);
        let bounds = display.bounds();
        let pixels = (display.pixels_wide(), display.pixels_high());
        let display_scale = pixels.0 as f64 / bounds.size.width.max(1.0);
        println!(
            "display id={} main={} origin_pt=({:.1},{:.1}) size_pt=({:.1},{:.1}) pixels={}x{} scale={:.4} (CG scale is NOT the backing scale under HiDPI)",
            id,
            id == CGDisplay::main().id,
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            bounds.size.height,
            pixels.0,
            pixels.1,
            display_scale
        );
    }

    // Cursor position: global display points, top-left origin (never moves it).
    if let Ok(source) = core_graphics::event_source::CGEventSource::new(
        core_graphics::event_source::CGEventSourceStateID::CombinedSessionState,
    ) {
        if let Ok(event) = core_graphics::event::CGEvent::new(source) {
            let p = event.location();
            println!("cursor_pt=({:.2},{:.2})", p.x, p.y);
            println!(
                "cursor_in_cocoa=(x={:.2}, y={:.2})  [primary_height - cg_y]",
                p.x,
                main_bounds.size.height - p.y
            );
        }
    }

    // Worked example: the Paint S window observed as 1262x551 pt with a
    // 2524x1102 px image. Show the exact scale and a sample mapping.
    let (win_w_pt, win_h_pt, img_w_px, img_h_px) = (1262.0f64, 551.0, 2524.0f64, 1102.0);
    println!(
        "example window: image_scale_x={:.4} image_scale_y={:.4} (expected 2.0 on Retina)",
        img_w_px / win_w_pt,
        img_h_px / win_h_pt
    );
    Ok(())
}
