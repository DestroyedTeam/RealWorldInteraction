//! Explicit own-fixture calibration, never a production input route.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
pub fn split(
    args: impl IntoIterator<Item = String>,
) -> Result<(Vec<String>, Option<String>), String> {
    let mut args = args.into_iter();
    let mut rest = Vec::new();
    let mut source = None;
    while let Some(arg) = args.next() {
        if arg == "--own-input-source" {
            if source.is_some() {
                return Err("duplicate own input source".into());
            }
            let id = args.next().ok_or("own input source ID missing")?;
            if !valid_id(&id) {
                return Err("bounded ASCII input source identifier required".into());
            }
            source = Some(id);
        } else {
            rest.push(arg);
        }
    }
    Ok((rest, source))
}
pub fn validate(
    requested: bool,
    trace: bool,
    regular: bool,
    sampled: bool,
    lifetime: u32,
    ordinary: bool,
) -> Result<(), String> {
    if requested && !(trace && regular && sampled && lifetime <= 300_000 && ordinary) {
        return Err("own input source requires a traced regular sampled ordinary fixture, lifetime<=300000; no modal/actor".into());
    }
    Ok(())
}
pub fn should_restore(
    applied: bool,
    original: Option<&str>,
    current: Option<&str>,
    requested: &str,
) -> bool {
    applied
        && original.is_some_and(valid_id)
        && original != Some(requested)
        && current == Some(requested)
}
pub const CATALOG_LIMIT: usize = 512;
#[derive(serde::Serialize)]
pub struct CatalogSample {
    pub state: &'static str,
    pub count: Option<usize>,
    pub identifiers: Vec<String>,
    pub unreadable: usize,
    pub budget_omissions: Option<usize>,
    pub requested_available: Option<bool>,
}
pub fn catalog_sample(
    count: Option<usize>,
    rows: &[Option<String>],
    requested: &str,
) -> CatalogSample {
    let expected = count.map(|n| n.min(CATALOG_LIMIT));
    let consistent = expected == Some(rows.len());
    let inspected = expected.unwrap_or(0).min(rows.len());
    let identifiers: Vec<_> = rows
        .iter()
        .take(inspected)
        .flatten()
        .filter(|s| valid_id(s))
        .cloned()
        .collect();
    let unreadable = inspected - identifiers.len();
    let complete = consistent && count.is_some_and(|n| n <= CATALOG_LIMIT) && unreadable == 0;
    let found = consistent && valid_id(requested) && identifiers.iter().any(|s| s == requested);
    CatalogSample {
        state: if count.is_none() {
            "unavailable"
        } else if complete {
            "complete"
        } else {
            "partial"
        },
        count,
        identifiers,
        unreadable,
        budget_omissions: count.map(|n| n.saturating_sub(inspected)),
        requested_available: if found {
            Some(true)
        } else if complete {
            Some(false)
        } else {
            None
        },
    }
}
#[cfg(test)]
mod catalog_tests {
    use super::*;
    #[test]
    fn f271_nil_catalog_is_unknown_not_absent() {
        let x = catalog_sample(None, &[], "wanted");
        assert_eq!(x.requested_available, None);
        assert_eq!(x.state, "unavailable");
    }
    #[test]
    fn f271_complete_empty_catalog_is_known_absent() {
        let x = catalog_sample(Some(0), &[], "wanted");
        assert_eq!(x.requested_available, Some(false));
        assert_eq!(x.state, "complete");
    }
    #[test]
    fn f271_exact_present_identifier_is_positive() {
        let x = catalog_sample(
            Some(2),
            &[Some("wanted".into()), Some("other".into())],
            "wanted",
        );
        assert_eq!(x.requested_available, Some(true));
        assert_eq!(x.identifiers.len(), 2);
    }
    #[test]
    fn f271_budget_omissions_do_not_prove_absence() {
        let mut rows = vec![Some("other".into()); CATALOG_LIMIT];
        let x = catalog_sample(Some(513), &rows, "wanted");
        assert_eq!(x.requested_available, None);
        assert_eq!(x.budget_omissions, Some(1));
        rows[0] = Some("wanted".into());
        assert_eq!(
            catalog_sample(Some(513), &rows, "wanted").requested_available,
            Some(true)
        );
    }
    #[test]
    fn f271_unreadable_identifiers_do_not_prove_absence() {
        for row in [None, Some("bad id".into())] {
            let x = catalog_sample(Some(1), &[row], "wanted");
            assert_eq!(x.requested_available, None);
            assert_eq!(x.unreadable, 1);
        }
    }
    #[test]
    fn f271_inconsistent_array_count_is_unknown() {
        assert_eq!(
            catalog_sample(Some(1), &[], "wanted").requested_available,
            None
        );
        assert_eq!(
            catalog_sample(
                Some(1),
                &[Some("wanted".into()), Some("extra".into())],
                "wanted"
            )
            .requested_available,
            None
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn f267_source_default_preserves_arguments() {
        let args = vec!["--open-window".into(), "--run-id".into(), "r".into()];
        assert_eq!(split(args.clone()).unwrap(), (args, None));
    }
    #[test]
    fn f267_source_extracts_only_explicit_option() {
        let (args, id) = split(
            [
                "--open-window",
                "--own-input-source",
                "com.apple.inputmethod.SCIM.ITABC",
                "--check-only",
            ]
            .map(str::to_owned),
        )
        .unwrap();
        assert_eq!(args, ["--open-window", "--check-only"]);
        assert_eq!(id.as_deref(), Some("com.apple.inputmethod.SCIM.ITABC"));
    }
    #[test]
    fn f267_source_rejects_duplicate_missing_and_invalid_ids() {
        for args in [
            vec!["--own-input-source"],
            vec!["--own-input-source", ""],
            vec!["--own-input-source", "bad id"],
            vec!["--own-input-source", "a", "--own-input-source", "a"],
        ] {
            assert!(split(args.into_iter().map(str::to_owned)).is_err());
        }
        assert!(!valid_id(&"a".repeat(257)));
        assert!(!valid_id("a\0b"));
    }
    #[test]
    fn f267_source_requires_explicit_ordinary_traced_sampled_scope() {
        assert!(validate(true, true, true, true, 300_000, true).is_ok());
        for values in [
            (false, true, true, 1000, true),
            (true, false, true, 1000, true),
            (true, true, false, 1000, true),
            (true, true, true, 300_001, true),
            (true, true, true, 1000, false),
        ] {
            assert!(validate(true, values.0, values.1, values.2, values.3, values.4).is_err());
        }
    }
    #[test]
    fn f267_source_default_has_no_new_gate() {
        assert!(validate(false, false, false, false, 600_000, false).is_ok());
    }
    #[test]
    fn f267_source_restores_only_its_own_applied_selection() {
        assert!(should_restore(
            true,
            Some("original"),
            Some("requested"),
            "requested"
        ));
        for (applied, original, current) in [
            (false, Some("original"), Some("requested")),
            (true, None, Some("requested")),
            (true, Some("requested"), Some("requested")),
            (true, Some("original"), Some("user.changed")),
            (true, Some("bad id"), Some("requested")),
        ] {
            assert!(!should_restore(applied, original, current, "requested"));
        }
    }
}
