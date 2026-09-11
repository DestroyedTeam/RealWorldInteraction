//! Typed device routing: replay keys never alias monitor handles.
use super::*;
impl Gateway {
    pub(super) fn remember(&self, id: &str, value: &Value) {
        fn visit(map: &mut BTreeMap<String, BTreeSet<String>>, id: &str, v: &Value) {
            match v {
                Value::Object(o) => {
                    for (k, v) in o {
                        if matches!(
                            k.as_str(),
                            "target"
                                | "frame"
                                | "artifact_id"
                                | "lease"
                                | "view_id"
                                | "operation_id"
                                | "request_id"
                                | "monitor_id"
                        ) && let Some(s) = v.as_str()
                        {
                            // Persisted v1 reservations call the replay key
                            // operation_id, but expose a separate monitor_id.
                            let kind = if k == "monitor_id" {
                                "operation_id"
                            } else if k == "operation_id" && o.contains_key("monitor_id") {
                                "request_id"
                            } else {
                                k.as_str()
                            };
                            map.entry(format!("{kind}:{s}"))
                                .or_default()
                                .insert(id.into());
                        }
                        visit(map, id, v)
                    }
                }
                Value::Array(a) => {
                    for v in a {
                        visit(map, id, v)
                    }
                }
                _ => {}
            }
        }
        if let Ok(mut handles) = self.handles.lock() {
            if handles.len() > 8192 {
                handles.retain(|_, devices| {
                    devices.remove(id);
                    !devices.is_empty()
                });
            }
            visit(&mut handles, id, value);
            while handles.len() > 8192 {
                handles.pop_first();
            }
        }
    }
    pub(super) fn route_id(&self, tool: &str, args: &Value) -> Result<String, DeviceResult> {
        let map = self
            .handles
            .lock()
            .map_err(|_| DeviceResult::error("handle_registry_unavailable", "Re-query targets."))?;
        let mut candidates = args["device_id"]
            .as_str()
            .map(|id| BTreeSet::from([id.to_owned()]));
        for kind in [
            "target",
            "artifact_id",
            "lease",
            "view_id",
            "frame",
            "operation_id",
        ] {
            if kind == "operation_id" && !matches!(tool, "computer.operation" | "computer.cancel") {
                continue;
            }
            if let Some(value) = args[kind].as_str() {
                let kinds: &[&str] = if kind == "operation_id" && tool == "computer.operation" {
                    match args["id_kind"].as_str().unwrap_or("auto") {
                        "monitor" => &["operation_id"],
                        "request" => &["request_id"],
                        _ => &["operation_id", "request_id"],
                    }
                } else {
                    &[kind]
                };
                let devices: BTreeSet<_> = kinds
                    .iter()
                    .filter_map(|k| map.get(&format!("{k}:{value}")))
                    .flatten()
                    .cloned()
                    .collect();
                if devices.is_empty() {
                    continue;
                }
                candidates = Some(match candidates {
                    Some(set) => set.intersection(&devices).cloned().collect(),
                    None => devices,
                });
            }
        }
        let Some(set) = candidates else {
            return Err(DeviceResult::error(
                "resource_unknown",
                "Use a returned handle, or explicit device_id for durable operation queries.",
            ));
        };
        match set.len() {
            1 => Ok(set.into_iter().next().unwrap()),
            0 => Err(DeviceResult::error(
                "foreign_resource",
                "Handles belong to different devices.",
            )),
            _ => Err(DeviceResult::error(
                "ambiguous_resource",
                "This handle exists on multiple devices. Supply device_id; no device was selected.",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn request_and_monitor_handles_route_in_distinct_namespaces() {
        let gateway = Gateway::new(Ok(AdapterProfile::default()));
        gateway.remember(
            "a",
            &json!({"operation_id":"shared","request_id":"request-a"}),
        );
        gateway.remember(
            "b",
            &json!({"operation_id":"monitor-b","request_id":"shared"}),
        );
        let route = |kind: &str| {
            gateway.route_id(
                "computer.operation",
                &json!({"operation_id":"shared","id_kind":kind}),
            )
        };
        assert_eq!(
            route("auto").unwrap_err().data["code"],
            "ambiguous_resource"
        );
        assert_eq!(route("monitor").unwrap(), "a");
        assert_eq!(route("request").unwrap(), "b");
        assert_eq!(
            gateway
                .route_id("computer.cancel", &json!({"operation_id":"shared"}))
                .unwrap(),
            "a"
        );
        // A legacy reservation stores the replay key under operation_id;
        // observing its history must not poison the monitor routing table.
        gateway.remember("c", &json!({"reservation":{"operation_id":"request-c","monitor_id":"monitor-c","payload_sha":"hash"}}));
        assert_eq!(
            gateway
                .route_id(
                    "computer.operation",
                    &json!({"operation_id":"request-c","id_kind":"monitor"})
                )
                .unwrap_err()
                .data["code"],
            "resource_unknown"
        );
        assert_eq!(
            gateway
                .route_id(
                    "computer.operation",
                    &json!({"operation_id":"request-c","id_kind":"request"})
                )
                .unwrap(),
            "c"
        );
    }
}
