//! Explicit foreground transitions are separate from background input.
use crate::{Result, fault};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ForegroundState {
    pub pid: u32,
    pub window: String,
    pub process_start: String,
    pub native_epoch: String,
}
impl ForegroundState {
    pub fn validate(&self) -> Result<()> {
        if self.pid == 0
            || self.pid > i32::MAX as u32
            || self.window.is_empty()
            || self.window.len() > 128
            || self.process_start.is_empty()
            || self.process_start.len() > 256
            || self.native_epoch.len() != 64
            || !self.native_epoch.bytes().all(|b| b.is_ascii_hexdigit())
            || self.window.contains('\0')
            || self.process_start.contains('\0')
        {
            return Err(fault(
                "invalid_foreground_tuple",
                "Use a fresh native PID/window/process-start/epoch tuple; unknown is not null foreground.",
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn foreground_tuple_is_bounded_and_does_not_grant_input() {
        let mut v = ForegroundState {
            pid: 42,
            window: "cgwindow:7".into(),
            process_start: "42:1:2".into(),
            native_epoch: "a".repeat(64),
        };
        assert!(v.validate().is_ok());
        v.pid = 0;
        assert!(v.validate().is_err());
        v.pid = 42;
        v.native_epoch = "unknown".into();
        assert!(v.validate().is_err());
    }
}

mod engine;
#[cfg(test)]
#[path = "foreground/tests.rs"]
mod engine_tests;
mod hotkey;
#[cfg(test)]
mod hotkey_tests;
