//! Positive exact-window facts for a bounded read-only AX relay.
use super::*;

pub(super) struct RelayFacts<'a> {
    pub(super) native_type: bool,
    pub(super) pid: Option<u32>,
    pub(super) window: Option<u32>,
    pub(super) rect: Option<Rect>,
    pub(super) role: Option<&'a str>,
}
pub(super) fn exact_relay_matches(
    pid: u32,
    window: u32,
    expected: Rect,
    facts: RelayFacts<'_>,
) -> bool {
    facts.native_type
        && facts.pid == Some(pid)
        && facts.window == Some(window)
        && facts.rect.is_some_and(|r| r.same(expected))
        && matches!(facts.role, Some("AXWindow" | "AXSheet"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rect() -> Rect {
        Rect {
            x: 1800.,
            y: 24.,
            w: 660.,
            h: 552.,
        }
    }
    fn facts() -> RelayFacts<'static> {
        RelayFacts {
            native_type: true,
            pid: Some(42),
            window: Some(7),
            rect: Some(rect()),
            role: Some("AXWindow"),
        }
    }
    #[test]
    fn exact_relay_requires_all_positive_facts_not_geometry_or_pid_alone() {
        assert!(exact_relay_matches(42, 7, rect(), facts()));
        assert!(exact_relay_matches(
            42,
            7,
            rect(),
            RelayFacts {
                role: Some("AXSheet"),
                ..facts()
            }
        ));
        for candidate in [
            RelayFacts {
                native_type: false,
                ..facts()
            },
            RelayFacts {
                pid: None,
                ..facts()
            },
            RelayFacts {
                pid: Some(43),
                ..facts()
            },
            RelayFacts {
                window: None,
                ..facts()
            },
            RelayFacts {
                window: Some(8),
                ..facts()
            },
            RelayFacts {
                rect: None,
                ..facts()
            },
            RelayFacts {
                rect: Some(Rect {
                    x: rect().x + 9.,
                    ..rect()
                }),
                ..facts()
            },
            RelayFacts {
                role: None,
                ..facts()
            },
            RelayFacts {
                role: Some("AXButton"),
                ..facts()
            },
        ] {
            assert!(!exact_relay_matches(42, 7, rect(), candidate));
        }
    }
}

/// A focused element from another provider cannot introduce a top-level object.
pub(super) fn focused_provider_matches(pid: u32, native_type: bool, observed: Option<u32>) -> bool {
    native_type && observed == Some(pid)
}

/// Only one read-only AXWindows messaging failure with no positive alias may
/// get a separately bounded inventory read. Never retry a mutation or resolve
/// an ambiguous alias by selecting another surface.
pub(super) fn inventory_retry_allowed(error: &rwi_core::Fault, aliases: usize) -> bool {
    aliases == 0 && ffi::observation_read_can_settle(error)
}
#[cfg(test)]
mod inventory_retry_tests {
    use super::*;
    fn original() -> rwi_core::Fault {
        let mut error = fault("ax_attribute_unavailable", "test inventory read");
        error.diagnostic = Some(Box::new(serde_json::json!({
            "api":"AXUIElementCopyAttributeValue", "attribute":"AXWindows",
            "ax_error":-25204,"operation_kind":"read","replay_allowed":false
        })));
        error
    }
    #[test]
    fn inventory_recovery_never_retries_input_denial_unknown_or_ambiguous_aliases() {
        assert!(inventory_retry_allowed(&original(), 0));
        assert!(!inventory_retry_allowed(&original(), 1));
        assert!(!inventory_retry_allowed(&original(), 2));
        for (key, value) in [
            ("api", serde_json::json!("AXUIElementPerformAction")),
            ("attribute", serde_json::json!("AXValue")),
            ("ax_error", serde_json::json!(-25211)),
            ("operation_kind", serde_json::json!("write")),
            ("replay_allowed", serde_json::json!(true)),
        ] {
            let mut error = original();
            error.diagnostic.as_mut().unwrap()[key] = value;
            assert!(!inventory_retry_allowed(&error, 0));
        }
        assert!(!inventory_retry_allowed(
            &fault("native_action_unconfirmed", "unknown"),
            0
        ));
    }
}
#[cfg(test)]
mod focused_provider_tests {
    #[test]
    fn missing_foreign_or_non_ax_provider_never_introduces_a_top_level_candidate() {
        use super::focused_provider_matches;
        assert!(focused_provider_matches(17, true, Some(17)));
        assert!(!focused_provider_matches(17, false, Some(17)));
        assert!(!focused_provider_matches(17, true, None));
        assert!(!focused_provider_matches(17, true, Some(18)));
    }
}
