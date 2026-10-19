//! Negotiated Host-only Live registration. No HTTP listener or display fallback.
use super::*;

pub(in crate::gateway) fn reserved_arguments(args: &Value) -> bool {
    args.pointer("/options/host_image_only").is_some()
        || args.get("view").is_some_and(|value| !value.is_null())
        || args
            .pointer("/options/host_view_id")
            .is_some_and(|value| !value.is_null())
}

impl Views {
    fn register_component(
        &self,
        target: &str,
        mut options: rwi_core::LiveOptions,
    ) -> Result<(String, rwi_core::LiveOptions), DeviceResult> {
        if options.display.is_some() || options.host_view_id.is_some() || options.host_image_only {
            return Err(err(
                "invalid_live_presentation",
                "Conversation-only registration requires no display or caller-owned Host fields.",
            ));
        }
        let binding = token();
        options.host_image_only = true;
        options.host_view_id = Some(binding.clone());
        // Validate all bounds before mutating registry state or doing Native I/O.
        if let Err(error) = options.validate() {
            return Err(err(&error.code, &error.next_step));
        }
        let mut views = self
            .entries
            .lock()
            .map_err(|_| err("live_view_unavailable", "View registry unavailable."))?;
        views.retain(|_, view| view.expires > Instant::now());
        if views.values().any(|view| view.target == target) {
            return Err(err(
                "live_presentation_restart_required",
                "Stop the existing target preview before starting a conversation-only presentation; no binding or lifetime was changed.",
            ));
        }
        if views.len() >= MAX_VIEWS {
            return Err(err(
                "live_view_capacity",
                "Close an existing preview before starting another.",
            ));
        }
        views.insert(
            token(),
            View {
                target: target.into(),
                binding: binding.clone(),
                page: None,
                ui_sequence: 0,
                device_sequence: 0,
                expires: Instant::now() + Duration::from_millis(options.duration_ms),
                image: image::State::default(),
                dom: dom_observation::Log::default(),
            },
        );
        Ok((binding, options))
    }

    fn remove_component(&self, target: &str, binding: &str) {
        if let Ok(mut views) = self.entries.lock() {
            views.retain(|_, view| view.target != target || view.binding != binding);
        }
    }
}

