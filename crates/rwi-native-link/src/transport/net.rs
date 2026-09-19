//! Real configured TCP + pinned mTLS. No DNS discovery, reconnect, replay, or
//! built-in trust. Model attach/end are consumed here, never emitted as MCP.
use crate::{
    frame, invalid,
    types::{AgentConfig, Attach, ClientConfig, ModelEnd, Role, digest_shape, identifier},
};
use rwi_link_trust::{PeerPin, server_name};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncBufRead, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    net::TcpStream,
    sync::mpsc,
    task::JoinHandle,
};

#[path = "model_attach.rs"]
mod model_attach;
pub use model_attach::StartupFailure;

pub const IO_TIMEOUT: Duration = Duration::from_secs(5);
pub const MODEL_LIMIT: Duration = Duration::from_secs(1200);
pub const END_ACK_TIMEOUT: Duration = Duration::from_secs(30);
pub const MODEL_PREPARE: Duration = Duration::from_millis(crate::budget::MODEL_PREPARE_MS);
pub const MAX_PENDING: usize = 512;
pub const MAX_CLIENT_FRAMES: usize = 1024;
const QUEUE: usize = 2;

fn endpoint(value: SocketAddr) -> io::Result<()> {
    if value.port() == 0
        || value.ip().is_unspecified()
        || value.ip().is_multicast()
        || value.ip() == std::net::IpAddr::V4(std::net::Ipv4Addr::BROADCAST)
    {
        return Err(invalid("configured_endpoint_invalid"));
    }
    Ok(())
}

pub async fn connect(
    config: &ClientConfig,
) -> io::Result<tokio_rustls::client::TlsStream<TcpStream>> {
    if config.version != 1
        || !identifier(&config.task, 64)
        || !digest_shape(&config.approval_sha256)
    {
        return Err(invalid("client_configuration_identity_invalid"));
    }
    endpoint(config.endpoint)?;
    let name = server_name(&config.server_name)?;
    let expected = PeerPin::parse(&config.agent_peer_sha256)?;
    #[cfg(windows)]
    let identity = config.trust.load_with(crate::private::read_private)?;
    #[cfg(not(windows))]
    let identity = config.trust.load()?;
    let tls = identity.client_config(&expected)?;
    tokio::time::timeout(IO_TIMEOUT, async {
        let tcp = TcpStream::connect(config.endpoint).await?;
        tcp.set_nodelay(true)?;
        let stream = tokio_rustls::TlsConnector::from(tls)
            .connect(name, tcp)
            .await?;
        rwi_link_trust::verify_peer(stream.get_ref().1.peer_certificates(), &expected)?;
        if stream.get_ref().1.protocol_version() != Some(rustls::ProtocolVersion::TLSv1_3) {
            return Err(invalid("non_tls13_connection_refused"));
        }
        Ok(stream)
    })
    .await
    .map_err(|_| invalid("connect_and_tls_deadline"))?
}

pub fn server_tls(config: &AgentConfig) -> io::Result<Arc<rustls::ServerConfig>> {
    if config.version != 1 {
        return Err(invalid("agent_configuration_version"));
    }
    server_name(&config.server_name)?;
    let pins = [
        PeerPin::parse(&config.controller_peer_sha256)?,
        PeerPin::parse(&config.model_peer_sha256)?,
    ];
    #[cfg(windows)]
    let identity = config.trust.load_with(crate::private::read_private)?;
    #[cfg(not(windows))]
    let identity = config.trust.load()?;
    identity.server_config(&pins)
}

pub async fn accept(
    tcp: TcpStream,
    config: Arc<rustls::ServerConfig>,
) -> io::Result<tokio_rustls::server::TlsStream<TcpStream>> {
    tcp.set_nodelay(true)?;
    let stream = tokio::time::timeout(
        IO_TIMEOUT,
        tokio_rustls::TlsAcceptor::from(config).accept(tcp),
    )
    .await
    .map_err(|_| invalid("server_tls_deadline"))??;
    if stream.get_ref().1.protocol_version() != Some(rustls::ProtocolVersion::TLSv1_3) {
        return Err(invalid("non_tls13_connection_refused"));
    }
    rwi_link_trust::peer_sha(stream.get_ref().1.peer_certificates())?;
    Ok(stream)
}

