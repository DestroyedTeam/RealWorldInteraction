mod capabilities;
mod dispatch;
#[cfg(all(test, feature = "fixture"))]
mod dispatch_tests;
mod execution;
mod namespace;
#[cfg(all(test, feature = "fixture"))]
mod visual_tests;
pub use namespace::NamespaceRepair;
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
    collections::BTreeMap,
    io,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
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
    generation: String,
    executions: Mutex<BTreeMap<String, Arc<execution::Execution>>>,
    sequence: AtomicU64,
    principal: String,
    executor: Arc<Mutex<Executor>>,
    gate: Arc<tokio::sync::Semaphore>,
    registry: Arc<Registry>,
    touched: Mutex<Instant>,
    target: String,
    entered: Arc<AtomicBool>,
}
/// Orderly service ownership ends here, not when an incidental duplicate OS
/// descriptor (e.g. during concurrent process creation) is eventually closed.
/// This field is dropped last, after all contexts and durable state handles.
struct StateOwner(std::fs::File);
impl Drop for StateOwner {
    fn drop(&mut self) {
        if let Err(error) = self.0.unlock() {
            eprintln!("rcu-agent: state owner unlock unconfirmed: {error}");
        }
    }
}
pub struct Runtime {
    pub config: Config,
    pub instance: String,
    contexts: Mutex<BTreeMap<String, Arc<Context>>>,
    ledger: Mutex<io::Result<Ledger>>,
    domains: Mutex<BTreeMap<String, Arc<InputDomain>>>,
    _owner: StateOwner,
}
impl Runtime {
    /// Explicit offline operator repair. No RPC/native endpoint exposes this.
    pub fn repair_namespace(
        config: &Config,
        proof: &NamespaceRepair,
        backup: &[u8],
    ) -> io::Result<serde_json::Value> {
        config.validate()?;
        let owner = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(config.state_root.join("service.lock"))?;
        owner
            .try_lock()
            .map_err(|_| io::Error::other("service_state_already_owned"))?;
        let _owner = StateOwner(owner);
        namespace::repair(&config.state_root, &config.device_id, proof, backup)
    }
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
        let owner = StateOwner(owner);
        namespace::bind(&config.state_root, &config.device_id)?;
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
                    if c.touched
                        .lock()
                        .map(|t| t.elapsed().as_secs() < 600)
                        .unwrap_or(true)
                    {
                        return true;
                    }
                    let Ok(_idle) = c.gate.clone().try_acquire_owned() else {
                        return true;
                    };
                    if !c.native_quiescent() {
                        return true;
                    }
                    c.closed.store(true, Ordering::Release);
                    false
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
                        generation: token.clone(),
                        executions: Mutex::new(BTreeMap::new()),
                        sequence: AtomicU64::new(1),
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
                if !context.native_quiescent() {
                    return DeviceResult::error(
                        "native_resources_pending",
                        "The context remains owned until native termination or isolation recovery is established.",
                    );
                }
                context.closed.store(true, Ordering::Release);
                if let Ok(mut contexts) = self.contexts.lock() {
                    contexts.remove(token.unwrap());
                }
                DeviceResult::value(json!({"closed":true,"business_goal_inferred":false}))
            }
            Call::Prepare { tool, arguments } => self.prepare(peer, policy, token, tool, arguments),
            Call::Execute { operation_ref } => self.execute(peer, policy, &operation_ref).await,
            Call::Cached { tool, arguments } => {
                self.cached(peer, policy, token, tool, arguments).await
            }
            Call::Operation { selector } => self.query(peer, &selector),
            Call::Cancel { operation_ref } => self.cancel(peer, &operation_ref),
        }
    }
}
