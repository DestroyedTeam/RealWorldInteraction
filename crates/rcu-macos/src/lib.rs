//! Native macOS backend, linked directly to Apple frameworks. No subprocess runtime.
#[cfg(target_os = "macos")]
mod capture;
#[cfg(target_os = "macos")]
mod ffi;
#[cfg(any(target_os = "macos", test))]
mod geometry;
#[cfg(target_os = "macos")]
mod native;
#[cfg(target_os = "macos")]
pub use native::*;
