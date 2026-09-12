//! Exclusive connection checkout; uncertain calls discard their stream, never retry.
use super::*;
use rcu_core::device::Call;
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use tokio::{sync::Semaphore, time::Instant};

type Stream = BufReader<tokio_rustls::client::TlsStream<TcpStream>>;

#[cfg(all(test, unix))]
mod tests;

struct Connection {
    stream: Stream,
    persistent: bool,
    instance: Option<String>,
    idle_since: Instant,
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
    fn take(&self) -> Option<Connection> {
        let mut idle = self.idle.lock().unwrap_or_else(|e| e.into_inner());
        idle.retain(|c| c.idle_since.elapsed() < CLIENT_IDLE_TIMEOUT);
        idle.pop()
    }
    fn put(&self, mut connection: Connection) {
        connection.idle_since = Instant::now();
        self.idle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(connection);
    }
}
struct Shared {
    work: Lane,
    control: Lane,
    connected: AtomicU64,
    negotiated: AtomicU64,
    reused: AtomicU64,
    write_started: AtomicU64,
    replies_validated: AtomicU64,
}
/// Adapter-local counters, not native execution/effect or network latency evidence.
#[derive(Clone, Debug, Serialize)]
pub struct TransportStats {
    pub authenticated_connections: u64,
    pub keepalive_connections: u64,
    pub reused_checkouts: u64,
    pub request_writes_started: u64,
    pub replies_validated: u64,
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
                connected: AtomicU64::new(0),
                negotiated: AtomicU64::new(0),
                reused: AtomicU64::new(0),
                write_started: AtomicU64::new(0),
                replies_validated: AtomicU64::new(0),
            }),
        })
    }
    pub fn transport_stats(&self) -> TransportStats {
        let s = &self.shared;
        TransportStats {
            authenticated_connections: s.connected.load(Ordering::Relaxed),
            keepalive_connections: s.negotiated.load(Ordering::Relaxed),
            reused_checkouts: s.reused.load(Ordering::Relaxed),
            request_writes_started: s.write_started.load(Ordering::Relaxed),
            replies_validated: s.replies_validated.load(Ordering::Relaxed),
        }
    }
    async fn connect(&self) -> io::Result<Connection> {
        tokio::time::timeout(CONNECT_TIMEOUT, async {
            let tcp = TcpStream::connect(self.device.endpoint).await?;
            tcp.set_nodelay(true)?;
            let stream = TlsConnector::from(self.tls.clone())
                .connect(
                    rcu_link_trust::server_name(rcu_link_trust::SERVER_DNS_NAME)?,
                    tcp,
                )
                .await?;
            rcu_link_trust::verify_peer(
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
            })
        })
        .await
        .map_err(|_| invalid("device_connect_timeout"))?
    }
    /// One send only, including when a formerly healthy idle connection was closed.
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
        let lane = if matches!(
            request.call,
            Call::Cancel { .. } | Call::Operation { .. } | Call::Describe | Call::CloseContext
        ) {
            &self.shared.control
        } else {
            &self.shared.work
        };
        let _permit = tokio::time::timeout(CONNECT_TIMEOUT, lane.permits.acquire())
            .await
            .map_err(|_| invalid("device_pool_busy_not_sent"))?
            .map_err(|_| invalid("device_pool_closed_not_sent"))?;
        let mut connection = match lane.take() {
            Some(c) => {
                self.shared.reused.fetch_add(1, Ordering::Relaxed);
                c
            }
            None => self.connect().await?,
        };
        let reply = tokio::time::timeout(CALL_TIMEOUT, async {
            self.shared.write_started.fetch_add(1, Ordering::Relaxed);
            connection.stream.write_all(&bytes).await?;
            connection.stream.flush().await?;
            let raw = crate::wire::read_frame(&mut connection.stream, RESPONSE_CAP).await?
                .ok_or_else(|| invalid("device_response_lost"))?;
            let reply: Reply = serde_json::from_slice(&raw)?;
            if reply.version != PROTOCOL_VERSION {
                return Err(invalid("device_protocol_version_unsupported; update adapter and agent together; no downgrade or retry"));
            }
            if reply.id != request.id || reply.device != request.device || !valid_id(&reply.instance)
                || connection.instance.as_ref().is_some_and(|i| i != &reply.instance)
            { return Err(invalid("device_reply_binding")); }
            Ok(reply)
        }).await.map_err(|_| invalid("device_call_timeout_effect_unknown"))??;
        self.shared
            .replies_validated
            .fetch_add(1, Ordering::Relaxed);
        connection.instance = Some(reply.instance.clone());
        if connection.persistent {
            lane.put(connection);
        }
        Ok(reply)
    }
}
