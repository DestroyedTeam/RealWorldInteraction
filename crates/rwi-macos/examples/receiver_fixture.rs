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

#[path = "receiver_fixture/ime_source.rs"]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod ime_source;
#[cfg(target_os = "macos")]
#[path = "receiver_fixture/ime_source_native.rs"]
mod ime_source_native;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (arguments, own_source) = ime_source::split(std::env::args().skip(1))?;
    let Some(options) = model::Options::parse(arguments)? else {
        if own_source.is_some() {
            return Err("own source requires allocated window configuration".into());
        }
        println!(
            "{}\nDefault-off --own-input-source ID requires a traced regular sampled ordinary fixture; source changes/restoration are own-client calibration only.",
            model::USAGE
        );
        return Ok(());
    };
    ime_source::validate(
        own_source.is_some(),
        options.text_client_trace,
        options.regular_app,
        options.environment_sample_ms.is_some(),
        options.lifetime_ms,
        !options.modal_controls
            && !options.system_panel_controls
            && options.command_tab.is_none()
            && options.self_foreground_after_ms.is_none()
            && options.self_foreground_on_first_down_after_ms.is_none()
            && !options.legacy_self_foreground,
    )?;
    if options.check_only {
        println!(
            "{}",
            serde_json::json!({
                "configuration_valid":true,"gui_created":false,"input_sent":false,
                "save_panel_requested":options.save_panel,"native_save_panel_observed":false,
                "visibility_controls_requested":options.visibility_controls,"native_hidden_observed":false,
                "native_binding_verified":false,"native_dnd_controls":options.native_dnd_controls,
                "command_tab":options.command_tab,
                "first_mouse_acceptance_requested":options.first_mouse_acceptance,
                "first_mouse_native_selector_observed":false,
                "ax_focused_window_once_delay_ms":options.ax_focused_window_once_delay_ms,
                "semantic_alternate_ui_requested":options.semantic_alternate_ui,
                "semantic_native_callbacks_observed":false,
                "text_client_trace_requested":options.text_client_trace,
                "own_input_source_requested":own_source.as_deref(),
                "own_input_source_selected":false,
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
        ime_source_native::configure(own_source, &options.state_dir)?;
        appkit::run(options)?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = options;
        Err("receiver_fixture requires macOS; no window was created".into())
    }
}
