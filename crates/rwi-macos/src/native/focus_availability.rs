//! Bounded observation-only AXFocused writability; never an action advertisement.
//! Missing classification, probe errors and budget omissions never become false.
use super::{AxElement, TextClass};
use serde_json::{Value, json};

const PROBE_LIMIT: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Availability {
    settable: Option<bool>,
    native_status: Option<i32>,
}

#[derive(Default)]
pub(super) struct Budget {
    attempted: usize,
}
impl Budget {
    /// Only the ordinary bounded observation tree owns this local budget.
    pub(super) fn observe(
        &mut self,
        class: TextClass,
        read: impl FnOnce() -> (i32, u8),
    ) -> Option<Availability> {
        if class != TextClass::Plain {
            return None;
        }
        if self.attempted >= PROBE_LIMIT {
            return Some(Availability {
                settable: None,
                native_status: None,
            });
        }
        self.attempted += 1;
        let (status, raw) = read();
        Some(Availability {
            settable: (status == 0).then_some(raw != 0),
            native_status: Some(status),
        })
    }
}

pub(super) fn project(elements: &[AxElement]) -> Option<Value> {
    // Count omissions without allocating/serializing a row for each one.
    let availability = elements
        .iter()
        .filter_map(|e| e.focus_availability.as_ref());
    let attempted = availability
        .clone()
        .filter(|a| a.native_status.is_some())
        .count();
    let omitted = availability.filter(|a| a.native_status.is_none()).count();
    let rows = elements
        .iter()
        .filter_map(|element| {
            let availability = element.focus_availability.as_ref()?;
            let status = availability.native_status?;
            Some(json!({
                "element": element.element.id,
                "state": if status == 0 { "read" } else { "unknown" },
                "settable": availability.settable,
                "native_status": status,
            }))
        })
        .take(PROBE_LIMIT)
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return None;
    }
    Some(json!({
        "api": "AXUIElementIsAttributeSettable",
        "attribute": "AXFocused",
        "scope": "returned_classified_plain_text_roles_only",
        "probe_limit": PROBE_LIMIT,
        "attempted": attempted,
        "budget_omitted": omitted,
        "missing_entry": "unknown_or_not_probed; never false",
        "absence_proven": false,
        "input_authority": false,
        "elements": rows,
    }))
}

#[cfg(test)]
mod tests {
    use super::super::Element;
    use super::*;

    fn element(id: &str, availability: Option<Availability>) -> AxElement {
        AxElement {
            element: Element {
                id: id.into(),
                name: "Plain field".into(),
                role: Some("AXTextField".into()),
                actions: vec![],
                enabled: Some(true),
                selected: None,
                description: None,
                value: None,
                value_truncated: false,
                point: [1., 1.],
                visible: Some(true),
                occluded: Some(false),
                state_revision: Some("stable".into()),
            },
            text_class: TextClass::Plain,
            value: None,
            focus_availability: availability,
        }
    }

    #[test]
    fn known_true_and_false_require_success_status() {
        let mut budget = Budget::default();
        assert_eq!(
            budget
                .observe(TextClass::Plain, || (0, 1))
                .unwrap()
                .settable,
            Some(true)
        );
        assert_eq!(
            budget
                .observe(TextClass::Plain, || (0, 0))
                .unwrap()
                .settable,
            Some(false)
        );
    }

    #[test]
    fn errors_preserve_literal_status_and_ignore_boolean_out_parameter() {
        for raw in [0, 1] {
            let mut budget = Budget::default();
            let value = budget.observe(TextClass::Plain, || (-25204, raw)).unwrap();
            assert_eq!(value.settable, None);
            assert_eq!(value.native_status, Some(-25204));
        }
    }

    #[test]
    fn secure_unknown_and_nontext_never_query_native_writability() {
        let mut budget = Budget::default();
        for class in [TextClass::Secure, TextClass::Unknown, TextClass::NotText] {
            assert!(
                budget
                    .observe(class, || panic!(
                        "unclassified provider must not be queried"
                    ))
                    .is_none()
            );
        }
        assert_eq!(budget.attempted, 0);
    }

    #[test]
    fn four_probe_limit_is_local_and_omission_is_not_false() {
        let mut budget = Budget::default();
        for _ in 0..PROBE_LIMIT {
            assert_eq!(
                budget
                    .observe(TextClass::Plain, || (0, 0))
                    .unwrap()
                    .settable,
                Some(false)
            );
        }
        let omitted = budget
            .observe(TextClass::Plain, || panic!("budget exhausted"))
            .unwrap();
        assert_eq!(omitted.settable, None);
        assert_eq!(omitted.native_status, None);
        assert_eq!(budget.attempted, PROBE_LIMIT);
        assert_eq!(
            Budget::default()
                .observe(TextClass::Plain, || (0, 1))
                .unwrap()
                .settable,
            Some(true)
        );
    }

    #[test]
    fn metadata_keeps_object_ids_unknown_and_budget_omitted_without_authority() {
        let mut budget = Budget::default();
        let mut elements = Vec::new();
        for (i, status) in [0, 0, -25204, 0, 0].into_iter().enumerate() {
            elements.push(element(
                &format!("object-{i}"),
                budget.observe(TextClass::Plain, || (status, 1)),
            ));
        }
        let revision = elements[0].element.revision();
        let report = project(&elements).unwrap();
        assert_eq!(report["attempted"], 4);
        assert_eq!(report["budget_omitted"], 1);
        assert_eq!(report["input_authority"], false);
        assert_eq!(report["elements"][2]["state"], "unknown");
        assert_eq!(report["elements"][2]["native_status"], -25204);
        assert!(report["elements"][2]["settable"].is_null());
        assert_eq!(report["elements"].as_array().unwrap().len(), 4);
        assert_eq!(
            report["missing_entry"],
            "unknown_or_not_probed; never false"
        );
        assert_eq!(report["absence_proven"], false);
        assert_eq!(elements[0].element.revision(), revision);
        assert!(elements.iter().all(|e| e.element.actions.is_empty()));
        assert!(project(&[element("not-probed", None)]).is_none());
    }

    #[test]
    fn five_hundred_plain_nodes_keep_four_rows_and_aggregate_omissions() {
        let mut budget = Budget::default();
        let mut reads = 0;
        let elements = (0..500)
            .map(|i| {
                element(
                    &format!("many-{i}"),
                    budget.observe(TextClass::Plain, || {
                        reads += 1;
                        (0, 1)
                    }),
                )
            })
            .collect::<Vec<_>>();
        let report = project(&elements).unwrap();
        assert_eq!(reads, 4);
        assert_eq!(report["attempted"], 4);
        assert_eq!(report["budget_omitted"], 496);
        assert_eq!(report["elements"].as_array().unwrap().len(), 4);
        assert_eq!(
            report["missing_entry"],
            "unknown_or_not_probed; never false"
        );
        assert_eq!(report["absence_proven"], false);
        assert_eq!(report["input_authority"], false);
    }

    #[test]
    fn unclassified_rows_are_unqueried_not_known_false() {
        let mut budget = Budget::default();
        let mut elements = Vec::new();
        for class in [TextClass::Secure, TextClass::Unknown, TextClass::NotText] {
            let availability = budget.observe(class, || panic!("must not query"));
            assert!(availability.is_none());
            let mut node = element("unclassified", availability);
            node.text_class = class;
            elements.push(node);
        }
        assert!(project(&elements).is_none());
        assert_eq!(budget.attempted, 0);
        assert!(elements.iter().all(|e| e.focus_availability.is_none()));
    }
}