/// Only a fixture-enabled preparation build can allocate test listeners here.
/// Production listeners are selected/approved by the root agent's fixed config.
#[cfg(feature = "fixture")]
pub async fn bind_fixture_loopback(address: SocketAddr) -> io::Result<tokio::net::TcpListener> {
    if !address.ip().is_loopback() || address.port() != 0 {
        return Err(invalid("fixture_listener_requires_loopback_ephemeral"));
    }
    tokio::net::TcpListener::bind(address).await
}

pub async fn read_frame<R: AsyncBufRead + Unpin>(
    input: &mut R,
    cap: usize,
) -> io::Result<Option<Vec<u8>>> {
    frame::read_frame(input, cap).await
}

/// The bound includes LF. Callers already parsed frames; bytes are not rewritten.
/// The deadline covers the entire write AND flush, not only the first write.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    output: &mut W,
    bytes: &[u8],
    cap: usize,
) -> io::Result<()> {
    if cap == 0
        || cap > frame::OUTBOUND
        || bytes.is_empty()
        || bytes.len() > cap
        || bytes.last() != Some(&b'\n')
    {
        return Err(invalid("outgoing_frame_bound_or_delimiter"));
    }
    tokio::time::timeout(IO_TIMEOUT, async {
        output.write_all(bytes).await?;
        output.flush().await
    })
    .await
    .map_err(|_| invalid("whole_write_flush_deadline"))?
}

pub async fn write_json<W: AsyncWrite + Unpin, T: Serialize>(
    output: &mut W,
    value: &T,
    cap: usize,
) -> io::Result<()> {
    let bytes = json_frame(value, cap)?;
    write_frame(output, &bytes, cap).await
}

fn json_frame<T: Serialize>(value: &T, cap: usize) -> io::Result<Vec<u8>> {
    struct Bounded {
        data: Vec<u8>,
        cap: usize,
    }
    impl io::Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > self.cap.saturating_sub(self.data.len()) {
                return Err(invalid("outgoing_json_cap"));
            }
            self.data.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    if !(3..=frame::OUTBOUND).contains(&cap) {
        return Err(invalid("outgoing_json_bound"));
    }
    let mut output = Bounded {
        data: Vec::new(),
        cap: cap - 1,
    };
    serde_json::to_writer(&mut output, value)?;
    output.data.push(b'\n');
    Ok(output.data)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AttachAck {
    version: u8,
    attached: bool,
    task: String,
    nonce: String,
    role: Role,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EndAck {
    transport_version: u8,
    model_end_ack: bool,
    task: String,
    stage_receipt: String,
    complete: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelReady {
    pub transport_version: u8,
    pub model_ready: bool,
    pub task: String,
    pub nonce: String,
    pub usable_ms: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Writer {
    Agent,
    Stdout,
}
struct WriteJob {
    ticket: u64,
    raw: Vec<u8>,
    dispatch_deadline: Option<tokio::time::Instant>,
}
struct WriteAck {
    writer: Writer,
    ticket: u64,
    result: io::Result<()>,
}

type FrameReceiver = mpsc::Receiver<io::Result<Option<Vec<u8>>>>;
fn reader<R: AsyncRead + Unpin + Send + 'static>(
    input: R,
    cap: usize,
) -> (FrameReceiver, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel(QUEUE);
    let handle = tokio::spawn(async move {
        let mut input = BufReader::new(input);
        loop {
            let next = frame::read_frame(&mut input, cap).await;
            let stop = !matches!(&next, Ok(Some(_)));
            if tx.send(next).await.is_err() || stop {
                break;
            }
        }
    });
    (rx, handle)
}

fn writer<W: AsyncWrite + Unpin + Send + 'static>(
    mut output: W,
    cap: usize,
    kind: Writer,
    events: mpsc::Sender<WriteAck>,
) -> (mpsc::Sender<WriteJob>, JoinHandle<io::Result<()>>) {
    let (tx, mut rx) = mpsc::channel::<WriteJob>(QUEUE);
    let handle = tokio::spawn(async move {
        while let Some(job) = rx.recv().await {
            let result = if job
                .dispatch_deadline
                .is_some_and(|deadline| tokio::time::Instant::now() >= deadline)
            {
                Err(invalid("active_model_dispatch_deadline_hold"))
            } else {
                write_frame(&mut output, &job.raw, cap).await
            };
            let failed = result.is_err();
            if events
                .send(WriteAck {
                    writer: kind,
                    ticket: job.ticket,
                    result,
                })
                .await
                .is_err()
            {
                return Err(invalid("writer_owner_gone"));
            }
            if failed {
                return Err(invalid("writer_failed_no_retry"));
            }
        }
        tokio::time::timeout(IO_TIMEOUT, output.shutdown())
            .await
            .map_err(|_| invalid("writer_shutdown_deadline"))?
    });
    (tx, handle)
}

struct AbortOnDrop(Vec<tokio::task::AbortHandle>);
impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        for task in &self.0 {
            task.abort();
        }
    }
}

