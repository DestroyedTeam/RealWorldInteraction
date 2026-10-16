//! A value's missing pattern is not the same as missing safety classification.
use rwi_core::{Result, fault};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Class {
    pub control: i32,
    pub provider: i32,
    pub runtime: Vec<i32>,
    pub password: bool,
    pub offscreen: bool,
}
impl Class {
    fn check(self) -> Result<Self> {
        if !matches!(self.control, 50004 | 50030)
            || self.provider <= 0
            || self.password
            || self.offscreen
            || self.runtime.is_empty()
            || self.runtime.len() > 64
        {
            Err(fault(
                "uia_value_classification_changed",
                "Value target is sensitive, hidden or no longer a classified native Edit/Document.",
            ))
        } else {
            Ok(self)
        }
    }
}
pub(crate) fn blocks_input(code: &str) -> bool {
    code.starts_with("uia_value_classification_")
}
pub(crate) fn read<T>(
    mut classify: impl FnMut() -> Result<Class>,
    value: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let mut checked = || {
        classify()
            .map_err(|e| {
                if matches!(
                    e.code.as_str(),
                    "operation_cancelled" | "operation_deadline"
                ) {
                    e
                } else {
                    fault(
                        "uia_value_classification_unavailable",
                        &format!(
                            "Current value classification failed ({}); not a readable-value proof.",
                            e.code
                        ),
                    )
                }
            })?
            .check()
    };
    let before = checked()?;
    let value = value();
    if value.as_ref().err().is_some_and(|e| {
        matches!(
            e.code.as_str(),
            "operation_cancelled" | "operation_deadline"
        )
    }) {
        return value;
    }
    if checked()? != before {
        return Err(fault(
            "uia_value_classification_changed",
            "Native value identity or classification changed during the read.",
        ));
    }
    value
}
#[cfg(test)]
mod tests {
    use super::*;
    fn class() -> Class {
        Class {
            control: 50004,
            provider: 3,
            runtime: vec![42, 1],
            password: false,
            offscreen: false,
        }
    }
    #[test]
    fn unknown_or_sensitive_classification_never_invokes_the_value_provider() {
        let mut sensitive = class();
        sensitive.password = true;
        for result in [Err(fault("uia_password_unknown", "fixture")), Ok(sensitive)] {
            let e = read(
                || result.clone(),
                || -> Result<String> { panic!("must not read") },
            )
            .unwrap_err();
            assert!(blocks_input(&e.code));
        }
    }
    #[test]
    fn identity_or_classification_change_discards_value_instead_of_hashing_it() {
        for field in ["provider", "runtime", "password", "offscreen", "control"] {
            let before = class();
            let mut after = before.clone();
            match field {
                "provider" => after.provider += 1,
                "runtime" => after.runtime.push(2),
                "password" => after.password = true,
                "offscreen" => after.offscreen = true,
                _ => after.control = 50030,
            }
            let mut samples = [Ok(before), Ok(after)].into_iter();
            let e = read(|| samples.next().unwrap(), || Ok("value")).unwrap_err();
            assert!(blocks_input(&e.code));
        }
    }
    #[test]
    fn missing_pattern_stays_unknown_and_is_not_a_new_global_keyboard_gate() {
        let e = read(
            || Ok(class()),
            || -> Result<String> { Err(fault("uia_document_pattern_unavailable", "fixture")) },
        )
        .unwrap_err();
        assert!(!blocks_input(&e.code));
        assert_eq!(read(|| Ok(class()), || Ok("known")).unwrap(), "known");
    }
    #[test]
    fn failing_value_provider_does_not_hide_a_subsequent_safety_change() {
        let mut after = class();
        after.password = true;
        let mut samples = [Ok(class()), Ok(after)].into_iter();
        let e = read(
            || samples.next().unwrap(),
            || -> Result<String> { Err(fault("uia_document_pattern_unavailable", "fixture")) },
        )
        .unwrap_err();
        assert!(blocks_input(&e.code));
    }
}
