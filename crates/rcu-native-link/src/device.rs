//! Task-independent authenticated device RPC. Callers own platform setup.
use crate::invalid;
use rcu_core::device::{PROTOCOL_VERSION, Reply, Request, valid_id};
use rcu_link_trust::{Identity, IdentityPaths, PeerPin};
use serde::{Deserialize, Serialize};
use std::{io, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};
use tokio_rustls::{TlsAcceptor, TlsConnector};

pub const REQUEST_CAP: usize = 1024 * 1024;
pub const RESPONSE_CAP: usize = 32 * 1024 * 1024;
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const CALL_TIMEOUT: Duration = Duration::from_secs(90);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairedDevice {
    pub device_id: String,
    pub endpoint: SocketAddr,
    pub identity: IdentityPaths,
    pub agent_pin: PeerPin,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterProfile {
    pub devices: Vec<PairedDevice>,
}
impl AdapterProfile {
    pub fn validate(&self) -> io::Result<()> {
        let mut ids = std::collections::BTreeSet::new();
        if self.devices.len() > 32 {
            return Err(invalid("device_limit"));
        }
        for d in &self.devices {
            if !valid_id(&d.device_id)
                || !ids.insert(&d.device_id)
                || d.endpoint.port() == 0
                || d.endpoint.ip().is_unspecified()
                || d.endpoint.ip().is_multicast()
            {
                return Err(invalid("invalid_or_duplicate_device"));
            }
        }
        Ok(())
    }
}
#[derive(Clone)]
pub struct Client {
    device: PairedDevice,
    tls: Arc<rustls::ClientConfig>,
}
impl Client {
    pub fn new(device: PairedDevice, identity: Identity) -> io::Result<Self> {
        AdapterProfile {
            devices: vec![device.clone()],
        }
        .validate()?;
        let tls = identity.client_config(&device.agent_pin)?;
        Ok(Self { device, tls })
    }
    /// One send only. A new connection is permitted, but this method never retries
    /// a call after an uncertain transport result, including a lost response.
    pub async fn call(&self, request: &Request) -> io::Result<Reply> {
        if request.version != PROTOCOL_VERSION
            || request.device != self.device.device_id
            || !valid_id(&request.id)
        {
            return Err(invalid("request_binding"));
        }
        let stream = tokio::time::timeout(CONNECT_TIMEOUT, async {
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
            Ok::<_, io::Error>(stream)
        })
        .await
        .map_err(|_| invalid("device_connect_timeout"))??;
        tokio::time::timeout(CALL_TIMEOUT, async {
            let mut stream = BufReader::new(stream);
            write(&mut stream, request, REQUEST_CAP).await?;
            let raw = super::wire::read_frame(&mut stream, RESPONSE_CAP)
                .await?
                .ok_or_else(|| invalid("device_response_lost"))?;
            let reply: Reply = serde_json::from_slice(&raw)?;
            if reply.version != PROTOCOL_VERSION
                || reply.id != request.id
                || reply.device != request.device
                || !valid_id(&reply.instance)
            {
                return Err(invalid("device_reply_binding"));
            }
            Ok(reply)
        })
        .await
        .map_err(|_| invalid("device_call_timeout_effect_unknown"))?
    }
}
pub async fn write<W: tokio::io::AsyncWrite + Unpin>(
    output: &mut W,
    value: &impl Serialize,
    cap: usize,
) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(b'\n');
    if bytes.len() > cap {
        return Err(invalid("device_frame_limit"));
    }
    output.write_all(&bytes).await?;
    output.flush().await
}

pub async fn serve<H, F>(
    listener: TcpListener,
    tls: Arc<rustls::ServerConfig>,
    handler: H,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> io::Result<()>
where
    H: Fn(PeerPin, Request) -> F + Send + Sync + 'static,
    F: std::future::Future<Output = Reply> + Send + 'static,
{
    let handler = Arc::new(handler);
    let limit = Arc::new(tokio::sync::Semaphore::new(32));
    let mut tasks = tokio::task::JoinSet::new();
    let mut shutdown = shutdown;
    loop {
        tokio::select! {
            _ = shutdown.changed() => { break; },
            Some(_) = tasks.join_next(), if !tasks.is_empty() => {},
            incoming = listener.accept() => {
                let (tcp, _) = incoming?;
                let Ok(permit) = limit.clone().try_acquire_owned() else { drop(tcp); continue; };
                let tls = tls.clone(); let handler = handler.clone();
                tasks.spawn(async move {
                    let _permit = permit;
                    let (peer, request, mut stream) = tokio::time::timeout(CONNECT_TIMEOUT, async {
                        let stream = TlsAcceptor::from(tls).accept(tcp).await?;
                        let peer = rcu_link_trust::peer_sha(stream.get_ref().1.peer_certificates())?;
                        let mut stream = BufReader::new(stream);
                        let raw = super::wire::read_frame(&mut stream, REQUEST_CAP).await?.ok_or_else(|| invalid("empty_request"))?;
                        let request: Request = serde_json::from_slice(&raw)?;
                        Ok::<_, io::Error>((peer, request, stream))
                    }).await.map_err(|_| invalid("handshake_deadline"))??;
                    let reply = handler(peer, request).await;
                    tokio::time::timeout(Duration::from_secs(10), write(&mut stream, &reply, RESPONSE_CAP)).await.map_err(|_| invalid("reply_timeout"))??;
                    let _ = stream.shutdown().await;
                    Ok::<_, io::Error>(())
                });
            }
        }
    }
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    Ok(())
}
