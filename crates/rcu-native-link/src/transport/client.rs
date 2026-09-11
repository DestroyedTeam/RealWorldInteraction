use crate::{
    frame, invalid, net,
    types::{Attach, ClientConfig, ControlRequest, Role},
};
use serde_json::{Value, json};
use std::{io, time::Duration};
use tokio::io::{AsyncWriteExt, BufReader};

pub async fn control(config: &ClientConfig, request: &ControlRequest) -> io::Result<Value> {
    if config.role != Role::Controller || request.version != 1 {
        return Err(invalid("controller_role_and_version_required"));
    }
    let attach = Attach {
        version: crate::TRANSPORT_VERSION,
        task: config.task.clone(),
        approval_sha256: config.approval_sha256.clone(),
        role: Role::Controller,
        nonce: uuid::Uuid::new_v4().to_string(),
    };
    let stream = net::connect(config).await?;
    let (input, mut output) = tokio::io::split(stream);
    let mut input = BufReader::new(input);
    net::write_json(
        &mut output,
        &serde_json::to_value(&attach)?,
        frame::ATTACH_CAP,
    )
    .await?;
    let raw = tokio::time::timeout(
        net::IO_TIMEOUT,
        frame::read_frame(&mut input, frame::ATTACH_CAP),
    )
    .await??
    .ok_or_else(|| invalid("controller_attach_eof"))?;
    let ack: Value = serde_json::from_slice(&raw)?;
    if ack
        != json!({"version":crate::TRANSPORT_VERSION,"attached":true,"task":attach.task,"nonce":attach.nonce,"role":"controller"})
    {
        return Err(invalid("controller_attach_ack_mismatch"));
    }
    net::write_json(
        &mut output,
        &serde_json::to_value(request)?,
        frame::ATTACH_CAP,
    )
    .await?;
    let raw = tokio::time::timeout(
        Duration::from_secs(120),
        frame::read_frame(&mut input, frame::OUTBOUND),
    )
    .await??
    .ok_or_else(|| invalid("controller_response_lost_readonly_status_required"))?;
    let response: Value = serde_json::from_slice(&raw)?;
    if response["version"] != 1
        || response["request_id"] != request.request_id
        || !response["ok"].is_boolean()
    {
        return Err(invalid("controller_response_binding"));
    }
    tokio::time::timeout(net::IO_TIMEOUT, output.shutdown()).await??;
    Ok(response)
}