fn rpc_id(value: &Value) -> io::Result<String> {
    match value {
        Value::String(s) if !s.is_empty() && s.len() <= 256 => {
            serde_json::to_string(value).map_err(io::Error::other)
        }
        Value::Number(n) if n.is_i64() || n.is_u64() => Ok(n.to_string()),
        _ => Err(invalid("jsonrpc_id_must_be_bounded_string_or_integer")),
    }
}

#[derive(Default)]
struct Ledger {
    seen: BTreeSet<String>,
    pending: BTreeMap<String, bool>, // false = awaits reply; true = stdout queued
    client_frames: usize,
    server_notifications: usize,
}
impl Ledger {
    fn client(&mut self, raw: &[u8]) -> io::Result<()> {
        self.client_frames += 1;
        if self.client_frames > MAX_CLIENT_FRAMES {
            return Err(invalid("client_frame_budget_exhausted"));
        }
        let value: Value = serde_json::from_slice(raw)?;
        let object = value
            .as_object()
            .ok_or_else(|| invalid("stdin_jsonrpc_object"))?;
        if object.get("jsonrpc") != Some(&Value::String("2.0".into()))
            || object
                .keys()
                .any(|k| !["jsonrpc", "id", "method", "params"].contains(&k.as_str()))
            || !object
                .get("method")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty() && s.len() <= 128)
            || object
                .get("params")
                .is_some_and(|v| !v.is_object() && !v.is_array())
        {
            return Err(invalid("stdin_only_jsonrpc_requests_or_notifications"));
        }
        if let Some(id) = object.get("id") {
            let id = rpc_id(id)?;
            if self.pending.len() >= MAX_PENDING || !self.seen.insert(id.clone()) {
                return Err(invalid("jsonrpc_pending_limit_or_id_reuse"));
            }
            self.pending.insert(id, false);
        }
        Ok(())
    }

    fn server(&mut self, raw: &[u8]) -> io::Result<Option<String>> {
        let value: Value = serde_json::from_slice(raw)?;
        let object = value
            .as_object()
            .ok_or_else(|| invalid("peer_jsonrpc_object"))?;
        if object.get("jsonrpc") != Some(&Value::String("2.0".into())) {
            return Err(invalid("peer_control_or_pollution_not_mcp"));
        }
        if let Some(id) = object.get("id") {
            if object
                .keys()
                .any(|k| !["jsonrpc", "id", "result", "error"].contains(&k.as_str()))
                || object.contains_key("result") == object.contains_key("error")
                || object.get("error").is_some_and(|error| {
                    !error.as_object().is_some_and(|error| {
                        error
                            .keys()
                            .all(|key| ["code", "message", "data"].contains(&key.as_str()))
                            && error.get("code").and_then(Value::as_i64).is_some()
                            && error.get("message").and_then(Value::as_str).is_some()
                    })
                })
            {
                return Err(invalid("peer_jsonrpc_response_shape"));
            }
            let id = rpc_id(id)?;
            let state = self
                .pending
                .get_mut(&id)
                .ok_or_else(|| invalid("peer_unknown_or_duplicate_reply_id"))?;
            if *state {
                return Err(invalid("peer_duplicate_reply_in_stdout_queue"));
            }
            *state = true;
            Ok(Some(id))
        } else {
            self.server_notifications += 1;
            if self.server_notifications > MAX_CLIENT_FRAMES
                || object
                    .keys()
                    .any(|k| !["jsonrpc", "method", "params"].contains(&k.as_str()))
                || !object
                    .get("method")
                    .and_then(Value::as_str)
                    .is_some_and(|s| s.starts_with("notifications/") && s.len() <= 128)
                || object
                    .get("params")
                    .is_some_and(|v| !v.is_object() && !v.is_array())
            {
                return Err(invalid("peer_notification_budget_or_shape"));
            }
            Ok(None)
        }
    }
}

