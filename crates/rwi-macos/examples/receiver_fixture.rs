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
#[path = "receiver_fixture/model.rs"]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod model;
#[path = "receiver_fixture/native_dnd.rs"]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod native_dnd;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(options) = model::Options::parse(std::env::args().skip(1))? else {
        println!("{}", model::USAGE);
        return Ok(());
    };
    if options.check_only {
        println!(
            "{}",
            serde_json::json!({"configuration_valid":true,"gui_created":false,"input_sent":false,"native_binding_verified":false,"native_dnd_controls":options.native_dnd_controls,"command_tab":options.command_tab})
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
