mod capabilities;
mod dispatch;
#[cfg(all(test, feature = "fixture"))]
mod dispatch_tests;
mod execution;
mod live_control;
mod live_image;
mod namespace;
mod read_admission;
#[cfg(all(test, feature = "fixture"))]
mod visual_tests;
#[cfg(all(test, feature = "fixture"))]
mod window_tests;
pub use namespace::NamespaceRepair;
mod recovery;
use crate::{
    config::{Config, Principal},
    executor::Executor,
    ledger::{Ledger, Reservation},
    platform,
};
use rwi_core::{
    device::*,
    operation::{Operation, Registry},
};
use rwi_link_trust::PeerPin;
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
    read_waiters: Arc<tokio::sync::Semaphore>,
    registry: Arc<Registry>,
    live_gate: Arc<tokio::sync::Semaphore>,
    image_gate: Arc<tokio::sync::Semaphore>,
    image_rate: Mutex<BTreeMap<String, Instant>>,
    live_controls: Arc<Mutex<rwi_core::LiveControllers>>,
    touched: Mutex<Instant>,
    target: String,
    target_catalog: Arc<Mutex<serde_json::Value>>,
    entered: Arc<AtomicBool>,
}
impl Context {
    fn owns_target(&self, target: &serde_json::Value) -> bool {
        target == &self.target
            || self
                .target_catalog
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_array()
                .is_some_and(|rows| rows.iter().any(|row| &row["target"] == target))
    }
}
/// Orderly service ownership ends here, not when an incidental duplicate OS
/// descriptor (e.g. during concurrent process creation) is eventually closed.
/// This field is dropped last, after all contexts and durable state handles.
struct StateOwner(std::fs::File);
impl Drop for StateOwner {
    fn drop(&mut self) {
        if let Err(error) = self.0.unlock() {
            eprintln!("rwi-agent: state owner unlock unconfirmed: {error}");
        }
    }
}
pub struct Runtime {
    pub config: Config,
    pub instance: String,
    browser: Arc<crate::browser_content::BrowserRuntime>,
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
        let instance = uuid::Uuid::new_v4().to_string();
        let browser = crate::browser_content::BrowserRuntime::new(&config, &instance);
        Ok(Arc::new(Self {
            config,
            instance,
            browser,
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
            self.call(
                policy.identity(),
                policy,
                request.context.as_deref(),
                request.call,
            )
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
            Call::BrowserSession { arguments } => {
                self.browser.clone().call(policy, arguments).await
            }
            Call::BrowserSnapshot { arguments } => {
                self.browser.clone().snapshot(policy, arguments).await
            }
            Call::BrowserAct { arguments } => self.browser.clone().act(policy, arguments).await,
            Call::Describe => {
                let mut r = self.describe(policy);
                let context = self.context(peer, token);
                r.data["context_valid"] = json!(token.is_none() || context.is_some());
                if let Some(context) = context {
                    r.data["targets"] = context
                        .target_catalog
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone();
                    let facts = r.data["live_desktop_facts"].clone();
                    capabilities::refresh_volatile_target_facts(&mut r.data["targets"], &facts);
                }
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
                let mut engine = match rwi_core::Engine::with_lease_root(
                    self.config
                        .state_root
                        .join("principals")
                        .join(rwi_core::digest(peer.as_str().as_bytes())),
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
                    let Ok(_live_idle) = c.live_gate.clone().try_acquire_owned() else {
                        return true;
                    };
                    let Ok(_image_idle) = c
                        .image_gate
                        .clone()
                        .try_acquire_many_owned(live_image::READERS)
                    else {
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
                let executor = Executor::new(engine);
                let target_catalog = executor.target_catalog.clone();
                let live_controls = executor.live_controls.clone();
                contexts.insert(
                    token.clone(),
                    Arc::new(Context {
                        domain,
                        closed: AtomicBool::new(false),
                        generation: token.clone(),
                        executions: Mutex::new(BTreeMap::new()),
                        sequence: AtomicU64::new(1),
                        principal: peer.as_str().into(),
                        executor: Arc::new(Mutex::new(executor)),
                        target_catalog,
                        gate: Arc::new(tokio::sync::Semaphore::new(1)),
                        read_waiters: Arc::new(tokio::sync::Semaphore::new(read_admission::LIMIT)),
                        registry: Arc::new(Registry::default()),
                        live_gate: Arc::new(tokio::sync::Semaphore::new(1)),
                        image_gate: Arc::new(tokio::sync::Semaphore::new(
                            live_image::READERS as usize,
                        )),
                        image_rate: Mutex::new(BTreeMap::new()),
                        live_controls,
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
                let Ok(_live_idle) = context.live_gate.clone().try_acquire_owned() else {
                    return DeviceResult::error(
                        "context_busy",
                        "Owned Live control is pending; query its operation before closing.",
                    );
                };
                let Ok(_image_idle) = context
                    .image_gate
                    .clone()
                    .try_acquire_many_owned(live_image::READERS)
                else {
                    return DeviceResult::error(
                        "context_busy",
                        "Cached Live image read is pending; close after it is idle.",
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