/// Standard stdio model link. The returned value is an agent stage-receipt SHA,
/// not a native goal assertion. No retry occurs after any loss or uncertainty.
pub async fn relay_model<R, W>(config: &ClientConfig, input: R, output: W) -> io::Result<String>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    if config.role != Role::Model {
        return Err(invalid("relay_requires_model_role"));
    }
    let preparation_deadline = tokio::time::Instant::now() + MODEL_PREPARE;
    tokio::time::timeout(
        MODEL_PREPARE + MODEL_LIMIT + IO_TIMEOUT + END_ACK_TIMEOUT,
        model_inner(config, input, output, preparation_deadline),
    )
    .await
    .map_err(|_| invalid("model_and_cleanup_total_deadline_hold"))?
}

async fn model_inner<R, W>(
    config: &ClientConfig,
    input: R,
    output: W,
    preparation_deadline: tokio::time::Instant,
) -> io::Result<String>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let presenter = if config.model_output == crate::presentation::OutputMode::CompactV1 {
        Some(crate::presentation::Presenter::open(&config.root)?)
    } else {
        None
    };
    let (stream, nonce) = model_attach::open(config)
        .await
        .map_err(|e| io::Error::other(StartupFailure(e)))?;
    // Keep BufReader's already-received bytes; extracting its inner stream would
    // discard a coalesced MCP frame following AttachAck.
    let (remote_read, remote_write) = tokio::io::split(stream);
    let (mut stdin_rx, stdin_task) = reader(input, frame::INBOUND);
    let (mut peer_rx, peer_task) = reader(remote_read, frame::OUTBOUND);
    let (event_tx, mut event_rx) = mpsc::channel(4);
    let (agent_tx, agent_task) = writer(
        remote_write,
        frame::INBOUND,
        Writer::Agent,
        event_tx.clone(),
    );
    let (stdout_tx, stdout_task) = writer(output, frame::OUTBOUND, Writer::Stdout, event_tx);
    let guard = AbortOnDrop(vec![
        stdin_task.abort_handle(),
        peer_task.abort_handle(),
        agent_task.abort_handle(),
        stdout_task.abort_handle(),
    ]);
    let mut cleanup_deadline = None;
    let result = async {
        let mut ledger = Ledger::default();
        let mut ticket = 0u64;
        let mut agent_jobs = BTreeSet::new();
        let mut stdout_jobs: BTreeMap<u64, Option<String>> = BTreeMap::new();
        let mut eof = false;
        let mut ending = false;
        let mut end_receipt: Option<String> = None;
        let mut active_deadline = preparation_deadline;
        let mut ready = false;
        let mut deadline = preparation_deadline;
        loop {
            if eof && ready && !ending && ledger.pending.is_empty() && agent_jobs.is_empty() && stdout_jobs.is_empty() {
                ticket += 1;
                let end = ModelEnd { transport_version: crate::TRANSPORT_VERSION, model_end: true, task: config.task.clone(), nonce: nonce.clone() };
                let raw = json_frame(&end, frame::ATTACH_CAP)?;
                agent_tx.try_send(WriteJob { ticket, raw, dispatch_deadline: None }).map_err(|_| invalid("model_end_queue_unavailable_hold"))?;
                agent_jobs.insert(ticket);
                ending = true;
                deadline = tokio::time::Instant::now() + END_ACK_TIMEOUT;
                cleanup_deadline = Some(deadline);
            }
            if let Some(receipt) = &end_receipt
                && agent_jobs.is_empty() && stdout_jobs.is_empty() {
                break Ok(receipt.clone());
            }
            tokio::select! {
                _ = tokio::time::sleep_until(active_deadline), if !eof || !ready => return Err(invalid(if ready { "active_model_1200s_deadline_hold" } else { "model_actual_mcp_preparation_deadline_hold" })),
                _ = tokio::time::sleep_until(deadline), if eof => return Err(invalid(if ending { "model_end_ack_deadline_hold" } else { "stdin_eof_pending_drain_deadline_hold" })),
                next = stdin_rx.recv(), if !eof => {
                    match next.ok_or_else(|| invalid("stdin_reader_lost"))?? {
                        Some(raw) => {
                            if tokio::time::Instant::now() >= active_deadline { return Err(invalid("active_model_dispatch_deadline_hold")); }
                            if !ready {
                                let value: Value = serde_json::from_slice(&raw)?;
                                if !matches!(value["method"].as_str(), Some("initialize" | "notifications/initialized" | "tools/list")) {
                                    return Err(invalid("stdin_business_before_actual_mcp_ready"));
                                }
                            }
                            ledger.client(&raw)?;
                            ticket += 1;
                            agent_tx.try_send(WriteJob { ticket, raw, dispatch_deadline: Some(active_deadline) }).map_err(|_| invalid("agent_write_queue_full_hold"))?;
                            agent_jobs.insert(ticket);
                        }
                        None => { eof = true; deadline = tokio::time::Instant::now() + IO_TIMEOUT; }
                    }
                }
                next = peer_rx.recv(), if end_receipt.is_none() => {
                    let raw = next.ok_or_else(|| invalid("peer_reader_lost_hold"))??.ok_or_else(|| invalid("tls_eof_without_confirmed_model_end_hold"))?;
                    if ending {
                        let end: EndAck = serde_json::from_slice(&raw)?;
                        if end.transport_version != crate::TRANSPORT_VERSION || !end.model_end_ack || end.task != config.task || !end.complete || !digest_shape(&end.stage_receipt) {
                            return Err(invalid("model_end_ack_invalid_hold"));
                        }
                        end_receipt = Some(end.stage_receipt);
                    } else {
                        let value: Value = serde_json::from_slice(&raw)?;
                        if value.get("model_ready").is_some() {
                            let received: ModelReady = serde_json::from_value(value)?;
                            if ready || received.transport_version != crate::TRANSPORT_VERSION || !received.model_ready
                                || received.task != config.task || received.nonce != nonce
                                || received.usable_ms != crate::budget::MODEL_USABLE_MS {
                                return Err(invalid("actual_model_ready_binding_or_full_budget"));
                            }
                            ready = true;
                            active_deadline = tokio::time::Instant::now() + MODEL_LIMIT;
                            continue;
                        }
                        let id = ledger.server(&raw)?;
                        // Validate original reply first. Projection never feeds the
                        // server ledger, snapshot, action guard or verification.
                        let raw = match &presenter {
                            Some(presenter) => presenter.render(raw)?,
                            None => raw,
                        };
                        ticket += 1;
                        stdout_tx.try_send(WriteJob { ticket, raw, dispatch_deadline: None }).map_err(|_| invalid("stdout_write_queue_full_hold"))?;
                        stdout_jobs.insert(ticket, id);
                    }
                }
                event = event_rx.recv() => {
                    let event = event.ok_or_else(|| invalid("owned_writer_event_lost_hold"))?;
                    event.result?;
                    if event.writer == Writer::Agent {
                        if !agent_jobs.remove(&event.ticket) { return Err(invalid("untracked_agent_write_ack")); }
                    } else {
                        let id = stdout_jobs.remove(&event.ticket).ok_or_else(|| invalid("untracked_stdout_write_ack"))?;
                        if let Some(id) = id
                            && ledger.pending.remove(&id) != Some(true) {
                            return Err(invalid("reply_delivery_state_mismatch"));
                        }
                    }
                }
            }
        }
    }.await;
    stdin_task.abort();
    peer_task.abort();
    drop(agent_tx);
    drop(stdout_tx);
    if result.is_err() {
        agent_task.abort();
        stdout_task.abort();
    }
    let _ = tokio::time::timeout(Duration::from_secs(1), stdin_task).await;
    let _ = tokio::time::timeout(Duration::from_secs(1), peer_task).await;
    let cleanup_deadline = cleanup_deadline
        .unwrap_or_else(|| tokio::time::Instant::now() + IO_TIMEOUT + Duration::from_secs(1));
    let agent_cleanup = tokio::time::timeout_at(cleanup_deadline, agent_task).await;
    let stdout_cleanup = tokio::time::timeout_at(cleanup_deadline, stdout_task).await;
    if result.is_ok() {
        agent_cleanup
            .map_err(|_| invalid("agent_writer_join_deadline_hold"))?
            .map_err(|_| invalid("agent_writer_join_failed_hold"))??;
        stdout_cleanup
            .map_err(|_| invalid("stdout_writer_join_deadline_hold"))?
            .map_err(|_| invalid("stdout_writer_join_failed_hold"))??;
    }
    drop(guard);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::AsyncReadExt;

    fn request(id: Value) -> Vec<u8> {
        json_frame(
            &json!({"jsonrpc":"2.0","id":id,"method":"test.protocol"}),
            frame::INBOUND,
        )
        .unwrap()
    }
    #[test]
    fn rpc_ids_pending_and_total_budgets_are_distinct() {
        let mut ledger = Ledger::default();
        for id in 0..MAX_PENDING {
            ledger.client(&request(json!(id))).unwrap();
        }
        assert!(ledger.client(&request(json!(MAX_PENDING))).is_err());
        let response = json_frame(
            &json!({"jsonrpc":"2.0","id":0,"result":null}),
            frame::OUTBOUND,
        )
        .unwrap();
        assert_eq!(ledger.server(&response).unwrap(), Some("0".into()));
        assert!(ledger.server(&response).is_err());
        assert_eq!(ledger.pending.remove("0"), Some(true));
        assert!(
            ledger.client(&request(json!(0))).is_err(),
            "delivered IDs still cannot replay"
        );
        let mut notifications = Ledger::default();
        let notification = json_frame(
            &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            frame::INBOUND,
        )
        .unwrap();
        for _ in 0..MAX_CLIENT_FRAMES {
            notifications.client(&notification).unwrap();
        }
        assert!(notifications.client(&notification).is_err());
        for id in [
            Value::Null,
            json!(1.5),
            json!(true),
            json!(""),
            json!("x".repeat(257)),
        ] {
            assert!(Ledger::default().client(&request(id)).is_err());
        }
    }
    #[test]
    fn internal_controls_and_invalid_responses_never_become_stdio_mcp() {
        let mut ledger = Ledger::default();
        ledger.client(&request(json!(1))).unwrap();
        for value in [
            json!({"transport_version":1,"model_end_ack":true}),
            json!({"jsonrpc":"2.0","id":2,"result":{}}),
            json!({"jsonrpc":"2.0","id":1,"result":{},"error":{}}),
            json!({"jsonrpc":"2.0","id":1,"error":true}),
            json!({"jsonrpc":"2.0","id":1,"method":"request_to_client"}),
            json!({"jsonrpc":"2.0","method":"control.not_a_notification"}),
        ] {
            assert!(
                ledger
                    .server(&json_frame(&value, frame::OUTBOUND).unwrap())
                    .is_err()
            );
        }
        for value in [
            json!({"version":1,"task":"unapproved"}),
            json!({"jsonrpc":"2.0","id":1,"method":"ok","transport_version":1}),
            json!({"jsonrpc":"2.0","id":1,"result":{}}),
        ] {
            assert!(
                Ledger::default()
                    .client(&json_frame(&value, frame::INBOUND).unwrap())
                    .is_err()
            );
        }
        let valid_error = json_frame(&json!({"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"invalid params","data":{}}}), frame::OUTBOUND).unwrap();
        assert_eq!(ledger.server(&valid_error).unwrap(), Some("1".into()));
    }
    #[tokio::test]
    async fn expired_active_job_is_rejected_before_writer_dispatch() {
        let (output, mut capture) = tokio::io::duplex(1024);
        let (events, mut receive) = mpsc::channel(1);
        let (tx, task) = writer(output, frame::INBOUND, Writer::Agent, events);
        tx.send(WriteJob {
            ticket: 1,
            raw: b"{}\n".to_vec(),
            dispatch_deadline: Some(tokio::time::Instant::now() - Duration::from_millis(1)),
        })
        .await
        .unwrap();
        let event = receive.recv().await.unwrap();
        assert!(
            event
                .result
                .unwrap_err()
                .to_string()
                .contains("active_model_dispatch_deadline_hold")
        );
        assert!(task.await.unwrap().is_err());
        let mut bytes = Vec::new();
        capture.read_to_end(&mut bytes).await.unwrap();
        assert!(bytes.is_empty());
        assert_eq!(MODEL_LIMIT.as_secs(), 1200);
        assert_eq!(END_ACK_TIMEOUT.as_secs(), 30);
    }
}
