//! Known, model-facing targets detail reduction. Immutable raw cache is separate.
//! IDs, permissions, capability booleans/reasons and unexpected values are never
//! shortened, invented or discarded just to satisfy the presentation budget.
use serde_json::Value;

const CAPABILITY_PROSE: &[&str] = &[
    "background_operation",
    "binding_identity",
    "mapping",
    "modal_policy",
    "interruption_coverage",
    "target_scope",
    "scope",
    "selection",
    "revalidation",
    "replacement_or_eviction",
    "meaning",
    "motion_history",
    "point_policy",
    "route",
    "readback",
    "write_budget",
    "ownership",
    "presentation",
    "source",
    "capture_scope",
    "retention",
    "source_space",
    "coordinate_space",
    "sampling_policy",
    "required_destination_readback",
    "receiver_preparation_scope",
    "authentication_requested_scope",
    "window_creation_time_assurance",
];

fn omit_exact_object_duplicate(
    object: &mut serde_json::Map<String, Value>,
    redundant: &str,
    canonical: &str,
) -> bool {
    let equal = object.get(canonical).is_some_and(Value::is_object)
        && object.get(redundant) == object.get(canonical);
    if equal {
        object.remove(redundant);
    }
    equal
}
const TRANSPORT_STATISTICS: &[&str] = &[
    "authenticated_connections",
    "describe_reconnect_attempts",
    "describe_reconnect_successes",
    "keepalive_connections",
    "last_describe_reconnect",
    "pool_invalidations",
    "replies_validated",
    "request_writes_started",
    "retired_returns_discarded",
    "reused_checkouts",
];
const DOCUMENTED_BRANCHES: &[&str] = &[
    "background_input",
    "receiver_preparation",
    "capture_visual",
    "focus_element",
    "select_window_local",
    "foreground_activation",
    "foreground_keyboard",
    "live",
    "explicit_additional_visual_windows",
    "local_pixel_grounding",
    "paired_pointer_drag",
    "pointer_drag",
    "pointer_move",
    "background_appkit_experimental",
    "background_skylight_experimental",
    "foreground_system",
    "explicit_delta",
    "semantic_dialog_discovery",
    "semantic_object_identity",
    "visual_only_snapshot",
    "window_attachment",
    "window_discovery",
];
fn native_capability_prose(value: &mut Value) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    let before = object.len();
    object.retain(|key, value| !(CAPABILITY_PROSE.contains(&key.as_str()) && value.is_string()));
    let mut omitted = before != object.len();
    for (key, value) in object.iter_mut() {
        // Future/custom nested policy, arrays, values, identity tuples and native
        // coordinate objects are not documented prose sections; leave them exact.
        if DOCUMENTED_BRANCHES.contains(&key.as_str()) {
            omitted |= native_capability_prose(value);
        }
    }
    omitted
}

pub(super) fn project_known_fields(data: &mut Value, omitted: &mut Vec<&'static str>) {
    let native_bindings: Vec<(String, Value)> = data
        .get("targets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|target| {
            target["capabilities"]["adapter"].as_str() == Some("rust-macos-native")
                && target["identity"].is_object()
        })
        .filter_map(|target| {
            Some((
                target["target"].as_str()?.to_owned(),
                target["identity"].clone(),
            ))
        })
        .collect();
    // Preserve the legacy targets omission declaration, including empty peers.
    let mut repeated_targets = data.get("devices").is_some_and(Value::is_array);
    let mut transport = false;
    let mut recent = false;
    let mut repeated_status = false;
    let mut repeated_context = false;
    if let Some(devices) = data.get_mut("devices").and_then(Value::as_array_mut) {
        for device in devices {
            let Some(device) = device.as_object_mut() else {
                continue;
            };
            if let Some(state) = device.get_mut("state").and_then(Value::as_object_mut) {
                // This was already omitted by the legacy formatter for all peers.
                repeated_targets |= state.remove("targets").is_some();
            }
            let known_native = device.get("state").is_some_and(|state| {
                let (Some(device_id), Some(instance), Some(context)) = (
                    device.get("device_id").and_then(Value::as_str),
                    state.get("instance").and_then(Value::as_str),
                    state.get("native_context").filter(|v| v.is_object()),
                ) else {
                    return false;
                };
                let prefix = format!("{device_id}:{instance}:");
                !device_id.is_empty()
                    && !instance.is_empty()
                    && state.get("device_id").and_then(Value::as_str) == Some(device_id)
                    && state.get("platform").and_then(Value::as_str) == Some("macos")
                    && native_bindings.iter().any(|(target, identity)| {
                        target.starts_with(&prefix) && identity == context
                    })
            });
            // Unknown/future adapters and legacy text-only fixtures keep their
            // previous structured shape; nicknames are not native provenance.
            if !known_native {
                continue;
            }
            // Connection pool statistics are diagnostics, not device authority.
            if let Some(stats) = device
                .get_mut("adapter_transport")
                .and_then(Value::as_object_mut)
            {
                let before = stats.len();
                stats.retain(|key, _| !TRANSPORT_STATISTICS.contains(&key.as_str()));
                transport |= before != stats.len();
                if stats.is_empty() {
                    device.remove("adapter_transport");
                }
            }
            let Some(state) = device.get_mut("state").and_then(Value::as_object_mut) else {
                continue;
            };
            if state.get("recent_operations").is_some_and(Value::is_array) {
                state.remove("recent_operations");
                recent = true;
            }
            // Conflicting, null or future status shapes stay visible. Only exact
            // object equality permits one redundant service status to disappear.
            repeated_status |=
                omit_exact_object_duplicate(state, "service_process_status", "runtime_status");
            let context = state
                .get("native_context")
                .filter(|v| v.is_object())
                .cloned();
            if let Some(runtime) = state
                .get_mut("runtime_status")
                .and_then(Value::as_object_mut)
                && context
                    .as_ref()
                    .is_some_and(|c| runtime.get("native_context") == Some(c))
            {
                runtime.remove("native_context");
                repeated_context = true;
            }
        }
    }
    if repeated_targets {
        omitted.push("/devices/*/state/targets (see /targets)");
    }
    if transport {
        omitted.push("/devices/*/adapter_transport");
    }
    if recent {
        omitted.push("/devices/*/state/recent_operations");
    }
    if repeated_status {
        omitted.push("/devices/*/state/service_process_status (exact duplicate of runtime_status)");
    }
    if repeated_context {
        omitted.push("/devices/*/state/runtime_status/native_context (exact duplicate of state/native_context)");
    }

    let mut prose = false;
    if let Some(targets) = data.get_mut("targets").and_then(Value::as_array_mut) {
        for target in targets {
            let Some(caps) = target.get_mut("capabilities") else {
                continue;
            };
            // No inference from nickname/platform or unknown future adapters.
            if caps.get("adapter").and_then(Value::as_str) != Some("rust-macos-native") {
                continue;
            }
            prose |= native_capability_prose(caps);
        }
    }
    if prose {
        omitted.push("/targets/*/capabilities documented native prose (raw in computer.result)");
    }
}
