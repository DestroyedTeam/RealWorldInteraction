mod capabilities;
mod recovery;
use crate::{
    config::{Config, Principal},
    executor::Executor,
    ledger::{Ledger, Reservation},
    platform,
};
use rcu_core::{
    device::*,
    operation::{Operation, Registry},
};
use rcu_link_trust::PeerPin;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

struct InputDomain {
    key: String,
    gate: Arc<tokio::sync::Semaphore>,
    fenced: AtomicBool,
}
struct Context {
    domain: Arc<InputDomain>,
    closed: AtomicBool,
    cancelled_ids: Mutex<BTreeSet<String>>,
    principal: String,
    executor: Arc<Mutex<Executor>>,
    gate: Arc<tokio::sync::Semaphore>,
    registry: Arc<Registry>,
    touched: Mutex<Instant>,
    target: String,
    entered: Arc<AtomicBool>,
}
pub struct Runtime {
    pub config: Config,
    pub instance: String,
    contexts: Mutex<BTreeMap<String, Arc<Context>>>,
    ledger: Mutex<io::Result<Ledger>>,
    domains: Mutex<BTreeMap<String, Arc<InputDomain>>>,
    _owner: std::fs::File,
}
impl Runtime {
    pub fn new(config: Config) -> io::Result<Arc<Self>> {
        config.validate()?;
        std::fs::create_dir_all(&config.state_root)?;
        let owner = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(config.state_root.join("service.lock"))?;
        owner
            .try_lock()
            .map_err(|_| io::Error::other("service_state_already_owned"))?;
        let ledger = Ledger::open(config.state_root.join("operations"));
        Ok(Arc::new(Self {
            config,
            instance: uuid::Uuid::new_v4().to_string(),
            contexts: Mutex::new(BTreeMap::new()),
            ledger: Mutex::new(ledger),
            domains: Mutex::new(BTreeMap::new()),
            _owner: owner,
        }))
    }
    fn context(&self, peer: &PeerPin, token: Option<&str>) -> Option<Arc<Context>> {
        let contexts = self.contexts.lock().ok()?;
        let context = contexts.get(token?)?;
        if context.principal != peer.as_str() || context.closed.load(Ordering::Acquire) {
            return None;
        }
        *context.touched.lock().ok()? = Instant::now();
        Some(context.clone())
    }
    pub async fn handle(self: Arc<Self>, peer: PeerPin, request: Request) -> Reply {
        let result = if request.version != PROTOCOL_VERSION
            || request.device != self.config.device_id
            || !valid_id(&request.id)
        {
            DeviceResult::error(
                "request_binding",
                "Wrong protocol version, device or request ID.",
            )
        } else if let Some(policy) = self
            .config
            .principals
            .iter()
            .find(|p| p.certificate == peer)
        {
            self.call(&peer, policy, request.context.as_deref(), request.call)
                .await
        } else {
            DeviceResult::error("principal_denied", "Peer has no device policy.")
        };
        Reply {
            version: PROTOCOL_VERSION,
            id: request.id,
            device: self.config.device_id.clone(),
            instance: self.instance.clone(),
            result,
        }
    }
    async fn call(
        self: &Arc<Self>,
        peer: &PeerPin,
        policy: &Principal,
        token: Option<&str>,
        call: Call,
    ) -> DeviceResult {
        match call {
            Call::Describe => {
                let mut r = self.describe(policy);
                r.data["context_valid"] =
                    json!(token.is_none() || self.context(peer, token).is_some());
                r
            }
            Call::RecoverInput { resource_domain } => self.recover(policy, &resource_domain).await,
            Call::OpenContext => {
                if !policy.observe {
                    return DeviceResult::error(
                        "permission_denied",
                        "No observation policy for this principal.",
                    );
                }
                let token = uuid::Uuid::new_v4().to_string();
                let backend = match platform::backend(&self.config, policy, &token) {
                    Ok(b) => b,
                    Err(e) => return DeviceResult::error(&e.code, &e.next_step),
                };
                let domain = match self.domain(&backend.identity().key()) {
                    Ok(d) => d,
                    Err(e) => return e,
                };
                let mut engine = match rcu_core::Engine::with_lease_root(
                    self.config
                        .state_root
                        .join("principals")
                        .join(rcu_core::digest(peer.as_str().as_bytes())),
                    self.config.state_root.join("shared-native"),
                ) {
                    Ok(e) => e,
                    Err(e) => return DeviceResult::error(&e.code, &e.next_step),
                };
                let target = format!("{}:{}:{}", self.config.device_id, self.instance, token);
                let entered = Arc::new(AtomicBool::new(false));
                if let Err(e) = engine.register(
                    target.clone(),
                    Box::new(crate::observed_backend::Observed {
                        inner: backend,
                        entered: entered.clone(),
                    }),
                ) {
                    return DeviceResult::error(&e.code, &e.next_step);
                }
                let targets = engine.targets();
                let mut contexts = match self.contexts.lock() {
                    Ok(c) => c,
                    Err(_) => {
                        return DeviceResult::error(
                            "context_registry_error",
                            "Read-only service metadata remains available.",
                        );
                    }
                };
                contexts.retain(|_, c| {
                    c.gate.available_permits() == 0
                        || c.touched
                            .lock()
                            .map(|t| t.elapsed().as_secs() < 600)
                            .unwrap_or(true)
                });
                if contexts.len() >= 16 {
                    return DeviceResult::error(
                        "context_capacity",
                        "Close idle contexts; metadata remains available.",
                    );
                }
                contexts.insert(
                    token.clone(),
                    Arc::new(Context {
                        domain,
                        closed: AtomicBool::new(false),
                        cancelled_ids: Mutex::new(BTreeSet::new()),
                        principal: peer.as_str().into(),
                        executor: Arc::new(Mutex::new(Executor::new(engine))),
                        gate: Arc::new(tokio::sync::Semaphore::new(1)),
                        registry: Arc::new(Registry::default()),
                        touched: Mutex::new(Instant::now()),
                        target,
                        entered,
                    }),
                );
                DeviceResult::value(
                    json!({"context":token,"targets":targets,"instance":self.instance,"service":"healthy"}),
                )
            }
            Call::CloseContext => {
                let Some(context) = self.context(peer, token) else {
                    return DeviceResult::error(
                        "context_unknown",
                        "No matching authenticated context.",
                    );
                };
                let Ok(_idle) = context.gate.clone().try_acquire_owned() else {
                    return DeviceResult::error(
                        "context_busy",
                        "Pending native resources remain owned; query/cancel the operation.",
                    );
                };
                context.closed.store(true, Ordering::Release);
                if let Ok(mut contexts) = self.contexts.lock() {
                    contexts.remove(token.unwrap());
                }
                DeviceResult::value(json!({"closed":true,"business_goal_inferred":false}))
            }
            Call::Cancel { operation_id } => self.operation(peer, token, &operation_id, true),
            Call::Operation { operation_id } => {
                // Operation/cancellation requests are served separately below by
                // the common tool route; this path never acquires an Engine lock.
                self.operation(peer, token, &operation_id, false)
            }
            Call::Invoke { tool, arguments } => {
                if tool == "computer.operation" || tool == "computer.cancel" {
                    return self.operation(
                        peer,
                        token,
                        arguments["operation_id"].as_str().unwrap_or_default(),
                        tool == "computer.cancel",
                    );
                }
                if let Err(e) = catalog::validate(&tool, &arguments) {
                    return DeviceResult::error("invalid_arguments", &e.to_string());
                }
                let Some(context) = self.context(peer, token) else {
                    return DeviceResult::error(
                        "context_unknown",
                        "Open a fresh authenticated context; never replay old actions.",
                    );
                };
                if arguments
                    .get("target")
                    .is_some_and(|t| t != &context.target)
                {
                    return DeviceResult::error(
                        "foreign_target",
                        "Use the target returned by this device and client context.",
                    );
                }
                let writes = catalog::definitions()
                    .iter()
                    .any(|t| t.name == tool && t.writes);
                if !policy.observe || (writes || tool == "computer.claim") && !policy.input {
                    return DeviceResult::error(
                        "permission_denied",
                        "No capability grant for this principal.",
                    );
                }
                let Ok(permit) = context.gate.clone().try_acquire_owned() else {
                    return DeviceResult::error(
                        "context_busy",
                        "Only this context is busy. Device metadata and other contexts remain available.",
                    );
                };
                let input_permit = if writes {
                    if context.domain.fenced.load(Ordering::Acquire) {
                        return DeviceResult::error(
                            "input_domain_fenced",
                            "Inspect uncertain native resources; reads and unrelated devices remain available.",
                        );
                    }
                    match context.domain.gate.clone().try_acquire_owned() {
                        Ok(p) => {
                            if context.domain.fenced.load(Ordering::Acquire) {
                                return DeviceResult::error(
                                    "input_domain_fenced",
                                    "The previous writer left uncertain native resources; reconcile before a new action.",
                                );
                            }
                            Some(p)
                        }
                        Err(_) => {
                            return DeviceResult::error(
                                "input_busy",
                                "This desktop has an active writer; do not steal focus.",
                            );
                        }
                    }
                } else {
                    None
                };
                let mutation_id = arguments["request_id"].as_str().map(str::to_owned);
                let monitor_id = arguments["operation_id"]
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| mutation_id.clone())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                if !valid_id(&monitor_id) {
                    return DeviceResult::error(
                        "invalid_operation_id",
                        "Use a bounded operation identifier.",
                    );
                }
                if context.closed.load(Ordering::Acquire)
                    || context
                        .cancelled_ids
                        .lock()
                        .map(|set| set.contains(&monitor_id))
                        .unwrap_or(true)
                {
                    return DeviceResult::error(
                        "operation_cancelled_before_dispatch",
                        "Context closed or this operation was cancelled before registration.",
                    );
                }
                let mut reservation = None;
                if writes {
                    let Some(id) = &mutation_id else {
                        return DeviceResult::error(
                            "operation_id_required",
                            "Mutations require a stable request_id.",
                        );
                    };
                    if !valid_id(id) {
                        return DeviceResult::error(
                            "invalid_operation_id",
                            "Use a bounded stable operation identifier.",
                        );
                    }
                    let mut storage = match self.ledger.lock() {
                        Ok(l) => l,
                        Err(_) => {
                            return DeviceResult::error(
                                "operation_storage_unavailable",
                                "No dispatch without reservation.",
                            );
                        }
                    };
                    let ledger = match storage.as_mut() {
                        Ok(l) => l,
                        Err(e) => {
                            return DeviceResult::error(
                                "operation_storage_unavailable",
                                &e.to_string(),
                            );
                        }
                    };
                    match ledger.reserve(
                        peer.as_str(),
                        id,
                        &json!({"tool":tool,"arguments":arguments}),
                        &monitor_id,
                        &context.domain.key,
                    ) {
                        Ok(Reservation::New(key)) => {
                            reservation = Some(key);
                        }
                        Ok(Reservation::Existing(r)) => return r,
                        Err(e) => {
                            return DeviceResult::error(
                                "operation_reservation_failed",
                                &e.to_string(),
                            );
                        }
                    }
                }
                let op = match context.registry.begin(Some(monitor_id.clone())) {
                    Ok(o) => o,
                    Err(e) => {
                        let result = DeviceResult::error(&e.code, &e.next_step);
                        if let Some(key) = &reservation
                            && let Ok(storage) = self.ledger.lock()
                            && let Ok(ledger) = storage.as_ref()
                        {
                            let _ = ledger.finish(key, &result);
                        }
                        return result;
                    }
                };
                if context
                    .cancelled_ids
                    .lock()
                    .map(|set| set.contains(&monitor_id))
                    .unwrap_or(true)
                {
                    op.cancel();
                }
                let domain = context.domain.clone();
                let executor = context.executor.clone();
                let worker_op = op.clone();
                let service = self.clone();
                let entered = context.entered.clone();
                entered.store(false, Ordering::Release);
                let result = tokio::task::spawn_blocking(move || {
                    struct Finish(Operation);
                    impl Drop for Finish {
                        fn drop(&mut self) {
                            self.0.finish();
                        }
                    }
                    let _finish = Finish(worker_op.clone());
                    let _permit = permit;
                    let _input = input_permit;
                    let result = rcu_core::operation::run(&worker_op, || {
                        executor
                            .lock()
                            .map_err(|_| {
                                rcu_core::fault(
                                    "executor_unavailable",
                                    "Native context failed; other contexts remain available.",
                                )
                            })?
                            .invoke(&tool, arguments)
                    });
                    let mut result = match result {
                        Ok(r) => r,
                        Err(e) => DeviceResult::error(&e.code, &e.next_step),
                    };
                    drop(_finish);
                    result.data["operation_id"] = json!(worker_op.status().operation_id);
                    result.data["operation"] = json!(worker_op.status());
                    result.data["native_dispatch_entered"] = json!(entered.load(Ordering::Acquire));
                    if writes
                        && entered.load(Ordering::Acquire)
                        && (result.is_error
                            || result.data["dispatch"] == "uncertain"
                            || result.data["state"] == "uncertain"
                            || !worker_op.status().resources_released)
                    {
                        domain.fenced.store(true, Ordering::Release);
                    }
                    result.data["input_domain_fenced"] =
                        json!(domain.fenced.load(Ordering::Acquire));
                    if let Some(key) = reservation {
                        let saved = service
                            .ledger
                            .lock()
                            .ok()
                            .and_then(|l| {
                                l.as_ref()
                                    .ok()
                                    .map(|ledger| ledger.finish(&key, &result).is_ok())
                            })
                            .unwrap_or(false);
                        if !saved {
                            domain.fenced.store(true, Ordering::Release);
                            return DeviceResult::error(
                                "effect_recording_uncertain",
                                "Effect may have occurred; query/reconcile without replay.",
                            );
                        }
                    }
                    result
                })
                .await;
                let mut result = match result {
                    Ok(r) => r,
                    Err(_) => {
                        if writes {
                            context.domain.fenced.store(true, Ordering::Release);
                        }
                        DeviceResult::error(
                            "native_worker_failed",
                            "Effect may be unknown; inspect operation and target without replay.",
                        )
                    }
                };
                result.data["operation_id"] = json!(op.status().operation_id);
                result.data["operation"] = json!(op.status());
                result
            }
        }
    }
    fn operation(
        &self,
        peer: &PeerPin,
        token: Option<&str>,
        id: &str,
        cancel: bool,
    ) -> DeviceResult {
        if let Some(context) = self.context(peer, token) {
            if let Ok(status) = context.registry.get(id, cancel) {
                return DeviceResult::value(status);
            }
            if cancel
                && valid_id(id)
                && let Ok(mut ids) = context.cancelled_ids.lock()
                && (ids.len() < 256 || ids.contains(id))
            {
                ids.insert(id.into());
                return DeviceResult::value(
                    json!({"operation_id":id,"cancel_requested":true,"state":"pending_registration","terminal":false,"resources_released":false}),
                );
            }
        }
        if !cancel
            && let Ok(storage) = self.ledger.lock()
            && let Ok(ledger) = storage.as_ref()
            && let Some(status) = ledger.status(peer.as_str(), id)
        {
            return DeviceResult::value(status);
        }
        DeviceResult::error(
            "operation_unknown",
            "No operation with this ID belongs to the authenticated principal/context.",
        )
    }
}
