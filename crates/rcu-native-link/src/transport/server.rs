//! Fixed configured user-agent listener. Authentication errors do not cancel a
//! different admitted operation. Only explicit controller Cancel or owned stage
//! failure enters durable HOLD.
use crate::{
    control, frame, invalid, model, net,
    runtime::Runtime,
    types::{Attach, ControlRequest, Role, Stage, identifier},
    worker::{Read, Write},
};
use serde_json::json;
use std::{
    collections::BTreeSet,
    io,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::Semaphore,
    task::JoinSet,
};

pub fn validate_attach(runtime: &Runtime, a: &Attach, peer: &str) -> io::Result<()> {
    let pin = match a.role {
        Role::Controller => &runtime.config.controller_peer_sha256,
        Role::Model => &runtime.config.model_peer_sha256,
    };
    if a.version != crate::TRANSPORT_VERSION
        || a.task != runtime.approval.task
        || a.approval_sha256 != runtime.approval.digest()
        || a.nonce.len() != 36
        || uuid::Uuid::parse_str(&a.nonce).is_err()
        || pin != peer
    {
        return Err(invalid("peer_role_task_manifest_or_nonce_mismatch"));
    }
    Ok(())
}
fn nonce(nonces: &Mutex<BTreeSet<String>>, attach: &Attach) -> io::Result<()> {
    let prefix = format!("{:?}:", attach.role);
    let key = format!("{prefix}{}", attach.nonce);
    let mut set = nonces.lock().map_err(|_| invalid("attach_nonce_lock"))?;
    if set.iter().filter(|n| n.starts_with(&prefix)).count() >= 256 || !set.insert(key) {
        return Err(invalid("attach_nonce_reuse_or_role_cap"));
    }
    Ok(())
}
async fn connection(
    runtime: Arc<Runtime>,
    tcp: TcpStream,
    tls: Arc<rustls::ServerConfig>,
    nonces: Arc<Mutex<BTreeSet<String>>>,
) -> io::Result<()> {
    let stream = net::accept(tcp, tls).await?;
    let peer = rcu_link_trust::peer_sha(stream.get_ref().1.peer_certificates())?;
    let (input, output) = tokio::io::split(stream);
    let mut input = BufReader::new(Box::new(input) as Read);
    let mut output = Box::new(output) as Write;
    let raw = tokio::time::timeout(
        net::IO_TIMEOUT,
        frame::read_frame(&mut input, frame::ATTACH_CAP),
    )
    .await??
    .ok_or_else(|| invalid("missing_attach"))?;
    let attach: Attach = serde_json::from_slice(&raw)?;
    validate_attach(&runtime, &attach, peer.as_str())?;
    if attach.role == Role::Model {
        // This is the atomic ownership boundary. A wrong-order/duplicate model
        // attach is denied without touching a different active stage.
        let admission = runtime
            .previous()
            .and_then(|previous| runtime.begin(Stage::Model, &previous));
        if let Err(error) = admission {
            let (hold_reason, next_stage) = {
                let journal = runtime.journal()?;
                (
                    journal.state.as_ref().and_then(|s| s.hold_reason.clone()),
                    journal.state.as_ref().map(|s| s.next),
                )
            };
            // Only the already authenticated matching Model peer receives this
            // denial. No nonce, worker, retry, input or stage progress is minted.
            net::write_json(
                &mut output,
                &json!({"version":crate::TRANSPORT_VERSION,
                "attached":false,"task":attach.task,"nonce":attach.nonce,"role":"model",
                "reason":error.to_string().chars().take(256).collect::<String>(),
                "hold_reason":hold_reason,"next_stage":next_stage}),
                frame::ATTACH_CAP,
            )
            .await?;
            return Err(error);
        }
        let result = async {
            // Rejected model attaches do not consume a controller nonce budget.
            nonce(&nonces,&attach)?;
            net::write_json(&mut output, &json!({"version":crate::TRANSPORT_VERSION,"attached":true,"task":attach.task,"nonce":attach.nonce,"role":"model"}), frame::ATTACH_CAP).await?;
            let (receipt, mut output) = model::admitted(runtime.clone(), &attach, input, output).await?;
            net::write_json(&mut output, &json!({"transport_version":crate::TRANSPORT_VERSION,"model_end_ack":true,"task":attach.task,"stage_receipt":receipt,"complete":true}), frame::ATTACH_CAP).await?;
            tokio::time::timeout(net::IO_TIMEOUT, output.shutdown()).await??;
            Ok::<_, io::Error>(())
        }.await;
        if let Err(e) = &result {
            let _ = runtime.hold(&format!("owned_model_connection_failed:{e}"));
            let cleanup = runtime.cleanup_owned().await;
            let _ = runtime
                .journal()
                .and_then(|mut j| j.event("model_connection_owned_cleanup", cleanup));
        }
        result
    } else {
        nonce(&nonces, &attach)?;
        net::write_json(&mut output, &json!({"version":crate::TRANSPORT_VERSION,"attached":true,"task":attach.task,"nonce":attach.nonce,"role":"controller"}), frame::ATTACH_CAP).await?;
        // One typed request per connection. Read-only reconnection is supported;
        // controller effects never reconnect/replay themselves.
        let raw = tokio::time::timeout(
            net::IO_TIMEOUT,
            frame::read_frame(&mut input, frame::ATTACH_CAP),
        )
        .await??
        .ok_or_else(|| invalid("missing_control_request"))?;
        let request: ControlRequest = serde_json::from_slice(&raw)?;
        if request.version != 1 || !identifier(&request.request_id, 128) {
            return Err(invalid("control_request_version_or_id"));
        }
        let response = match control::execute(&runtime, &request).await {
            Ok(value) => value,
            Err(e) => {
                json!({"version":1,"request_id":request.request_id,"ok":false,"error":e.to_string(),"status":runtime.journal()?.public_status()})
            }
        };
        net::write_json(&mut output, &response, frame::OUTBOUND).await?;
        tokio::time::timeout(net::IO_TIMEOUT, output.shutdown()).await??;
        Ok(())
    }
}

