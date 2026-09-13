//! Windows-native observation and opt-in grounded input. No implicit target/focus.
#[cfg(windows)]
pub mod agent_binding;
pub mod agent_contract;
mod contract;
#[cfg(any(windows, test))]
mod document_context;
#[cfg(windows)]
pub mod recovery;
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
#[cfg(any(windows, test))]
mod input_trajectory;
#[cfg(windows)]
mod native;
#[cfg(windows)]
pub mod pipe;
#[cfg(any(windows, test))]
mod pointer_feedback;
#[cfg(any(windows, test))]
mod revalidation;
#[cfg(windows)]
mod uia;
#[cfg(any(windows, test))]
mod uia_ancestry;
#[cfg(windows)]
pub use native::*;

/// Explicit service lifecycle, not a side effect of discovery/observation.
#[cfg(windows)]
pub fn start_pointer_feedback() -> rcu_core::Result<()> {
    pointer_feedback::start()
}
#[cfg(windows)]
pub fn stop_pointer_feedback() {
    pointer_feedback::stop();
}
