//! Exclusive checkout; only a disconnected, reused Describe may reconnect once.
mod diagnostics;
mod pool;
mod response;
use super::*;
use diagnostics::ReconnectLog;
pub use diagnostics::{DescribeReconnect, TransportFailure};
use pool::Generation;
use rwi_core::device::Call;
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use tokio::{sync::Semaphore, time::Instant};

type Stream = BufReader<tokio_rustls::client::TlsStream<TcpStream>>;

fn uses_control_lane(call: &Call) -> bool {
    matches!(
        call,
        Call::Cancel { .. }
            | Call::Operation { .. }
            | Call::Describe
            | Call::CloseContext
            | Call::BrowserSession {
                arguments: rwi_core::device::browser::SessionArgs::Query { .. }
                    | rwi_core::device::browser::SessionArgs::Close { .. }
            }
    )
}
#[cfg(test)]
#[test]
fn f14_browser_status_close_have_control_capacity_not_native_work_lane() {
    use rwi_core::device::browser::SessionArgs;
    assert!(uses_control_lane(&Call::BrowserSession {
        arguments: SessionArgs::Query {
            device_id: "mac".into(),
            request_id: "one".into()
        }
    }));
    assert!(uses_control_lane(&Call::BrowserSession {
        arguments: SessionArgs::Close {
            device_id: "mac".into(),
            request_id: "two".into(),
            session_ref: "browser:one".into()
        }
    }));
    assert!(!uses_control_lane(&Call::BrowserSession {
        arguments: SessionArgs::Create {
            device_id: "mac".into(),
            request_id: "three".into(),
            url: "about:blank".into()
        }
    }));
}

#[cfg(all(test, unix))]
mod tests;

struct Connection {
    stream: Stream,
    persistent: bool,
    instance: Option<String>,
    idle_since: Instant,
    generation: Arc<()>,
}
struct Lane {
    permits: Semaphore,
    idle: Mutex<Vec<Connection>>,
}
impl Lane {
    fn new(limit: usize) -> Self {
        Self {
            permits: Semaphore::new(limit),
            idle: Mutex::new(Vec::new()),
        }
    }
}
struct Shared {
    work: Lane,
    control: Lane,
    generation: Mutex<Generation>,
    reconnects: Mutex<ReconnectLog>,
    connected: AtomicU64,
    negotiated: AtomicU64,
    reused: AtomicU64,
    write_started: AtomicU64,
    replies_validated: AtomicU64,
    pool_invalidations: AtomicU64,
    retired_returns_discarded: AtomicU64,
}
/// Adapter-local counters, not native execution/effect or network latency evidence.
#[derive(Clone, Debug, Serialize)]
pub struct TransportStats {
    pub authenticated_connections: u64,
    pub keepalive_connections: u64,
    pub reused_checkouts: u64,
    pub request_writes_started: u64,
    pub replies_validated: u64,
    pub pool_invalidations: u64,
    pub retired_returns_discarded: u64,
    pub describe_reconnect_attempts: u64,
    pub describe_reconnect_successes: u64,
    pub last_describe_reconnect: Option<DescribeReconnect>,
}

struct Failure {
    error: io::Error,
    disconnected: bool,
}
impl Failure {
    fn other(error: io::Error) -> Self {
        Self {
            error,
            disconnected: false,
        }
    }
    fn transport(error: io::Error) -> Self {
        // Deliberately exclude timeout, UnexpectedEof (including partial frames),
        // TLS InvalidData/authentication, framing/JSON and envelope failures.
        let disconnected = matches!(
            error.kind(),
            io::ErrorKind::BrokenPipe
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::NotConnected
        );
        Self {
            error,
            disconnected,
        }
    }
}
struct AttemptFailure {
    failure: Failure,
    reused: bool,
}

