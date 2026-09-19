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
                            "target" | "frame" | "artifact_id" | "lease" | "view_id"
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
        // Routing fields are hints; the agent validates the complete authenticated reference.
        if tool == "computer.operation" {
            let query: device::api::OperationQueryArgs = serde_json::from_value(args.clone())
                .map_err(|e| DeviceResult::error("invalid_arguments", &e.to_string()))?;
            return Ok(match query.selector {
                device::OperationSelector::Execution { operation_ref } => operation_ref.device_id,
                device::OperationSelector::Request { device_id, .. } => device_id,
            });
        }
        if tool == "computer.cancel" {
            let query: device::api::OperationArgs = serde_json::from_value(args.clone())
                .map_err(|e| DeviceResult::error("invalid_arguments", &e.to_string()))?;
            return Ok(query.operation_ref.device_id);
        }
        let map = self
            .handles
            .lock()
            .map_err(|_| DeviceResult::error("handle_registry_unavailable", "Re-query targets."))?;
        let mut candidates = args["device_id"]
            .as_str()
            .map(|id| BTreeSet::from([id.to_owned()]));
        for kind in ["target", "artifact_id", "lease", "view_id", "frame"] {
            if let Some(value) = args[kind].as_str() {
                let devices = map
                    .get(&format!("{kind}:{value}"))
                    .cloned()
                    .unwrap_or_default();
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
    fn plugin_gateway_rejects_cross_device_input_handles_and_explicit_overrides() {
        let g = Gateway::new(Ok(AdapterProfile::default()));
        g.remember(
            "mac",
            &json!({"target":"mac-window","frame":"mac-frame","lease":"mac-lease"}),
        );
        g.remember(
            "windows",
            &json!({"target":"win-window","frame":"win-frame","lease":"win-lease"}),
        );
        for args in [
            json!({"target":"mac-window","frame":"win-frame","lease":"mac-lease"}),
            json!({"target":"win-window","lease":"mac-lease"}),
            json!({"device_id":"windows","target":"mac-window"}),
        ] {
            assert_eq!(
                g.route_id("computer.act", &args).unwrap_err().data["code"],
                "foreign_resource"
            );
        }
        assert_eq!(
            g.route_id(
                "computer.act",
                &json!({"target":"mac-window","frame":"mac-frame","lease":"mac-lease"})
            )
            .unwrap(),
            "mac"
        );
    }

    #[test]
    fn plugin_gateway_requires_disambiguation_for_colliding_device_artifacts() {
        let g = Gateway::new(Ok(AdapterProfile::default()));
        for id in ["mac-one", "mac-two"] {
            g.remember(id, &json!({"artifact_id":"same-local-id"}));
        }
        assert_eq!(
            g.route_id("computer.image", &json!({"artifact_id":"same-local-id"}))
                .unwrap_err()
                .data["code"],
            "ambiguous_resource"
        );
        assert_eq!(
            g.route_id(
                "computer.image",
                &json!({"device_id":"mac-two","artifact_id":"same-local-id"})
            )
            .unwrap(),
            "mac-two"
        );
        assert_eq!(
            g.route_id(
                "computer.image",
                &json!({"device_id":"windows","artifact_id":"same-local-id"})
            )
            .unwrap_err()
            .data["code"],
            "foreign_resource"
        );
    }

    #[test]
    fn fresh_gateway_routes_only_explicit_execution_or_request_selectors() {
        let g = Gateway::new(Ok(AdapterProfile::default()));
        let reference = json!({"ref_version":1,"device_id":"a","agent_instance":"instance","context_generation":"context","execution_nonce":"nonce"});
        g.remember(
            "wrong",
            &json!({"operation_id":"nonce","request_id":"request","monitor_id":"nonce"}),
        );
        assert!(g.handles.lock().unwrap().is_empty());
        assert_eq!(
            g.route_id(
                "computer.operation",
                &json!({"selector":{"kind":"execution","operation_ref":reference}})
            )
            .unwrap(),
            "a"
        );
        assert_eq!(
            g.route_id("computer.cancel", &json!({"operation_ref":reference}))
                .unwrap(),
            "a"
        );
        assert_eq!(
            g.route_id(
                "computer.operation",
                &json!({"selector":{"kind":"request","device_id":"b","request_id":"request"}})
            )
            .unwrap(),
            "b"
        );
        assert!(
            g.route_id("computer.cancel", &json!({"operation_id":"nonce"}))
                .is_err()
        );
    }
}
