//! The only startup-failure boundary: no MCP stdin has been consumed here.
use super::*;

#[derive(Debug)]
pub struct StartupFailure(pub(super) io::Error);
impl std::fmt::Display for StartupFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for StartupFailure {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rejected {
    version: u8,
    attached: bool,
    task: String,
    nonce: String,
    role: Role,
    reason: String,
    hold_reason: Option<String>,
    next_stage: Option<crate::types::Stage>,
}
fn refusal(raw: &[u8], config: &ClientConfig, nonce: &str) -> io::Result<()> {
    let value: Value = serde_json::from_slice(raw)?;
    if value["attached"] != false {
        return Ok(());
    }
    let rejected: Rejected = serde_json::from_value(value)?;
    if rejected.attached
        || rejected.version != crate::TRANSPORT_VERSION
        || rejected.task != config.task
        || rejected.nonce != nonce
        || rejected.role != Role::Model
        || rejected.reason.chars().count() > 256
        || rejected
            .hold_reason
            .as_ref()
            .is_some_and(|s| s.chars().count() > 256)
    {
        return Err(invalid("attach_rejection_binding_mismatch"));
    }
    Err(invalid(&format!(
        "model_admission_rejected:{}; hold={}; next={:?}",
        rejected.reason,
        rejected.hold_reason.as_deref().unwrap_or("none"),
        rejected.next_stage
    )))
}

pub(super) async fn open(
    config: &ClientConfig,
) -> io::Result<(
    BufReader<tokio_rustls::client::TlsStream<TcpStream>>,
    String,
)> {
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
    refusal(&raw, config, &nonce)?;
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
