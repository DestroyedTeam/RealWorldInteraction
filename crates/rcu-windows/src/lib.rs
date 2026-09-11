//! Windows-native observation and opt-in grounded input. No implicit target/focus.
#[cfg(windows)]
pub mod agent_binding;
pub mod agent_contract;
mod contract;
#[cfg(any(windows, test))]
mod document_context;
pub use contract::*;
#[cfg(any(windows, test))]
mod application;
#[cfg(windows)]
pub use application::WindowsLifecycleDriver;
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
