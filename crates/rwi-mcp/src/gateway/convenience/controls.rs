use super::*;
use std::collections::BTreeSet;

/// Display-only evidence from the original macOS observation. These IDs were
/// already classified Plain by Native; writability/focus never grants input here.
pub(super) fn native_plain_display_candidates(observation: &Value) -> BTreeSet<String> {
    let empty = BTreeSet::new;
    if observation["adapter_status"]["adapter"] != "rust-macos-native"
        || observation["scene"]["assurance"] != "native-attested"
        || !observation["scene"]["binding"].is_object()
    {
        return empty();
    }
    let Some(sidecar) = observation.pointer(
        "/adapter_status/observation_consistency/ax_coverage_after/editable_focus_availability",
    ) else {
        return empty();
    };
    if sidecar["api"] != "AXUIElementIsAttributeSettable"
        || sidecar["attribute"] != "AXFocused"
        || sidecar["scope"] != "returned_classified_plain_text_roles_only"
        || sidecar["input_authority"] != false
        || sidecar["absence_proven"] != false
        || sidecar["probe_limit"] != 4
    {
        return empty();
    }
    let (Some(entries), Some(rows)) = (
        sidecar["elements"].as_array(),
        observation["scene"]["elements"].as_array(),
    ) else {
        return empty();
    };
    if entries.len() > 4 {
        return empty();
    }
    entries
        .iter()
        .filter_map(|entry| {
            let id = entry["element"].as_str()?;
            let status = i32::try_from(entry["native_status"].as_i64()?).ok()?;
            let shape_known = if status == 0 {
                entry["state"] == "read" && entry["settable"].is_boolean()
            } else {
                entry["state"] == "unknown" && entry.get("settable").is_some_and(Value::is_null)
            };
            // False/error availability still identifies a classified Plain row.
            // Missing/budget-omitted IDs and foreign or ambiguous IDs stay unknown.
            let mut exact = rows.iter().filter(|row| row["id"] == id);
            let row = exact.next()?;
            (shape_known
                && !id.is_empty()
                && exact.next().is_none()
                && matches!(
                    row["role"].as_str(),
                    Some("AXTextField" | "AXTextArea" | "AXComboBox")
                ))
            .then(|| id.to_owned())
        })
        .collect()
}

pub(super) fn eligible_control(e: &Value) -> bool {
    e["actions"]
        .as_array()
        .is_some_and(|a| a.iter().any(standard_action))
        || !e["control_type"].is_null()
}
fn eligible_with_candidates(e: &Value, candidates: &BTreeSet<String>) -> bool {
    eligible_control(e) || e["id"].as_str().is_some_and(|id| candidates.contains(id))
}
#[cfg(test)]
pub(super) fn control_page(rows: &[Value], args: &ControlsArgs) -> DeviceResult {
    control_page_with_candidates(rows, args, &BTreeSet::new())
}
pub(super) fn control_page_with_candidates(
    rows: &[Value],
    args: &ControlsArgs,
    candidates: &BTreeSet<String>,
) -> DeviceResult {
    let query = args.query.to_lowercase();
    let matched: Vec<_> = rows
        .iter()
        .filter(|e| {
            eligible_with_candidates(e, candidates)
                && (query.is_empty()
                    || ["name", "description", "role"].iter().any(|key| {
                        e[*key]
                            .as_str()
                            .is_some_and(|s| s.to_lowercase().contains(&query))
                    }))
        })
        .collect();
    let page: Vec<_> = matched
        .iter()
        .skip(args.offset)
        .take(args.limit)
        .map(|e| (*e).clone())
        .collect();
    let (elements, _) = compact_controls_in_order_with_candidates(&page, args.limit, candidates);
    if elements.is_empty() && !page.is_empty() {
        return DeviceResult::error(
            "control_display_budget",
            "A control exceeds display budget; inspect the original observation with computer.result.",
        );
    }
    let end = args.offset.saturating_add(elements.len());
    let next = (end < matched.len()).then_some(end);
    DeviceResult::value(
        json!({"frame":args.frame,"observation_ref":args.observation_ref,
        "query":args.query,"elements":elements,"matches":matched.len(),"next_offset":next,
        "frame_refreshed":false,"input_sent":false,"native_complete":"unknown"}),
    )
}