impl Gateway {
    pub(in crate::gateway) async fn host_component_start(
        &self,
        target: &str,
        options: rwi_core::LiveOptions,
        context: RequestContext<RoleServer>,
    ) -> DeviceResult {
        if !live_ui::negotiated(&context) {
            return err(
                "live_ui_not_negotiated",
                "No conversation-only Live was started.",
            );
        }
        let (binding, options) = match self.presence.register_component(target, options) {
            Ok(registered) => registered,
            Err(error) => return error,
        };
        // This trusted adapter attaches its own binding. The public tool path
        // still rejects private fields; device validation/authorization is intact.
        let mut result = self
            .invoke_device(
                "computer.live",
                json!({"target":target,"action":"start","options":options}),
                Some(context),
            )
            .await;
        if result.is_error {
            self.presence.remove_component(target, &binding);
            return result;
        }
        if result.data["host_view_binding"] != binding {
            self.presence.remove_component(target, &binding);
            let start = result.data;
            result = err(
                "live_view_binding_unconfirmed",
                "Native start did not confirm this new Host binding. Inspect the retained start result; do not replay start or infer rendered pixels.",
            );
            result.data["live_start_result"] = start;
            return result;
        }
        if let Some(ms) = result.data["expires_in_ms"].as_u64()
            && let Ok(mut views) = self.presence.entries.lock()
            && let Some(view) = views
                .values_mut()
                .find(|view| view.target == target && view.binding == binding)
        {
            view.expires = Instant::now() + Duration::from_millis(ms.min(600_000));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> rwi_core::LiveOptions {
        serde_json::from_value(json!({})).unwrap()
    }

    #[test]
    fn caller_host_fields_are_rejected_before_false_can_be_erased() {
        for value in [
            json!({"options":{"host_image_only":false}}),
            json!({"options":{"host_image_only":true}}),
            json!({"options":{"host_image_only":null}}),
            json!({"options":{"host_view_id":"a".repeat(32)}}),
            json!({"view":{}}),
        ] {
            assert!(reserved_arguments(&value));
        }
        assert!(!reserved_arguments(&json!({"options":{}})));
        assert!(!reserved_arguments(
            &json!({"view":null,"options":{"host_view_id":null}})
        ));
    }

    #[tokio::test]
    async fn component_registration_is_owned_bounded_and_opens_no_http_listener() {
        let views = Views::default();
        let (binding, options) = views
            .register_component("fixture:owned", options())
            .unwrap();
        assert!(options.display.is_none());
        assert!(options.host_image_only);
        assert_eq!(options.host_view_id.as_deref(), Some(binding.as_str()));
        assert_eq!(binding.len(), 32);
        assert!(options.validate().is_ok());
        assert!(views.address.lock().await.is_none());
        let (_, _, hidden) = views.app_page("fixture:owned").unwrap();
        assert!(!hidden.visible);
        assert_eq!(hidden.view_id, binding);
    }

    #[test]
    fn invalid_component_options_never_mutate_the_registry() {
        let views = Views::default();
        for value in [
            json!({"duration_ms":0}),
            json!({"duration_ms":u64::MAX}),
            json!({"max_edge":63}),
            json!({"interval_ms":249}),
            json!({"label":""}),
            json!({"label":"\n"}),
            json!({"host_image_only":true}),
            json!({"host_view_id":"a".repeat(32)}),
        ] {
            let options = serde_json::from_value(value).unwrap();
            assert!(
                views
                    .register_component("fixture:invalid", options)
                    .is_err()
            );
            assert!(views.entries.lock().unwrap().is_empty());
        }
    }

    #[test]
    fn component_registration_never_discards_an_explicit_display() {
        let views = Views::default();
        let options = serde_json::from_value(json!({"display":{"built_in":true}})).unwrap();
        let error = views
            .register_component("fixture:display", options)
            .unwrap_err();
        assert_eq!(error.data["code"], "invalid_live_presentation");
        assert!(views.entries.lock().unwrap().is_empty());
    }

    #[test]
    fn component_registration_preserves_valid_capture_and_lifetime_bounds() {
        let views = Views::default();
        for duration in [1000, 600_000] {
            let mut requested = options();
            requested.duration_ms = duration;
            requested.max_edge = 1536;
            requested.interval_ms = 750;
            requested.label = "Owned preview".into();
            let (_, accepted) = views
                .register_component(&format!("fixture:{duration}"), requested)
                .unwrap();
            assert_eq!(accepted.duration_ms, duration);
            assert_eq!(accepted.max_edge, 1536);
            assert_eq!(accepted.interval_ms, 750);
            assert_eq!(accepted.label, "Owned preview");
        }
    }

    #[test]
    fn existing_preview_is_not_rebound_refreshed_or_replaced() {
        let views = Views::default();
        let (key, binding) = views.register("fixture:existing", 1000).unwrap();
        let (_, page, _) = views.app_page("fixture:existing").unwrap();
        views.app_event(&key, page.clone(), 1, true).unwrap();
        let expires = views.entries.lock().unwrap()[&key].expires;
        let error = views
            .register_component("fixture:existing", options())
            .unwrap_err();
        assert_eq!(error.data["code"], "live_presentation_restart_required");
        let entries = views.entries.lock().unwrap();
        let view = &entries[&key];
        assert_eq!(view.binding, binding);
        assert_eq!(view.page.as_deref(), Some(page.as_str()));
        assert_eq!(view.ui_sequence, 1);
        assert_eq!(view.expires, expires);
    }

    #[test]
    fn capacity_is_bounded_and_only_expired_views_are_pruned() {
        let views = Views::default();
        for n in 0..MAX_VIEWS {
            views
                .register_component(&format!("fixture:{n}"), options())
                .unwrap();
        }
        let error = views
            .register_component("fixture:overflow", options())
            .unwrap_err();
        assert_eq!(error.data["code"], "live_view_capacity");
        views
            .entries
            .lock()
            .unwrap()
            .values_mut()
            .next()
            .unwrap()
            .expires = Instant::now();
        assert!(views.register_component("fixture:new", options()).is_ok());
        assert_eq!(views.entries.lock().unwrap().len(), MAX_VIEWS);
    }

    #[test]
    fn failed_start_cleanup_cannot_remove_a_different_binding() {
        let views = Views::default();
        let (binding, _) = views
            .register_component("fixture:owned", options())
            .unwrap();
        views.remove_component("fixture:owned", &"0".repeat(32));
        assert_eq!(views.entries.lock().unwrap().len(), 1);
        views.remove_component("fixture:foreign", &binding);
        assert_eq!(views.entries.lock().unwrap().len(), 1);
        views.remove_component("fixture:owned", &binding);
        assert!(views.entries.lock().unwrap().is_empty());
    }
}