#[derive(Clone)]
pub struct Client {
    device: PairedDevice,
    tls: Arc<rustls::ClientConfig>,
    shared: Arc<Shared>,
}
impl Client {
    pub fn new(device: PairedDevice, identity: Identity) -> io::Result<Self> {
        AdapterProfile {
            devices: vec![device.clone()],
        }
        .validate()?;
        let mut tls = (*identity.client_config(&device.agent_pin)?).clone();
        tls.alpn_protocols = vec![KEEPALIVE_ALPN.to_vec()];
        Ok(Self {
            device,
            tls: Arc::new(tls),
            shared: Arc::new(Shared {
                work: Lane::new(4),
                control: Lane::new(2),
                generation: Mutex::new(Generation::default()),
                reconnects: Mutex::new(ReconnectLog::default()),
                connected: AtomicU64::new(0),
                negotiated: AtomicU64::new(0),
                reused: AtomicU64::new(0),
                write_started: AtomicU64::new(0),
                replies_validated: AtomicU64::new(0),
                pool_invalidations: AtomicU64::new(0),
                retired_returns_discarded: AtomicU64::new(0),
            }),
        })
    }
    pub fn transport_stats(&self) -> TransportStats {
        let s = &self.shared;
        let reconnects = s.reconnects.lock().unwrap_or_else(|e| e.into_inner());
        TransportStats {
            authenticated_connections: s.connected.load(Ordering::Relaxed),
            keepalive_connections: s.negotiated.load(Ordering::Relaxed),
            reused_checkouts: s.reused.load(Ordering::Relaxed),
            request_writes_started: s.write_started.load(Ordering::Relaxed),
            replies_validated: s.replies_validated.load(Ordering::Relaxed),
            pool_invalidations: s.pool_invalidations.load(Ordering::Relaxed),
            retired_returns_discarded: s.retired_returns_discarded.load(Ordering::Relaxed),
            describe_reconnect_attempts: reconnects.attempts,
            describe_reconnect_successes: reconnects.successes,
            last_describe_reconnect: reconnects.last(),
        }
    }
    async fn connect(&self, generation: Arc<()>) -> io::Result<Connection> {
        tokio::time::timeout(CONNECT_TIMEOUT, async {
            let tcp = TcpStream::connect(self.device.endpoint).await?;
            tcp.set_nodelay(true)?;
            let stream = TlsConnector::from(self.tls.clone())
                .connect(
                    rwi_link_trust::server_name(rwi_link_trust::SERVER_DNS_NAME)?,
                    tcp,
                )
                .await?;
            rwi_link_trust::verify_peer(
                stream.get_ref().1.peer_certificates(),
                &self.device.agent_pin,
            )?;
            let persistent = stream.get_ref().1.alpn_protocol() == Some(KEEPALIVE_ALPN);
            self.shared.connected.fetch_add(1, Ordering::Relaxed);
            if persistent {
                self.shared.negotiated.fetch_add(1, Ordering::Relaxed);
            }
            Ok(Connection {
                stream: BufReader::new(stream),
                persistent,
                instance: None,
                idle_since: Instant::now(),
                generation,
            })
        })
        .await
        .map_err(|_| invalid("device_connect_timeout"))?
    }
    /// One send for every call except Describe on a reused, explicitly disconnected
    /// stream: that control-plane query may reconnect/authenticate once. Describe's
    /// existing context-idle touch is bookkeeping, not input authority renewal.
    /// A dropped future owns/drops its in-flight stream. Only validated replies
    /// return a connection to the pool; leases and operation history are independent.
    pub async fn call(&self, request: &Request) -> io::Result<Reply> {
        if request.version != PROTOCOL_VERSION
            || request.device != self.device.device_id
            || !valid_id(&request.id)
        {
            return Err(invalid("request_binding"));
        }
        let mut bytes = serde_json::to_vec(request)?;
        bytes.push(b'\n');
        if bytes.len() > REQUEST_CAP {
            return Err(invalid("device_frame_limit"));
        }
        // Independent capacity: a long Execute never holds the control lane's
        // permit or stream. No mutex is held while doing network I/O.
        let lane = if uses_control_lane(&request.call) {
            &self.shared.control
        } else {
            &self.shared.work
        };
        let _permit = tokio::time::timeout(CONNECT_TIMEOUT, lane.permits.acquire())
            .await
            .map_err(|_| invalid("device_pool_busy_not_sent"))?
            .map_err(|_| invalid("device_pool_closed_not_sent"))?;
        let original = match self.attempt(lane, request, &bytes, false).await {
            Ok(reply) => return Ok(reply),
            Err(error) => error,
        };
        if !matches!(request.call, Call::Describe)
            || !original.reused
            || !original.failure.disconnected
        {
            return Err(original.failure.error);
        }
        let ticket = self
            .shared
            .reconnects
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .begin(request, &original.failure.error);
        // No loop and no pool checkout: the single exception must perform a new
        // authenticated handshake. OpenContext/Execute/etc. never reach here.
        let result = self
            .attempt(lane, request, &bytes, true)
            .await
            .map_err(|error| error.failure.error);
        self.shared
            .reconnects
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .finish(&ticket, &result);
        result.map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "{}; describe_reconnect_failed: {error}; no further retry",
                    original.failure.error
                ),
            )
        })
    }
    async fn attempt(
        &self,
        lane: &Lane,
        request: &Request,
        bytes: &[u8],
        fresh: bool,
    ) -> Result<Reply, AttemptFailure> {
        let (cached, generation) = self.shared.checkout(lane, fresh);
        let reused = cached.is_some();
        let mut connection = match cached {
            Some(c) => {
                self.shared.reused.fetch_add(1, Ordering::Relaxed);
                c
            }
            None => match self.connect(generation.clone()).await {
                Ok(c) => c,
                Err(error) => {
                    self.shared.retire(&generation);
                    return Err(AttemptFailure {
                        failure: Failure::other(error),
                        reused,
                    });
                }
            },
        };
        let result = self.exchange(&mut connection, request, bytes).await;
        match result {
            Ok(reply) => {
                self.shared
                    .replies_validated
                    .fetch_add(1, Ordering::Relaxed);
                self.shared.release(lane, connection, &reply.instance);
                Ok(reply)
            }
            Err(failure) => {
                self.shared.retire(&connection.generation);
                Err(AttemptFailure { failure, reused })
            }
        }
    }
    async fn exchange(
        &self,
        connection: &mut Connection,
        request: &Request,
        bytes: &[u8],
    ) -> Result<Reply, Failure> {
        if !self.shared.send_permitted(&connection.generation) {
            return Err(Failure::other(invalid("device_pool_retired_not_sent")));
        }
        tokio::time::timeout(CALL_TIMEOUT, async {
            self.shared.write_started.fetch_add(1, Ordering::Relaxed);
            connection.stream.write_all(bytes).await.map_err(Failure::transport)?;
            connection.stream.flush().await.map_err(Failure::transport)?;
            let raw = response::read(&mut connection.stream).await?
                .ok_or_else(|| Failure { error: invalid("device_response_lost"), disconnected: true })?;
            let reply: Reply = serde_json::from_slice(&raw)
                .map_err(|error| Failure::other(error.into()))?;
            if reply.version != PROTOCOL_VERSION {
                return Err(Failure::other(invalid("device_protocol_version_unsupported; update adapter and agent together; no downgrade or retry")));
            }
            if reply.id != request.id || reply.device != request.device || !valid_id(&reply.instance)
                || connection.instance.as_ref().is_some_and(|i| i != &reply.instance)
            { return Err(Failure::other(invalid("device_reply_binding"))); }
            Ok(reply)
        }).await.map_err(|_| Failure::other(invalid("device_call_timeout_effect_unknown")))?
    }
}
