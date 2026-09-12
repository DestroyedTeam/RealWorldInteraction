//! Task-independent authenticated device RPC. Callers own platform setup.
mod client;
mod server;
use crate::invalid;
pub use client::{Client, TransportStats};
use rcu_core::device::{PROTOCOL_VERSION, Reply, Request, valid_id};
use rcu_link_trust::{Identity, IdentityPaths, PeerPin};
use serde::{Deserialize, Serialize};
pub use server::serve;
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
/// Only peers that negotiate this token may reuse the framed connection.
const KEEPALIVE_ALPN: &[u8] = b"rcu-device-v2-keepalive-v1";
const SERVER_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const CLIENT_IDLE_TIMEOUT: Duration = Duration::from_secs(15);

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

/// Reject obsolete/invalid calls before invoking any device handler or native metadata.
fn decode_request(raw: &[u8]) -> io::Result<Result<Request, Reply>> {
    let value: serde_json::Value = serde_json::from_slice(raw)?;
    let parsed = serde_json::from_value::<Request>(value.clone());
    if value["version"] == PROTOCOL_VERSION
        && let Ok(request) = parsed
    {
        return Ok(Ok(request));
    }
    let id = value["id"]
        .as_str()
        .filter(|s| valid_id(s))
        .ok_or_else(|| invalid("invalid_envelope_id"))?;
    let device = value["device"]
        .as_str()
        .filter(|s| valid_id(s))
        .ok_or_else(|| invalid("invalid_envelope_device"))?;
    let code = if value["version"] != PROTOCOL_VERSION {
        "device_protocol_version_unsupported"
    } else {
        "invalid_device_call"
    };
    Ok(Err(Reply {
        version: PROTOCOL_VERSION,
        id: id.into(),
        device: device.into(),
        instance: "protocol-rejected".into(),
        result: rcu_core::device::DeviceResult::error(
            code,
            "No device handler ran. Use device wire v2 selectors and acceptance references; no fallback or replay.",
        ),
    }))
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    #[test]
    fn old_wire_and_bare_labels_fail_without_native_handler() {
        for value in [
            serde_json::json!({"version":1,"id":"id","device":"device","call":{"kind":"invoke","tool":"computer.observe","arguments":{}}}),
            serde_json::json!({"version":2,"id":"id","device":"device","call":{"kind":"cancel","operation_id":"old-label"}}),
        ] {
            let result = decode_request(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .unwrap_err();
            assert!(result.result.is_error);
            assert_eq!(result.instance, "protocol-rejected");
        }
    }
}
