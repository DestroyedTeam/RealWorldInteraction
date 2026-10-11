use super::*;

pub(super) fn eligible_control(e: &Value) -> bool {
    e["actions"]
        .as_array()
        .is_some_and(|a| a.iter().any(standard_action))
        || !e["control_type"].is_null()
}
pub(super) fn control_page(rows: &[Value], args: &ControlsArgs) -> DeviceResult {
    let query = args.query.to_lowercase();
    let matched: Vec<_> = rows
        .iter()
        .filter(|e| {
            eligible_control(e)
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
    let (elements, _) = compact_controls_in_order(&page, args.limit);
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
pub(super) fn compact_controls(rows: &[Value], limit: usize) -> (Vec<Value>, usize) {
    let eligible: Vec<_> = rows.iter().filter(|e| eligible_control(e)).collect();
    let preferred: Vec<_> = eligible
        .iter()
        .filter(|e| !low_priority_unnamed_container(e))
        .map(|e| (*e).clone())
        .collect();
    // Collapse display-only noise when meaningful controls are available. Full
    // metadata and explicit pages are untouched; an all-container tree retains
    // its rows rather than pretending there are no controls.
    if preferred.is_empty() {
        return compact_controls_in_order(rows, limit);
    }
    let (out, _) = compact_controls_in_order(&preferred, limit);
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
pub(super) fn compact_controls_in_order(rows: &[Value], limit: usize) -> (Vec<Value>, usize) {
    let eligible: Vec<_> = rows.iter().filter(|e| eligible_control(e)).collect();
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
