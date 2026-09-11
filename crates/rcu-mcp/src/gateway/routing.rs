//! Device handle routing, separated from MCP service orchestration.
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
                        ) && let Some(s) = v.as_str()
                        {
                            map.entry(format!("{k}:{s}")).or_default().insert(id.into());
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
            if let Some(value) = args[kind].as_str()
                && let Some(devices) = map.get(&format!("{kind}:{value}"))
            {
                candidates = Some(match candidates {
                    Some(set) => set.intersection(devices).cloned().collect(),
                    None => devices.clone(),
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
