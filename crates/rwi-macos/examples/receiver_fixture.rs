//! Opt-in receiving fixture, never a production backend.
//! An explicit peer-pinned Command+Tab actor is separately gated and default-off.
#[cfg(target_os = "macos")]
#[path = "receiver_fixture/appkit.rs"]
mod appkit;
#[cfg(target_os = "macos")]
#[path = "receiver_fixture/ax_delay.rs"]
mod ax_delay;
#[path = "receiver_fixture/command_tab.rs"]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod command_tab;
#[path = "receiver_fixture/first_mouse.rs"]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod first_mouse;
#[path = "receiver_fixture/model.rs"]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod model;
#[path = "receiver_fixture/native_dnd.rs"]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod native_dnd;
#[path = "receiver_fixture/semantic_alternate_ui.rs"]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod semantic_alternate_ui;
#[path = "receiver_fixture/text_client.rs"]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod text_client;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(options) = model::Options::parse(std::env::args().skip(1))? else {
        println!("{}", model::USAGE);
        return Ok(());
    };
    if options.check_only {
        println!(
            "{}",
            serde_json::json!({
                "configuration_valid":true,"gui_created":false,"input_sent":false,
                "native_binding_verified":false,"native_dnd_controls":options.native_dnd_controls,
                "command_tab":options.command_tab,
                "first_mouse_acceptance_requested":options.first_mouse_acceptance,
                "first_mouse_native_selector_observed":false,
                "ax_focused_window_once_delay_ms":options.ax_focused_window_once_delay_ms,
                "semantic_alternate_ui_requested":options.semantic_alternate_ui,
                "semantic_native_callbacks_observed":false,
                "text_client_trace_requested":options.text_client_trace,
                "text_client_native_callbacks_observed":false,
                "legacy_tracking_calibration_requested":options.legacy_tracking_calibration,
                "legacy_tracking_registered":false,
                "cooperative_owner_configured":options.cooperative_owner.is_some(),
                "cooperative_owner_native_peer_authenticated":false,
                "cooperative_native_dispatches":0,
                "legacy_tracking_native_callbacks_observed":false,
                "native_timeline":{
                    "implemented":true,"enabled":options.native_dnd_controls,
                    "max_events":native_dnd::MAX_TIMELINE_EVENTS,
                    "clock_scope":"process_local_monotonic_plus_unix_samples",
                    "native_callbacks_observed":false
                }
            })
        );
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        appkit::run(options)?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = options;
        Err("receiver_fixture requires macOS; no window was created".into())
    }
}
