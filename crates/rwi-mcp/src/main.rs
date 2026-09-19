//! Thin MCP executable; protocol adapter and process bootstrap are separate.
#![cfg_attr(windows, windows_subsystem = "windows")]
#[cfg(feature = "native-agent-worker")]
mod agent_worker;
#[cfg(any(feature = "legacy-local", test))]
#[cfg_attr(test, allow(dead_code))]
mod bootstrap;
#[cfg(any(feature = "legacy-local", test))]
#[cfg_attr(test, allow(dead_code))]
mod eof_diagnostics;
mod gateway;
#[cfg(feature = "link-fixture")]
mod link_fixture;
#[cfg(any(feature = "legacy-local", test))]
mod server;

fn main() {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() == 1 && args[0] == "--js-worker" {
        if let Err(e) = rwi_js::run() {
            eprintln!("rwi-js worker: {e}");
            std::process::exit(1);
        }
        return;
    }
    if args.is_empty() || (args.len() == 2 && args[0] == "--profile") {
        if let Err(e) = gateway::run(args.get(1).map(std::path::Path::new)) {
            eprintln!("rwi-mcp: {e}");
            std::process::exit(1);
        }
        return;
    }
    #[cfg(feature = "legacy-local")]
    bootstrap::run();
    #[cfg(not(feature = "legacy-local"))]
    {
        eprintln!(
            "usage: rwi-mcp [--profile ABS_PRIVATE_PROFILE]; legacy local/harness CLI is not a production mode"
        );
        std::process::exit(1);
    }
}
