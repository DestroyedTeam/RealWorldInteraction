//! Extracted authenticated device transport, behavior unchanged.
use super::*;

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
            let raw = crate::wire::read_frame(&mut stream, RESPONSE_CAP)
                .await?
                .ok_or_else(|| invalid("device_response_lost"))?;
            let reply: Reply = serde_json::from_slice(&raw)?;
            if reply.version != PROTOCOL_VERSION {
                return Err(invalid("device_protocol_version_unsupported; update adapter and agent together; no downgrade or retry"));
            }
            if reply.id != request.id
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
