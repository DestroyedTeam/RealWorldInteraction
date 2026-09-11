//! Model TLS attachment only; no MCP input is consumed here.
use super::*;

pub(super) async fn open(
    config: &ClientConfig,
) -> io::Result<(BufReader<tokio_rustls::client::TlsStream<TcpStream>>, String)> {
    let mut stream = connect(config).await?;
    let nonce = uuid::Uuid::new_v4().to_string();
    let attach = Attach {
        version: crate::TRANSPORT_VERSION,
        task: config.task.clone(),
        approval_sha256: config.approval_sha256.clone(),
        role: Role::Model,
        nonce: nonce.clone(),
    };
    write_json(&mut stream, &attach, frame::ATTACH_CAP).await?;
    let mut stream = BufReader::new(stream);
    let raw = tokio::time::timeout(
        IO_TIMEOUT,
        frame::read_frame(&mut stream, frame::ATTACH_CAP),
    )
    .await
    .map_err(|_| invalid("attach_ack_deadline"))??
    .ok_or_else(|| invalid("attach_ack_missing"))?;
    let ack: AttachAck = serde_json::from_slice(&raw)?;
    if ack.version != crate::TRANSPORT_VERSION
        || !ack.attached
        || ack.task != config.task
        || ack.nonce != nonce
        || ack.role != Role::Model
    {
        return Err(invalid("attach_ack_exact_binding_mismatch"));
    }
    Ok((stream, nonce))
}
