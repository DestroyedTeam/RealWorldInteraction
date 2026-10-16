//! Explicit owned-page input: one retained observation and durable no-replay request.
use super::*;
use rwi_core::device::browser::ActArgs;

const MAX_AGE: Duration = Duration::from_secs(30);
pub(super) struct Grounding {
    pub reference: String,
    pub document: Value,
    pub viewport: Value,
    pub legacy: Value,
    pub expires: Instant,
    pub elements: BTreeMap<String, u64>,
}
impl Grounding {
    pub(super) fn retain(r: &mut DeviceResult) -> Option<Self> {
        let document = r.data["document"].clone();
        if document["backend_document_node"].as_u64()? == 0
            || ["frame_id", "loader_id", "url"].iter().any(|k| {
                !document[k]
                    .as_str()
                    .is_some_and(|s| !s.is_empty() && s.len() <= 8192)
            })
        {
            return None;
        }
        let viewport = r.data["viewport_css"].clone();
        let legacy = r.data["viewport_legacy"].clone();
        if !viewport.is_object() || !legacy.is_object() {
            return None;
        }
        let reference = r.data["observation_ref"].as_str()?.to_owned();
        let nodes = r.data["nodes"].as_array_mut()?;
        if nodes.len() > 512 {
            return None;
        }
        let mut elements = BTreeMap::new();
        for (i, node) in nodes.iter_mut().enumerate() {
            if let Some(id) = node["backend_node_id"].as_u64().filter(|id| *id > 0)
                && node["bounds_css_document"].as_array().is_some_and(|r| {
                    r.len() == 4
                        && r.iter().all(|n| n.as_f64().is_some_and(f64::is_finite))
                        && r[2].as_f64().unwrap() > 0.
                        && r[3].as_f64().unwrap() > 0.
                })
            {
                let token = format!("e{}", i + 1);
                elements.insert(token.clone(), id);
                node["element_ref"] = json!(token);
            }
        }
        r.data["content_input_reference_max_age_ms"] = json!(MAX_AGE.as_millis());
        Some(Self {
            reference,
            document,
            viewport,
            legacy,
            expires: Instant::now() + MAX_AGE,
            elements,
        })
    }
}
impl BrowserRuntime {
    fn input_allowed(&self, principal: &Principal) -> bool {
        self.policy
            .as_ref()
            .ok()
            .and_then(|p| p.grant(principal))
            .is_some_and(|g| g.input && g.snapshot && g.observe)
    }
    pub fn input_capability(&self, principal: &Principal) -> Capability {
        let authorized = self.input_allowed(principal);
        let ready = self.ledger.lock().map(|l| l.is_ok()).unwrap_or(false);
        let implemented = cfg!(target_os = "macos");
        Capability {
            id: "browser.act".into(),
            implemented,
            authorized,
            available: implemented && authorized && ready,
            reason: if !implemented {
                Some("browser_platform_unsupported".into())
            } else if !authorized {
                Some("browser_input_permission_denied".into())
            } else if !ready {
                Some("browser_journal_unavailable".into())
            } else {
                None
            },
        }
    }
    pub async fn act(self: Arc<Self>, principal: &Principal, args: ActArgs) -> DeviceResult {
        if let Err(e) = args.validate() {
            return error("invalid_arguments", e);
        }
        if args.device_id != self.device {
            return error("browser_device_mismatch", "No page input was dispatched.");
        }
        if !self.input_allowed(principal) {
            return error(
                "browser_input_permission_denied",
                "Separate input, snapshot and query grants are required; Native/lifecycle grants never authorize page input.",
            );
        }
        let permit = match self.workers.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                return error(
                    "browser_busy_not_dispatched",
                    "No request reserved or input sent; close/query capacity is separate.",
                );
            }
        };
        let owner = principal.certificate.as_str().to_owned();
        match tokio::task::spawn_blocking(move || {
            let _permit = permit;
            self.dispatch_input(&owner, args)
        })
        .await
        {
            Ok(r) => r,
            Err(e) => error(
                "browser_result_unknown",
                format!("{e}; query the ORIGINAL browser.session request_id. Never replay input."),
            ),
        }
    }
    fn dispatch_input(&self, owner: &str, args: ActArgs) -> DeviceResult {
        let payload = serde_json::to_value(&args).expect("typed page action");
        let session = self
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&args.session_ref)
            .filter(|s| s.owner == owner)
            .cloned();
        let nonce = session
            .as_ref()
            .map(|s| s.nonce.clone())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        // Existing durable requests are returned even after close/restart. No page I/O
        // or resource lookup can turn an uncertain prior reservation into a new input.
        let key = match self.reserve(owner, &args.request_id, &payload, &nonce) {
            Ok(Reservation::New(k)) => k,
            Ok(Reservation::Existing(r)) => return r,
            Err(e) => return e,
        };
        let result = (|| {
            let Some(s) = session else {
                return error(
                    "browser_session_unknown",
                    "No other session or principal is searched.",
                );
            };
            let Ok(mut state) = s.inner.try_lock() else {
                return error(
                    "browser_session_busy_not_dispatched",
                    "Only this page is busy.",
                );
            };
            if !state.valid || Instant::now() >= s.expires {
                return error(
                    "browser_session_closed_or_expired",
                    "No page input was sent.",
                );
            }
            let Some(observation) = state.observation.as_ref().filter(|o| {
                o.reference == args.observation_ref
                    && Instant::now() < o.expires
                    && o.document.is_object()
                    && o.viewport.is_object()
                    && o.legacy.is_object()
            }) else {
                return error(
                    "browser_observation_unknown_or_expired",
                    "Take a new snapshot; no input or Native fallback was sent.",
                );
            };
            let Some(id) = observation.elements.get(&args.element_ref).copied() else {
                return error(
                    "browser_element_unknown",
                    "Only this snapshot's element refs are accepted.",
                );
            };
            let observation = state.observation.take().expect("checked exact observation");
            let Some(driver) = state.driver.as_mut() else {
                return error(
                    "browser_session_not_ready",
                    "No page input driver is ready.",
                );
            };
            // Consume before calling the driver, including preflight failure or lost ACK.
            // A later action needs a new observation, never a retry of this intent.
            driver.act(&args, &observation, id)
        })();
        let mut result = result;
        result.data["device_id"] = json!(self.device);
        result.data["agent_instance"] = json!(self.instance);
        result.data["session_ref"] = json!(args.session_ref);
        result.data["request_id"] = json!(args.request_id);
        result.data["family"] = json!("browser-content");
        result.data["native_input"] = json!(false);
        result.data["activation_requested"] = json!(false);
        result.data["automatic_replay"] = json!(false);
        result.data["goal_verified"] = json!(false);
        self.finish(&key, result)
    }
}
