//! Native macOS backend, linked directly to Apple frameworks. No subprocess runtime.
#[cfg(target_os = "macos")]
mod background;
#[cfg(target_os = "macos")]
mod capture;
#[cfg(target_os = "macos")]
mod catalog;
#[cfg(target_os = "macos")]
mod desktop;
#[cfg(target_os = "macos")]
mod ffi;
#[cfg(any(target_os = "macos", test))]
mod geometry;
#[cfg(target_os = "macos")]
mod native;
#[cfg(target_os = "macos")]
mod recovery;
#[cfg(target_os = "macos")]
mod session;
#[cfg(target_os = "macos")]
pub use desktop::NativeMacDesktop;
#[cfg(target_os = "macos")]
pub use native::*;
#[cfg(target_os = "macos")]
pub use recovery::input_resources;
#[cfg(target_os = "macos")]
pub use session::process_status;