pub(super) fn standard_action(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|s| s.starts_with("AX") && s.bytes().all(|c| c.is_ascii_alphanumeric()))
}
// Stable default display ranking only. Every native row remains in the
// retained observation and explicit control pages keep their original order.
#[cfg(test)]
pub(super) fn compact_controls(rows: &[Value], limit: usize) -> (Vec<Value>, usize) {
    compact_controls_with_candidates(rows, limit, &BTreeSet::new())
}
pub(super) fn compact_controls_with_candidates(
    rows: &[Value],
    limit: usize,
    candidates: &BTreeSet<String>,
) -> (Vec<Value>, usize) {
    let eligible: Vec<_> = rows
        .iter()
        .filter(|e| eligible_with_candidates(e, candidates))
        .collect();
    let preferred: Vec<_> = eligible
        .iter()
        .filter(|e| !low_priority_unnamed_container(e))
        .map(|e| (*e).clone())
        .collect();
    // Collapse display-only noise when meaningful controls are available. Full
    // metadata and explicit pages are untouched; an all-container tree retains
    // its rows rather than pretending there are no controls.
    if preferred.is_empty() {
        return compact_controls_in_order_with_candidates(rows, limit, candidates);
    }
    let (out, _) = compact_controls_in_order_with_candidates(&preferred, limit, candidates);
    (out, eligible.len())
}
pub(super) fn low_priority_unnamed_container(e: &Value) -> bool {
    let unnamed = ["name", "description"]
        .iter()
        .all(|key| e[*key].as_str().is_none_or(|s| s.trim().is_empty()));
    let unavailable_point = e["visible"] == false || e["occluded"] == true;
    // Ordinary buttons (including anonymous close/minimize/maximize clusters),
    // named controls and unknown visibility are never demoted by this heuristic.
    // Covered semantic buttons remain useful and retain their exact coordinates.
    let meaningful_state =
        e["selected"] == true || e["value"].as_str().is_some_and(|v| !v.is_empty());
    unnamed
        && !meaningful_state
        && unavailable_point
        && matches!(
            e["role"].as_str(),
            Some("AXRow" | "AXColumn" | "AXGroup" | "AXToolbar")
        )
}
// Bounded display projection, never truncation of native authority or evidence.
#[cfg(test)]
pub(super) fn compact_controls_in_order(rows: &[Value], limit: usize) -> (Vec<Value>, usize) {
    compact_controls_in_order_with_candidates(rows, limit, &BTreeSet::new())
}
fn compact_controls_in_order_with_candidates(
    rows: &[Value],
    limit: usize,
    candidates: &BTreeSet<String>,
) -> (Vec<Value>, usize) {
    let eligible: Vec<_> = rows
        .iter()
        .filter(|e| eligible_with_candidates(e, candidates))
        .collect();
    let mut out = Vec::new();
    let mut used = 0;
    for e in eligible.iter().take(limit) {
        let mut row = serde_json::Map::new();
        for key in [
            "id",
            "name",
            "description",
            "role",
            "control_type",
            "value",
            "value_truncated",
            "actions",
            "point",
            "enabled",
            "selected",
            "visible",
            "occluded",
        ] {
            if let Some(v) = e.get(key)
                && !v.is_null()
            {
                let mut v = v.clone();
                if let Some(text) = v.as_str()
                    && text.chars().count() > 256
                    && key != "id"
                {
                    v = json!(text.chars().take(256).collect::<String>());
                    row.insert("display_truncated".into(), json!(true));
                }
                row.insert(key.into(), v);
            }
        }
        if let Some(actions) = row.get_mut("actions").and_then(Value::as_array_mut) {
            actions.retain(standard_action);
        }
        let name = row
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if name.is_empty() {
            if let Some(description) = row.remove("description") {
                row.insert("name".into(), description);
            }
        } else if row.get("description").and_then(Value::as_str) == Some(name.as_str()) {
            row.remove("description");
        }
        for (key, redundant) in [("enabled", true), ("visible", true), ("occluded", false)] {
            if row.get(key).and_then(Value::as_bool) == Some(redundant) {
                row.remove(key);
            }
        }
        if e["id"].as_str().is_some_and(|id| candidates.contains(id)) {
            row.insert(
                "display_candidate".into(),
                json!({"kind":"classified_plain_text","input_authority":false}),
            );
        }
        let v = Value::Object(row);
        let bytes = serde_json::to_vec(&v).unwrap().len();
        if used + bytes > 12 * 1024 {
            break;
        }
        used += bytes;
        out.push(v);
    }
    (out, eligible.len())
}
