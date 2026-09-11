//! Windows-native observation and opt-in grounded input. No implicit target/focus.
mod contract;
pub use contract::*;
#[cfg(windows)]
mod capture;
#[cfg(windows)]
mod desktop;
#[cfg(windows)]
mod input;
#[cfg(any(windows, test))]
mod input_plan;
#[cfg(windows)]
mod native;
#[cfg(windows)]
pub mod pipe;
#[cfg(windows)]
mod uia;
#[cfg(windows)]
pub use native::*;
