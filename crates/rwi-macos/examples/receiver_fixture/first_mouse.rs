//! Fixture-only first-mouse policy; no framework calls, input or activation.
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct FirstMouseProbe {
    pub configured_accept: bool,
    pub selector_calls: u64,
    pub last_returned_accept: Option<bool>,
    pub counter_saturated: bool,
}
impl FirstMouseProbe {
    pub const fn new(configured_accept: bool) -> Self {
        Self {
            configured_accept,
            selector_calls: 0,
            last_returned_accept: None,
            counter_saturated: false,
        }
    }
    pub fn called(&mut self) -> bool {
        self.counter_saturated |= self.selector_calls == u64::MAX;
        self.selector_calls = self.selector_calls.saturating_add(1);
        self.last_returned_accept = Some(self.configured_accept);
        self.configured_accept
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Options;
    fn options(extra: &[&str]) -> Result<Options, String> {
        Options::parse(
            [
                "--open-window",
                "--run-id",
                "first-mouse-policy",
                "--state-dir",
                "/tmp/rwi-first-mouse-check-only",
                "--display-id",
                "4",
                "--check-only",
            ]
            .into_iter()
            .chain(extra.iter().copied())
            .map(str::to_owned),
        )?
        .ok_or_else(|| "Expected allocated configuration".into())
    }
    #[test]
    fn r102_default_yes_and_explicit_true_preserve_old_receiving_policy() {
        assert!(options(&[]).unwrap().first_mouse_acceptance);
        assert!(
            options(&["--first-mouse=true"])
                .unwrap()
                .first_mouse_acceptance
        );
    }
    #[test]
    fn r102_false_is_explicit_fixture_policy_and_check_only_is_not_native() {
        let o = options(&["--first-mouse=false"]).unwrap();
        assert!(!o.first_mouse_acceptance);
        assert!(o.check_only);
        let json = serde_json::to_value(FirstMouseProbe::new(false)).unwrap();
        assert_eq!(json["configured_accept"], false);
        assert_eq!(json["selector_calls"], 0);
        assert!(json["last_returned_accept"].is_null());
    }
    #[test]
    fn r102_unknown_duplicate_or_unallocated_first_mouse_flags_refuse() {
        for flags in [
            vec!["--first-mouse=TRUE"],
            vec!["--first-mouse=other"],
            vec!["--first-mouse=false", "--first-mouse=true"],
            vec!["--first-mouse=false", "--first-mouse=false"],
        ] {
            assert!(options(&flags).is_err());
        }
        assert!(Options::parse(["--first-mouse=false"].into_iter().map(str::to_owned)).is_err());
    }
    #[test]
    fn r102_selector_record_is_distinct_from_application_window_or_button_effect() {
        for accepted in [true, false] {
            let mut probe = FirstMouseProbe::new(accepted);
            assert_eq!(probe.called(), accepted);
            assert_eq!(probe.selector_calls, 1);
            assert_eq!(probe.last_returned_accept, Some(accepted));
            assert!(!probe.counter_saturated);
        }
    }
    #[test]
    fn r102_selector_counter_does_not_wrap_or_hide_saturation() {
        let mut probe = FirstMouseProbe::new(false);
        probe.selector_calls = u64::MAX;
        assert!(!probe.called());
        assert_eq!(probe.selector_calls, u64::MAX);
        assert!(probe.counter_saturated);
    }
    #[test]
    fn r102_cell_probe_has_no_reentrant_ui_state_borrow() {
        let state = std::cell::Cell::new(FirstMouseProbe::new(false));
        let mut probe = state.get();
        let result = probe.called();
        state.set(probe);
        assert!(!result);
        assert_eq!(state.get().selector_calls, 1);
    }
}
