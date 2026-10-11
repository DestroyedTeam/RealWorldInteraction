//! Opt-in AppKit receiving application, not an RWI backend or input sender.
#[cfg(target_os = "macos")]
#[path = "receiver_fixture/appkit.rs"]
mod appkit;
#[cfg(target_os = "macos")]
#[path = "receiver_fixture/ax_delay.rs"]
mod ax_delay;
#[path = "receiver_fixture/model.rs"]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod model;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(options) = model::Options::parse(std::env::args().skip(1))? else {
        println!("{}", model::USAGE);
        return Ok(());
    };
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