/// Tests pass their own real loopback ephemeral listener. The executable uses
/// only AgentConfig.bind. A fixture binary never permits non-loopback traffic.
pub async fn serve(runtime: Arc<Runtime>, listener: TcpListener) -> io::Result<()> {
    if listener.local_addr()?.ip() != runtime.config.bind.ip()
        || (runtime.config.bind.port() != 0
            && listener.local_addr()?.port() != runtime.config.bind.port())
        || (!runtime.config.backend.is_windows() && !listener.local_addr()?.ip().is_loopback())
    {
        return Err(invalid("listener_not_exact_local_configuration"));
    }
    let tls = net::server_tls(&runtime.config)?;
    let permits = Arc::new(Semaphore::new(2));
    let nonces = Arc::new(Mutex::new(BTreeSet::new()));
    let mut tasks = JoinSet::new();
    let duration = runtime
        .budget
        .as_ref()
        .map(|b| b.remaining())
        .unwrap_or(Duration::from_secs(60));
    let until = tokio::time::Instant::now() + duration;
    let mut accepted = 0usize;
    let mut rate_window = tokio::time::Instant::now();
    let mut state_tick = tokio::time::interval(Duration::from_millis(250));
    state_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let run_result:io::Result<()> = async { loop {
        tokio::select! {
            _ = tokio::time::sleep_until(until) => break,
            _ = tokio::signal::ctrl_c() => break,
            Some(_) = tasks.join_next(), if !tasks.is_empty() => {},
            _ = state_tick.tick() => {
                if rate_window.elapsed()>=Duration::from_secs(1){accepted=0;rate_window=tokio::time::Instant::now();}
                if tasks.is_empty() && runtime.journal()?.state.as_ref().is_some_and(|s| s.complete && !s.hold && s.task==runtime.approval.task && s.approval_sha256==runtime.approval.digest()) { break; }
            }
            socket = listener.accept(), if accepted<32 => {
                let (tcp, _) = socket?;
                accepted += 1;
                let Ok(permit) = permits.clone().try_acquire_owned() else { drop(tcp); continue; };
                let rt = runtime.clone(); let config = tls.clone(); let seen = nonces.clone();
                tasks.spawn(async move { let _permit = permit; connection(rt,tcp,config,seen).await });
            }
        }
    } Ok(()) }.await;
    drop(listener);
    if run_result.is_err()
        || runtime
            .journal()
            .map(|j| j.state.as_ref().is_some_and(|s| !s.complete))
            .unwrap_or(true)
    {
        let _ = runtime.hold("agent_shutdown_or_total_deadline");
    }
    tasks.abort_all();
    let joined = tokio::time::timeout(Duration::from_secs(3), async {
        while tasks.join_next().await.is_some() {}
    })
    .await
    .is_ok();
    let cleanup = runtime.cleanup_owned().await;
    let recorded = runtime.journal().and_then(|mut journal| {
        if journal.state.is_none() {
            return Ok(());
        }
        journal.event(
            "agent_shutdown_owned_cleanup",
            json!({"connection_tasks_joined":joined,"cleanup":cleanup}),
        )
    });
    if !joined {
        return Err(invalid("agent_connection_cleanup_incomplete"));
    }
    recorded?;
    run_result
}
